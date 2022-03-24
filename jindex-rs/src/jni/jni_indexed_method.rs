use crate::class_index::{ClassIndex, IndexedMethod};
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
    let class_index = &*(env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap() as *mut ClassIndex);
    let indexed_method =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedMethod);

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
    let indexed_method =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedMethod);

    indexed_method.access_flags() as jshort
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedMethod_getReturnTypeSignature(
    env: JNIEnv,
    this: jobject,
) -> jobject {
    let class_index_pointer = env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap();

    let indexed_method =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedMethod);

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
    let class_index_pointer = env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap();

    let indexed_method =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedMethod);

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
    let class_index = &*(env
        .get_field(this, "classIndexPointer", "J")
        .unwrap()
        .j()
        .unwrap() as *const ClassIndex);

    let indexed_method =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedMethod);

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
