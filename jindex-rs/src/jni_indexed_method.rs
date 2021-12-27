use crate::class_index::IndexedMethod;
use crate::ClassIndex;
use jni::sys::{jobject, jstring};
use jni::JNIEnv;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getName(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let class_index = &*(env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap() as *mut ClassIndex);
    let indexed_method =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedMethod);

    env.new_string(indexed_method.method_name(class_index.constant_pool()))
        .unwrap()
        .into_inner()
}
