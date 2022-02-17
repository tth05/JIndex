use crate::constant_pool::ClassIndexConstantPool;
use ascii::{AsAsciiStr, AsciiStr, AsciiString, IntoAsciiString};
use cafebabe::{
    parse_class_with_options, ClassAccessFlags, FieldAccessFlags, MethodAccessFlags, ParseOptions,
};
use jni::signature::{JavaType, TypeSignature};
use speedy::{Readable, Writable};
use std::cell::{Ref, RefCell, RefMut};
use std::cmp::min;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::ops::{Div, Range};
use std::path::Path;
use std::slice::Iter;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use zip::ZipArchive;

pub struct ClassIndex {
    constant_pool: RefCell<ClassIndexConstantPool>,
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
            constant_pool: RefCell::new(constant_pool),
            classes,
            class_prefix_range_map: range_map,
        }
    }

    pub fn find_classes(
        &self,
        name: &AsciiStr,
        limit: usize,
    ) -> anyhow::Result<Vec<(u32, &IndexedClass)>> {
        let lower_case_iter =
            self.class_iter_for_char(name.get_ascii(0).unwrap().to_ascii_lowercase().as_byte());
        let upper_case_iter =
            self.class_iter_for_char(name.get_ascii(0).unwrap().to_ascii_uppercase().as_byte());

        let mut index = 0;
        let mut res: Vec<(u32, &IndexedClass)> = lower_case_iter
            .1
            .filter_map(|class| {
                let mut result = None;
                if self
                    .constant_pool()
                    .string_view_at(class.name_index)
                    .starts_with(&self.constant_pool(), name, true)
                {
                    result = Some((lower_case_iter.0.start + index, class))
                }

                index += 1;
                result
            })
            .take(limit)
            .collect();

        index = 0;
        //TODO: Duplicated code
        upper_case_iter
            .1
            .filter_map(|class| {
                let mut result = None;
                if self
                    .constant_pool()
                    .string_view_at(class.name_index)
                    .starts_with(&self.constant_pool(), name, true)
                {
                    result = Some((upper_case_iter.0.start + index, class))
                }

                index += 1;
                result
            })
            .take(limit.saturating_sub(res.len()))
            .for_each(|el| res.push(el));

        Ok(res)
    }

    pub fn find_class(
        &self,
        package_name: &AsciiStr,
        class_name: &AsciiStr,
    ) -> Option<(u32, &IndexedClass)> {
        let classes: Vec<_> = self
            .find_classes(class_name.as_ascii_str().unwrap(), usize::MAX)
            .expect("Find classes failed");

        for class in classes.into_iter() {
            if !self
                .constant_pool()
                .package_at(class.1.package_index())
                .package_name_with_parents_equals(
                    &self.constant_pool(),
                    package_name.as_ascii_str().unwrap(),
                )
            {
                continue;
            }

            return Some(class);
        }

        None
    }

    pub fn class_at_index(&self, index: u32) -> &IndexedClass {
        self.classes().get(index as usize).unwrap()
    }

    pub fn find_methods(
        &self,
        name: &AsciiStr,
        limit: usize,
    ) -> anyhow::Result<Vec<IndexedMethod>> {
        let mut res = Vec::new();
        'outer: for c in self.classes.iter() {
            for method in c.methods().iter() {
                if res.len() > limit {
                    break 'outer;
                }

                if !self
                    .constant_pool()
                    .string_view_at(method.name_index)
                    .starts_with(&self.constant_pool(), name, false)
                {
                    continue;
                }
                //TODO: Somehow make this work without clone?
                res.push(method.clone());
            }
        }
        /*let res = self
        .classes
        .iter()
        .flat_map(|class| *class.methods())
        .filter(|method| {
            self.constant_pool()
                .string_view_at(method.name_index)
                .starts_with(&self.constant_pool(), name, false)
        })
        .take(limit)
        .collect();*/
        Ok(res)
    }

    pub fn classes(&self) -> &Vec<IndexedClass> {
        &self.classes
    }

    pub fn constant_pool(&self) -> Ref<ClassIndexConstantPool> {
        self.constant_pool.borrow()
    }

    pub fn constant_pool_mut(&self) -> RefMut<ClassIndexConstantPool> {
        self.constant_pool.borrow_mut()
    }

    fn class_iter_for_char(&self, char: u8) -> (Range<u32>, Iter<IndexedClass>) {
        self.class_prefix_range_map.get(&char).map_or_else(
            || (0..0, self.classes[0..0].iter()),
            |r| {
                (
                    r.clone(),
                    self.classes[r.start as usize..r.end as usize].iter(),
                )
            },
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

            let indexed_fields = Vec::with_capacity(class_info.fields.len());
            let indexed_methods = Vec::with_capacity(class_info.methods.len());

            let indexed_class = IndexedClass::new(
                constant_pool
                    .get_or_add_package(&class_info.package_name)
                    .unwrap()
                    .index(),
                class_name_index,
                class_info.access_flags.bits(),
                indexed_fields,
                indexed_methods,
            );

            classes.push(indexed_class);
        }

        let class_index = ClassIndex::new(constant_pool, classes);

        let mut time = 0;
        for class_info in vec.iter() {
            let indexed_class = class_index
                .find_class(&class_info.package_name, &class_info.class_name)
                .unwrap()
                .1;
            let mut indexed_fields = indexed_class.fields_mut();

            for field_info in class_info.fields.iter() {
                let field_name = field_info.field_name.as_ascii_str().unwrap();

                let field_name_index = self.get_index_from_pool(
                    field_name,
                    &mut constant_pool_map,
                    &mut class_index.constant_pool_mut(),
                );

                indexed_fields.push(IndexedField::new(
                    field_name_index,
                    match &field_info.descriptor {
                        JavaType::Object(full_class_name) => {
                            let split_pair = full_class_name
                                .rsplit_once("/")
                                .unwrap_or(("", full_class_name));

                            let package_name = split_pair.0.into_ascii_string().unwrap();
                            let class_name = split_pair.1.into_ascii_string().unwrap();

                            let t = Instant::now();
                            let option = class_index.find_class(&package_name, &class_name);
                            time += t.elapsed().as_nanos();
                            if option.is_none() {
                                -4
                            } else {
                                option
                                    .unwrap_or_else(|| {
                                        panic!(
                                            "Field type not found {:?}, {:?}",
                                            &package_name, class_name
                                        )
                                    })
                                    .0 as i32
                            }
                        }
                        JavaType::Primitive(p) => -1,
                        _ => -2,
                    },
                    field_info.access_flags.bits(),
                ));
            }

            let mut indexed_methods = indexed_class.methods_mut();

            for method_info in class_info.methods.iter() {
                let method_name = method_info.method_name.as_ascii_str().unwrap();

                let method_name_index = self.get_index_from_pool(
                    method_name,
                    &mut constant_pool_map,
                    &mut class_index.constant_pool_mut(),
                );

                indexed_methods.push(IndexedMethod::new(
                    method_name_index,
                    method_info.access_flags.bits(),
                ));
            }
        }

        println!("Time spent in findClass {:?}", time.div(1_000_000));

        class_index
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
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
}

