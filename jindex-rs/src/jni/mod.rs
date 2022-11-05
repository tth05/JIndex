use crate::class_index::ClassIndex;
use crate::class_index_members::IndexedClass;
use crate::signature::{IndexedSignatureType, IndexedTypeParameterData, SignatureType};
use ascii::AsAsciiStr;
use cafebabe::attributes::InnerClassAccessFlags;
use jni::objects::JObject;
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

unsafe fn get_enum_ordinal(env: JNIEnv, enum_object: JObject) -> u32 {
    env.call_method(enum_object, "ordinal", "()I", &[])
        .expect("Failed to call ordinal")
        .i()
        .unwrap() as u32
}

macro_rules! propagate_error {
    ($env:ident, $result:expr) => {
        propagate_error!($env, $result, ())
    };
    ($env:ident, $result:expr, $return_value:expr) => {
        match $result {
            Ok(value) => value,
            Err(error) => {
                $env.throw_new(
                    "com/github/tth05/jindex/ClassIndexBuildingException",
                    error.to_string(),
                )
                .expect("Failed to throw exception");
                return $return_value;
            }
        }
    };
}

pub(crate) use propagate_error;

fn is_basic_signature_type(s: &IndexedSignatureType) -> bool {
    match s {
        SignatureType::Array(inner) => is_basic_signature_type(inner),
        SignatureType::Unresolved | SignatureType::Primitive(_) | SignatureType::Object(_) => true,
        _ => false,
    }
}

fn collect_type_parameters<'a>(
    current_class: &'a IndexedClass,
    class_index: &'a ClassIndex,
    type_parameters: &mut Vec<&'a IndexedTypeParameterData>,
) {
    if let Some(vec) = current_class.signature().generic_data() {
        type_parameters.extend(vec);
    }

    // Don't check enclosing classes for static inner classes
    if current_class.access_flags() & InnerClassAccessFlags::STATIC.bits() != 0 {
        return;
    }

    if let Some(enclosing_class) = current_class.enclosing_class(class_index) {
        collect_type_parameters(enclosing_class, class_index, type_parameters);
    }
}
