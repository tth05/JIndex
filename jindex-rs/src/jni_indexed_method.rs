use crate::class_index::{IndexedMethod, IndexedSignature};
use crate::ClassIndex;
use jni::objects::{JObject, JValue};
use jni::sys::{jlong, jobject, jobjectArray, jshort, jsize, jstring};
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

    env.new_object(
        result_class,
        "(JJ)V",
        &[
            JValue::Long(class_index_pointer),
            JValue::Long(
                (indexed_method.method_signature().return_type() as *const IndexedSignature)
                    as jlong,
            ),
        ],
    )
    .expect("Failed to create instance")
    .into_inner()
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

    let parameter_signatures = &indexed_method.method_signature().params();
    let array = env
        .new_object_array(
            parameter_signatures.len() as jsize,
            result_class,
            JObject::null(),
        )
        .expect("Failed to create array");

    for (index, signature) in parameter_signatures.iter().enumerate() {
        let object = env
            .new_object(
                result_class,
                "(JJ)V",
                &[
                    JValue::Long(class_index_pointer),
                    JValue::Long((signature as *const IndexedSignature) as jlong),
                ],
            )
            .expect("Failed to create instance")
            .into_inner();

        env.set_object_array_element(array, index as jsize, object)
            .expect("Failed to set array element");
    }

    array
}
