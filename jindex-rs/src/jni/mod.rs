use crate::class_index::ClassIndex;
use crate::class_index_members::IndexedClass;
use crate::signature::{IndexedSignatureType, IndexedTypeParameterData, SignatureType};
use ascii::AsAsciiStr;
use jni::objects::JObject;
use jni::{jni_sig, jni_str, Env};

const ACC_STATIC: u16 = 0x0008;

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

unsafe fn get_enum_ordinal(env: &mut Env<'_>, enum_object: JObject) -> u32 {
    env.call_method(enum_object, jni_str!("ordinal"), jni_sig!("()I"), &[])
        .expect("Failed to call ordinal")
        .i()
        .unwrap() as u32
}

macro_rules! with_jni_env {
    ($env:ident, $body:block) => {{
        $env.with_env(|$env| -> jni::errors::Result<_> { Ok(unsafe { $body }) })
            .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
    }};
}

pub(crate) use with_jni_env;

macro_rules! propagate_error {
    ($env:ident, $result:expr) => {
        propagate_error!($env, $result, ())
    };
    ($env:ident, $result:expr, $return_value:expr) => {
        match $result {
            Ok(value) => value,
            Err(error) => {
                match $env.throw_new(
                    jni_str!("com/github/tth05/jindex/ClassIndexBuildingException"),
                    JNIString::new(format!("{error:#}")),
                ) {
                    Err(jni::errors::Error::JavaException) => {}
                    other => panic!("Failed to throw ClassIndexBuildingException: {:?}", other),
                }
                return Ok($return_value);
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
    if current_class.access_flags() & ACC_STATIC != 0 {
        return;
    }

    if let Some(enclosing_class) = current_class.enclosing_class(class_index) {
        collect_type_parameters(enclosing_class, class_index, type_parameters);
    }
}
