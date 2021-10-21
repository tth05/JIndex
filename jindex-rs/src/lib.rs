use crate::class_index::{ClassIndex, ClassIndexBuilder, ClassInfo};
use ascii::AsAsciiStr;
use jni::objects::{JClass, JString};
use jni::sys::{jint, jlong, jobjectArray};
use jni::JNIEnv;
use std::time::Instant;

pub mod class_index;
pub mod constant_pool;
mod prefix_tree;

#[cfg(test)]
mod tests {
    use crate::class_index::{ClassIndexBuilder, ClassInfo};
    use ascii::{AsAsciiStr, AsciiStr};
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    #[test]
    fn test_it() -> anyhow::Result<()> {
        //Read test data
        let time = Instant::now();

        let data = std::fs::read_to_string("../classes1.txt")?;
        let entries: Vec<ClassInfo> = data
            .lines()
            .map(|part| {
                let parts: Vec<_> = part.split_terminator(';').collect();
                ClassInfo {
                    package_name: parts[0][0..parts[0].rfind(".").unwrap_or(0)]
                        .as_ascii_str()
                        .unwrap(),
                    class_name: parts[0][parts[0].rfind(".").unwrap_or(0) + 1..]
                        .as_ascii_str()
                        .unwrap(),
                    methods: parts
                        .into_iter()
                        .skip(1)
                        .map(|str| str.as_ascii_str().unwrap())
                        .collect(),
                }
            })
            .collect();

        println!("Other took {}ms", time.elapsed().as_millis(),);

        let method_count = entries.iter().map(|e| e.methods.len() as u32).sum();

        let time = Instant::now();
        let mut class_index = ClassIndexBuilder::default()
            .with_expected_method_count(method_count)
            .build(entries);

        println!("Full build method took {}ms", time.elapsed().as_millis());

        let time = Instant::now();
        let classes: Vec<_> = class_index
            .find_classes("ClassIndex".as_ascii_str().unwrap(), u32::MAX)?
            .into_iter()
            .map(|i| {
                (
                    class_index
                        .get_constant_pool()
                        .string_view_at(i.class_name_index())
                        .to_ascii_string(class_index.get_constant_pool()),
                    class_index
                        .get_constant_pool()
                        .get_methods_at(i.method_data_index(), i.method_count())
                        .iter()
                        .map(|m| {
                            class_index
                                .get_constant_pool()
                                .string_view_at(*m)
                                .to_ascii_string(class_index.get_constant_pool())
                        })
                        .collect(),
                ) as (&AsciiStr, Vec<&AsciiStr>)
            })
            .collect();
        println!(
            "Found classes in {}ms - {:?}",
            time.elapsed().as_nanos() as f64 / 1_000_000f64,
            classes
        );

        let time = Instant::now();
        let methods: Vec<_> = class_index
            .find_methods("findClass".as_ascii_str().unwrap(), 2)?
            .iter()
            .map(|i| {
                class_index
                    .get_constant_pool()
                    .string_view_at(*i)
                    .to_ascii_string(class_index.get_constant_pool())
            })
            .collect();
        println!(
            "Found methods in {}ms - {:?}",
            time.elapsed().as_nanos() as f64 / 1_000_000f64,
            methods
        );

        drop(data);
        // sleep(Duration::from_millis(10000));
        Ok(())
    }
}

#[no_mangle]
pub extern "system" fn Java_com_github_tth05_jindex_NativeTest_createClassIndex(
    _env: JNIEnv,
    _class: JClass,
) -> jlong {
    let data = std::fs::read_to_string("./classes1.txt").expect("Read file failed");
    let entries: Vec<ClassInfo> = data
        .lines()
        .map(|part| {
            let parts: Vec<_> = part.split_terminator(';').collect();
            ClassInfo {
                package_name: parts[0][0..parts[0].rfind('.').unwrap_or(0)]
                    .as_ascii_str()
                    .unwrap(),
                class_name: parts[0][parts[0].rfind('.').unwrap_or(0) + 1..]
                    .as_ascii_str()
                    .unwrap(),
                methods: parts
                    .into_iter()
                    .skip(1)
                    .map(|str| str.as_ascii_str().unwrap())
                    .collect(),
            }
        })
        .collect();

    let method_count = entries.iter().map(|e| e.methods.len() as u32).sum();

    let class_index = ClassIndexBuilder::default()
        .with_expected_method_count(method_count)
        .build(entries);

    Box::into_raw(Box::new(class_index)) as jlong
}

#[no_mangle]
/// # Safety
/// The given pointer has to be valid...
pub unsafe extern "system" fn Java_com_github_tth05_jindex_NativeTest_findClasses(
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

    let class_index = &mut *(class_index_pointer as *mut ClassIndex);

    let classes: Vec<_> = class_index
        .find_classes(input.as_ascii_str().unwrap(), limit as u32)
        .expect("Find classes failed")
        .into_iter()
        .map(|i| {
            (
                env.new_string(
                    class_index
                        .get_constant_pool()
                        .string_view_at(i.class_name_index())
                        .to_ascii_string(class_index.get_constant_pool()),
                )
                .unwrap(),
                class_index
                    .get_constant_pool()
                    .get_methods_at(i.method_data_index(), i.method_count())
                    .iter()
                    .map(|m| {
                        env.new_string(
                            class_index
                                .get_constant_pool()
                                .string_view_at(*m)
                                .to_ascii_string(class_index.get_constant_pool()),
                        )
                        .unwrap()
                    })
                    .collect(),
            ) as (JString, Vec<JString>)
        })
        .collect();

    // Finally, extract the raw pointer to return.
    let result_class = env
        .find_class("com/github/tth05/jindex/FindClassesResult")
        .expect("Class not found");
    let string_class = env.find_class("java/lang/String").expect("Class not found");
    let array = env
        .new_object_array(
            classes.len() as i32,
            result_class,
            env.new_object(env.find_class("java/lang/Object").unwrap(), "()V", &[])
                .unwrap(),
        )
        .expect("Thingy failed");
    for (i, pair) in classes.into_iter().enumerate() {
        let method_name_array = env
            .new_object_array(
                pair.1.len() as i32,
                string_class,
                env.new_string("").expect("String"),
            )
            .unwrap();
        for (i, str) in pair.1.into_iter().enumerate() {
            env.set_object_array_element(method_name_array, i as i32, str)
                .expect("Array set failed");
        }

        let args = [pair.0.into(), method_name_array.into()];
        // println!("{:?}", args);
        let object = env
            .new_object(
                result_class,
                "(Ljava/lang/String;[Ljava/lang/String;)V",
                &args,
            )
            .expect("Failed to create result object");
        env.set_object_array_element(array, i as i32, object)
            .expect("Array set failed");
    }
    array
}
