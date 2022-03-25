use crate::class_index::IndexedMethod;
use crate::jni::{get_class_index, get_pointer_field, is_basic_signature_type};
use crate::signature::indexed_signature::ToStringIndexedType;
use jni::objects::JObject;
use jni::sys::{jobject, jshort, jstring};
use jni::JNIEnv;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getName(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_method = get_pointer_field::<IndexedMethod>(env, this);

    env.new_string(indexed_method.method_name(&class_index.constant_pool()))
        .unwrap()
        .into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getAccessFlags(
    env: JNIEnv,
    this: jobject,
) -> jshort {
    let indexed_method = get_pointer_field::<IndexedMethod>(env, this);

    indexed_method.access_flags() as jshort
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getGenericSignatureString(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_method = get_pointer_field::<IndexedMethod>(env, this);
    let signature = indexed_method.method_signature();

    //No generic signature available
    if signature.generic_data().is_none()
        && signature.parameters().iter().all(is_basic_signature_type)
        && is_basic_signature_type(signature.return_type())
    {
        return JObject::null().into_inner();
    }

    env.new_string(signature.to_string(class_index))
        .expect("Unable to create descriptor String")
        .into_inner()
}
