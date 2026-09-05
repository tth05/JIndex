use crate::builder::{BuildTimeInfo, ClassIndexBuilder, ClassInfo};
use crate::class_index::ClassIndex;
use anyhow::Context;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use std::time::Instant;
use zip::result::ZipError;
use zip::ZipArchive;

fn do_multi_threaded<I, F, O>(queue: Vec<I>, func: &F) -> anyhow::Result<Vec<O>>
where
    O: Send,
    F: (Fn(I) -> anyhow::Result<O>) + Sync,
    I: Sync + Send,
{
    queue
        .into_par_iter()
        .map(func)
        .collect::<anyhow::Result<Vec<O>>>()
}

pub(crate) struct ArchiveSource {
    pub source_id: u32,
    pub input_order: u32,
    pub target_java_release: u32,
    pub file_name: String,
}

pub(crate) struct DirectSource {
    pub source_id: u32,
    pub input_order: u32,
    pub bytes: Vec<u8>,
}

fn process_jar_worker(source: ArchiveSource) -> anyhow::Result<Vec<ClassInfo>> {
    let ArchiveSource {
        source_id,
        input_order,
        target_java_release,
        file_name,
    } = source;
    let mut file_buf = Vec::new();
    let mut output = Vec::new();
    let file_path = Path::new(&file_name)
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize path {}", file_name))?;
    let mut file =
        File::open(file_path).with_context(|| format!("Failed to open file {}", file_name))?;
    file.read_to_end(&mut file_buf)?;

    let mut archive = ZipArchive::new(Cursor::new(&file_buf))
        .with_context(|| format!("Failed to read zip file {}", file_name))?;
    let multi_release = is_multi_release(&mut archive)?;
    let mut selected_entries: FxHashMap<String, (u32, usize)> = FxHashMap::default();

    for i in 0..archive.len() {
        let entry = archive.by_index_raw(i)?;
        if entry.is_dir() {
            continue;
        }

        let Some((version, logical_name)) = archive_class_name(entry.name()) else {
            continue;
        };
        if logical_name == "module-info.class"
            || version > target_java_release
            || (version > 0 && !multi_release)
        {
            continue;
        }
        selected_entries
            .entry(logical_name.to_owned())
            .and_modify(|selected| {
                if version > selected.0 {
                    *selected = (version, i);
                }
            })
            .or_insert((version, i));
    }

    let mut selected_entries = selected_entries.into_iter().collect::<Vec<_>>();
    selected_entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    for (_, (_, entry_index)) in selected_entries {
        let mut entry = archive.by_index(entry_index)?;

        let mut data = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut data)
            .with_context(|| format!("Failed to read {},{}", file_name, entry.name()))?;

        // NOTE: While processing the class immediately makes this a bit slower, because the
        // workload is split less evenly (e.g. a single jar file has way more classes than a
        // different one), we get the benefit of using way less memory while indexing.
        output.push(
            process_class(&data, source_id, input_order).with_context(|| {
                format!("Failed to parse class {} from {}", entry.name(), file_name)
            })?,
        );
    }

    Ok(output)
}

