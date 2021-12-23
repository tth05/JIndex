use crate::class_index::{ClassIndex, ClassIndexBuilder, ClassInfo};
use ascii::{AsAsciiStr, IntoAsciiString};
use cafebabe::parse_class;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jint, jlong, jobject, jobjectArray};
use jni::JNIEnv;

pub mod class_index;
pub mod constant_pool;
pub mod io;
pub mod prefix_tree;

#[no_mangle]
pub extern "system" fn Java_com_github_tth05_jindex_ClassIndex_createClassIndex(
    env: JNIEnv,
    _class: JClass,
    byte_array_list: jobject,
) -> jlong {
    let mut class_info_list: Vec<ClassInfo> = Vec::new();
    let list = env.get_list(byte_array_list.into()).unwrap();
    for byte_array in list.iter().unwrap() {
        let bytes = env.convert_byte_array(byte_array.cast()).unwrap();
        if let Ok(class) = parse_class(&bytes[..]) {
            //TODO: This unwrap will fail for classes without a package
            let full_class_name = class.this_class.to_string();
            let split_pair = full_class_name.rsplit_once("/").unwrap();
            let package_name = split_pair.0.replace("/", ".").into_ascii_string().unwrap();
            let class_name = split_pair.1.into_ascii_string().unwrap();

            class_info_list.push(ClassInfo {
                package_name,
                class_name,
                methods: class
                    .methods
                    .iter()
                    .map(|m| m.name.to_string().into_ascii_string().unwrap())
                    .filter(|name| name[0] != 60) // Filter <init>, <clinit>
                    .collect(),
            })
        }
    }

    let method_count = class_info_list.iter().map(|e| e.methods.len() as u32).sum();

    let class_index = ClassIndexBuilder::default()
        .with_expected_method_count(method_count)
        .build(class_info_list);

    Box::into_raw(Box::new(class_index)) as jlong
}

#[no_mangle]
/// # Safety
/// The given pointer has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_ClassIndex_findClasses(
    env: JNIEnv,
    _class: JClass,
    class_index_pointer: jlong,
    input: JString,
    limit: jint,
) -> jobjectArray {
    let input: String = env
        .get_string(input)
        .expect("Couldn't get java string!")
        .into();

    let string_class = env
        .find_class("java/lang/String")
        .expect("String class not found");
    let result_class = env
        .find_class("com/github/tth05/jindex/FindClassesResult")
        .expect("Result class not found");

    let class_index = &mut *(class_index_pointer as *mut ClassIndex);

    let classes: Vec<_> = class_index
        .find_classes(input.as_ascii_str().unwrap(), limit as u32)
        .expect("Find classes failed")
        .into_iter()
        .map(|i| {
            let method_name_array = env
                .new_object_array(i.method_count() as i32, string_class, JObject::null())
                .unwrap();
            for (i, method_index) in i
                .method_indexes(class_index.get_constant_pool())
                .iter()
                .enumerate()
            {
                env.set_object_array_element(
                    method_name_array,
                    i as i32,
                    env.new_string(
                        class_index
                            .get_constant_pool()
                            .string_view_at(*method_index)
                            .to_ascii_string(class_index.get_constant_pool()),
                    )
                    .unwrap(),
                )
                .expect("Failed to set element in method name array");
            }

            (
                env.new_string(i.class_name_with_package(class_index.get_constant_pool()))
                    .unwrap(),
                method_name_array,
            )
        })
        .collect();

    let result_array = env
        .new_object_array(classes.len() as i32, result_class, JObject::null())
        .expect("Failed to create result array");
    for (i, pair) in classes.into_iter().enumerate() {
        let object = env
            .new_object(
                result_class,
                "(Ljava/lang/String;[Ljava/lang/String;)V",
                &[pair.0.into(), pair.1.into()],
            )
            .expect("Failed to create result object");
        env.set_object_array_element(result_array, i as i32, object)
            .expect("Failed to set element into result array");
    }

    result_array
}
