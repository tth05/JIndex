use crate::class_index::{ClassIndex, IndexedClass};
use crate::signature::{IndexedSignatureType, SignatureType};
use ascii::AsAsciiStr;

mod cache;
pub mod jni_class_index;
pub mod jni_indexed_class;
pub mod jni_indexed_field;
pub mod jni_indexed_method;

unsafe fn get_java_lang_object(class_index: &ClassIndex) -> Option<&IndexedClass> {
    class_index
        .find_class(
            "java/lang".as_ascii_str_unchecked(),
            "Object".as_ascii_str_unchecked(),
        )
        .map(|p| p.1)
}

fn is_basic_signature_type(s: &IndexedSignatureType) -> bool {
    match s {
        SignatureType::Array(inner) => is_basic_signature_type(inner),
        SignatureType::Unresolved | SignatureType::Primitive(_) | SignatureType::Object(_) => true,
        _ => false,
    }
}
