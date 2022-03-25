use crate::class_index::IndexedClass;
use crate::jni::{get_class_index, get_pointer_field};
use crate::signature::IndexedSignatureType;
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
    let indexed_signature = get_pointer_field::<IndexedSignatureType>(env, this);

    matches!(indexed_signature, IndexedSignatureType::Array(_)).into()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedSignature_isPrimitive(
    env: JNIEnv,
    this: jobject,
) -> jboolean {
    let indexed_signature = get_pointer_field::<IndexedSignatureType>(env, this);

    matches!(indexed_signature, IndexedSignatureType::Primitive(_)).into()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedSignature_isVoid(
    env: JNIEnv,
    this: jobject,
) -> jboolean {
    let indexed_signature = get_pointer_field::<IndexedSignatureType>(env, this);

    matches!(
        indexed_signature,
        IndexedSignatureType::Primitive(jni::signature::Primitive::Void)
    )
    .into()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedSignature_isUnresolved(
    env: JNIEnv,
    this: jobject,
) -> jboolean {
    let indexed_signature = get_pointer_field::<IndexedSignatureType>(env, this);

    matches!(indexed_signature, IndexedSignatureType::Unresolved).into()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedSignature_getType(
    env: JNIEnv,
    this: jobject,
) -> jobject {
    let (class_index_pointer, class_index) = get_class_index(env, this);
    let indexed_signature = get_pointer_field::<IndexedSignatureType>(env, this);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    match indexed_signature {
        IndexedSignatureType::Object(index) => {
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
    let indexed_signature = get_pointer_field::<IndexedSignatureType>(env, this);

    match indexed_signature {
        IndexedSignatureType::Primitive(index) => {
            let primitive_name = match index {
                jni::signature::Primitive::Boolean => "boolean",
                jni::signature::Primitive::Byte => "byte",
                jni::signature::Primitive::Char => "char",
                jni::signature::Primitive::Double => "double",
                jni::signature::Primitive::Float => "float",
                jni::signature::Primitive::Int => "int",
                jni::signature::Primitive::Long => "long",
                jni::signature::Primitive::Short => "short",
                //TODO: Void?
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
    let (class_index_pointer, _) = get_class_index(env, this);
    let indexed_signature = get_pointer_field::<IndexedSignatureType>(env, this);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedSignature")
        .expect("Result class not found");

    match indexed_signature {
        IndexedSignatureType::Array(component) => env
            .new_object(
                result_class,
                "(JJ)V",
                &[
                    JValue::Long(class_index_pointer),
                    JValue::Long(((&**component) as *const IndexedSignatureType) as jlong),
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
    let (_, _) = get_class_index(env, this);
    let _ = get_pointer_field::<IndexedSignatureType>(env, this);

    //TODO: Signature into string
    env.new_string("" /*indexed_signature.signature_string(class_index)*/)
        .expect("Unable to create String")
        .into_inner()
}
