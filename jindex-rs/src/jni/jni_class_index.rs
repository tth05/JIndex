use crate::builder::workers::{
    create_class_index_from_bytes, create_class_index_from_jars, create_class_index_from_sources,
    ArchiveSource, DirectSource,
};
use crate::builder::BuildTimeInfo;
use anyhow::{anyhow, ensure};
use ascii::IntoAsciiString;
use jni::objects::{JByteArray, JIntArray, JList, JObject, JString, JValue};
use jni::strings::JNIString;
use jni::sys::{jint, jlong, jobject, jobjectArray};
use jni::{jni_sig, jni_str, Env, EnvUnowned};
use std::ops::Deref;

use crate::class_index::ClassIndex;
use crate::class_index_members::IndexedClass;
use crate::constant_pool::{MatchMode, SearchMode, SearchOptions};
use crate::io::{load_class_index_from_file, save_class_index_to_file};
use crate::jni::cache::{get_class_index, init_field_ids};
use crate::jni::{get_enum_ordinal, propagate_error, with_jni_env};
use crate::package_index::IndexedPackage;
use crate::semantic_index::SymbolKind;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_destroyPointer(
    _env: EnvUnowned<'_>,
    _class: JObject,
    pointer: jlong,
) {
    let _class_index = Box::from_raw(pointer as *mut ClassIndex);
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
                JValue::Long(0),
                JValue::Long(0),
                JValue::Long(0),
            ],
        )
        .expect("Unable to create statistics")
        .into_raw()
    })
}

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
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findClassesNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
    input: JString,
    options: JObject,
) -> jobjectArray {
    with_jni_env!(env, {
        let input = java_to_ascii_string!(env, input);

        let result_class = env
            .find_class(jni_str!("com/github/tth05/jindex/IndexedClass"))
            .expect("Result class not found");

        let (class_index_pointer, class_index) = get_class_index(env, &this);

        let classes: Vec<_> = class_index.find_classes(
            &input,
            propagate_error!(
                env,
                convert_search_options(env, options),
                JObject::null().into_raw()
            ),
        );

        let result_array = env
            .new_object_array(classes.len() as i32, &result_class, JObject::null())
            .expect("Failed to create result array");
        for (index, class) in classes.into_iter().enumerate() {
            let object = env
                .new_object(
                    &result_class,
                    jni_sig!("(JJ)V"),
                    &[
                        JValue::from(class_index_pointer as jlong),
                        JValue::from((class as *const IndexedClass) as jlong),
                    ],
                )
                .expect("Failed to create result object");
            result_array
                .set_element(env, index, &object)
                .expect("Failed to set element into result array");
        }

        result_array.into_raw()
    })
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
) -> jobjectArray {
    with_jni_env!(env, {
        let input = java_to_ascii_string!(env, input);
        let result_class = env
            .find_class(jni_str!("com/github/tth05/jindex/SymbolSearchResult"))
            .expect("Result class not found");
        let (_, class_index) = get_class_index(env, &this);
        let results = class_index.semantic_index().find_members(
            class_index,
            &input,
            propagate_error!(
                env,
                convert_search_options(env, options),
                JObject::null().into_raw()
            ),
            kind_mask & 1 != 0,
            kind_mask & 2 != 0,
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

        result_array.into_raw()
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

    Ok(SearchOptions {
        limit: limit as usize,
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
                    JValue::from((package.deref() as *const IndexedPackage) as jlong),
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
                        JValue::from((package.deref() as *const IndexedPackage) as jlong),
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
