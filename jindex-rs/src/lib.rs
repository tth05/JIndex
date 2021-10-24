use crate::class_index::{ClassIndex, ClassIndexBuilder, ClassInfo};
use ascii::AsAsciiStr;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jint, jlong, jobjectArray};
use jni::JNIEnv;
use std::time::Instant;

pub mod class_index;
pub mod constant_pool;
mod prefix_tree;

#[cfg(test)]
mod tests {
    use crate::class_index::{ClassIndexBuilder, ClassInfo};
    use crate::constant_pool::ClassIndexConstantPool;
    use ascii::{AsAsciiStr, AsciiStr};
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    #[test]
    fn package_test() {
        let mut pool = ClassIndexConstantPool::new(69);
        {
            let package = pool
                .get_or_add_package("net.minecraft.item".as_ascii_str().unwrap())
                .unwrap();
        }
        println!("{:?}", pool.string_view_at(4 + 10).to_ascii_string(&pool));
    }

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
                    i.class_name(class_index.get_constant_pool()),
                    i.method_indexes(class_index.get_constant_pool())
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
    let t = Instant::now();
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

    println!("{}", t.elapsed().as_millis());
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
                env.new_string(i.class_name(class_index.get_constant_pool()))
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
