use crate::builder::workers::{create_class_index_from_bytes, create_class_index_from_jars};
use crate::builder::BuildTimeInfo;
use anyhow::anyhow;
use ascii::IntoAsciiString;
use jni::objects::{JByteArray, JList, JObject, JString, JValue};
use jni::strings::JNIString;
use jni::sys::{jlong, jobject, jobjectArray};
use jni::{jni_sig, jni_str, Env, EnvUnowned};
use std::ops::Deref;

use crate::class_index::ClassIndex;
use crate::class_index_members::IndexedClass;
use crate::constant_pool::{MatchMode, SearchMode, SearchOptions};
use crate::io::{load_class_index_from_file, save_class_index_to_file};
use crate::jni::cache::{get_class_index, init_field_ids};
use crate::jni::{get_enum_ordinal, propagate_error, with_jni_env};
use crate::package_index::IndexedPackage;

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

        let (info, class_index) = propagate_error!(
            env,
            create_class_index_from_jars(jar_names),
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

macro_rules! java_to_ascii_string {
    ($env:expr, $jstring:ident) => {
        java_to_ascii_string!($env, $jstring, |s| s)
    };
    ($env:expr, $jstring:ident, $mapper:expr) => {{
        let env_str: String = $mapper($jstring.try_to_string($env).expect("Not a string"));

        match env_str.into_ascii_string() {
            Ok(s) => s,
            Err(e) => {
                $env.throw_new(
                    jni_str!("java/lang/IllegalArgumentException"),
                    JNIString::new(format!("'{}' is not an ASCII string", e.into_source())),
                )
                .expect("Unable to throw exception");
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