#[cfg(test)]
pub fn create_class_index_from_jars(
    jar_names: Vec<String>,
    target_java_release: u32,
) -> anyhow::Result<(BuildTimeInfo, ClassIndex)> {
    let jar_sources = jar_names
        .into_iter()
        .enumerate()
        .map(|(index, file_name)| {
            let input_order = u32::try_from(index)
                .map_err(|_| anyhow::anyhow!("More than 4294967295 archive sources"))?;
            Ok(ArchiveSource {
                source_id: input_order,
                input_order,
                target_java_release,
                file_name,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    create_class_index_from_sources(jar_sources, Vec::new())
}

fn archive_class_name(name: &str) -> Option<(u32, &str)> {
    const VERSIONS_PREFIX: &str = "META-INF/versions/";

    if !name.ends_with(".class") {
        return None;
    }
    let Some(versioned_name) = name.strip_prefix(VERSIONS_PREFIX) else {
        return Some((0, name));
    };
    let (version, logical_name) = versioned_name.split_once('/')?;
    let version = version.parse::<u32>().ok()?;
    (version >= 9 && !logical_name.is_empty()).then_some((version, logical_name))
}

fn is_multi_release<R: Read + Seek>(archive: &mut ZipArchive<R>) -> anyhow::Result<bool> {
    let mut manifest = match archive.by_name("META-INF/MANIFEST.MF") {
        Ok(manifest) => manifest,
        Err(ZipError::FileNotFound) => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::with_capacity(manifest.size() as usize);
    manifest.read_to_end(&mut bytes)?;
    Ok(manifest_attribute(&bytes, "Multi-Release")
        .is_some_and(|value| value.eq_ignore_ascii_case("true")))
}

fn manifest_attribute(manifest: &[u8], requested_name: &str) -> Option<String> {
    let text = String::from_utf8_lossy(manifest);
    let mut current_name = None::<&str>;
    let mut current_value = String::new();

    for raw_line in text.lines() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() {
            break;
        }
        if let Some(continuation) = line.strip_prefix(' ') {
            current_value.push_str(continuation);
            continue;
        }
        if current_name.is_some_and(|name| name.eq_ignore_ascii_case(requested_name)) {
            return Some(current_value.trim().to_owned());
        }
        let Some((name, value)) = line.split_once(':') else {
            current_name = None;
            current_value.clear();
            continue;
        };
        current_name = Some(name);
        current_value.clear();
        current_value.push_str(value.trim_start());
    }

    current_name
        .is_some_and(|name| name.eq_ignore_ascii_case(requested_name))
        .then(|| current_value.trim().to_owned())
}

fn process_class_bytes_worker(source: DirectSource) -> anyhow::Result<ClassInfo> {
    process_class(&source.bytes, source.source_id, source.input_order)
}

fn process_class(bytes: &[u8], source_id: u32, input_order: u32) -> anyhow::Result<ClassInfo> {
    super::classfile_parser::parse_class(bytes, source_id, input_order)
}

#[cfg(test)]
pub fn create_class_index_from_bytes(
    class_bytes: Vec<Vec<u8>>,
) -> anyhow::Result<(BuildTimeInfo, ClassIndex)> {
    let direct_sources = class_bytes
        .into_iter()
        .enumerate()
        .map(|(index, bytes)| {
            let input_order = u32::try_from(index)
                .map_err(|_| anyhow::anyhow!("More than 4294967295 direct class sources"))?;
            Ok(DirectSource {
                source_id: input_order,
                input_order,
                bytes,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    create_class_index_from_sources(Vec::new(), direct_sources)
}

pub(crate) fn create_class_index_from_sources(
    jar_sources: Vec<ArchiveSource>,
    direct_sources: Vec<DirectSource>,
) -> anyhow::Result<(BuildTimeInfo, ClassIndex)> {
    let now = Instant::now();
    let (jar_result, class_bytes_result) = rayon::join(
        || do_multi_threaded(jar_sources, &process_jar_worker),
        || do_multi_threaded(direct_sources, &process_class_bytes_worker),
    );

    let mut jar_class_infos: Vec<ClassInfo> = jar_result?.into_iter().flatten().collect();
    let class_bytes_infos = class_bytes_result?;

    jar_class_infos.extend(class_bytes_infos);

    let mut info = BuildTimeInfo {
        class_reading_time: now.elapsed().as_millis(),
        ..Default::default()
    };

    let (other_info, class_index) = create_class_index_from_infos(jar_class_infos)?;
    info.merge(other_info);
    Ok((info, class_index))
}

fn create_class_index_from_infos(
    mut class_info_list: Vec<ClassInfo>,
) -> anyhow::Result<(BuildTimeInfo, ClassIndex)> {
    let now = Instant::now();

    // Select one deterministic source for each class. The caller's input order is the precedence
    // order, independently of whether a source is an archive or one class file.
    class_info_list.par_sort_unstable_by(|a, b| {
        a.class_name
            .cmp(&b.class_name)
            .then_with(|| a.package_name.cmp(&b.package_name))
            .then_with(|| a.input_order.cmp(&b.input_order))
            .then_with(|| a.source_id.cmp(&b.source_id))
    });
    let mut selected_classes = Vec::with_capacity(class_info_list.len());
    for class_info in class_info_list {
        let duplicate = selected_classes.last().is_some_and(|previous: &ClassInfo| {
            previous.class_name == class_info.class_name
                && previous.package_name == class_info.package_name
        });
        if !duplicate {
            selected_classes.push(class_info);
        }
    }
    let class_info_list = selected_classes;

    let mut build_time_info = BuildTimeInfo {
        class_reading_time: now.elapsed().as_millis(),
        ..Default::default()
    };

    let field_count = class_info_list
        .iter()
        .map(|entry| entry.fields.len() as u32)
        .sum();
    let method_count = class_info_list
        .iter()
        .map(|entry| entry.methods.len() as u32)
        .sum();

    let (other_info, class_index) = ClassIndexBuilder::default()
        .with_expected_field_count(field_count)
        .with_expected_method_count(method_count)
        .build(class_info_list)?;

    build_time_info.merge(other_info);
    Ok((build_time_info, class_index))
}
