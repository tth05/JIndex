use crate::constant_pool::ClassIndexConstantPool;
use crate::prefix_tree::PrefixTree;
use ascii::{AsAsciiStr, AsciiStr, AsciiString};
use speedy::{Readable, Writable};
use std::collections::HashMap;

#[derive(Readable, Writable)]
pub struct ClassIndex {
    pub constant_pool: ClassIndexConstantPool,
    pub class_prefix_tree: PrefixTree<IndexedClass>,
    pub method_prefix_tree: PrefixTree<u32>,
}

impl ClassIndex {
    pub fn find_classes(
        &self,
        name: &AsciiStr,
        mut limit: u32,
    ) -> anyhow::Result<Vec<&IndexedClass>> {
        let res = self
            .class_prefix_tree
            .find_all_starting_with(&self.constant_pool, name, &mut limit)?
            .into_iter()
            .collect();
        Ok(res)
    }

    pub fn find_methods(&mut self, name: &AsciiStr, mut limit: u32) -> anyhow::Result<Vec<u32>> {
        let res = self
            .method_prefix_tree
            .find_all_starting_with(&self.constant_pool, name, &mut limit)?
            .into_iter()
            .copied()
            .collect();
        Ok(res)
    }

    pub fn get_constant_pool(&self) -> &ClassIndexConstantPool {
        &self.constant_pool
    }
}

pub struct ClassIndexBuilder {
    expected_method_count: u32,
    average_class_name_size: u32,
    average_method_name_size: u32,
}

impl ClassIndexBuilder {
    pub fn new() -> Self {
        Self {
            expected_method_count: 0,
            average_class_name_size: 15,
            average_method_name_size: 8,
        }
    }

    pub fn with_expected_method_count(mut self, count: u32) -> Self {
        self.expected_method_count = count;
        self
    }

    pub fn with_average_class_name_size(mut self, size: u32) -> Self {
        self.average_class_name_size = size;
        self
    }

    pub fn with_average_method_name_size(mut self, size: u32) -> Self {
        self.average_method_name_size = size;
        self
    }

    pub fn build(self, vec: Vec<ClassInfo>) -> ClassIndex {
        let element_count = vec.len() as u32;

        let mut constant_pool = ClassIndexConstantPool::new(
            ((element_count * self.average_class_name_size
                + self.expected_method_count * self.average_method_name_size) as f32
                * 0.8) as u32,
        );

        let mut class_prefix_tree: PrefixTree<IndexedClass> = PrefixTree::new(2);
        let mut method_prefix_tree: PrefixTree<u32> = PrefixTree::new(2);
        let mut constant_pool_map: HashMap<&AsciiStr, u32> = HashMap::new();

        for c in vec.iter() {
            let class_name = c.class_name.as_ascii_str().unwrap();
            let class_name_index = if let Some(i) = constant_pool_map.get(class_name) {
                *i
            } else {
                let index = constant_pool.add_string(c.class_name.as_bytes()).unwrap();
                constant_pool_map.insert(class_name, index);
                index
            };

            let mut method_indexes = Vec::new();
            let method_count = c.methods.len() as u16;

            for method_name in c.methods.iter().map(|s| s.as_ascii_str().unwrap()) {
                let method_name_index = if let Some(i) = constant_pool_map.get(method_name) {
                    *i
                } else {
                    let index = constant_pool.add_string(method_name.as_bytes()).unwrap();
                    constant_pool_map.insert(method_name, index);
                    index
                };

                method_indexes.push(method_name_index);

                method_prefix_tree.put(
                    &constant_pool,
                    constant_pool.string_view_at(method_name_index),
                    method_name_index,
                );
            }

            let method_data_index = constant_pool.add_methods(&method_indexes);
            let indexed_class = IndexedClass::new(
                class_name_index,
                method_data_index,
                method_count,
                constant_pool
                    .get_or_add_package(c.package_name.as_ascii_str().unwrap())
                    .unwrap()
                    .index(),
            );

            class_prefix_tree.put(
                &constant_pool,
                constant_pool.string_view_at(indexed_class.class_name_index),
                indexed_class,
            )
        }

        constant_pool.clear_sub_packages();

        ClassIndex {
            constant_pool,
            class_prefix_tree,
            method_prefix_tree,
        }
    }
}

