use crate::constant_pool::ClassIndexConstantPool;
use ascii::{AsAsciiStr, AsciiStr, AsciiString};
use cafebabe::{ClassAccessFlags, MethodAccessFlags};
use speedy::{Readable, Writable};
use std::collections::HashMap;
use std::ops::Range;
use std::slice::Iter;

#[derive(Readable, Writable)]
pub struct ClassIndex {
    constant_pool: ClassIndexConstantPool,
    class_prefix_range_map: HashMap<u8, Range<u32>>,
    classes: Vec<IndexedClass>,
}

impl ClassIndex {
    pub fn new(mut constant_pool: ClassIndexConstantPool, mut classes: Vec<IndexedClass>) -> Self {
        //Free up some memory, packages only need one way references
        constant_pool.clear_sub_packages();

        //Construct prefix range map
        let mut prefix_count_map: HashMap<u8, u32> = HashMap::new();
        classes.sort_by_cached_key(|c| {
            let name = c.class_name(&constant_pool);
            let count = prefix_count_map
                .entry(name.get_ascii(0).unwrap().as_byte())
                .or_insert(0);
            *count += 1;
            name
        });

        let mut range_map: HashMap<u8, Range<u32>> = HashMap::new();
        let mut total = 0u32;
        for i in 0..=127u8 {
            let prefix_count = prefix_count_map.get(&i);
            if prefix_count.is_none() {
                continue;
            }

            let prefix_count = prefix_count.unwrap();
            range_map.insert(i, total..(total + prefix_count));
            total += prefix_count;
        }

        Self {
            constant_pool,
            classes,
            class_prefix_range_map: range_map,
        }
    }

    pub fn find_classes(
        &self,
        name: &AsciiStr,
        limit: usize,
    ) -> anyhow::Result<Vec<&IndexedClass>> {
        let lower_case_iter =
            self.class_iter_for_char(name.get_ascii(0).unwrap().to_ascii_lowercase().as_byte());
        let upper_case_iter =
            self.class_iter_for_char(name.get_ascii(0).unwrap().to_ascii_uppercase().as_byte());

        let res = lower_case_iter
            .chain(upper_case_iter)
            .filter(|class| {
                self.constant_pool
                    .string_view_at(class.name_index)
                    .starts_with(&self.constant_pool, name, true)
            })
            .take(limit)
            .collect();

        Ok(res)
    }

    pub fn find_methods(
        &mut self,
        name: &AsciiStr,
        limit: usize,
    ) -> anyhow::Result<Vec<&IndexedMethod>> {
        let res = self
            .classes
            .iter()
            .flat_map(|class| class.methods())
            .filter(|method| {
                self.constant_pool
                    .string_view_at(method.name_index)
                    .starts_with(&self.constant_pool, name, false)
            })
            .take(limit)
            .collect();
        Ok(res)
    }

    pub fn constant_pool(&self) -> &ClassIndexConstantPool {
        &self.constant_pool
    }

    fn class_iter_for_char(&self, char: u8) -> Iter<IndexedClass> {
        self.class_prefix_range_map.get(&char).map_or_else(
            || self.classes[0..0].iter(),
            |r| self.classes[r.start as usize..r.end as usize].iter(),
        )
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

        let mut classes: Vec<IndexedClass> = Vec::with_capacity(vec.len());
        let mut constant_pool_map: HashMap<&AsciiStr, u32> =
            HashMap::with_capacity(vec.len() + self.expected_method_count as usize);

        for class_info in vec.iter() {
            let class_name = class_info.class_name.as_ascii_str().unwrap();
            let class_name_index =
                self.get_index_from_pool(class_name, &mut constant_pool_map, &mut constant_pool);

            let mut indexed_methods = Vec::new();

            for method_info in class_info.methods.iter() {
                let method_name = method_info.method_name.as_ascii_str().unwrap();

                let method_name_index = self.get_index_from_pool(
                    method_name,
                    &mut constant_pool_map,
                    &mut constant_pool,
                );

                indexed_methods.push(IndexedMethod::new(
                    method_name_index,
                    method_info.access_flags.bits(),
                ));
            }

            let indexed_class = IndexedClass::new(
                constant_pool
                    .get_or_add_package(&class_info.package_name)
                    .unwrap()
                    .index(),
                class_name_index,
                class_info.access_flags.bits(),
                indexed_methods,
            );

            classes.push(indexed_class);
        }

        ClassIndex::new(constant_pool, classes)
    }

    fn get_index_from_pool<'a>(
        &self,
        value: &'a AsciiStr,
        map: &mut HashMap<&'a AsciiStr, u32>,
        pool: &mut ClassIndexConstantPool,
    ) -> u32 {
        if let Some(i) = map.get(value) {
            *i
        } else {
            let index = pool.add_string(value.as_bytes()).unwrap();
            map.insert(value, index);
            index
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
    pub access_flags: ClassAccessFlags,
    pub methods: Vec<MethodInfo>,
}

pub struct MethodInfo {
    pub method_name: AsciiString,
    pub access_flags: MethodAccessFlags,
}

#[derive(Readable, Writable)]
pub struct IndexedClass {
    package_index: u32,
    name_index: u32,
    access_flags: u16,
    methods: Vec<IndexedMethod>,
}

impl IndexedClass {
    pub fn new(
        package_index: u32,
        class_name_index: u32,
        access_flags: u16,
        methods: Vec<IndexedMethod>,
    ) -> Self {
        Self {
            package_index,
            name_index: class_name_index,
            access_flags,
            methods,
        }
    }

    pub fn class_name<'a>(&self, constant_pool: &'a ClassIndexConstantPool) -> &'a AsciiStr {
        constant_pool
            .string_view_at(self.name_index)
            .to_ascii_string(constant_pool)
    }

    pub fn class_name_with_package(&self, constant_pool: &ClassIndexConstantPool) -> AsciiString {
        let package_name = constant_pool
            .package_at(self.package_index)
            .package_name_with_parents(constant_pool);
        let class_name = constant_pool
            .string_view_at(self.name_index)
            .to_ascii_string(constant_pool);

        package_name + ".".as_ascii_str().unwrap() + class_name
    }

    pub fn class_name_index(&self) -> u32 {
        self.name_index
    }

    pub fn method_count(&self) -> u16 {
        self.methods.len() as u16
    }

    pub fn package_index(&self) -> u32 {
        self.package_index
    }

    pub fn methods(&self) -> &Vec<IndexedMethod> {
        &self.methods
    }
}

#[derive(Readable, Writable)]
pub struct IndexedMethod {
    name_index: u32,
    access_flags: u16,
}

impl IndexedMethod {
    pub fn new(name_index: u32, access_flags: u16) -> Self {
        Self {
            name_index,
            access_flags,
        }
    }

    pub fn method_name<'a>(&self, constant_pool: &'a ClassIndexConstantPool) -> &'a AsciiStr {
        constant_pool
            .string_view_at(self.name_index)
            .to_ascii_string(constant_pool)
    }
}

#[derive(Readable, Writable)]
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
