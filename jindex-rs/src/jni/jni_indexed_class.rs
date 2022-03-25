use crate::class_index::{ClassIndex, IndexedClass, IndexedField, IndexedMethod};
use jni::objects::{JObject, JValue};
use jni::sys::{jlong, jobject, jobjectArray, jshort, jsize, jstring};
use jni::JNIEnv;
use crate::jni::get_class_index;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getName(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (class_index_pointer, class_index) = get_class_index(env, this);
    let indexed_class =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedClass);

    env.new_string(indexed_class.class_name(&class_index.constant_pool()))
        .unwrap()
        .into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getPackage(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (class_index_pointer, class_index) = get_class_index(env, this);
    let indexed_class =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedClass);

    env.new_string(
        class_index
            .constant_pool()
            .package_at(indexed_class.package_index())
            .package_name_with_parents(&class_index.constant_pool()),
    )
    .unwrap()
    .into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getNameWithPackage(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (class_index_pointer, class_index) = get_class_index(env, this);
    let indexed_class =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedClass);

    env.new_string(indexed_class.class_name_with_package(&class_index.constant_pool()))
        .unwrap()
        .into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getAccessFlags(
    env: JNIEnv,
    this: jobject,
) -> jshort {
    let indexed_class =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedClass);

    indexed_class.access_flags() as jshort
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getFields(
    env: JNIEnv,
    this: jobject,
) -> jobjectArray {
    let (class_index_pointer, class_index) = get_class_index(env, this);

    let indexed_class =
        &mut *(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedClass);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedField")
        .expect("Result class not found");

    let result_array = env
        .new_object_array(
            indexed_class.field_count() as i32,
            result_class,
            JObject::null(),
        )
        .expect("Failed to create result array");

    for (index, field) in indexed_class.fields().iter().enumerate() {
        let object = env
            .new_object(
                result_class,
                "(JJ)V",
                &[
                    JValue::from(class_index_pointer as jlong),
                    JValue::from((field as *const IndexedField) as jlong),
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
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getMethods(
    env: JNIEnv,
    this: jobject,
) -> jobjectArray {
    let (class_index_pointer, class_index) = get_class_index(env, this);

    let indexed_class =
        &mut *(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut IndexedClass);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedMethod")
        .expect("Result class not found");

    let result_array = env
        .new_object_array(
            indexed_class.method_count() as i32,
            result_class,
            JObject::null(),
        )
        .expect("Failed to create result array");

    for (index, method) in indexed_class.methods().iter().enumerate() {
        let object = env
            .new_object(
                result_class,
                "(JJ)V",
                &[
                    JValue::from(class_index_pointer as jlong),
                    JValue::from((method as *const IndexedMethod) as jlong),
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
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getSuperClass(
    env: JNIEnv,
    this: jobject,
) -> jobject {
    let (class_index_pointer, class_index) = get_class_index(env, this);

    let indexed_class =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *const IndexedClass);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    if let Some(index) = indexed_class.signature().super_class() {
        let class = class_index.class_at_index(*index.extract_base_object_type());
        env.new_object(
            result_class,
            "(JJ)V",
            &[
                JValue::from(class_index_pointer as jlong),
                JValue::from((class as *const IndexedClass) as jlong),
            ],
        )
        .expect("Failed to create result object")
        .into_inner()
    } else {
        JObject::null().into_inner()
    }
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getInterfaces(
    env: JNIEnv,
    this: jobject,
) -> jobjectArray {
    let (class_index_pointer, class_index) = get_class_index(env, this);

    let indexed_class =
        &*(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *const IndexedClass);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    let interface_indicies = indexed_class.signature().interfaces();

    let array_length = interface_indicies.map_or(0, |v| v.len());
    if array_length == 0 {
        return JObject::null().into_inner();
    }

    let result_array = env
        .new_object_array(array_length as jsize, result_class, JObject::null())
        .expect("Failed to create result array");

    for (index, interface_index) in interface_indicies.as_ref().unwrap().iter().enumerate() {
        let class = class_index.class_at_index(*interface_index.extract_base_object_type());

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
