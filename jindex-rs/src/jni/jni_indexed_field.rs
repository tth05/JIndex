use jni::objects::JObject;
use jni::sys::{jobject, jshort, jstring};
use jni::JNIEnv;

use crate::class_index::IndexedField;
use crate::jni::{get_class_index, get_pointer_field, is_basic_signature_type};
use crate::signature::indexed_signature::ToStringIndexedType;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedField_getName(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_field = get_pointer_field::<IndexedField>(env, this);

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
    let indexed_field = get_pointer_field::<IndexedField>(env, this);

    indexed_field.access_flags() as jshort
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedField_getGenericSignatureString(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_method = get_pointer_field::<IndexedField>(env, this);
    let signature = indexed_method.field_signature();

    //No generic signature available
    if is_basic_signature_type(signature) {
        return JObject::null().into_inner();
    }

    env.new_string(signature.to_string(class_index))
        .expect("Unable to create descriptor String")
        .into_inner()
}
