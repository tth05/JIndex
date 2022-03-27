use crate::class_index::{IndexedClass, IndexedField, IndexedMethod};
use crate::jni::{
    cached_field_ids, get_class_index, get_field_with_id, get_java_lang_object,
    is_basic_signature_type,
};
use crate::signature::indexed_signature::ToSignatureIndexedType;
use crate::signature::SignatureType;
use jni::objects::{JObject, JValue};
use jni::sys::{jlong, jobject, jobjectArray, jshort, jsize, jstring};
use jni::JNIEnv;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getName(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(
        env,
        this,
        &cached_field_ids().indexed_class_index_pointer_id,
    );
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().indexed_class_pointer_id);

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
    let (_, class_index) = get_class_index(
        env,
        this,
        &cached_field_ids().indexed_class_index_pointer_id,
    );
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().indexed_class_pointer_id);

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
    let (_, class_index) = get_class_index(
        env,
        this,
        &cached_field_ids().indexed_class_index_pointer_id,
    );
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().indexed_class_pointer_id);

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
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().indexed_class_pointer_id);

    indexed_class.access_flags() as jshort
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getFields(
    env: JNIEnv,
    this: jobject,
) -> jobjectArray {
    let (class_index_pointer, _) = get_class_index(
        env,
        this,
        &cached_field_ids().indexed_class_index_pointer_id,
    );
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().indexed_class_pointer_id);

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
                "(JJJ)V",
                &[
                    JValue::from(class_index_pointer as jlong),
                    JValue::from((indexed_class as *const IndexedClass) as jlong),
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
    let (class_index_pointer, _) = get_class_index(
        env,
        this,
        &cached_field_ids().indexed_class_index_pointer_id,
    );
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().indexed_class_pointer_id);

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
                "(JJJ)V",
                &[
                    JValue::from(class_index_pointer as jlong),
                    JValue::from((indexed_class as *const IndexedClass) as jlong),
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
    let (class_index_pointer, class_index) = get_class_index(
        env,
        this,
        &cached_field_ids().indexed_class_index_pointer_id,
    );
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().indexed_class_pointer_id);

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    let super_class = indexed_class.signature().super_class().map_or_else(
        || {
            //Object has no super class
            if indexed_class.class_name_with_package(&class_index.constant_pool())
                == "java/lang/Object"
            {
                None
            } else {
                get_java_lang_object(class_index)
            }
        },
        |s| match s {
            SignatureType::Unresolved => None,
            _ => Some(class_index.class_at_index(*s.extract_base_object_type())),
        },
    );

    if let Some(class) = super_class {
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
    let (class_index_pointer, class_index) = get_class_index(
        env,
        this,
        &cached_field_ids().indexed_class_index_pointer_id,
    );
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().indexed_class_pointer_id);

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

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getGenericSignatureString(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(
        env,
        this,
        &cached_field_ids().indexed_class_index_pointer_id,
    );
    let indexed_class =
        get_field_with_id::<IndexedClass>(env, this, &cached_field_ids().indexed_class_pointer_id);
    let signature = indexed_class.signature();

    //No generic signature available
    if signature.generic_data().is_none()
        && signature
            .interfaces()
            .map_or(true, |v| v.iter().all(is_basic_signature_type))
        && signature
            .super_class()
            .map_or(true, is_basic_signature_type)
        //Object has no signature
        || indexed_class.class_name_with_package(&class_index.constant_pool()) == "java/lang/Object"
    {
        return JObject::null().into_inner();
    }

    env.new_string(signature.to_signature_string(class_index))
        .expect("Unable to create descriptor String")
        .into_inner()
}
