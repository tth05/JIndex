use crate::class_index_members::{IndexedClass, IndexedField};
use jni::objects::JObject;
use jni::sys::{jint, jstring};
use jni::JNIEnv;

use crate::jni::cache::{cached_field_ids, get_class_index, get_field_with_id};
use crate::jni::{collect_type_parameters, is_basic_signature_type};
use crate::signature::indexed_signature::{ToDescriptorIndexedType, ToSignatureIndexedType};

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedField_getName(
    env: JNIEnv,
    this: JObject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_field = get_field_with_id::<IndexedField>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

    env.new_string(indexed_field.field_name(class_index.constant_pool()))
        .unwrap()
        .into_raw()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedField_getAccessFlags(
    env: JNIEnv,
    this: JObject,
) -> jint {
    let indexed_field = get_field_with_id::<IndexedField>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

    indexed_field.access_flags() as jint
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedField_getDescriptorString(
    env: JNIEnv,
    this: JObject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_field = get_field_with_id::<IndexedField>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().class_child_class_pointer);
    let signature = indexed_field.field_signature();

    let mut type_parameters = Vec::new();
    collect_type_parameters(indexed_class, class_index, &mut type_parameters);

    env.new_string(signature.to_descriptor_string(
        class_index,
        //TODO: Pass generic data of super classes
        &type_parameters,
    ))
    .expect("Unable to create generic signature String")
    .into_raw()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedField_getGenericSignatureString(
    env: JNIEnv,
    this: JObject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_method = get_field_with_id::<IndexedField>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );
    let signature = indexed_method.field_signature();

    //No generic signature available
    if is_basic_signature_type(signature) {
        return JObject::null().into_raw();
    }

    env.new_string(signature.to_signature_string(class_index))
        .expect("Unable to create descriptor String")
        .into_raw()
}
