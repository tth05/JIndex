use crate::builder::{BuildTimeInfo, ClassIndexBuilder, ClassInfo, FieldInfo, MethodInfo};
use crate::class_index::ClassIndex;
use crate::rsplit_once;
use crate::signature::{
    InnerClassType, RawClassSignature, RawEnclosingTypeInfo, RawMethodSignature, RawSignatureType,
};
use anyhow::{anyhow, bail, Context};
use ascii::{AsAsciiStr, AsciiChar, AsciiStr, AsciiString, IntoAsciiString};
use cafebabe::attributes::{AttributeData, AttributeInfo, InnerClassEntry};
use cafebabe::constant_pool::NameAndType;
use cafebabe::{parse_class_with_options, MethodAccessFlags, ParseOptions};
use compact_str::{CompactString, ToCompactString};
use rayon::prelude::*;
use std::borrow::Cow;
use std::fs::File;
use std::io::{Cursor, Read};
use std::ops::BitOr;
use std::path::Path;
use std::str::FromStr;
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

fn process_jar_worker(file_name: String) -> anyhow::Result<Vec<ClassInfo>> {
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
        output.push(match process_class(&data) {
            Ok(x) => x,
            Err(_) => continue,
        });
    }

    Ok(output)
}

pub fn create_class_index_from_jars(
    jar_names: Vec<String>,
) -> anyhow::Result<(BuildTimeInfo, ClassIndex)> {
    let now = Instant::now();
    let class_info_list = do_multi_threaded(jar_names, &process_jar_worker)?
        .into_iter()
        .flatten()
        .collect();

    let mut info = BuildTimeInfo {
        class_reading_time: now.elapsed().as_millis(),
        ..Default::default()
    };

    let (other_info, class_index) = create_class_index_from_infos(class_info_list)?;
    info.merge(other_info);
    Ok((info, class_index))
}

macro_rules! get_attribute_info {
    ($attributes: expr, $match: pat_param) => {
        $attributes.iter().find(|a| matches!(&a.data, $match))
    };
}

macro_rules! get_attribute_data {
    ($attributes: expr, $info_match: pat_param, $data_var: expr, $default: expr) => {
        get_attribute_info!($attributes, $info_match).map_or($default, |a| {
            if let $info_match = &a.data {
                return $data_var;
            }
            unreachable!();
        })
    };
}

fn process_class_bytes_worker(bytes_queue: Vec<u8>) -> anyhow::Result<ClassInfo> {
    process_class(&bytes_queue)
}

fn process_class(bytes: &[u8]) -> anyhow::Result<ClassInfo> {
    let class_file = parse_class_with_options(bytes, ParseOptions::default().parse_bytecode(false))
        .map_err(|parse_error| anyhow!("{}", parse_error))
        .with_context(|| format!("Failed to parse class file {:?}", bytes))?;

    let ConvertedInnerClassInfo {
        package_name,
        full_class_name,
        class_name_start_index,
        inner_class_access_flags,
        enclosing_type,
        member_classes,
    } = convert_enclosing_type_and_inner_classes(
        class_file.this_class,
        get_attribute_data!(
            &class_file.attributes,
            AttributeData::EnclosingMethod { class_name, method },
            Option::Some((class_name, method)),
            Option::None
        ),
        get_attribute_data!(
            &class_file.attributes,
            AttributeData::InnerClasses(vec),
            Option::Some(vec),
            Option::None
        ),
    )?;

    let parsed_signature = parse_class_signature(
        &class_file.attributes,
        class_file.super_class,
        class_file.interfaces,
    )?;

    Ok(ClassInfo {
        package_name,
        class_name: full_class_name,
        class_name_start_index,
        access_flags: class_file
            .access_flags
            .bits()
            .bitor(inner_class_access_flags),
        signature: parsed_signature,
        enclosing_type,
        member_classes,
        fields: class_file
            .fields
            .into_iter()
            .filter_map(|f| {
                let name = match f.name.as_ascii_str() {
                    Ok(x) => x.to_compact_string(),
                    Err(_) => return None,
                };

                Some(get_attribute_data!(
                    &f.attributes,
                    AttributeData::Signature(s),
                    s,
                    &f.descriptor
                ))
                .and_then(|signature| {
                    RawSignatureType::from_str(signature)
                        .ok()
                        .map(|signature_type| FieldInfo {
                            field_name: name,
                            descriptor: signature_type,
                            access_flags: f.access_flags,
                        })
                })
            })
            .collect(),
        methods: class_file
            .methods
            .into_iter()
            .filter_map(|m| {
                let name = match m.name.as_ascii_str() {
                    Ok(x) => x.to_compact_string(),
                    Err(_) => return None,
                };

                if m.access_flags.contains(MethodAccessFlags::SYNTHETIC) {
                    return None;
                }

                Some(get_attribute_data!(
                    &m.attributes,
                    AttributeData::Signature(s),
                    s,
                    &m.descriptor
                ))
                .and_then(|signature| {
                    RawMethodSignature::from_data(signature, &|| {
                        get_attribute_data!(
                            &m.attributes,
                            AttributeData::Exceptions(vec),
                            Option::Some(vec),
                            Option::None
                        )
                    })
                    .ok()
                    .map(|signature_type| MethodInfo {
                        method_name: name,
                        signature: signature_type,
                        access_flags: m.access_flags,
                    })
                })
            })
            .collect(),
    })
}

