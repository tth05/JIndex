use crate::builder::workers::{
    create_class_index_from_bytes, create_class_index_from_jars, create_class_index_from_sources,
    ArchiveSource, DirectSource,
};
use crate::builder::BuildTimeInfo;
use anyhow::{anyhow, ensure};
use ascii::{AsAsciiStr, AsciiStr, IntoAsciiString};
use jni::objects::{JByteArray, JIntArray, JList, JObject, JObjectArray, JString, JValue};
use jni::strings::JNIString;
use jni::sys::{jint, jlong, jobject};
use jni::{jni_sig, jni_str, Env, EnvUnowned};
use jvmti_bindings::mutf8;
use std::ffi::CString;

use crate::class_index::ClassIndex;
use crate::class_index_members::IndexedClass;
use crate::constant_pool::{MatchMode, SearchMode, SearchOptions};
use crate::io::{load_class_index_from_file, save_class_index_to_file};
use crate::jni::cache::{get_class_index, init_field_ids};
use crate::jni::{get_enum_ordinal, propagate_error, with_jni_env};
use crate::package_index::IndexedPackage;
use crate::reference_relations::{public_mask, STRING_LITERAL_PUBLIC_MASK};
use crate::semantic_index::{
    PackedLiteralSite, PackedReferenceSite, ReferenceSiteKind, SymbolId, SymbolKind,
};

macro_rules! java_to_ascii_string {
    ($env:expr, $jstring:ident) => {
        java_to_ascii_string!($env, $jstring, |s| s)
    };
    ($env:expr, $jstring:ident, $mapper:expr) => {{
        let env_str: String = $mapper($jstring.try_to_string($env).expect("Not a string"));

        match env_str.into_ascii_string() {
            Ok(s) => s,
            Err(e) => {
                match $env.throw_new(
                    jni_str!("java/lang/IllegalArgumentException"),
                    JNIString::new(format!("'{}' is not an ASCII string", e.into_source())),
                ) {
                    Err(jni::errors::Error::JavaException) => {}
                    other => panic!("Failed to throw IllegalArgumentException: {:?}", other),
                }
                return Ok(JObject::null().into_raw());
            }
        }
    }};
}

