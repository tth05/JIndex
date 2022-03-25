use crate::class_index::ClassIndex;
use jni::sys::{jlong, jobject};
use jni::JNIEnv;

pub mod jni_class_index;
pub mod jni_indexed_class;
pub mod jni_indexed_field;
pub mod jni_indexed_method;
pub mod jni_indexed_signature;

unsafe fn get_pointer_field<T>(env: JNIEnv, this: jobject) -> &T {
    &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut T)
}

unsafe fn get_class_index(env: JNIEnv, this: jobject) -> (jlong, &ClassIndex) {
    let class_index_pointer = env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap();
    let class_index = &*(class_index_pointer as *const ClassIndex);
    (class_index_pointer, class_index)
}
