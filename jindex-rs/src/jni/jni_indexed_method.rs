use crate::class_index::{IndexedClass, IndexedMethod};
use crate::jni::cache::{cached_field_ids, get_class_index, get_field_with_id};
use crate::jni::is_basic_signature_type;
use crate::signature::indexed_signature::{ToDescriptorIndexedType, ToSignatureIndexedType};
use jni::objects::JObject;
use jni::sys::{jint, jobject, jstring};
use jni::JNIEnv;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getName(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_method = get_field_with_id::<IndexedMethod>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

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
) -> jint {
    let indexed_method = get_field_with_id::<IndexedMethod>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

    indexed_method.access_flags() as jint
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getDescriptorString(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_method = get_field_with_id::<IndexedMethod>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().class_child_class_pointer);
    let signature = indexed_method.method_signature();

    let mut type_parameters = Vec::new();
    if let Some(vec) = signature.generic_data() {
        type_parameters.extend(vec);
    }
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
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getGenericSignatureString(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_method = get_field_with_id::<IndexedMethod>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );
    let signature = indexed_method.method_signature();

    //No generic signature available
    if signature.generic_data().is_none()
        && signature.parameters().iter().all(is_basic_signature_type)
        && is_basic_signature_type(signature.return_type())
    {
        return JObject::null().into_inner();
    }

    env.new_string(signature.to_signature_string(class_index))
        .expect("Unable to create generic signature String")
        .into_inner()
}