#[no_mangle]
/// # Safety
/// A nonzero pointer must be a live allocation returned by `Box::into_raw` for
/// this index. Java's cleanup action claims it at most once after all operations
/// have released the lifecycle read lock, or after the owner becomes unreachable.
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_destroyPointer(
    mut _env: EnvUnowned<'_>,
    _class: JObject,
    pointer: jlong,
) {
    with_jni_env!(_env, {
        if pointer != 0 {
            drop(Box::from_raw(pointer as *mut ClassIndex));
        }
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_createClassIndexFromBytes(
    mut env: EnvUnowned<'_>,
    this: JObject,
    byte_array_list: JObject,
) -> jobject {
    with_jni_env!(env, {
        propagate_error!(env, init_field_ids(env), JObject::null().into_raw());

        let java_list = env.cast_local::<JList>(byte_array_list).unwrap();
        let list_size = java_list.size(env).unwrap();
        let mut class_bytes = Vec::with_capacity(list_size as usize);
        for index in 0..list_size {
            let ar = java_list.get(env, index).unwrap();
            let byte_array = env.cast_local::<JByteArray>(ar).unwrap();
            class_bytes.push(env.convert_byte_array(&byte_array).unwrap());
        }

        let (info, class_index) = propagate_error!(
            env,
            create_class_index_from_bytes(class_bytes),
            JObject::null().into_raw()
        );

        env.set_field(
            &this,
            jni_str!("classIndexPointer"),
            jni_sig!("J"),
            JValue::Long(Box::into_raw(Box::new(class_index)) as jlong),
        )
        .expect("Unable to set field");

        convert_build_time_info(env, info)
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_createClassIndexFromJars(
    mut env: EnvUnowned<'_>,
    this: JObject,
    jar_names_list: JObject,
    target_java_release: jint,
) -> jobject {
    with_jni_env!(env, {
        propagate_error!(env, init_field_ids(env), JObject::null().into_raw());

        let java_list = env.cast_local::<JList>(jar_names_list).unwrap();
        let list_size = java_list.size(env).unwrap();
        let mut jar_names = Vec::with_capacity(list_size as usize);
        for index in 0..list_size {
            let ar = java_list.get(env, index).unwrap();
            let string = env.cast_local::<JString>(ar).unwrap();
            jar_names.push(string.try_to_string(env).expect("Not a string"));
        }
        let target_java_release = propagate_error!(
            env,
            require_positive_java_release(target_java_release),
            JObject::null().into_raw()
        );

        let (info, class_index) = propagate_error!(
            env,
            create_class_index_from_jars(jar_names, target_java_release),
            JObject::null().into_raw()
        );

        env.set_field(
            &this,
            jni_str!("classIndexPointer"),
            jni_sig!("J"),
            JValue::Long(Box::into_raw(Box::new(class_index)) as jlong),
        )
        .expect("Unable to set field");

        convert_build_time_info(env, info)
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_createClassIndexFromExplicitSources(
    mut env: EnvUnowned<'_>,
    this: JObject,
    jar_names_list: JObject,
    jar_source_ids_array: JObject,
    jar_input_orders_array: JObject,
    byte_array_list: JObject,
    class_source_ids_array: JObject,
    class_input_orders_array: JObject,
    target_java_release: jint,
) -> jobject {
    with_jni_env!(env, {
        propagate_error!(env, init_field_ids(env), JObject::null().into_raw());

        let java_jar_list = env.cast_local::<JList>(jar_names_list).unwrap();
        let jar_list_size = java_jar_list.size(env).unwrap();
        let mut jar_names = Vec::with_capacity(jar_list_size as usize);
        for index in 0..jar_list_size {
            let value = java_jar_list.get(env, index).unwrap();
            let string = env.cast_local::<JString>(value).unwrap();
            jar_names.push(string.try_to_string(env).expect("Not a string"));
        }
        let jar_source_ids = propagate_error!(
            env,
            read_nonnegative_int_array(
                env,
                jar_source_ids_array,
                jar_list_size as usize,
                "jarSourceIds"
            ),
            JObject::null().into_raw()
        );
        let jar_input_orders = propagate_error!(
            env,
            read_nonnegative_int_array(
                env,
                jar_input_orders_array,
                jar_list_size as usize,
                "jarInputOrders"
            ),
            JObject::null().into_raw()
        );

        let java_byte_list = env.cast_local::<JList>(byte_array_list).unwrap();
        let byte_list_size = java_byte_list.size(env).unwrap();
        let mut class_bytes = Vec::with_capacity(byte_list_size as usize);
        for index in 0..byte_list_size {
            let value = java_byte_list.get(env, index).unwrap();
            let byte_array = env.cast_local::<JByteArray>(value).unwrap();
            class_bytes.push(env.convert_byte_array(&byte_array).unwrap());
        }
        let class_source_ids = propagate_error!(
            env,
            read_nonnegative_int_array(
                env,
                class_source_ids_array,
                byte_list_size as usize,
                "classSourceIds"
            ),
            JObject::null().into_raw()
        );
        let class_input_orders = propagate_error!(
            env,
            read_nonnegative_int_array(
                env,
                class_input_orders_array,
                byte_list_size as usize,
                "classInputOrders"
            ),
            JObject::null().into_raw()
        );

        let target_java_release = propagate_error!(
            env,
            require_positive_java_release(target_java_release),
            JObject::null().into_raw()
        );
        let jar_sources = jar_names
            .into_iter()
            .zip(jar_source_ids)
            .zip(jar_input_orders)
            .map(|((file_name, source_id), input_order)| ArchiveSource {
                source_id,
                input_order,
                target_java_release,
                file_name,
            })
            .collect();
        let direct_sources = class_bytes
            .into_iter()
            .zip(class_source_ids)
            .zip(class_input_orders)
            .map(|((bytes, source_id), input_order)| DirectSource {
                source_id,
                input_order,
                bytes,
            })
            .collect();

        let (info, class_index) = propagate_error!(
            env,
            create_class_index_from_sources(jar_sources, direct_sources),
            JObject::null().into_raw()
        );

        env.set_field(
            &this,
            jni_str!("classIndexPointer"),
            jni_sig!("J"),
            JValue::Long(Box::into_raw(Box::new(class_index)) as jlong),
        )
        .expect("Unable to set field");

        convert_build_time_info(env, info)
    })
}

fn require_positive_java_release(release: jint) -> anyhow::Result<u32> {
    let release =
        u32::try_from(release).map_err(|_| anyhow!("targetJavaRelease must be positive"))?;
    ensure!(release > 0, "targetJavaRelease must be positive");
    Ok(release)
}

fn read_nonnegative_int_array(
    env: &mut Env<'_>,
    array: JObject<'_>,
    expected_length: usize,
    name: &str,
) -> anyhow::Result<Vec<u32>> {
    let array = env.cast_local::<JIntArray>(array)?;
    let length = array.len(env)?;
    ensure!(
        length == expected_length,
        "{} has length {}; expected {}",
        name,
        length,
        expected_length
    );
    let mut values = vec![0; length];
    array.get_region(env, 0, &mut values)?;
    values
        .into_iter()
        .map(|value| {
            u32::try_from(value).map_err(|_| anyhow!("{} contains a negative value", name))
        })
        .collect()
}

fn read_optional_source_ids(
    env: &mut Env<'_>,
    array: JObject<'_>,
) -> anyhow::Result<Option<Vec<u32>>> {
    if array.is_null() {
        return Ok(None);
    }
    let array = env.cast_local::<JIntArray>(array)?;
    let length = array.len(env)?;
    let mut values = vec![0; length];
    array.get_region(env, 0, &mut values)?;
    let mut values = values
        .into_iter()
        .map(|value| {
            u32::try_from(value).map_err(|_| anyhow!("sourceIds contains a negative value"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    values.sort_unstable();
    values.dedup();
    Ok(Some(values))
}

fn require_positive_limit(limit: jint) -> anyhow::Result<usize> {
    let limit = usize::try_from(limit).map_err(|_| anyhow!("limit must be positive"))?;
    ensure!(limit > 0, "limit must be positive");
    Ok(limit)
}

fn resolve_reference_target(
    class_index: &ClassIndex,
    target_kind: jint,
    owner_internal_name: &AsciiStr,
    name: &AsciiStr,
    descriptor: &AsciiStr,
) -> anyhow::Result<Option<SymbolId>> {
    let owner_name = owner_internal_name.as_str();
    let (package_name, class_name) = owner_name.rsplit_once('/').unwrap_or(("", owner_name));
    let Some(owner) = class_index.find_class(
        package_name
            .as_ascii_str()
            .expect("Validated owner package is ASCII"),
        class_name
            .as_ascii_str()
            .expect("Validated owner class is ASCII"),
    ) else {
        return Ok(None);
    };

    let target = match target_kind {
        0 => SymbolId::new(SymbolKind::Class, u64::from(owner.index())),
        1 => owner
            .fields()
            .iter()
            .enumerate()
            .find(|(member_index, field)| {
                field.field_name(class_index.constant_pool()) == name
                    && class_index.semantic_index().descriptor(
                        SymbolKind::Field,
                        owner.index(),
                        *member_index as u16,
                    ) == descriptor
            })
            .map(|(member_index, _)| {
                class_index.semantic_index().symbol_id(
                    SymbolKind::Field,
                    owner.index(),
                    member_index as u16,
                )
            })
            .transpose()?
            .ok_or_else(|| anyhow!("Reference target field was not found")),
        2 => owner
            .methods()
            .iter()
            .enumerate()
            .find(|(member_index, method)| {
                method.method_name(class_index.constant_pool()) == name
                    && class_index.semantic_index().descriptor(
                        SymbolKind::Method,
                        owner.index(),
                        *member_index as u16,
                    ) == descriptor
            })
            .map(|(member_index, _)| {
                class_index.semantic_index().symbol_id(
                    SymbolKind::Method,
                    owner.index(),
                    member_index as u16,
                )
            })
            .transpose()?
            .ok_or_else(|| anyhow!("Reference target method was not found")),
        _ => Err(anyhow!("Unknown reference target kind {target_kind}")),
    }?;
    Ok(Some(target))
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_saveToFileNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
    path: JString,
) {
    with_jni_env!(env, {
        let path = path.try_to_string(env).expect("Invalid path");

        let (_, class_index) = get_class_index(env, &this);

        propagate_error!(env, save_class_index_to_file(class_index, path));
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_loadClassIndexFromFile(
    mut env: EnvUnowned<'_>,
    this: JObject,
    path: JString,
) -> jobject {
    with_jni_env!(env, {
        propagate_error!(env, init_field_ids(env), JObject::null().into_raw());

        let path = path.try_to_string(env).expect("Invalid path");
        let (info, class_index) = propagate_error!(
            env,
            load_class_index_from_file(path),
            JObject::null().into_raw()
        );

        env.set_field(
            &this,
            jni_str!("classIndexPointer"),
            jni_sig!("J"),
            JValue::Long(Box::into_raw(Box::new(class_index)) as jlong),
        )
        .expect("Unable to set field");

        convert_build_time_info(env, info)
    })
}

unsafe fn convert_build_time_info(env: &mut Env<'_>, info: BuildTimeInfo) -> jobject {
    let result_class = env
        .find_class(jni_str!("com/github/tth05/jindex/BuildTimeInfo"))
        .expect("Unable to find class");
    env.new_object(
        &result_class,
        jni_sig!("(JJJ)V"),
        &[
            JValue::Long(info.deserialization_time as jlong),
            JValue::Long(info.class_reading_time as jlong),
            JValue::Long(info.indexing_time as jlong),
        ],
    )
    .expect("Unable to create object")
    .into_raw()
}

#[no_mangle]
/// # Safety
/// The pointer field has to identify a live class index.
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_getStatisticsNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jobject {
    with_jni_env!(env, {
        let (_, class_index) = get_class_index(env, &this);
        let result_class = env
            .find_class(jni_str!("com/github/tth05/jindex/IndexStatistics"))
            .expect("Unable to find statistics class");
        env.new_object(
            &result_class,
            jni_sig!("(JJJJJJ)V"),
            &[
                JValue::Long(class_index.class_count() as jlong),
                JValue::Long(class_index.semantic_index().field_count() as jlong),
                JValue::Long(class_index.semantic_index().method_count() as jlong),
                JValue::Long(class_index.semantic_index().reference_site_count() as jlong),
                JValue::Long(class_index.semantic_index().literal_count() as jlong),
                JValue::Long(class_index.semantic_index().literal_occurrence_count() as jlong),
            ],
        )
        .expect("Unable to create statistics")
        .into_raw()
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to identify a live class index.
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_getReferenceStorageStatisticsNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jobject {
    with_jni_env!(env, {
        let (_, class_index) = get_class_index(env, &this);
        let statistics = class_index.semantic_index().reference_storage_statistics();
        let result_class = env
            .find_class(jni_str!(
                "com/github/tth05/jindex/ReferenceStorageStatistics"
            ))
            .expect("Unable to find reference storage statistics class");
        env.new_object(
            &result_class,
            jni_sig!("(JJJJJJ)V"),
            &[
                JValue::Long(statistics.site_count as jlong),
                JValue::Long(statistics.occurrence_count as jlong),
                JValue::Long(statistics.single_occurrence_site_count as jlong),
                JValue::Long(statistics.count_over_255_site_count as jlong),
                JValue::Long(statistics.count_over_65535_site_count as jlong),
                JValue::Long(u64::from(statistics.maximum_occurrence_count) as jlong),
            ],
        )
        .expect("Unable to create reference storage statistics")
        .into_raw()
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to identify a live class index.
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findReferencesNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
    target_kind: jint,
    owner_internal_name: JString,
    name: JString,
    descriptor: JString,
    source_ids_array: JObject,
    limit: jint,
) -> jobject {
    with_jni_env!(env, {
        let owner_internal_name = java_to_ascii_string!(env, owner_internal_name);
        let name = java_to_ascii_string!(env, name);
        let descriptor = java_to_ascii_string!(env, descriptor);
        let source_ids = propagate_error!(
            env,
            read_optional_source_ids(env, source_ids_array),
            JObject::null().into_raw()
        );
        let limit = propagate_error!(
            env,
            require_positive_limit(limit),
            JObject::null().into_raw()
        );
        let (_, class_index) = get_class_index(env, &this);

        let target = propagate_error!(
            env,
            resolve_reference_target(
                class_index,
                target_kind,
                &owner_internal_name,
                &name,
                &descriptor,
            ),
            JObject::null().into_raw()
        );
        let Some(target) = target else {
            return Ok(create_reference_search_page(
                env,
                class_index,
                &[] as &[PackedReferenceSite],
                source_ids.as_deref(),
                limit,
                |_| 1,
            )?
            .into_raw());
        };
        let references = propagate_error!(
            env,
            class_index.semantic_index().references_to(target),
            JObject::null().into_raw()
        );
        create_reference_search_page(
            env,
            class_index,
            references,
            source_ids.as_deref(),
            limit,
            |reference| public_mask(target.kind(), reference.relation_mask()),
        )?
        .into_raw()
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to identify a live class index.
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_summarizeReferencesNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
    target_kind: jint,
    owner_internal_name: JString,
    name: JString,
    descriptor: JString,
) -> jobject {
    with_jni_env!(env, {
        let owner_internal_name = java_to_ascii_string!(env, owner_internal_name);
        let name = java_to_ascii_string!(env, name);
        let descriptor = java_to_ascii_string!(env, descriptor);
        let (_, class_index) = get_class_index(env, &this);
        let target = propagate_error!(
            env,
            resolve_reference_target(
                class_index,
                target_kind,
                &owner_internal_name,
                &name,
                &descriptor,
            ),
            JObject::null().into_raw()
        );

        let (site_count, occurrence_count) = if let Some(target) = target {
            let references = propagate_error!(
                env,
                class_index.semantic_index().references_to(target),
                JObject::null().into_raw()
            );
            (
                references.len() as u64,
                references
                    .iter()
                    .map(|reference| u64::from(reference.occurrence_count()))
                    .sum(),
            )
        } else {
            (0, 0)
        };

        let summary_class = env
            .find_class(jni_str!("com/github/tth05/jindex/ReferenceSummary"))
            .expect("ReferenceSummary class not found");
        env.new_object(
            &summary_class,
            jni_sig!("(JJ)V"),
            &[
                JValue::Long(site_count as jlong),
                JValue::Long(occurrence_count as jlong),
            ],
        )
        .expect("Unable to create ReferenceSummary")
        .into_raw()
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to identify a live class index.
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findLiteralReferencesNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
    literal: JString,
    source_ids_array: JObject,
    limit: jint,
) -> jobject {
    with_jni_env!(env, {
        let literal = propagate_error!(
            env,
            java_string_to_utf16(env, &literal),
            JObject::null().into_raw()
        );
        let source_ids = propagate_error!(
            env,
            read_optional_source_ids(env, source_ids_array),
            JObject::null().into_raw()
        );
        let limit = propagate_error!(
            env,
            require_positive_limit(limit),
            JObject::null().into_raw()
        );
        let (_, class_index) = get_class_index(env, &this);
        let references = class_index
            .semantic_index()
            .find_literal(&literal)
            .map(|literal| class_index.semantic_index().literal_references(literal))
            .unwrap_or_default();
        create_reference_search_page(
            env,
            class_index,
            references,
            source_ids.as_deref(),
            limit,
            |_| STRING_LITERAL_PUBLIC_MASK,
        )?
        .into_raw()
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to identify a live class index.
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findLiteralsContainingNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
    query: JString,
    source_ids_array: JObject,
    limit: jint,
) -> jobject {
    with_jni_env!(env, {
        let query = propagate_error!(
            env,
            java_string_to_utf16(env, &query),
            JObject::null().into_raw()
        );
        let limit = propagate_error!(
            env,
            usize::try_from(limit).map_err(anyhow::Error::from),
            JObject::null().into_raw()
        );
        let source_ids = propagate_error!(
            env,
            read_optional_source_ids(env, source_ids_array),
            JObject::null().into_raw()
        );
        let (_, class_index) = get_class_index(env, &this);
        let (matches, truncated) = class_index
            .semantic_index()
            .find_literals_containing_in_sources(&query, limit, source_ids.as_deref());
        let result_class =
            env.find_class(jni_str!("com/github/tth05/jindex/LiteralSearchResult"))?;
        let results = env.new_object_array(matches.len() as i32, &result_class, JObject::null())?;
        for (index, literal) in matches.into_iter().enumerate() {
            env.with_local_frame(6, |env| -> jni::errors::Result<()> {
                let value =
                    new_java_string_from_utf16(env, class_index.semantic_index().literal(literal))?;
                let source_ids = class_index
                    .semantic_index()
                    .literal_source_ids(literal)
                    .into_iter()
                    .filter(|source_id| {
                        source_ids
                            .as_ref()
                            .is_none_or(|selected| selected.binary_search(source_id).is_ok())
                    })
                    .map(|value| value as jint)
                    .collect::<Vec<_>>();
                let source_ids_array = env.new_int_array(source_ids.len())?;
                source_ids_array.set_region(env, 0, &source_ids)?;
                let result = env.new_object(
                    &result_class,
                    jni_sig!("(Ljava/lang/String;[I)V"),
                    &[JValue::Object(&value), JValue::Object(&source_ids_array)],
                )?;
                results.set_element(env, index, &result)
            })?;
        }
        let page_class = env.find_class(jni_str!("com/github/tth05/jindex/LiteralSearchPage"))?;
        env.new_object(
            &page_class,
            jni_sig!("([Lcom/github/tth05/jindex/LiteralSearchResult;Z)V"),
            &[JValue::Object(&results), JValue::Bool(truncated)],
        )?
        .into_raw()
    })
}

fn java_string_to_utf16(env: &Env<'_>, value: &JString<'_>) -> anyhow::Result<Vec<u16>> {
    let chars = value.mutf8_chars(env)?;
    Ok(mutf8::decode_utf16(chars.to_bytes())?)
}

fn new_java_string_from_utf16<'local>(
    env: &mut Env<'local>,
    value: &[u16],
) -> jni::errors::Result<JString<'local>> {
    let encoded = mutf8::encode_utf16(value);
    let encoded = CString::new(encoded).expect("Modified UTF-8 contains no zero bytes");
    let encoded = unsafe { JNIString::from_cstring(encoded) };
    JString::from_jni_str(env, &encoded)
}

trait StoredReferenceSite: Copy {
    fn storage_kind(self) -> u8;
    fn ordinal(self) -> u32;
    fn occurrence_count(self) -> u32;
    fn identity(self) -> u32;
}

impl StoredReferenceSite for PackedReferenceSite {
    fn storage_kind(self) -> u8 {
        self.storage_kind()
    }

    fn ordinal(self) -> u32 {
        self.ordinal()
    }

    fn occurrence_count(self) -> u32 {
        self.occurrence_count()
    }

    fn identity(self) -> u32 {
        self.identity()
    }
}

impl StoredReferenceSite for PackedLiteralSite {
    fn storage_kind(self) -> u8 {
        self.storage_kind()
    }

    fn ordinal(self) -> u32 {
        self.ordinal()
    }

    fn occurrence_count(self) -> u32 {
        self.occurrence_count()
    }

    fn identity(self) -> u32 {
        self.identity()
    }
}

fn create_reference_results<'local, R, F>(
    env: &mut Env<'local>,
    class_index: &ClassIndex,
    references: &[R],
    relation_mask: F,
) -> jni::errors::Result<JObjectArray<'local>>
where
    R: StoredReferenceSite,
    F: Fn(R) -> u16,
{
    let result_class = env
        .find_class(jni_str!("com/github/tth05/jindex/ReferenceResult"))
        .expect("Reference result class not found");
    let result_array =
        env.new_object_array(references.len() as i32, &result_class, JObject::null())?;

    for (index, reference) in references.iter().enumerate() {
        env.with_local_frame(8, |env| -> jni::errors::Result<()> {
            let (kind, owner_index, name, descriptor) = match reference.storage_kind() {
                0 => (
                    ReferenceSiteKind::Class,
                    reference.ordinal(),
                    env.new_string("")?,
                    env.new_string("")?,
                ),
                1 | 2 => {
                    let symbol_kind = if reference.storage_kind() == 1 {
                        SymbolKind::Field
                    } else {
                        SymbolKind::Method
                    };
                    let member = class_index
                        .semantic_index()
                        .member_from_ordinal(symbol_kind, reference.ordinal())
                        .expect("Stored reference site has an invalid member ordinal");
                    let owner = class_index.class_at_index(member.class_index());
                    let member_name = match symbol_kind {
                        SymbolKind::Field => owner.fields()[member.member_index() as usize]
                            .field_name(class_index.constant_pool()),
                        SymbolKind::Method => owner.methods()[member.member_index() as usize]
                            .method_name(class_index.constant_pool()),
                        SymbolKind::Class => unreachable!(),
                    };
                    (
                        if matches!(symbol_kind, SymbolKind::Field) {
                            ReferenceSiteKind::Field
                        } else {
                            ReferenceSiteKind::Method
                        },
                        member.class_index(),
                        env.new_string(member_name)?,
                        env.new_string(class_index.semantic_index().descriptor(
                            symbol_kind,
                            member.class_index(),
                            member.member_index(),
                        ))?,
                    )
                }
                3 => {
                    let extra = class_index.semantic_index().extra_site(reference.ordinal());
                    let kind = match extra.kind {
                        1 => ReferenceSiteKind::Field,
                        2 => ReferenceSiteKind::Method,
                        3 => ReferenceSiteKind::RecordComponent,
                        value => panic!("Unknown extra reference-site kind {value}"),
                    };
                    (
                        kind,
                        extra.owner_class_index,
                        env.new_string(class_index.semantic_index().extra_site_name(extra))?,
                        env.new_string(class_index.semantic_index().extra_site_descriptor(extra))?,
                    )
                }
                value => panic!("Unknown reference-site storage kind {value}"),
            };
            let site_owner = class_index.class_at_index(owner_index);
            let owner_name = env.new_string(site_owner.class_name_with_package(
                class_index.package_index(),
                class_index.constant_pool(),
            ))?;
            let object = env.new_object(
                &result_class,
                jni_sig!("(JILjava/lang/String;Ljava/lang/String;Ljava/lang/String;IIJ)V"),
                &[
                    JValue::Long(i64::from(reference.identity())),
                    JValue::Int(kind as i32),
                    JValue::Object(&owner_name),
                    JValue::Object(&name),
                    JValue::Object(&descriptor),
                    JValue::Int(class_index.semantic_index().class_source_id(owner_index) as i32),
                    JValue::Int(i32::from(relation_mask(*reference))),
                    JValue::Long(i64::from(reference.occurrence_count())),
                ],
            )?;
            result_array.set_element(env, index, &object)?;
            Ok(())
        })?;
    }

    Ok(result_array)
}

fn create_reference_search_page<'local, R, F>(
    env: &mut Env<'local>,
    class_index: &ClassIndex,
    references: &[R],
    source_ids: Option<&[u32]>,
    limit: usize,
    relation_mask: F,
) -> jni::errors::Result<JObject<'local>>
where
    R: StoredReferenceSite,
    F: Fn(R) -> u16 + Copy,
{
    let mut selected = Vec::with_capacity(references.len().min(limit));
    let mut truncated = false;
    for reference in references {
        let owner_index = reference_owner_class_index(class_index, *reference);
        if source_ids.is_some_and(|source_ids| {
            source_ids
                .binary_search(&class_index.semantic_index().class_source_id(owner_index))
                .is_err()
        }) {
            continue;
        }
        if selected.len() == limit {
            truncated = true;
            break;
        }
        selected.push(*reference);
    }

    let results = create_reference_results(env, class_index, &selected, relation_mask)?;
    let page_class = env.find_class(jni_str!("com/github/tth05/jindex/ReferenceSearchPage"))?;
    env.new_object(
        &page_class,
        jni_sig!("([Lcom/github/tth05/jindex/ReferenceResult;Z)V"),
        &[JValue::Object(&results), JValue::Bool(truncated)],
    )
}

fn reference_owner_class_index<R: StoredReferenceSite>(
    class_index: &ClassIndex,
    reference: R,
) -> u32 {
    match reference.storage_kind() {
        0 => reference.ordinal(),
        1 | 2 => class_index
            .semantic_index()
            .member_from_ordinal(
                if reference.storage_kind() == 1 {
                    SymbolKind::Field
                } else {
                    SymbolKind::Method
                },
                reference.ordinal(),
            )
            .expect("Stored reference site has an invalid member ordinal")
            .class_index(),
        3 => {
            class_index
                .semantic_index()
                .extra_site(reference.ordinal())
                .owner_class_index
        }
        value => panic!("Unknown reference-site storage kind {value}"),
    }
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findClassesNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
    input: JString,
    options: JObject,
    source_ids_array: JObject,
) -> jobject {
    with_jni_env!(env, {
        let input = java_to_ascii_string!(env, input);

        let (class_index_pointer, class_index) = get_class_index(env, &this);
        let source_ids = propagate_error!(
            env,
            read_optional_source_ids(env, source_ids_array),
            JObject::null().into_raw()
        );

        let (classes, truncated) = class_index.find_classes(
            &input,
            propagate_error!(
                env,
                convert_search_options(env, options),
                JObject::null().into_raw()
            ),
            source_ids.as_deref(),
        );

        create_class_search_page(env, class_index_pointer, classes, truncated)?.into_raw()
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to identify a live class index.
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findClassesByBinaryNameNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
    input: JString,
    options: JObject,
    source_ids_array: JObject,
) -> jobject {
    with_jni_env!(env, {
        let input = java_to_ascii_string!(env, input);

        let (class_index_pointer, class_index) = get_class_index(env, &this);
        let source_ids = propagate_error!(
            env,
            read_optional_source_ids(env, source_ids_array),
            JObject::null().into_raw()
        );
        let (classes, truncated) = class_index.find_classes_by_binary_name(
            &input,
            propagate_error!(
                env,
                convert_search_options(env, options),
                JObject::null().into_raw()
            ),
            source_ids.as_deref(),
        );

        create_class_search_page(env, class_index_pointer, classes, truncated)?.into_raw()
    })
}

fn create_class_search_page<'local>(
    env: &mut Env<'local>,
    class_index_pointer: jlong,
    classes: Vec<&IndexedClass>,
    truncated: bool,
) -> jni::errors::Result<JObject<'local>> {
    let result_class = env.find_class(jni_str!("com/github/tth05/jindex/IndexedClass"))?;
    let result_array =
        env.new_object_array(classes.len() as i32, &result_class, JObject::null())?;
    for (index, class) in classes.into_iter().enumerate() {
        env.with_local_frame(1, |env| -> jni::errors::Result<()> {
            let object = env.new_object(
                &result_class,
                jni_sig!("(JJ)V"),
                &[
                    JValue::Long(class_index_pointer),
                    JValue::Long((class as *const IndexedClass) as jlong),
                ],
            )?;
            result_array.set_element(env, index, &object)
        })?;
    }

    let page_class = env.find_class(jni_str!("com/github/tth05/jindex/ClassSearchPage"))?;
    env.new_object(
        &page_class,
        jni_sig!("([Lcom/github/tth05/jindex/IndexedClass;Z)V"),
        &[JValue::Object(&result_array), JValue::Bool(truncated)],
    )
}

#[no_mangle]
/// # Safety
/// The pointer field has to identify a live class index.
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findSymbolsNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
    input: JString,
    options: JObject,
    kind_mask: jni::sys::jint,
    source_ids_array: JObject,
) -> jobject {
    with_jni_env!(env, {
        let input = java_to_ascii_string!(env, input);
        let result_class = env
            .find_class(jni_str!("com/github/tth05/jindex/SymbolSearchResult"))
            .expect("Result class not found");
        let (_, class_index) = get_class_index(env, &this);
        let source_ids = propagate_error!(
            env,
            read_optional_source_ids(env, source_ids_array),
            JObject::null().into_raw()
        );
        let (results, truncated) = class_index.semantic_index().find_members(
            class_index,
            &input,
            propagate_error!(
                env,
                convert_search_options(env, options),
                JObject::null().into_raw()
            ),
            kind_mask & 1 != 0,
            kind_mask & 2 != 0,
            source_ids.as_deref(),
        );
        let result_array = env
            .new_object_array(results.len() as i32, &result_class, JObject::null())
            .expect("Failed to create result array");

        for (index, result) in results.into_iter().enumerate() {
            env.with_local_frame(8, |env| -> jni::errors::Result<()> {
                let owner = class_index.class_at_index(result.member.class_index());
                let (name, access_flags) = match result.kind {
                    SymbolKind::Field => {
                        let field = &owner.fields()[result.member.member_index() as usize];
                        (
                            field.field_name(class_index.constant_pool()),
                            field.access_flags(),
                        )
                    }
                    SymbolKind::Method => {
                        let method = &owner.methods()[result.member.member_index() as usize];
                        (
                            method.method_name(class_index.constant_pool()),
                            method.access_flags(),
                        )
                    }
                    SymbolKind::Class => unreachable!(),
                };
                let symbol_id = class_index
                    .semantic_index()
                    .symbol_id(
                        result.kind,
                        result.member.class_index(),
                        result.member.member_index(),
                    )
                    .expect("Invalid symbol ordinal");
                let owner_name = env.new_string(owner.class_name_with_package(
                    class_index.package_index(),
                    class_index.constant_pool(),
                ))?;
                let name = env.new_string(name)?;
                let descriptor = env.new_string(class_index.semantic_index().descriptor(
                    result.kind,
                    result.member.class_index(),
                    result.member.member_index(),
                ))?;
                let object = env.new_object(
                    &result_class,
                    jni_sig!("(JILjava/lang/String;Ljava/lang/String;Ljava/lang/String;II)V"),
                    &[
                        JValue::Long(symbol_id.as_u64() as jlong),
                        JValue::Int(result.kind as i32),
                        JValue::Object(&owner_name),
                        JValue::Object(&name),
                        JValue::Object(&descriptor),
                        JValue::Int(
                            class_index.semantic_index().class_source_id(owner.index()) as i32
                        ),
                        JValue::Int(access_flags as i32),
                    ],
                )?;
                result_array.set_element(env, index, &object)?;
                Ok(())
            })
            .expect("Failed to create symbol result");
        }

        let page_class = env
            .find_class(jni_str!("com/github/tth05/jindex/SymbolSearchPage"))
            .expect("Symbol search page class not found");
        env.new_object(
            &page_class,
            jni_sig!("([Lcom/github/tth05/jindex/SymbolSearchResult;Z)V"),
            &[
                JValue::Object(result_array.as_ref()),
                JValue::Bool(truncated),
            ],
        )
        .expect("Unable to create symbol search page")
        .into_raw()
    })
}

unsafe fn convert_search_options(
    env: &mut Env<'_>,
    options: JObject<'_>,
) -> anyhow::Result<SearchOptions> {
    if options.is_null() {
        return Ok(SearchOptions::default());
    }

    let match_mode_object = env
        .get_field(
            &options,
            jni_str!("matchMode"),
            jni_sig!("Lcom/github/tth05/jindex/SearchOptions$MatchMode;"),
        )
        .expect("Field not found")
        .l()
        .unwrap();
    let match_mode = match get_enum_ordinal(env, match_mode_object) {
        0 => MatchMode::IgnoreCase,
        1 => MatchMode::MatchCase,
        2 => MatchMode::MatchCaseFirstCharOnly,
        _ => return Err(anyhow!("Invalid enum ordinal for match mode")),
    };

    let search_mode_object = env
        .get_field(
            &options,
            jni_str!("searchMode"),
            jni_sig!("Lcom/github/tth05/jindex/SearchOptions$SearchMode;"),
        )
        .expect("Field not found")
        .l()
        .unwrap();
    let search_mode = match get_enum_ordinal(env, search_mode_object) {
        0 => SearchMode::Prefix,
        1 => SearchMode::Contains,
        _ => return Err(anyhow!("Invalid enum ordinal for search mode")),
    };

    let limit = env
        .get_field(&options, jni_str!("limit"), jni_sig!("I"))
        .expect("Field not found")
        .i()
        .unwrap();
    let limit = usize::try_from(limit).map_err(|_| anyhow!("Search limit must not be negative"))?;

    Ok(SearchOptions {
        limit,
        match_mode,
        search_mode,
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findClassNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
    i_package_name: JString,
    i_class_name: JString,
) -> jobject {
    with_jni_env!(env, {
        let class_name = java_to_ascii_string!(env, i_class_name);
        let package_name =
            java_to_ascii_string!(env, i_package_name, |s: String| s.replace('.', "/"));

        let result_class = env
            .find_class(jni_str!("com/github/tth05/jindex/IndexedClass"))
            .expect("Result class not found");

        let (class_index_pointer, class_index) = get_class_index(env, &this);

        if let Some(class) = class_index.find_class(&package_name, &class_name) {
            env.new_object(
                &result_class,
                jni_sig!("(JJ)V"),
                &[
                    JValue::from(class_index_pointer as jlong),
                    JValue::from((class as *const IndexedClass) as jlong),
                ],
            )
            .expect("Failed to create result object")
            .into_raw()
        } else {
            JObject::null().into_raw()
        }
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findPackageNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
    i_package_name: JString,
) -> jobject {
    with_jni_env!(env, {
        let package_name =
            java_to_ascii_string!(env, i_package_name, |s: String| s.replace('.', "/"));

        let result_class = env
            .find_class(jni_str!("com/github/tth05/jindex/IndexedPackage"))
            .expect("Result class not found");

        let (class_index_pointer, class_index) = get_class_index(env, &this);

        if let Some(package) = class_index.find_package(&package_name) {
            env.new_object(
                &result_class,
                jni_sig!("(JJ)V"),
                &[
                    JValue::from(class_index_pointer as jlong),
                    JValue::from((package as *const IndexedPackage) as jlong),
                ],
            )
            .expect("Failed to create result object")
            .into_raw()
        } else {
            JObject::null().into_raw()
        }
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findPackagesNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
    query: JString,
) -> jobject {
    with_jni_env!(env, {
        let query = java_to_ascii_string!(env, query, |s: String| s.replace('.', "/"));

        let result_class = env
            .find_class(jni_str!("com/github/tth05/jindex/IndexedPackage"))
            .expect("Result class not found");

        let (class_index_pointer, class_index) = get_class_index(env, &this);

        let matching_packages = class_index.find_packages(&query);
        let result_array = env
            .new_object_array(
                matching_packages.len() as i32,
                &result_class,
                JObject::null(),
            )
            .expect("Failed to create result array");

        for (index, package) in matching_packages.iter().enumerate() {
            let obj = env
                .new_object(
                    &result_class,
                    jni_sig!("(JJ)V"),
                    &[
                        JValue::from(class_index_pointer as jlong),
                        JValue::from((*package as *const IndexedPackage) as jlong),
                    ],
                )
                .expect("Failed to create result object");

            result_array
                .set_element(env, index, &obj)
                .expect("Failed to set result array element");
        }

        result_array.into_raw()
    })
}
