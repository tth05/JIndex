use crate::builder::{BuildTimeInfo, ClassIndexBuilder, ClassInfo};
use crate::class_index::ClassIndex;
use anyhow::{anyhow, Context};
use rayon::prelude::*;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;
use std::time::Instant;
use zip::ZipArchive;

fn do_multi_threaded<I, F, O>(queue: Vec<I>, func: &F) -> anyhow::Result<Vec<O>>
where
    O: Send,
    F: (Fn(I) -> anyhow::Result<O>) + Sync,
    I: Sync + Send,
{
    Ok(queue
        .into_par_iter()
        .map(|el| func(el))
        .collect::<anyhow::Result<Vec<O>>>()?)
}

pub(crate) struct ArchiveSource {
    pub source_id: u32,
    pub input_order: u32,
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
        file_name,
    } = source;
    let mut file_buf = Vec::new();
    let mut output = Vec::new();
    let file_path = Path::new(&file_name)
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize path {}", file_name))?;
    if !file_path.exists() {
        return Err(anyhow!("File {} does not exist", file_name));
    }

    file_buf.clear();
    let mut file =
        File::open(file_path).with_context(|| format!("Failed to open file {}", file_name))?;
    file.read_to_end(&mut file_buf)?;

    let mut archive = ZipArchive::new(Cursor::new(&file_buf))
        .with_context(|| format!("Failed to read zip file {}", file_name))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir()
            || !entry.name().ends_with(".class")
            || entry.name() == "module-info.class"
        {
            continue;
        }

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

pub fn create_class_index_from_jars(
    jar_names: Vec<String>,
) -> anyhow::Result<(BuildTimeInfo, ClassIndex)> {
    let jar_sources = jar_names
        .into_iter()
        .enumerate()
        .map(|(index, file_name)| {
            let input_order = u32::try_from(index)
                .map_err(|_| anyhow!("More than 4294967295 archive sources"))?;
            Ok(ArchiveSource {
                source_id: input_order,
                input_order,
                file_name,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    create_class_index_from_sources(jar_sources, Vec::new())
}

fn process_class_bytes_worker(source: DirectSource) -> anyhow::Result<ClassInfo> {
    process_class(&source.bytes, source.source_id, source.input_order)
}

fn process_class(bytes: &[u8], source_id: u32, input_order: u32) -> anyhow::Result<ClassInfo> {
    super::classfile_parser::parse_class(bytes, source_id, input_order)
}

pub fn create_class_index_from_bytes(
    class_bytes: Vec<Vec<u8>>,
) -> anyhow::Result<(BuildTimeInfo, ClassIndex)> {
    let direct_sources = class_bytes
        .into_iter()
        .enumerate()
        .map(|(index, bytes)| {
            let input_order = u32::try_from(index)
                .map_err(|_| anyhow!("More than 4294967295 direct class sources"))?;
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
