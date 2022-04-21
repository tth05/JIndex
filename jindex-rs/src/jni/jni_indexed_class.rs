use crate::class_index::{IndexedClass, IndexedField, IndexedMethod, IndexedPackage};
use crate::jni::cache::{cached_field_ids, get_class_index, get_field_with_id};
use crate::jni::{get_java_lang_object, is_basic_signature_type};
use crate::signature::indexed_signature::{ToDescriptorIndexedType, ToSignatureIndexedType};
use crate::signature::SignatureType;
use ascii::AsAsciiStr;
use jni::objects::{JObject, JValue};
use jni::sys::{jint, jlong, jobject, jobjectArray, jsize, jstring};
use jni::JNIEnv;
use std::borrow::BorrowMut;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getName(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

    env.new_string(indexed_class.class_name(&class_index.constant_pool()))
        .unwrap()
        .into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getSourceName(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

    env.new_string(
        &indexed_class.class_name(&class_index.constant_pool())
            [indexed_class.class_name_start_index() as usize..],
    )
    .unwrap()
    .into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getPackage(
    env: JNIEnv,
    this: jobject,
) -> jobject {
    let (class_index_pointer, class_index) = get_class_index(env, this);
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

    let constant_pool = class_index.constant_pool();
    let indexed_package = constant_pool.package_at(indexed_class.package_index());

    env.new_object(
        env.find_class("com/github/tth05/jindex/IndexedPackage")
            .expect("Unable to find class"),
        "(JJ)V",
        &[
            JValue::from(class_index_pointer as jlong),
            JValue::from((indexed_package as *const IndexedPackage) as jlong),
        ],
    )
    .expect("Unable to create object")
    .into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getNameWithPackage(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

    env.new_string(indexed_class.class_name_with_package(&class_index.constant_pool()))
        .unwrap()
        .into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getNameWithPackageDot(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let (_, class_index) = get_class_index(env, this);
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

    env.new_string(
        indexed_class
            .class_name_with_package(&class_index.constant_pool())
            .to_string()
            .replace('/', "."),
    )
    .unwrap()
    .into_inner()
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getAccessFlags(
    env: JNIEnv,
    this: jobject,
) -> jint {
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

    indexed_class.access_flags() as jint
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getFields(
    env: JNIEnv,
    this: jobject,
) -> jobjectArray {
    let (class_index_pointer, _) = get_class_index(env, this);
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

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
    let (class_index_pointer, _) = get_class_index(env, this);
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

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
    let (class_index_pointer, class_index) = get_class_index(env, this);
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

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
    let (class_index_pointer, class_index) = get_class_index(env, this);
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

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
    let (_, class_index) = get_class_index(env, this);
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );
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

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getEnclosingClass(
    env: JNIEnv,
    this: jobject,
) -> jobject {
    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );
    let (class_index_pointer, class_index) = get_class_index(env, this);

    if let Some(enclosing_class) = indexed_class.enclosing_class(class_index) {
        env.new_object(
            result_class,
            "(JJ)V",
            &[
                JValue::from(class_index_pointer as jlong),
                JValue::from((enclosing_class as *const IndexedClass) as jlong),
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
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getInnerClassType0(
    env: JNIEnv,
    this: jobject,
) -> jint {
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

    if let Some(info) = indexed_class.enclosing_type_info() {
        info.inner_class_type().as_index() as jint
    } else {
        -1_i32
    }
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getEnclosingMethodNameAndDesc(
    env: JNIEnv,
    this: jobject,
) -> jstring {
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );
    let (_, class_index) = get_class_index(env, this);

    if let Some(info) = indexed_class.enclosing_type_info() {
        if info.method_name().is_none() || info.method_descriptor().is_none() {
            return JObject::null().into_inner();
        }

        let mut name = class_index
            .constant_pool()
            .string_view_at(*info.method_name().unwrap())
            .into_ascii_string(&class_index.constant_pool())
            .to_ascii_string();
        name.push_str(
            info.method_descriptor()
                .unwrap()
                //This signature was already a descriptor, meaning no generic param replacement is needed
                .to_descriptor_string(class_index, &Vec::new())
                .as_ascii_str_unchecked(),
        );

        env.new_string(name)
            .expect("Unable to create descriptor String")
            .into_inner()
    } else {
        JObject::null().into_inner()
    }
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedClass_getMemberClasses(
    env: JNIEnv,
    this: jobject,
) -> jobjectArray {
    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    let (class_index_pointer, class_index) = get_class_index(env, this);
    let indexed_class = get_field_with_id::<IndexedClass>(
        env,
        this,
        &cached_field_ids().class_index_child_self_pointer,
    );

    let classes = indexed_class.member_classes();

    let result_array = env
        .new_object_array(classes.len() as i32, result_class, JObject::null())
        .expect("Failed to create result array");
    for (index, class) in classes.iter().enumerate() {
        let object = env
            .new_object(
                result_class,
                "(JJ)V",
                &[
                    JValue::from(class_index_pointer as jlong),
                    JValue::from(
                        (class_index.class_at_index(*class) as *const IndexedClass) as jlong,
                    ),
                ],
            )
            .expect("Failed to create result object");
        env.set_object_array_element(result_array, index as i32, object)
            .expect("Failed to set element into result array");
    }

    result_array
}
