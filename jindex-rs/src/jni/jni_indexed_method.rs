use crate::class_index::ClassIndex;
use crate::class_index_members::{IndexedClass, IndexedMethod};
use crate::jni::cache::{cached_field_ids, get_class_index, get_field_with_id};
use crate::jni::{collect_type_parameters, is_basic_signature_type};
use crate::signature::indexed_signature::{ToDescriptorIndexedType, ToSignatureIndexedType};
use crate::signature::{IndexedMethodSignature, IndexedSignatureType, TypeParameterData};
use jni::objects::{JObject, JValue};
use jni::sys::{jint, jlong, jobject, jobjectArray, jsize, jstring};
use jni::JNIEnv;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getName(
    env: JNIEnv,
    this: JObject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_method = get_field_with_id::<IndexedMethod>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

    env.new_string(indexed_method.method_name(class_index.constant_pool()))
        .unwrap()
        .into_raw()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getDeclaringClass(
    env: JNIEnv,
    this: JObject,
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
    .into_raw()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getAccessFlags(
    env: JNIEnv,
    this: JObject,
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
    this: JObject,
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

    let type_parameters = collect_method_type_parameters(class_index, indexed_class, signature);

    env.new_string(signature.to_descriptor_string(
        class_index,
        //TODO: Pass generic data of super classes
        &type_parameters,
    ))
    .expect("Unable to create generic signature String")
    .into_raw()
}

unsafe fn collect_method_type_parameters<'a>(
    class_index: &'a ClassIndex,
    indexed_class: &'a IndexedClass,
    signature: &'a IndexedMethodSignature,
) -> Vec<&'a TypeParameterData<u32>> {
    let mut type_parameters = Vec::new();
    if let Some(vec) = signature.generic_data() {
        type_parameters.extend(vec);
    }

    collect_type_parameters(indexed_class, class_index, &mut type_parameters);
    type_parameters
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getGenericSignatureString(
    env: JNIEnv,
    this: JObject,
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
        && signature
            .parameters()
            .map(|v| !v.iter().any(|s| !is_basic_signature_type(s)))
            .unwrap_or(true)
        && is_basic_signature_type(signature.return_type())
        && signature
            .exceptions()
            .map_or(true, |v| !v.iter().any(|s| !is_basic_signature_type(s)))
    {
        return JObject::null().into_raw();
    }

    env.new_string(signature.to_signature_string(class_index))
        .expect("Unable to create generic signature String")
        .into_raw()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getExceptions(
    env: JNIEnv,
    this: JObject,
) -> jobjectArray {
    let (class_index_pointer, class_index) = get_class_index(env, this);

    let indexed_method = get_field_with_id::<IndexedMethod>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().class_child_class_pointer);

    let exceptions = indexed_method.method_signature().exceptions();
    let array_length = exceptions.map_or(0, |v| v.len());

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    let result_array = env
        .new_object_array(array_length as jsize, result_class, JObject::null())
        .expect("Failed to create result array");

    if array_length == 0 {
        return result_array;
    }

    for (index, exception_signature) in exceptions.unwrap().iter().enumerate() {
        let exception_class_index =
            exception_signature
                .extract_base_object_type()
                .or_else(|| match exception_signature {
                    IndexedSignatureType::Generic(_) => {
                        let generic_data = collect_method_type_parameters(
                            class_index,
                            indexed_class,
                            indexed_method.method_signature(),
                        );

                        exception_signature
                            .resolve_generic_type_bound(class_index, &generic_data)
                            .and_then(|s| s.extract_base_object_type())
                    }
                    _ => Option::None,
                });
        if exception_class_index.is_none() {
            continue;
        }

        let class = class_index.class_at_index(exception_class_index.unwrap());

        let object = env
            .new_object(
                result_class,
                "(JJ)V",
                &[
                    JValue::from(class_index_pointer as jlong),
                    JValue::from((class as *const IndexedClass) as jlong),
                ],
            )
            .expect("Failed to create result object");
        env.set_object_array_element(result_array, index as i32, object)
            .expect("Failed to set element into result array");
    }

    result_array
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_findImplementations(
    env: JNIEnv,
    this: JObject,
) -> jobjectArray {
    let (class_index_pointer, class_index) = get_class_index(env, this);

    let indexed_method = get_field_with_id::<IndexedMethod>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().class_child_class_pointer);

    let impls = class_index.find_implementations_of_method(indexed_class.index(), indexed_method);

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

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_findBaseMethods(
    env: JNIEnv,
    this: JObject,
) -> jobjectArray {
    let (class_index_pointer, class_index) = get_class_index(env, this);

    let indexed_method = get_field_with_id::<IndexedMethod>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().class_child_class_pointer);

    let impls = class_index.find_base_methods_of_method(indexed_class, indexed_method);

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