pub fn create_class_index_from_bytes(
    class_bytes: Vec<Vec<u8>>,
) -> anyhow::Result<(BuildTimeInfo, ClassIndex)> {
    let class_info_list: Vec<ClassInfo> =
        do_multi_threaded(class_bytes, &process_class_bytes_worker)?;

    create_class_index_from_infos(class_info_list)
}

fn create_class_index_from_infos(
    mut class_info_list: Vec<ClassInfo>,
) -> anyhow::Result<(BuildTimeInfo, ClassIndex)> {
    let now = Instant::now();

    //Removes duplicate classes
    class_info_list.par_sort_unstable_by(|a, b| {
        a.class_name
            .cmp(&b.class_name)
            .then_with(|| a.package_name.cmp(&b.package_name))
    });
    class_info_list
        .dedup_by(|a, b| a.class_name.eq(&b.class_name) && a.package_name.eq(&b.package_name));

    let mut build_time_info = BuildTimeInfo {
        class_reading_time: now.elapsed().as_millis(),
        ..Default::default()
    };

    let method_count = class_info_list.iter().map(|e| e.methods.len() as u32).sum();

    let (other_info, class_index) = ClassIndexBuilder::default()
        .with_expected_method_count(method_count)
        .build(class_info_list)?;

    build_time_info.merge(other_info);
    Ok((build_time_info, class_index))
}

struct ConvertedInnerClassInfo {
    package_name: CompactString,
    full_class_name: CompactString,
    class_name_start_index: usize,
    inner_class_access_flags: u16,
    enclosing_type: Option<RawEnclosingTypeInfo>,
    member_classes: Option<Vec<CompactString>>,
}

fn convert_enclosing_type_and_inner_classes(
    this_name: Cow<str>,
    enclosing_method_data: Option<(&Cow<str>, &Option<NameAndType>)>,
    inner_class_data: Option<&Vec<InnerClassEntry>>,
) -> anyhow::Result<ConvertedInnerClassInfo> {
    let this_name = this_name.as_ascii_str()?;
    let (package_name, class_name) = rsplit_once(this_name, AsciiChar::Slash);
    let mut class_name_start_index = 0;
    let mut access_flags = 0;

    let mut enclosing_type_info = None;
    let mut member_classes = None;
    let mut self_inner_class_index = None;

    // This blocks checks the first inner class entry which can represent this class. If there is
    // one, we extract the inner and outer class names from it.
    if let Some(vec) = inner_class_data {
        if let Some(first) = vec
            .iter()
            .enumerate()
            .find(|e| e.1.inner_class_info.as_ref() == this_name)
        {
            access_flags = first.1.access_flags.bits();

            let inner_class_type = if first.1.inner_name.is_none() {
                //No source code name -> Anonymous
                InnerClassType::Anonymous
            } else if first.1.outer_class_info.is_none() {
                //Enclosing method attribute will give us the outer class name
                InnerClassType::Local
            } else {
                //Normal direct member inner class
                InnerClassType::Member
            };

            if let Some((class_name, method_data)) = enclosing_method_data {
                let (method_name, method_descriptor) = match method_data {
                    Some(NameAndType { name, descriptor }) => (
                        Some(name.as_ascii_str()?.to_compact_string()),
                        Some(RawMethodSignature::from_data(descriptor, &|| Option::None)?),
                    ),
                    None => (None, None),
                };

                enclosing_type_info = Some(RawEnclosingTypeInfo::new(
                    Some(class_name.as_ascii_str()?.to_compact_string()),
                    inner_class_type,
                    method_name,
                    method_descriptor,
                ));
            } else {
                let (outer_name, inner_name_start) =
                    extract_outer_and_inner_name(class_name, first.1)?;

                class_name_start_index = inner_name_start;
                enclosing_type_info = Some(RawEnclosingTypeInfo::new(
                    Some(outer_name),
                    inner_class_type,
                    None,
                    None,
                ));
            }
            self_inner_class_index = Some(first.0);
        }
    }
    if let Some(vec) = inner_class_data {
        member_classes = Some(
            vec.iter()
                .enumerate()
                .filter_map(|e| {
                    if (self_inner_class_index.is_some() && self_inner_class_index.unwrap() == e.0)
                        || e.1.outer_class_info.is_none()
                        || e.1.inner_name.is_none()
                        || e.1.outer_class_info.as_ref().unwrap().as_ref() != this_name
                    {
                        None
                    } else if let Ok(name) = e.1.inner_class_info.as_ascii_str() {
                        Some(name.to_compact_string())
                    } else {
                        None
                    }
                })
                .collect(),
        );
    }

    Ok(ConvertedInnerClassInfo {
        package_name: package_name.to_compact_string(),
        full_class_name: class_name.to_compact_string(),
        class_name_start_index,
        inner_class_access_flags: access_flags,
        enclosing_type: enclosing_type_info,
        member_classes,
    })
}

