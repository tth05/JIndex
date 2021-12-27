use crate::class_index::{IndexedClass, IndexedMethod};
use crate::ClassIndex;
use jni::objects::{JObject, JValue};
use jni::sys::{jlong, jobject, jobjectArray, jstring};
use jni::JNIEnv;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getName(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let class_index = &*(env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap() as *mut ClassIndex);
    let indexed_class =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedClass);

    env.new_string(indexed_class.class_name(class_index.constant_pool()))
        .unwrap()
        .into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getNameWithPackage(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let class_index = &*(env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap() as *mut ClassIndex);
    let indexed_class =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedClass);

    env.new_string(indexed_class.class_name_with_package(class_index.constant_pool()))
        .unwrap()
        .into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getMethods(
    env: JNIEnv,
    this: jobject,
) -> jobjectArray {
    let class_index_pointer = env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap() as *mut ClassIndex;

    let indexed_class =
        &mut *(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedClass);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedMethod")
        .expect("Result class not found");

    let result_array = env
        .new_object_array(
            indexed_class.method_count() as i32,
            result_class,
            JObject::null(),
        )
        .expect("Failed to create result array");

    for (index, method) in indexed_class.methods().iter().enumerate() {
        let object = env
            .new_object(
                result_class,
                "(JJ)V",
                &[
                    JValue::from(class_index_pointer as jlong),
                    JValue::from((method as *const IndexedMethod) as jlong),
                ],
            )
            .expect("Failed to create result object");
        env.set_object_array_element(result_array, index as i32, object)
            .expect("Failed to set element into result array");
    }

    result_array
}
