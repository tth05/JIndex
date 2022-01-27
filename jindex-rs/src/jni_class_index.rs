use ascii::{AsAsciiStr, IntoAsciiString};
use cafebabe::parse_class;
use jni::objects::{JObject, JString, JValue};
use jni::sys::{jint, jlong, jobject, jobjectArray};
use jni::JNIEnv;

use crate::class_index::{
    ClassIndex, ClassIndexBuilder, ClassInfo, FieldInfo, IndexedClass, MethodInfo,
};
use crate::io::{load_class_index_from_file, save_class_index_to_file};

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_destroy(
    env: JNIEnv,
    this: jobject,
) {
    let _class_index =
        Box::from_raw(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut ClassIndex);

    env.set_field(this, "pointer", "J", JValue::Long(0i64))
        .expect("Unable to set field");
    env.set_field(this, "destroyed", "Z", JValue::Bool(1))
        .expect("Unable to set field");
}

#[no_mangle]
pub extern "system" fn Java_com_github_tth05_jindex_ClassIndex_createClassIndex(
    env: JNIEnv,
    this: jobject,
    byte_array_list: jobject,
) {
    let mut class_info_list: Vec<ClassInfo> = Vec::new();
    let list = env.get_list(byte_array_list.into()).unwrap();
    for byte_array in list.iter().unwrap() {
        let bytes = env.convert_byte_array(byte_array.cast()).unwrap();
        if let Ok(class) = parse_class(&bytes[..]) {
            let full_class_name = class.this_class.to_string();
            let split_pair = full_class_name
                .rsplit_once("/")
                .unwrap_or(("", &full_class_name));

            let package_name = split_pair.0.replace("/", ".").into_ascii_string().unwrap();
            let class_name = split_pair.1.into_ascii_string().unwrap();

            class_info_list.push(ClassInfo {
                package_name,
                class_name,
                access_flags: class.access_flags,
                fields: class
                    .fields
                    .iter()
                    .filter_map(|m| {
                        let name = m.name.to_string().into_ascii_string();
                        if name.is_err() {
                            return None;
                        }

                        Some(FieldInfo {
                            field_name: name.unwrap(),
                            access_flags: m.access_flags,
                        })
                    })
                    .collect(),
                methods: class
                    .methods
                    .iter()
                    .filter_map(|m| {
                        let name = m.name.to_string().into_ascii_string();
                        if name.is_err() {
                            return None;
                        }

                        Some(MethodInfo {
                            method_name: name.unwrap(),
                            access_flags: m.access_flags,
                        })
                    })
                    .collect(),
            })
        }
    }

    let method_count = class_info_list.iter().map(|e| e.methods.len() as u32).sum();

    let class_index = ClassIndexBuilder::default()
        .with_expected_method_count(method_count)
        .build(class_info_list);

    env.set_field(
        this,
        "pointer",
        "J",
        JValue::Long(Box::into_raw(Box::new(class_index)) as jlong),
    )
    .expect("Unable to set field");
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_saveToFile(
    env: JNIEnv,
    this: jobject,
    path: JString,
) {
    let path: String = env.get_string(path).expect("Invalid path").into();

    let class_index =
        &mut *(env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut ClassIndex);

    save_class_index_to_file(class_index, path);
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_loadClassIndexFromFile(
    env: JNIEnv,
    this: jobject,
    path: JString,
) {
    let path: String = env.get_string(path).expect("Invalid path").into();
    let class_index = load_class_index_from_file(path);

    env.set_field(
        this,
        "pointer",
        "J",
        JValue::Long(Box::into_raw(Box::new(class_index)) as jlong),
    )
    .expect("Unable to set field");
}

#[no_mangle]
/// # Safety
/// The pointer field has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findClasses(
    env: JNIEnv,
    this: jobject,
    input: JString,
    limit: jint,
) -> jobjectArray {
    let input: String = env
        .get_string(input)
        .expect("Couldn't get java string!")
        .into();

    let result_class = env
        .find_class("com/github/tth05/jindex/IndexedClass")
        .expect("Result class not found");

    let class_index_pointer =
        env.get_field(this, "pointer", "J").unwrap().j().unwrap() as *mut ClassIndex;
    let class_index = &*(class_index_pointer);

    let classes: Vec<_> = class_index
        .find_classes(input.as_ascii_str().unwrap(), limit as usize)
        .expect("Find classes failed");

    let result_array = env
        .new_object_array(classes.len() as i32, result_class, JObject::null())
        .expect("Failed to create result array");
    for (index, class) in classes.into_iter().enumerate() {
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
