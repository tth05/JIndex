use crate::class_index::{ClassIndex, IndexedClass};
use crate::signature::{IndexedSignatureType, SignatureType};
use ascii::AsAsciiStr;
use jni::sys::jobject;
use jni::JNIEnv;

mod cache;
pub mod jni_class_index;
pub mod jni_indexed_class;
pub mod jni_indexed_field;
pub mod jni_indexed_method;
pub mod jni_indexed_package;

unsafe fn get_java_lang_object(class_index: &ClassIndex) -> Option<&IndexedClass> {
    class_index.find_class(
        "java/lang".as_ascii_str_unchecked(),
        "Object".as_ascii_str_unchecked(),
    )
}

unsafe fn get_enum_ordinal(env: JNIEnv, enum_object: jobject) -> u32 {
    env.call_method(enum_object, "ordinal", "()I", &[])
        .expect("Failed to call ordinal")
        .i()
        .unwrap() as u32
}

fn is_basic_signature_type(s: &IndexedSignatureType) -> bool {
    match s {
        SignatureType::Array(inner) => is_basic_signature_type(inner),
        SignatureType::Unresolved | SignatureType::Primitive(_) | SignatureType::Object(_) => true,
        _ => false,
    }
}
