use crate::class_index::{ClassIndex, IndexedMethod};
use crate::jni::{get_class_index, get_pointer_field};
use crate::signature::indexed_signature::ToStringIndexedType;
use crate::signature::SignatureType;
use jni::objects::JObject;
use jni::sys::{jobject, jobjectArray, jshort, jstring};
use jni::JNIEnv;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getName(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (class_index_pointer, class_index) = get_class_index(env, this);
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
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getReturnTypeSignature(
    env: JNIEnv,
    this: jobject,
) -> jobject {
    let (class_index_pointer, class_index) = get_class_index(env, this);

    let indexed_method = get_pointer_field::<IndexedMethod>(env, this);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedSignature")
        .expect("Result class not found");

    //TODO: Method return type
    /*env.new_object(
        result_class,
        "(JJ)V",
        &[
            JValue::Long(class_index_pointer),
            JValue::Long(
                (indexed_method.method_signature().return_type() as *const IndexedSignatureType)
                    as jlong,
            ),
        ],
    )
    .expect("Failed to create instance")
    .into_inner()*/
    JObject::null().into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getParameterTypeSignatures(
    env: JNIEnv,
    this: jobject,
) -> jobjectArray {
    let (class_index_pointer, class_index) = get_class_index(env, this);

    let indexed_method = get_pointer_field::<IndexedMethod>(env, this);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedSignature")
        .expect("Result class not found");

    //TODO: Method parameter signatures
    /*let parameter_signatures_or_none = indexed_method.method_signature().params();

    let array_length = parameter_signatures_or_none.map_or(0, |v| v.len());
    let array = env
        .new_object_array(array_length as jsize, result_class, JObject::null())
        .expect("Failed to create array");

    if let Some(parameter_signatures) = parameter_signatures_or_none {
        for (index, signature) in parameter_signatures.iter().enumerate() {
            let object = env
                .new_object(
                    result_class,
                    "(JJ)V",
                    &[
                        JValue::Long(class_index_pointer),
                        JValue::Long((signature as *const IndexedSignatureType) as jlong),
                    ],
                )
                .expect("Failed to create instance")
                .into_inner();

            env.set_object_array_element(array, index as jsize, object)
                .expect("Failed to set array element");
        }
    }

    array*/
    JObject::null().into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getDescriptorString(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (class_index_pointer, class_index) = get_class_index(env, this);

    let indexed_method = get_pointer_field::<IndexedMethod>(env, this);

    /*let parameter_signatures_or_none = indexed_method.method_signature().params();

    let mut descriptor = String::from('(');
    if let Some(parameter_signatures) = parameter_signatures_or_none {
        for signature in parameter_signatures.iter() {
            descriptor.push_str(&signature.signature_string(class_index));
        }
    }

    descriptor.push(')');
    descriptor.push_str(
        &indexed_method
            .method_signature()
            .return_type()
            .signature_string(class_index),
    );*/

    //TODO: Method descriptor string
    env.new_string(/*descriptor*/ "")
        .expect("Unable to create descriptor String")
        .into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getGenericSignatureString(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (class_index_pointer, class_index) = get_class_index(env, this);
    let indexed_method = get_pointer_field::<IndexedMethod>(env, this);
    let signature = indexed_method.method_signature();

    fn is_basic_type(s: &SignatureType<u32>) -> bool {
        matches!(
            s,
            SignatureType::Unresolved | SignatureType::Primitive(_) | SignatureType::Object(_)
        )
    }

    //No generic signature available
    if signature.generic_data().is_none()
        && signature.parameters().iter().all(is_basic_type)
        && is_basic_type(signature.return_type())
    {
        return JObject::null().into_inner();
    }

    env.new_string(signature.to_string(class_index))
        .expect("Unable to create descriptor String")
        .into_inner()
}
