use jni::objects::JValue;
use jni::sys::{jlong, jobject, jshort, jstring};
use jni::JNIEnv;

use crate::class_index::{IndexedClass, IndexedField};
use crate::ClassIndex;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedField_getName(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let class_index = &*(env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap() as *mut ClassIndex);
    let indexed_field =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedField);

    env.new_string(indexed_field.field_name(&class_index.constant_pool()))
        .unwrap()
        .into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedField_getAccessFlags(
    env: JNIEnv,
    this: jobject,
) -> jshort {
    let indexed_field =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedField);

    indexed_field.access_flags() as jshort
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedField_getType(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let class_index = &*(env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap() as *mut ClassIndex);

    let indexed_field =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedField);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    /*env.new_object(
        result_class,
        "(JJ)V",
        &[
            JValue::from(class_index_pointer as jlong),
            JValue::from((class_index.get_class_at as *const IndexedClass) as jlong),
        ],
    )
        .expect("Failed to create result object")
        .into_inner()*/
    if indexed_field.type_class_index() < 0 {
        env.new_string(indexed_field.type_class_index().to_string())
            .unwrap()
            .into_inner()
    } else {
        env.new_string(
            class_index
                .class_at_index(indexed_field.type_class_index() as u32)
                .class_name_with_package(&class_index.constant_pool()),
        )
        .unwrap()
        .into_inner()
    }
}
