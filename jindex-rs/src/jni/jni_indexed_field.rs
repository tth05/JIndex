use jni::objects::JObject;
use jni::sys::{jobject, jshort, jstring};
use jni::JNIEnv;

use crate::class_index::{IndexedClass, IndexedField};
use crate::jni::{cached_field_ids, get_class_index, get_field_with_id, is_basic_signature_type};
use crate::signature::indexed_signature::{ToDescriptorIndexedType, ToSignatureIndexedType};

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedField_getName(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(
        env,
        this,
        &cached_field_ids().indexed_field_index_pointer_id,
    );
    let indexed_field =
        get_field_with_id::<IndexedField>(env, this, &cached_field_ids().indexed_field_pointer_id);

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
    let indexed_field =
        get_field_with_id::<IndexedField>(env, this, &cached_field_ids().indexed_field_pointer_id);

    indexed_field.access_flags() as jshort
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedField_getDescriptorString(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(
        env,
        this,
        &cached_field_ids().indexed_field_index_pointer_id,
    );
    let indexed_field =
        get_field_with_id::<IndexedField>(env, this, &cached_field_ids().indexed_field_pointer_id);
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().indexed_field_class_pointer_id,
    );
    let signature = indexed_field.field_signature();

    let mut type_parameters = Vec::new();
    if let Some(vec) = indexed_class.signature().generic_data() {
        type_parameters.extend(vec);
    }

    env.new_string(signature.to_descriptor_string(
        class_index,
        //TODO: Pass generic data of super classes
        &type_parameters,
    ))
    .expect("Unable to create generic signature String")
    .into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedField_getGenericSignatureString(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(
        env,
        this,
        &cached_field_ids().indexed_field_index_pointer_id,
    );
    let indexed_method =
        get_field_with_id::<IndexedField>(env, this, &cached_field_ids().indexed_field_pointer_id);
    let signature = indexed_method.field_signature();

    //No generic signature available
    if is_basic_signature_type(signature) {
        return JObject::null().into_inner();
    }

    env.new_string(signature.to_signature_string(class_index))
        .expect("Unable to create descriptor String")
        .into_inner()
}
