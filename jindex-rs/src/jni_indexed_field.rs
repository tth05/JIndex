use jni::sys::{jobject, jshort, jstring};
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

    match indexed_field.field_signature() {
        IndexedSignature::Primitive(p_index) => {
            env.new_string(p_index.to_string()).unwrap().into_inner()
        }
        IndexedSignature::Object(index) => env
            .new_string(
                class_index
                    .class_at_index(*index)
                    .class_name_with_package(&class_index.constant_pool()),
            )
            .unwrap()
            .into_inner(),
        _ => env.new_string("--Unresolved--").unwrap().into_inner(),
    }
}
