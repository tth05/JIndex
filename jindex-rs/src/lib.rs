use crate::class_index::{ClassIndex, ClassIndexBuilder, ClassInfo};

mod class_index;
mod constant_pool;
mod io;
pub mod jni_class_index;
pub mod jni_indexed_class;
pub mod jni_indexed_method;
mod prefix_tree;

mod test {
    use ascii::{AsAsciiStr, AsciiString, IntoAsciiString};
    use speedy::{Readable, Writable};

    use crate::{ClassIndex, ClassIndexBuilder, ClassInfo};

    #[test]
    fn test_thing() {
        let boi = ClassIndexBuilder::new().build(vec![
            ClassInfo {
                package_name: "test.class.thing".into_ascii_string().unwrap(),
                class_name: "Yeet".into_ascii_string().unwrap(),
                methods: vec!["coomer".into_ascii_string().unwrap()],
            },
            ClassInfo {
                package_name: "te23st.23lass.thi23ng".into_ascii_string().unwrap(),
                class_name: "Yeet2".into_ascii_string().unwrap(),
                methods: vec!["coomer12".into_ascii_string().unwrap()],
            },
        ]);
        let found: Vec<AsciiString> = boi
            .find_classes("yeet".as_ascii_str().unwrap(), 56)
            .unwrap()
            .iter()
            .map(|c| c.class_name_with_package(boi.constant_pool()))
            .collect();
        println!("{:?}", found);

        let buf = boi.write_to_vec().unwrap();
        println!("{}", buf.len());
        let boi = ClassIndex::read_from_buffer(&buf).unwrap();

        let found: Vec<AsciiString> = boi
            .find_classes("Yeet".as_ascii_str().unwrap(), 56)
            .unwrap()
            .iter()
            .map(|c| c.class_name_with_package(boi.constant_pool()))
            .collect();
        println!("{:?}", found);
    }
}
