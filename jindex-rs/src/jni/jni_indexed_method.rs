use crate::class_index::{ClassIndex, MethodWithClass};
use crate::class_index_members::{IndexedClass, IndexedMethod};
use crate::jni::cache::{cached_field_ids, get_class_index, get_field_with_id};
use crate::jni::{collect_type_parameters, is_basic_signature_type, member_position};
use crate::semantic_index::SymbolKind;
use crate::signature::indexed_signature::ToSignatureIndexedType;
use crate::signature::{IndexedMethodSignature, IndexedSignatureType, TypeParameterData};
use jni::objects::{JObject, JValue};
use jni::sys::{jint, jlong, jobject, jobjectArray, jsize, jstring};
use jni::{jni_sig, jni_str, EnvUnowned};

use crate::jni::with_jni_env;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getNameNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jstring {
    with_jni_env!(env, {
        let (_, class_index) = get_class_index(env, &this);
        let indexed_method = get_field_with_id::<IndexedMethod>(
            env,
            &this,
            &cached_field_ids().class_index_child_self_pointer,
        );

        env.new_string(indexed_method.method_name(class_index.constant_pool()))
            .unwrap()
            .into_raw()
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getDeclaringClassNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jobject {
    with_jni_env!(env, {
        let indexed_class = get_field_with_id::<IndexedClass>(
            env,
            &this,
            &cached_field_ids().class_child_class_pointer,
        );
        let (class_index_pointer, _) = get_class_index(env, &this);

        let result_class = env
            .find_class(jni_str!("com/github/tth05/jindex/IndexedClass"))
            .expect("Result class not found");
        env.new_object(
            &result_class,
            jni_sig!("(JJ)V"),
            &[
                JValue::from(class_index_pointer as jlong),
                JValue::from((indexed_class as *const IndexedClass) as jlong),
            ],
        )
        .expect("Failed to create result object")
        .into_raw()
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getAccessFlagsNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jint {
    with_jni_env!(env, {
        let indexed_method = get_field_with_id::<IndexedMethod>(
            env,
            &this,
            &cached_field_ids().class_index_child_self_pointer,
        );

        indexed_method.access_flags() as jint
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getDescriptorStringNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jstring {
    with_jni_env!(env, {
        let (_, class_index) = get_class_index(env, &this);
        let indexed_method = get_field_with_id::<IndexedMethod>(
            env,
            &this,
            &cached_field_ids().class_index_child_self_pointer,
        );
        let indexed_class = get_field_with_id::<IndexedClass>(
            env,
            &this,
            &cached_field_ids().class_child_class_pointer,
        );
        let member_index = member_position(indexed_class.methods(), indexed_method);

        env.new_string(class_index.semantic_index().descriptor(
            SymbolKind::Method,
            indexed_class.index(),
            member_index,
        ))
        .expect("Unable to create descriptor String")
        .into_raw()
    })
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
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getGenericSignatureStringNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jstring {
    with_jni_env!(env, {
        let (_, class_index) = get_class_index(env, &this);
        let indexed_method = get_field_with_id::<IndexedMethod>(
            env,
            &this,
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
                .is_none_or(|v| !v.iter().any(|s| !is_basic_signature_type(s)))
        {
            return Ok(JObject::null().into_raw());
        }

        env.new_string(signature.to_signature_string(class_index))
            .expect("Unable to create generic signature String")
            .into_raw()
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getExceptionsNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jobjectArray {
    with_jni_env!(env, {
        let (class_index_pointer, class_index) = get_class_index(env, &this);

        let indexed_method = get_field_with_id::<IndexedMethod>(
            env,
            &this,
            &cached_field_ids().class_index_child_self_pointer,
        );
        let indexed_class = get_field_with_id::<IndexedClass>(
            env,
            &this,
            &cached_field_ids().class_child_class_pointer,
        );

        let generic_data = collect_method_type_parameters(
            class_index,
            indexed_class,
            indexed_method.method_signature(),
        );
        let exceptions: Vec<u32> = indexed_method
            .method_signature()
            .exceptions()
            .into_iter()
            .flatten()
            .filter_map(|signature| {
                signature.extract_base_object_type().or_else(|| {
                    if matches!(signature, IndexedSignatureType::Generic(_)) {
                        signature
                            .resolve_generic_type_bound(class_index, &generic_data)
                            .and_then(|bound| bound.extract_base_object_type())
                    } else {
                        None
                    }
                })
            })
            .collect();
        let array_length = exceptions.len();

        let result_class = env
            .find_class(jni_str!("com/github/tth05/jindex/IndexedClass"))
            .expect("Result class not found");

        let result_array = env
            .new_object_array(array_length as jsize, &result_class, JObject::null())
            .expect("Failed to create result array");

        if array_length == 0 {
            return Ok(result_array.into_raw());
        }

        for (index, exception_class_index) in exceptions.into_iter().enumerate() {
            let class = class_index.class_at_index(exception_class_index);

            let object = env
                .new_object(
                    &result_class,
                    jni_sig!("(JJ)V"),
                    &[
                        JValue::from(class_index_pointer as jlong),
                        JValue::from((class as *const IndexedClass) as jlong),
                    ],
                )
                .expect("Failed to create result object");
            result_array
                .set_element(env, index, &object)
                .expect("Failed to set element into result array");
        }

        result_array.into_raw()
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_findImplementationsNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jobjectArray {
    with_jni_env!(env, {
        let (class_index_pointer, class_index) = get_class_index(env, &this);

        let indexed_method = get_field_with_id::<IndexedMethod>(
            env,
            &this,
            &cached_field_ids().class_index_child_self_pointer,
        );
        let indexed_class = get_field_with_id::<IndexedClass>(
            env,
            &this,
            &cached_field_ids().class_child_class_pointer,
        );

        let impls =
            class_index.find_implementations_of_method(indexed_class.index(), indexed_method);

        let result_class = env
            .find_class(jni_str!("com/github/tth05/jindex/IndexedMethod"))
            .expect("Result class not found");

        let result_array = env
            .new_object_array(impls.len() as i32, &result_class, JObject::null())
            .expect("Failed to create result array");

        for (index, (declaring_class, result_method)) in impls.iter().enumerate() {
            let object = env
                .new_object(
                    &result_class,
                    jni_sig!("(JJJ)V"),
                    &[
                        JValue::from(class_index_pointer as jlong),
                        JValue::from((*declaring_class as *const IndexedClass) as jlong),
                        JValue::from((*result_method as *const IndexedMethod) as jlong),
                    ],
                )
                .expect("Failed to create result object");
            result_array
                .set_element(env, index, &object)
                .expect("Failed to set element into result array");
        }

        result_array.into_raw()
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_findBaseMethodsNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jobjectArray {
    with_jni_env!(env, {
        let (class_index_pointer, class_index) = get_class_index(env, &this);

        let indexed_method = get_field_with_id::<IndexedMethod>(
            env,
            &this,
            &cached_field_ids().class_index_child_self_pointer,
        );
        let indexed_class = get_field_with_id::<IndexedClass>(
            env,
            &this,
            &cached_field_ids().class_child_class_pointer,
        );

        let impls = class_index.find_base_methods_of_method(indexed_class, indexed_method);

        let result_class = env
            .find_class(jni_str!("com/github/tth05/jindex/IndexedMethod"))
            .expect("Result class not found");

        let result_array = env
            .new_object_array(impls.len() as i32, &result_class, JObject::null())
            .expect("Failed to create result array");

        for (index, MethodWithClass { class, method }) in impls.iter().enumerate() {
            let object = env
                .new_object(
                    &result_class,
                    jni_sig!("(JJJ)V"),
                    &[
                        JValue::from(class_index_pointer as jlong),
                        JValue::from((*class as *const IndexedClass) as jlong),
                        JValue::from((*method as *const IndexedMethod) as jlong),
                    ],
                )
                .expect("Failed to create result object");
            result_array
                .set_element(env, index, &object)
                .expect("Failed to set element into result array");
        }

        result_array.into_raw()
    })
}
