use ascii::IntoAsciiString;
use jni::objects::{JObject, JString, JValue};
use jni::sys::{jlong, jobject, jobjectArray};
use jni::JNIEnv;
use std::ops::Deref;

use crate::class_index::{
    create_class_index, create_class_index_from_jars, ClassIndex, IndexedClass, IndexedPackage,
};
use crate::constant_pool::{MatchMode, SearchMode, SearchOptions};
use crate::io::{load_class_index_from_file, save_class_index_to_file};
use crate::jni::cache::{cached_field_ids, get_class_index, init_field_ids};
use crate::jni::get_enum_ordinal;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_destroy(
    env: JNIEnv,
    this: jobject,
) {
    let _class_index = Box::from_raw(
        env.get_field(this, "classIndexPointer", "J")
            .unwrap()
            .j()
            .unwrap() as *mut ClassIndex,
    );

    env.set_field(this, "classIndexPointer", "J", JValue::Long(0i64))
        .expect("Unable to set field");
    env.set_field(this, "destroyed", "Z", JValue::Bool(1))
        .expect("Unable to set field");
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_createClassIndex(
    env: JNIEnv,
    this: jobject,
    byte_array_list: jobject,
) {
    init_field_ids(env);

    let java_list = env.get_list(byte_array_list.into()).unwrap();
    let mut class_bytes = Vec::with_capacity(java_list.size().unwrap() as usize);
    for ar in java_list.iter().unwrap() {
        class_bytes.push(env.convert_byte_array(ar.cast()).unwrap());
    }

    let class_index = create_class_index(class_bytes);

    env.set_field(
        this,
        "classIndexPointer",
        "J",
        JValue::Long(Box::into_raw(Box::new(class_index)) as jlong),
    )
    .expect("Unable to set field");
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_createClassIndexFromJars(
    env: JNIEnv,
    this: jobject,
    jar_names_list: jobject,
) {
    init_field_ids(env);

    let java_list = env.get_list(jar_names_list.into()).unwrap();
    let mut jar_names = Vec::with_capacity(java_list.size().unwrap() as usize);
    for ar in java_list.iter().unwrap() {
        jar_names.push(
            env.get_string(JString::from(ar))
                .expect("Not a string")
                .into(),
        );
    }

    let class_index = create_class_index_from_jars(jar_names);

    env.set_field(
        this,
        "classIndexPointer",
        "J",
        JValue::Long(Box::into_raw(Box::new(class_index)) as jlong),
    )
    .expect("Unable to set field");
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_saveToFile(
    env: JNIEnv,
    this: jobject,
    path: JString,
) {
    let path: String = env.get_string(path).expect("Invalid path").into();

    let (_, class_index) = get_class_index(env, this, &cached_field_ids().class_index_pointer_id);

    save_class_index_to_file(class_index, path);
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_loadClassIndexFromFile(
    env: JNIEnv,
    this: jobject,
    path: JString,
) {
    init_field_ids(env);

    let path: String = env.get_string(path).expect("Invalid path").into();
    let class_index = load_class_index_from_file(path);

    env.set_field(
        this,
        "classIndexPointer",
        "J",
        JValue::Long(Box::into_raw(Box::new(class_index)) as jlong),
    )
    .expect("Unable to set field");
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
                return JObject::null().into_inner();
            }
        }
    }};
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findClasses(
    env: JNIEnv,
    this: jobject,
    input: JString,
    options: jobject,
) -> jobjectArray {
    let input = java_to_ascii_string!(&env, input);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    let (class_index_pointer, class_index) =
        get_class_index(env, this, &cached_field_ids().class_index_pointer_id);

    let classes: Vec<_> = class_index.find_classes(&input, convert_search_options(env, options));

    let result_array = env
        .new_object_array(classes.len() as i32, result_class, JObject::null())
        .expect("Failed to create result array");
    for (index, (_, class)) in classes.into_iter().enumerate() {
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

unsafe fn convert_search_options(env: JNIEnv, options: jobject) -> SearchOptions {
    if options.is_null() {
        return SearchOptions::default();
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
        .unwrap()
        .into_inner(),
    ) {
        0 => MatchMode::IgnoreCase,
        1 => MatchMode::MatchCase,
        2 => MatchMode::MatchCaseFirstCharOnly,
        _ => panic!("Invalid enum ordinal for match mode"),
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
        .unwrap()
        .into_inner(),
    ) {
        0 => SearchMode::Prefix,
        1 => SearchMode::Contains,
        _ => panic!("Invalid enum ordinal for match mode"),
    };

    let limit = env
        .get_field(options, "limit", "I")
        .expect("Field not found")
        .i()
        .unwrap();

    SearchOptions {
        limit: limit as usize,
        match_mode,
        search_mode,
    }
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findClass(
    env: JNIEnv,
    this: jobject,
    i_package_name: JString,
    i_class_name: JString,
) -> jobject {
    let class_name = java_to_ascii_string!(&env, i_class_name);
    let package_name = java_to_ascii_string!(&env, i_package_name, |s: String| s.replace('.', "/"));

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    let (class_index_pointer, class_index) =
        get_class_index(env, this, &cached_field_ids().class_index_pointer_id);

    if let Some((_, class)) = class_index.find_class(&package_name, &class_name) {
        env.new_object(
            result_class,
            "(JJ)V",
            &[
                JValue::from(class_index_pointer as jlong),
                JValue::from((class as *const IndexedClass) as jlong),
            ],
        )
        .expect("Failed to create result object")
        .into_inner()
    } else {
        JObject::null().into_inner()
    }
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findPackage(
    env: JNIEnv,
    this: jobject,
    i_package_name: JString,
) -> jobject {
    let package_name = java_to_ascii_string!(&env, i_package_name, |s: String| s.replace('.', "/"));

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedPackage")
        .expect("Result class not found");

    let (class_index_pointer, class_index) =
        get_class_index(env, this, &cached_field_ids().class_index_pointer_id);

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
        .into_inner()
    } else {
        JObject::null().into_inner()
    }
}
