use crate::constant_pool::ClassIndexConstantPool;
use crate::prefix_tree::PrefixTree;
use ascii::AsciiStr;
use std::collections::HashMap;

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

        let mut indexed_classes = Vec::new();
        for c in vec.into_iter() {
            let class_name_index = if let Some(i) = constant_pool_map.get(c.class_name) {
                *i
            } else {
                let index = constant_pool.add_string(c.class_name.as_bytes()).unwrap();
                constant_pool_map.insert(c.class_name, index);
                index
            };

            let mut method_indexes = Vec::new();
            let method_count = c.methods.len() as u16;

            for method_name in c.methods.iter() {
                let method_name_index = if let Some(i) = constant_pool_map.get(method_name) {
                    *i
                } else {
                    let index = constant_pool.add_string(method_name.as_bytes()).unwrap();
                    constant_pool_map.insert(method_name, index);
                    index
                };

                method_indexes.push(method_name_index);
            }

            let method_data_index = constant_pool.add_methods(&method_indexes);
            indexed_classes.push(IndexedClass::new(
                class_name_index,
                method_data_index,
                method_count,
            ));
            drop(c);
        }

        for indexed_class in indexed_classes.into_iter() {
            for cp_method_index in constant_pool
                .get_methods_at(indexed_class.method_data_index, indexed_class.method_count)
                .into_iter()
            {
                method_prefix_tree.put(
                    &constant_pool,
                    constant_pool.string_view_at(*cp_method_index),
                    *cp_method_index,
                );
            }

            class_prefix_tree.put(
                &constant_pool,
                constant_pool.string_view_at(indexed_class.class_name_index),
                indexed_class,
            );
        }

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

pub struct ClassInfo<'a> {
    pub package_name: &'a AsciiStr,
    pub class_name: &'a AsciiStr,
    pub methods: Vec<&'a AsciiStr>,
}

pub struct IndexedClass {
    class_name_index: u32,
    method_data_index: u32,
    method_count: u16,
}

impl IndexedClass {
    fn new(class_name_index: u32, method_data_index: u32, method_count: u16) -> Self {
        Self {
            class_name_index,
            method_data_index,
            method_count,
        }
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
}
