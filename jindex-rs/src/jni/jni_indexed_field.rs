use jni::objects::JValue;
use jni::sys::{jlong, jobject, jshort, jstring};
use jni::JNIEnv;

use crate::class_index::{IndexedField, IndexedSignature};
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
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedField_getTypeSignature(
    env: JNIEnv,
    this: jobject,
) -> jobject {
    let class_index_pointer = env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap();

    let indexed_field =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedField);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedSignature")
        .expect("Result class not found");

    env.new_object(
        result_class,
        "(JJ)V",
        &[
            JValue::Long(class_index_pointer),
            JValue::Long((indexed_field.field_signature() as *const IndexedSignature) as jlong),
        ],
    )
    .expect("Failed to create instance")
    .into_inner()
}
