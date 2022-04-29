use crate::class_index::{IndexedClass, IndexedMethod};
use crate::jni::cache::{cached_field_ids, get_class_index, get_field_with_id};
use crate::jni::is_basic_signature_type;
use crate::signature::indexed_signature::{ToDescriptorIndexedType, ToSignatureIndexedType};
use jni::objects::{JObject, JValue};
use jni::sys::{jboolean, jint, jlong, jobject, jobjectArray, jstring};
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
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getDeclaringClass(
    env: JNIEnv,
    this: jobject,
) -> jobject {
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().class_child_class_pointer);
    let (class_index_pointer, _) = get_class_index(env, this);

    env.new_object(
        env.find_class("com/github/tth05/jindex/IndexedClass")
            .expect("Result class not found"),
        "(JJ)V",
        &[
            JValue::from(class_index_pointer as jlong),
            JValue::from((indexed_class as *const IndexedClass) as jlong),
        ],
    )
    .expect("Failed to create result object")
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

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_findImplementations(
    env: JNIEnv,
    this: jobject,
    include_base_method: jboolean,
) -> jobjectArray {
    let (class_index_pointer, class_index) = get_class_index(env, this);

    let indexed_method = get_field_with_id::<IndexedMethod>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().class_child_class_pointer);

    let impls = class_index.find_implementations_of_method(
        indexed_class.index(),
        indexed_method,
        include_base_method != 0,
    );

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedMethod")
        .expect("Result class not found");

    let result_array = env
        .new_object_array(impls.len() as i32, result_class, JObject::null())
        .expect("Failed to create result array");

    for (index, (declaring_class, result_method)) in impls.iter().enumerate() {
        let object = env
            .new_object(
                result_class,
                "(JJJ)V",
                &[
                    JValue::from(class_index_pointer as jlong),
                    JValue::from((*declaring_class as *const IndexedClass) as jlong),
                    JValue::from((*result_method as *const IndexedMethod) as jlong),
                ],
            )
            .expect("Failed to create result object");
        env.set_object_array_element(result_array, index as i32, object)
            .expect("Failed to set element into result array");
    }

    result_array
}
