use crate::class_index_members::IndexedClass;
use crate::jni::cache::{cached_field_ids, get_class_index, get_field_with_id};
use crate::package_index::IndexedPackage;
use jni::objects::{JObject, JValue};
use jni::sys::{jlong, jobjectArray, jstring};
use jni::{jni_sig, jni_str, EnvUnowned};

use crate::jni::with_jni_env;

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedPackage_getNameNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jstring {
    with_jni_env!(env, {
        let (_, class_index) = get_class_index(env, &this);
        let indexed_package = get_field_with_id::<IndexedPackage>(
            env,
            &this,
            &cached_field_ids().class_index_child_self_pointer,
        );

        env.new_string(indexed_package.package_name(class_index.constant_pool()))
            .unwrap()
            .into_raw()
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedPackage_getNameWithParentsNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jstring {
    with_jni_env!(env, {
        let (_, class_index) = get_class_index(env, &this);
        let indexed_package = get_field_with_id::<IndexedPackage>(
            env,
            &this,
            &cached_field_ids().class_index_child_self_pointer,
        );

        env.new_string(
            indexed_package.package_name_with_parents(
                class_index.package_index(),
                class_index.constant_pool(),
            ),
        )
        .unwrap()
        .into_raw()
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedPackage_getNameWithParentsDotNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jstring {
    with_jni_env!(env, {
        let (_, class_index) = get_class_index(env, &this);
        let indexed_package = get_field_with_id::<IndexedPackage>(
            env,
            &this,
            &cached_field_ids().class_index_child_self_pointer,
        );

        env.new_string(
            indexed_package
                .package_name_with_parents(class_index.package_index(), class_index.constant_pool())
                .to_string()
                .replace('/', "."),
        )
        .unwrap()
        .into_raw()
    })
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedPackage_getSubPackagesNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jobjectArray {
    with_jni_env!(env, {
        let (class_index_pointer, class_index) = get_class_index(env, &this);
        let indexed_package = get_field_with_id::<IndexedPackage>(
            env,
            &this,
            &cached_field_ids().class_index_child_self_pointer,
        );

        let result_class = env
            .find_class(jni_str!("com/github/tth05/jindex/IndexedPackage"))
            .expect("Result class not found");

        let result_array = env
            .new_object_array(
                indexed_package.sub_packages_indices().len() as i32,
                &result_class,
                JObject::null(),
            )
            .expect("Failed to create result array");

        for (index, package) in indexed_package
            .sub_packages_indices()
            .iter()
            .map(|i| class_index.package_index().package_at(*i))
            .enumerate()
        {
            let object = env
                .new_object(
                    &result_class,
                    jni_sig!("(JJ)V"),
                    &[
                        JValue::from(class_index_pointer as jlong),
                        JValue::from((package as *const IndexedPackage) as jlong),
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
pub unsafe extern "system" fn Java_com_github_tth05_jindex_IndexedPackage_getClassesNative(
    mut env: EnvUnowned<'_>,
    this: JObject,
) -> jobjectArray {
    with_jni_env!(env, {
        let (class_index_pointer, class_index) = get_class_index(env, &this);
        let indexed_package = get_field_with_id::<IndexedPackage>(
            env,
            &this,
            &cached_field_ids().class_index_child_self_pointer,
        );

        let result_class = env
            .find_class(jni_str!("com/github/tth05/jindex/IndexedClass"))
            .expect("Result class not found");

        let result_array = env
            .new_object_array(
                indexed_package.sub_classes_indices().len() as i32,
                &result_class,
                JObject::null(),
            )
            .expect("Failed to create result array");

        for (index, class) in indexed_package
            .sub_classes_indices()
            .iter()
            .map(|i| class_index.class_at_index(*i))
            .enumerate()
        {
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
