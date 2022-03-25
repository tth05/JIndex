use ascii::AsAsciiStr;
use jni::objects::{JObject, JString, JValue};
use jni::sys::{jint, jlong, jobject, jobjectArray};
use jni::JNIEnv;

use crate::class_index::{
    create_class_index, create_class_index_from_jars, ClassIndex, IndexedClass,
};
use crate::io::{load_class_index_from_file, save_class_index_to_file};
use crate::jni::get_class_index;

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
pub extern "system" fn Java_com_github_tth05_jindex_ClassIndex_createClassIndex(
    env: JNIEnv,
    this: jobject,
    byte_array_list: jobject,
) {
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
pub extern "system" fn Java_com_github_tth05_jindex_ClassIndex_createClassIndexFromJars(
    env: JNIEnv,
    this: jobject,
    jar_names_list: jobject,
) {
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

    let (class_index_pointer, class_index) = get_class_index(env, this);

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

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findClasses(
    env: JNIEnv,
    this: jobject,
    input: JString,
    limit: jint,
) -> jobjectArray {
    let input: String = env
        .get_string(input)
        .expect("Couldn't get java string!")
        .into();

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    let (class_index_pointer, class_index) = get_class_index(env, this);

    let classes: Vec<_> = class_index
        .find_classes(input.as_ascii_str().unwrap(), limit as usize)
        .expect("Find classes failed");

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

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findClass(
    env: JNIEnv,
    this: jobject,
    i_package_name: JString,
    i_class_name: JString,
) -> jobject {
    let class_name: String = env
        .get_string(i_class_name)
        .expect("Couldn't get java string!")
        .into();
    let package_name: String = env
        .get_string(i_package_name)
        .expect("Couldn't get java string!")
        .into();

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    let (class_index_pointer, class_index) = get_class_index(env, this);

    if let Some((_, class)) = class_index.find_class(
        package_name.replace(".", "/").as_ascii_str().unwrap(),
        class_name.as_ascii_str().unwrap(),
    ) {
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