pub struct FieldInfo {
    pub field_name: AsciiString,
    pub descriptor: JavaType,
    pub access_flags: FieldAccessFlags,
}

pub struct MethodInfo {
    pub method_name: AsciiString,
    pub signature: Box<TypeSignature>,
    pub access_flags: MethodAccessFlags,
}

// #[derive(Readable, Writable)]
pub struct IndexedClass {
    package_index: u32,
    name_index: u32,
    access_flags: u16,
    fields: RefCell<Vec<IndexedField>>,
    methods: RefCell<Vec<IndexedMethod>>,
}

impl IndexedClass {
    pub fn new(
        package_index: u32,
        class_name_index: u32,
        access_flags: u16,
        fields: Vec<IndexedField>,
        methods: Vec<IndexedMethod>,
    ) -> Self {
        Self {
            package_index,
            name_index: class_name_index,
            access_flags,
            fields: RefCell::new(fields),
            methods: RefCell::new(methods),
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

    pub fn field_count(&self) -> u16 {
        self.fields.borrow().len() as u16
    }

    pub fn method_count(&self) -> u16 {
        self.methods.borrow().len() as u16
    }

    pub fn package_index(&self) -> u32 {
        self.package_index
    }

    pub fn fields(&self) -> Ref<Vec<IndexedField>> {
        self.fields.borrow()
    }

    pub fn fields_mut(&self) -> RefMut<Vec<IndexedField>> {
        self.fields.borrow_mut()
    }

    pub fn methods(&self) -> Ref<Vec<IndexedMethod>> {
        self.methods.borrow()
    }

    pub fn methods_mut(&self) -> RefMut<Vec<IndexedMethod>> {
        self.methods.borrow_mut()
    }

    pub fn access_flags(&self) -> u16 {
        self.access_flags
    }
}

#[derive(Readable, Writable)]
pub struct IndexedField {
    name_index: u32,
    type_class_index: i32,
    access_flags: u16,
}

impl IndexedField {
    pub fn new(name_index: u32, type_class_index: i32, access_flags: u16) -> Self {
        Self {
            name_index,
            type_class_index,
            access_flags,
        }
    }

    pub fn field_name<'a>(&self, constant_pool: &'a ClassIndexConstantPool) -> &'a AsciiStr {
        constant_pool
            .string_view_at(self.name_index)
            .to_ascii_string(constant_pool)
    }