/// Returns (0) the full outer class name including the package and (1) the index into the original
/// class name from where the inner class name starts
fn extract_outer_and_inner_name(
    original_class_name: &AsciiStr,
    e: &InnerClassEntry,
) -> anyhow::Result<(CompactString, usize)> {
    e.inner_name
        .as_ref()
        .filter(|n| !n.is_empty())
        .filter(|_| e.outer_class_info.is_some())
        .and_then(|n| {
            if let Ok(name) = e.outer_class_info.as_ref().unwrap().as_ascii_str() {
                Some((
                    name.to_compact_string(),
                    original_class_name.len() - n.len(),
                ))
            } else {
                None
            }
        })
        .or_else(|| {
            //If we don't have an inner name, we usually have an anonymous class like
            // java/lang/Object$1.
            let r: anyhow::Result<(_, _)> = try {
                match &e.outer_class_info {
                    //There might be an outer name which we can use to extract the inner name
                    Some(outer_name) => (
                        outer_name.as_ascii_str()?.to_compact_string(),
                        original_class_name.len()
                            - (e.inner_class_info.len() - (outer_name.len() + 1)),
                    ),
                    //Otherwise we trust the inner name info and split on '$'
                    None => {
                        let index = e
                            .inner_class_info
                            .rfind('$')
                            .ok_or_else(|| anyhow!("No '$' found"))?;
                        (
                            e.inner_class_info[..index]
                                .as_ascii_str()?
                                .to_compact_string(),
                            original_class_name.len() - (e.inner_class_info.len() - (index + 1)),
                        )
                    }
                }
            };
            r.ok()
        })
        .ok_or_else(|| anyhow::anyhow!("Failed to extract outer and inner name"))
}

fn parse_class_signature(
    attributes: &[AttributeInfo],
    super_class: Option<Cow<str>>,
    interfaces: Vec<Cow<str>>,
) -> anyhow::Result<RawClassSignature> {
    Ok(
        if let Some(attr) = get_attribute_info!(attributes, AttributeData::Signature(_)) {
            RawClassSignature::from_str(match &attr.data {
                AttributeData::Signature(s) => s,
                _ => bail!("Expected Signature attribute"),
            })
            .with_context(|| "Invalid class signature")?
        } else {
            RawClassSignature::new(
                super_class
                    .filter(|s| s != "java/lang/Object")
                    .and_then(|s| {
                        s.as_ascii_str()
                            .ok()
                            .map(|s| RawSignatureType::Object(s.to_compact_string()))
                    }),
                Some(
                    interfaces
                        .into_iter()
                        .filter_map(|s| {
                            s.as_ascii_str()
                                .ok()
                                .map(|s| RawSignatureType::Object(s.to_compact_string()))
                        })
                        .collect::<Vec<_>>(),
                )
                .filter(|v| !v.is_empty()),
            )
        },
    )
}
