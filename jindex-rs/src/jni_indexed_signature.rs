use crate::class_index::{IndexedClass, IndexedSignature};
use crate::ClassIndex;
use jni::objects::{JObject, JValue};
use jni::sys::{jboolean, jclass, jlong, jobject, jstring};
use jni::JNIEnv;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedSignature_isArray(
    env: JNIEnv,
    this: jobject,
) -> jboolean {
    let indexed_signature =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedSignature);

    matches!(indexed_signature, IndexedSignature::Array(_)).into()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedSignature_isPrimitive(
    env: JNIEnv,
    this: jobject,
) -> jboolean {
    let indexed_signature =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedSignature);

    matches!(indexed_signature, IndexedSignature::Primitive(_)).into()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedSignature_isVoid(
    env: JNIEnv,
    this: jobject,
) -> jboolean {
    let indexed_signature =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedSignature);

    matches!(indexed_signature, IndexedSignature::Void).into()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedSignature_isUnresolved(
    env: JNIEnv,
    this: jobject,
) -> jboolean {
    let indexed_signature =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedSignature);

    matches!(indexed_signature, IndexedSignature::Unresolved).into()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedSignature_getType(
    env: JNIEnv,
    this: jobject,
) -> jobject {
    let class_index_pointer = env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap();
    let class_index = &*(class_index_pointer as *const ClassIndex);

    let indexed_signature =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedSignature);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    match indexed_signature {
        IndexedSignature::Object(index) => {
            let pointer = class_index.class_at_index(*index) as *const IndexedClass;
            env.new_object(
                result_class,
                "(JJ)V",
                &[
                    JValue::Long(class_index_pointer),
                    JValue::Long(pointer as jlong),
                ],
            )
            .expect("Failed to create result object")
            .into_inner()
        }
        _ => JObject::null().into_inner(),
    }
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedSignature_getPrimitiveType(
    env: JNIEnv,
    this: jobject,
) -> jclass {
    let indexed_signature =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedSignature);

    match indexed_signature {
        IndexedSignature::Primitive(index) => {
            let primitive_name = match index {
                0 => "boolean",
                1 => "byte",
                2 => "char",
                3 => "double",
                4 => "float",
                5 => "int",
                6 => "long",
                7 => "short",
                _ => unreachable!(),
            };

            env.call_static_method(
                env.find_class("java/lang/Class").expect("Class not found"),
                "getPrimitiveClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(
                    env.new_string(primitive_name)
                        .expect("Unable to create parameter string")
                        .into(),
                )],
            )
            .expect("Call to getPrimitiveValue failed")
            .l()
            .unwrap()
            .into_inner()
        }
        _ => JObject::null().into_inner(),
    }
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedSignature_getArrayComponent(
    env: JNIEnv,
    this: jobject,
) -> jobject {
    let class_index_pointer = env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap();
    let class_index = &*(class_index_pointer as *const ClassIndex);

    let indexed_signature =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedSignature);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedSignature")
        .expect("Result class not found");

    match indexed_signature {
        IndexedSignature::Array(component) => env
            .new_object(
                result_class,
                "(JJ)V",
                &[
                    JValue::Long(class_index_pointer),
                    JValue::Long(((&**component) as *const IndexedSignature) as jlong),
                ],
            )
            .expect("Failed to create result object")
            .into_inner(),
        _ => JObject::null().into_inner(),
    }
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedSignature_toSignatureString(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let class_index = &*(env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap() as *const ClassIndex);

    let indexed_signature =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedSignature);

    env.new_string(signature_to_string(indexed_signature, class_index))
        .expect("Unable to create String")
        .into_inner()
}

fn signature_to_string(sig: &IndexedSignature, class_index: &ClassIndex) -> String {
    match sig {
        IndexedSignature::Primitive(i) => match i {
            0 => String::from("Z"),
            1 => String::from("B"),
            2 => String::from("C"),
            3 => String::from("D"),
            4 => String::from("F"),
            5 => String::from("I"),
            6 => String::from("J"),
            7 => String::from("S"),
            _ => unreachable!(),
        },
        IndexedSignature::Object(index) => {
            let mut result = String::from("L;");
            result.insert_str(
                1,
                class_index
                    .class_at_index(*index)
                    .class_name_with_package(&class_index.constant_pool())
                    .as_ref(),
            );
            result
        }
        IndexedSignature::Array(sig) => String::from("[") + &signature_to_string(sig, class_index),
        IndexedSignature::Void => String::from("V"),
        IndexedSignature::Unresolved => String::from(""),
    }
}