    pub fn access_flags(&self) -> u16 {
        self.access_flags
    }

    pub fn type_class_index(&self) -> i32 {
        self.type_class_index
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

    pub fn access_flags(&self) -> u16 {
        self.access_flags
    }
}

impl Clone for IndexedMethod {
    fn clone(&self) -> Self {
        IndexedMethod::new(self.name_index, self.access_flags)
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

    pub fn package_name_with_parents_equals(
        &self,
        constant_pool: &ClassIndexConstantPool,
        str: &AsciiStr,
    ) -> bool {
        //TODO: Support empty str parameter

        let mut index = str.len() - 1;

        let mut current_package = self;
        loop {
            let current_part = constant_pool.string_view_at(current_package.package_name_index);
            for i in (0..current_part.len()).rev() {
                if current_part.byte_at(constant_pool, i) != str[index] {
                    return false;
                }

                if index == 0 {
                    return true;
                }
                index -= 1;
            }

            //If we do not end a slash, the package names don't match
            if str[index] != '/' {
                return false;
            } else {
                index -= 1;
            }

            if current_package.previous_package_index == 0 {
                break;
            }

            current_package = constant_pool.package_at(current_package.previous_package_index)
        }

        false
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

pub fn create_class_index_from_jars(jar_names: Vec<String>) -> ClassIndex {
    let mut threads = Vec::with_capacity(min(jar_names.len(), num_cpus::get()));
    let arc = Arc::new(Mutex::new(jar_names));

    let now = Instant::now();
    for i in 0..threads.capacity() {
        threads.push(
            std::thread::Builder::new()
                .name(format!("JIndex Thread {}", i))
                .spawn({
                    let queue = Arc::clone(&arc);
                    move || {
                        let mut output = Vec::new();
                        loop {
                            let mut vec = queue.lock().unwrap();
                            if vec.is_empty() {
                                break;
                            }

                            let file_name = vec.remove(0);
                            drop(vec); //Release lock

                            let file_path = Path::new(&file_name);
                            if !file_path.exists() {
                                continue;
                            }

                            let mut archive =
                                ZipArchive::new(File::open(file_path).unwrap()).unwrap();

                            for i in 0..archive.len() {
                                let mut entry = archive.by_index(i).unwrap();
                                if entry.is_dir() || !entry.name().ends_with(".class") {
                                    continue;
                                }

                                let mut data = Vec::with_capacity(entry.size() as usize);
                                entry.read_to_end(&mut data).expect("Unable to read entry");
                                output.push(data);
                            }
                        }

                        output
                    }
                })
                .unwrap(),
        )
    }

    let class_bytes = threads
        .into_iter()
        .map(|t| t.join().unwrap())
        .reduce(|mut v1, v2| {
            v1.extend(v2);
            v1
        })
        .unwrap();

    println!(
        "read {} classes into ram in {}ms",
        class_bytes.len(),
        now.elapsed().as_millis()
    );

    create_class_index(class_bytes)
}

pub fn create_class_index(class_bytes: Vec<Vec<u8>>) -> ClassIndex {
    let mut now = Instant::now();
    let mut total = 0;

    let mut class_info_list: Vec<ClassInfo> = Vec::new();

    for bytes in class_bytes.iter() {
        let now2 = Instant::now();
        let thing =
            parse_class_with_options(&bytes[..], ParseOptions::default().parse_bytecode(false));
        total += now2.elapsed().as_nanos();

        if let Ok(class) = thing {
            let full_class_name = class.this_class.to_string();
            let split_pair = full_class_name
                .rsplit_once("/")
                .unwrap_or(("", &full_class_name));

            let package_name = split_pair.0.into_ascii_string().unwrap();
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
                            descriptor: JavaType::from_str(&m.descriptor)
                                .expect("Invalid field signature"),
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
                            signature: match JavaType::from_str(&m.descriptor)
                                .expect("Invalid type signature")
                            {
                                JavaType::Method(type_sig) => type_sig,
                                _ => panic!("Method descriptor was not a method signature"),
                            },
                            access_flags: m.access_flags,
                        })
                    })
                    .collect(),
            })
        }
    }

    println!("Just parsing took {:?}", total.div(1_000_000));
    println!("Reading took {:?}", now.elapsed().as_nanos().div(1_000_000));
    now = Instant::now();

    let method_count = class_info_list.iter().map(|e| e.methods.len() as u32).sum();

    let class_index = ClassIndexBuilder::default()
        .with_expected_method_count(method_count)
        .build(class_info_list);

    println!(
        "Building took {:?}",
        now.elapsed().as_nanos().div(1_000_000)
    );

    class_index
}
