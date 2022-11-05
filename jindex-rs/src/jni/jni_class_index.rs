use crate::builder::workers::{create_class_index_from_bytes, create_class_index_from_jars};
use crate::builder::BuildTimeInfo;
use anyhow::anyhow;
use ascii::IntoAsciiString;
use jni::objects::{JObject, JString, JValue};
use jni::sys::{jlong, jobject, jobjectArray};
use jni::JNIEnv;
use std::ops::Deref;

use crate::class_index::ClassIndex;
use crate::class_index_members::IndexedClass;
use crate::constant_pool::{MatchMode, SearchMode, SearchOptions};
use crate::io::{load_class_index_from_file, save_class_index_to_file};
use crate::jni::cache::{cached_field_ids, get_class_index, init_field_ids};
use crate::jni::{get_enum_ordinal, propagate_error};
use crate::package_index::IndexedPackage;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_destroy(
    env: JNIEnv,
    this: JObject,
) {
    let _class_index = Box::from_raw(get_class_index(env, this).0 as *mut ClassIndex);

    env.set_field_unchecked(
        this,
        cached_field_ids().class_index_pointer,
        JValue::Long(0i64),
    )
    .expect("Unable to set field");
    env.set_field(this, "destroyed", "Z", JValue::Bool(1))
        .expect("Unable to set field");
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_createClassIndex(
    env: JNIEnv,
    this: JObject,
    byte_array_list: JObject,
) -> jobject {
    propagate_error!(env, init_field_ids(env), JObject::null().into_raw());

    let java_list = env.get_list(byte_array_list).unwrap();
    let mut class_bytes = Vec::with_capacity(java_list.size().unwrap() as usize);
    for ar in java_list.iter().unwrap() {
        class_bytes.push(env.convert_byte_array(ar.cast()).unwrap());
    }

    let (info, class_index) = propagate_error!(
        env,
        create_class_index_from_bytes(class_bytes),
        JObject::null().into_raw()
    );

    env.set_field(
        this,
        "classIndexPointer",
        "J",
        JValue::Long(Box::into_raw(Box::new(class_index)) as jlong),
    )
    .expect("Unable to set field");

    convert_build_time_info(env, info)
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_createClassIndexFromJars(
    env: JNIEnv,
    this: JObject,
    jar_names_list: JObject,
) -> jobject {
    propagate_error!(env, init_field_ids(env), JObject::null().into_raw());

    let java_list = env.get_list(jar_names_list).unwrap();
    let mut jar_names = Vec::with_capacity(java_list.size().unwrap() as usize);
    for ar in java_list.iter().unwrap() {
        jar_names.push(
            env.get_string(JString::from(ar))
                .expect("Not a string")
                .into(),
        );
    }

    let (info, class_index) = propagate_error!(
        env,
        create_class_index_from_jars(jar_names),
        JObject::null().into_raw()
    );

    env.set_field(
        this,
        "classIndexPointer",
        "J",
        JValue::Long(Box::into_raw(Box::new(class_index)) as jlong),
    )
    .expect("Unable to set field");

    convert_build_time_info(env, info)
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_saveToFile(
    env: JNIEnv,
    this: JObject,
    path: JString,
) {
    let path: String = env.get_string(path).expect("Invalid path").into();

    let (_, class_index) = get_class_index(env, this);

    propagate_error!(env, save_class_index_to_file(class_index, path));
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_loadClassIndexFromFile(
    env: JNIEnv,
    this: JObject,
    path: JString,
) -> jobject {
    propagate_error!(env, init_field_ids(env), JObject::null().into_raw());

    let path: String = env.get_string(path).expect("Invalid path").into();
    let (info, class_index) = propagate_error!(
        env,
        load_class_index_from_file(path),
        JObject::null().into_raw()
    );

    env.set_field(
        this,
        "classIndexPointer",
        "J",
        JValue::Long(Box::into_raw(Box::new(class_index)) as jlong),
    )
    .expect("Unable to set field");

    convert_build_time_info(env, info)
}

unsafe fn convert_build_time_info(env: JNIEnv, info: BuildTimeInfo) -> jobject {
    env.new_object(
        env.find_class("com/github/tth05/jindex/BuildTimeInfo")
            .expect("Unable to find class"),
        "(JJJJ)V",
        &[
            JValue::Long(info.deserialization_time as jlong),
            JValue::Long(info.file_reading_time as jlong),
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
        let env_str: String = $mapper($env.get_string($jstring).expect("Not a string").into());

        match env_str.into_ascii_string() {
            Ok(s) => s,
            Err(e) => {
                $env.throw_new(
                    "java/lang/IllegalArgumentException",
                    format!("'{}' is not an ASCII string", e.into_source()),
                )
                .expect("Unable to throw exception");
                return JObject::null().into_raw();
            }
        }
    }};
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findClasses(
    env: JNIEnv,
    this: JObject,
    input: JString,
    options: JObject,
) -> jobjectArray {
    let input = java_to_ascii_string!(&env, input);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    let (class_index_pointer, class_index) = get_class_index(env, this);

    let classes: Vec<_> = class_index.find_classes(
        &input,
        propagate_error!(
            env,
            convert_search_options(env, options),
            JObject::null().into_raw()
        ),
    );

    let result_array = env
        .new_object_array(classes.len() as i32, result_class, JObject::null())
        .expect("Failed to create result array");
    for (index, class) in classes.into_iter().enumerate() {
        let object = env
            .new_object(
                result_class,
                "(JJ)V",
                &[
                    JValue::from(class_index_pointer as jlong),
                    JValue::from((class as *const IndexedClass) as jlong),
                ],
            )
            .expect("Failed to create result object");
        env.set_object_array_element(result_array, index as i32, object)
            .expect("Failed to set element into result array");
    }

    result_array
}

unsafe fn convert_search_options(env: JNIEnv, options: JObject) -> anyhow::Result<SearchOptions> {
    if options.is_null() {
        return Ok(SearchOptions::default());
    }

    let match_mode = match get_enum_ordinal(
        env,
        env.get_field(
            options,
            "matchMode",
            "Lcom/github/tth05/jindex/SearchOptions$MatchMode;",
        )
        .expect("Field not found")
        .l()
        .unwrap(),
    ) {
        0 => MatchMode::IgnoreCase,
        1 => MatchMode::MatchCase,
        2 => MatchMode::MatchCaseFirstCharOnly,
        _ => return Err(anyhow!("Invalid enum ordinal for match mode")),
    };

    let search_mode = match get_enum_ordinal(
        env,
        env.get_field(
            options,
            "searchMode",
            "Lcom/github/tth05/jindex/SearchOptions$SearchMode;",
        )
        .expect("Field not found")
        .l()
        .unwrap(),
    ) {
        0 => SearchMode::Prefix,
        1 => SearchMode::Contains,
        _ => return Err(anyhow!("Invalid enum ordinal for search mode")),
    };

    let limit = env
        .get_field(options, "limit", "I")
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
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findClass(
    env: JNIEnv,
    this: JObject,
    i_package_name: JString,
    i_class_name: JString,
) -> jobject {
    let class_name = java_to_ascii_string!(&env, i_class_name);
    let package_name = java_to_ascii_string!(&env, i_package_name, |s: String| s.replace('.', "/"));

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    let (class_index_pointer, class_index) = get_class_index(env, this);

    if let Some(class) = class_index.find_class(&package_name, &class_name) {
        env.new_object(
            result_class,
            "(JJ)V",
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
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findPackage(
    env: JNIEnv,
    this: JObject,
    i_package_name: JString,
) -> jobject {
    let package_name = java_to_ascii_string!(&env, i_package_name, |s: String| s.replace('.', "/"));

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedPackage")
        .expect("Result class not found");

    let (class_index_pointer, class_index) = get_class_index(env, this);

    if let Some(package) = class_index.find_package(&package_name) {
        env.new_object(
            result_class,
            "(JJ)V",
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
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findPackages(
    env: JNIEnv,
    this: JObject,
    query: JString,
) -> jobject {
    let query = java_to_ascii_string!(&env, query, |s: String| s.replace('.', "/"));

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedPackage")
        .expect("Result class not found");

    let (class_index_pointer, class_index) = get_class_index(env, this);

    let matching_packages = class_index.find_packages(&query);
    let result_array = env
        .new_object_array(
            matching_packages.len() as i32,
            result_class,
            JObject::null(),
        )
        .expect("Failed to create result array");

    for (index, package) in matching_packages.iter().enumerate() {
        let obj = env
            .new_object(
                result_class,
                "(JJ)V",
                &[
                    JValue::from(class_index_pointer as jlong),
                    JValue::from((package.deref() as *const IndexedPackage) as jlong),
                ],
            )
            .expect("Failed to create result object");

        env.set_object_array_element(result_array, index as i32, obj)
            .expect("Failed to set result array element");
    }

    result_array
}