impl Default for ClassIndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ClassInfo {
    pub package_name: AsciiString,
    pub class_name: AsciiString,
    pub methods: Vec<AsciiString>,
}

pub struct IndexedClass {
    class_name_index: u32,
    method_data_index: u32,
    method_count: u16,
    package_index: u32,
}

impl IndexedClass {
    pub fn new(
        class_name_index: u32,
        method_data_index: u32,
        method_count: u16,
        package_index: u32,
    ) -> Self {
        Self {
            class_name_index,
            method_data_index,
            method_count,
            package_index,
        }
    }

    pub fn class_name<'a>(&self, constant_pool: &'a ClassIndexConstantPool) -> &'a AsciiStr {
        constant_pool
            .string_view_at(self.class_name_index)
            .to_ascii_string(constant_pool)
    }

    pub fn class_name_with_package(&self, constant_pool: &ClassIndexConstantPool) -> AsciiString {
        let package_name = constant_pool
            .package_at(self.package_index)
            .package_name_with_parents(constant_pool);
        let class_name = constant_pool
            .string_view_at(self.class_name_index)
            .to_ascii_string(constant_pool);
        package_name.to_owned() + ".".as_ascii_str().unwrap() + class_name
    }

    pub fn method_indexes<'a>(&self, constant_pool: &'a ClassIndexConstantPool) -> &'a [u32] {
        constant_pool.get_methods_at(self.method_data_index, self.method_count)
    }

    pub fn class_name_index(&self) -> u32 {
        self.class_name_index
    }

    pub fn method_data_index(&self) -> u32 {
        self.method_data_index
    }

    pub fn method_count(&self) -> u16 {
        self.method_count
    }

    pub fn package_index(&self) -> u32 {
        self.package_index
    }
}

pub struct IndexedPackage {
    index: u32,
    package_name_index: u32,
    sub_packages_indexes: Vec<u32>,
    previous_package_index: u32,
}

impl IndexedPackage {
    pub fn new(index: u32, package_name_index: u32, previous_package_index: u32) -> Self {
        Self {
            index,
            package_name_index,
            sub_packages_indexes: Vec::new(),
            previous_package_index,
        }
    }

    pub fn clear_sub_packages(&mut self) {
        self.sub_packages_indexes.clear();
        self.sub_packages_indexes.truncate(0);
    }

    pub fn package_name<'a>(&self, constant_pool: &'a ClassIndexConstantPool) -> &'a AsciiStr {
        constant_pool
            .string_view_at(self.package_name_index)
            .to_ascii_string(constant_pool)
    }

    pub fn package_name_with_parents(&self, constant_pool: &ClassIndexConstantPool) -> AsciiString {
        let mut base = constant_pool
            .string_view_at(self.package_name_index)
            .to_ascii_string(constant_pool)
            .to_owned();

        let mut parent_index = self.previous_package_index;
        while parent_index != 0 {
            let parent_package = constant_pool.package_at(parent_index);
            base = parent_package.package_name(constant_pool).to_owned()
                + ".".as_ascii_str().unwrap()
                + &base;
            parent_index = parent_package.previous_package_index;
        }

        base
    }

    pub fn add_sub_package(&mut self, index: u32) {
        self.sub_packages_indexes.push(index);
    }

    pub fn sub_packages_indexes(&self) -> &[u32] {
        &self.sub_packages_indexes[..]
    }

    pub fn previous_package_index(&self) -> u32 {
        self.previous_package_index
    }

    pub fn index(&self) -> u32 {
        self.index
    }

    pub fn package_name_index(&self) -> u32 {
        self.package_name_index
    }
}
