use crate::constant_pool::ClassIndexConstantPool;
use crate::signature::indexed_signature::ToIndexedType;
use crate::signature::{
    ClassSignature, IndexedClassSignature, IndexedMethodSignature, IndexedSignatureType,
    MethodSignature, RawClassSignature, RawMethodSignature, RawSignatureType, SignatureType,
};
use ascii::{AsAsciiStr, AsciiChar, AsciiStr, AsciiString, IntoAsciiString};
use cafebabe::attributes::{AttributeData, AttributeInfo};
use cafebabe::{
    parse_class_with_options, ClassAccessFlags, FieldAccessFlags, MethodAccessFlags, ParseOptions,
};
use speedy::{Readable, Writable};
use std::cell::{Ref, RefCell, RefMut};
use std::cmp::{min, Ordering};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::lazy::OnceCell;
use std::ops::{Div, Range};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
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

        let time = Instant::now();
        classes.sort_by(|a, b| {
            let a_name = a.class_name(&constant_pool);
            let b_name = b.class_name(&constant_pool);
            a_name.cmp(b_name).then_with(|| {
                constant_pool
                    .package_at(a.package_index)
                    .package_name_with_parents_cmp(
                        &constant_pool,
                        &constant_pool
                            .package_at(b.package_index)
                            .package_name_with_parents(&constant_pool),
                    )
            })
        });

        for class in classes.iter() {
            let count = prefix_count_map
                .entry(
                    class
                        .class_name(&constant_pool)
                        .get_ascii(0)
                        .unwrap()
                        .as_byte(),
                )
                .or_insert(0);
            *count += 1;
        }
        println!("Sort {}", time.elapsed().as_millis());

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
            .iter()
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
            .iter()
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
        let class_iter = self.class_iter_for_char(class_name.get_ascii(0).unwrap().as_byte());

        let index = class_iter.1.binary_search_by(|a| {
            a.class_name(&self.constant_pool())
                .cmp(class_name)
                .then_with(|| {
                    self.constant_pool()
                        .package_at(a.package_index)
                        .package_name_with_parents_cmp(&self.constant_pool(), package_name)
                })
        });
        if let Ok(i) = index {
            return Some((class_iter.0.start + i as u32, class_iter.1.get(i).unwrap()));
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
    ) -> anyhow::Result<Vec<&IndexedMethod>> {
        let res = self
            .classes
            .iter()
            .flat_map(|class| class.methods())
            .filter(|method| {
                self.constant_pool()
                    .string_view_at(method.name_index)
                    .starts_with(&self.constant_pool(), name, false)
            })
            .take(limit)
            .collect();
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

    fn class_iter_for_char(&self, char: u8) -> (Range<u32>, &[IndexedClass]) {
        self.class_prefix_range_map.get(&char).map_or_else(
            || (0..0, &self.classes[0..0]),
            |r| (r.clone(), &self.classes[r.start as usize..r.end as usize]),
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
            let package_index = constant_pool
                .get_or_add_package(&class_info.package_name)
                .unwrap()
                .index();
            let class_name_index = ClassIndexBuilder::get_index_from_pool(
                &class_info.class_name,
                &mut constant_pool_map,
                &mut constant_pool,
            );

            let indexed_class = IndexedClass::new(
                package_index,
                class_name_index,
                class_info.access_flags.bits(),
            );

            classes.push(indexed_class);
        }

        let class_index = ClassIndex::new(constant_pool, classes);

        //TODO: Multi thread this loop using dashmap/flurry
        let mut time = 0u128;
        for class_info in vec.iter() {
            let t = Instant::now();
            let indexed_class = class_index
                .find_class(&class_info.package_name, &class_info.class_name)
                .unwrap()
                .1;
            time += t.elapsed().as_nanos();

            //Signature
            indexed_class.set_signature(
                class_info
                    .signature
                    .to_indexed_type(&class_index, &mut constant_pool_map),
            );

            //Fields
            let mut indexed_fields = Vec::with_capacity(class_info.fields.len());

            for field_info in class_info.fields.iter() {
                let field_name = field_info.field_name.as_ascii_str().unwrap();

                let field_name_index = ClassIndexBuilder::get_index_from_pool(
                    field_name,
                    &mut constant_pool_map,
                    &mut class_index.constant_pool_mut(),
                );

                indexed_fields.push(IndexedField::new(
                    field_name_index,
                    field_info.access_flags.bits(),
                    field_info
                        .descriptor
                        .to_indexed_type(&class_index, &mut constant_pool_map),
                ));
            }

            indexed_class.set_fields(indexed_fields).unwrap();

            //Methods
            let mut indexed_methods = Vec::with_capacity(class_info.methods.len());

            for method_info in class_info.methods.iter() {
                let method_name = method_info.method_name.as_ascii_str().unwrap();

                let method_name_index = ClassIndexBuilder::get_index_from_pool(
                    method_name,
                    &mut constant_pool_map,
                    &mut class_index.constant_pool_mut(),
                );

                indexed_methods.push(IndexedMethod::new(
                    method_name_index,
                    method_info.access_flags.bits(),
                    method_info
                        .signature
                        .to_indexed_type(&class_index, &mut constant_pool_map),
                ));
            }

            indexed_class.set_methods(indexed_methods).unwrap();
        }

        println!("Time spent in findClass {:?}", time.div(1_000_000));

        class_index
    }

    fn compute_signature_for_descriptor(
        signature_type: &RawSignatureType,
        class_index: &ClassIndex,
    ) -> IndexedSignatureType {
        match signature_type {
            SignatureType::Object(full_class_name) => {
                ClassIndexBuilder::compute_signature_for_object(full_class_name, class_index)
            }
            //TODO: Add back from_primitive_type method?
            SignatureType::Primitive(p) => IndexedSignatureType::Unresolved, /*IndexedSignatureType::from_primitive_type(p)*/
            SignatureType::Array(t) => IndexedSignatureType::Array(Box::new(
                ClassIndexBuilder::compute_signature_for_descriptor(t, class_index),
            )),
            _ => unreachable!(),
        }
    }

    fn compute_signature_for_object(
        full_class_name: &AsciiStr,
        class_index: &ClassIndex,
    ) -> IndexedSignatureType {
        let split_pair = rsplit_once(full_class_name, AsciiChar::Slash);

        let package_name = split_pair.0;
        let class_name = split_pair.1;

        // let t = Instant::now();
        let option = class_index.find_class(package_name, class_name);
        // time += t.elapsed().as_nanos();
        if option.is_none() {
            IndexedSignatureType::Unresolved
        } else {
            IndexedSignatureType::Object(option.unwrap().0)
        }
    }

    pub fn get_index_from_pool<'a>(
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

pub fn rsplit_once(str: &AsciiStr, separator: AsciiChar) -> (&AsciiStr, &AsciiStr) {
    for i in (0..str.len()).rev() {
        if str.get_ascii(i).unwrap() == separator {
            return (&str[0..i], &str[(i + 1)..]);
        }
    }

    ("".as_ascii_str().unwrap(), &str[..])
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
    pub signature: RawClassSignature,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
}

pub struct FieldInfo {
    pub field_name: AsciiString,
    pub descriptor: RawSignatureType,
    pub access_flags: FieldAccessFlags,
}

pub struct MethodInfo {
    pub method_name: AsciiString,
    pub signature: RawMethodSignature,
    pub access_flags: MethodAccessFlags,
}

// #[derive(Readable, Writable)]
pub struct IndexedClass {
    package_index: u32,
    name_index: u32,
    access_flags: u16,
    signature: OnceCell<IndexedClassSignature>,
    fields: OnceCell<Vec<IndexedField>>,
    methods: OnceCell<Vec<IndexedMethod>>,
}

impl IndexedClass {
    pub fn new(package_index: u32, class_name_index: u32, access_flags: u16) -> Self {
        Self {
            package_index,
            name_index: class_name_index,
            access_flags,
            signature: OnceCell::new(),
            fields: OnceCell::new(),
            methods: OnceCell::new(),
        }
    }

    pub fn class_name<'b>(&self, constant_pool: &'b ClassIndexConstantPool) -> &'b AsciiStr {
        constant_pool
            .string_view_at(self.name_index)
            .into_ascii_string(constant_pool)
    }

    pub fn class_name_with_package(&self, constant_pool: &ClassIndexConstantPool) -> AsciiString {
        let package_name = constant_pool
            .package_at(self.package_index)
            .package_name_with_parents(constant_pool);
        let class_name = constant_pool
            .string_view_at(self.name_index)
            .into_ascii_string(constant_pool);

        if package_name.is_empty() {
            class_name.to_ascii_string()
        } else {
            package_name + "/".as_ascii_str().unwrap() + class_name
        }
    }

    pub fn set_signature(&self, signature: IndexedClassSignature) {
        self.signature.set(signature).unwrap();
    }

    pub fn class_name_index(&self) -> u32 {
        self.name_index
    }

    pub fn field_count(&self) -> u16 {
        self.fields.get().unwrap().len() as u16
    }

    pub fn method_count(&self) -> u16 {
        self.methods.get().unwrap().len() as u16
    }

    pub fn package_index(&self) -> u32 {
        self.package_index
    }

    pub fn signature(&self) -> &IndexedClassSignature {
        self.signature.get().unwrap()
    }

    pub fn fields(&self) -> &Vec<IndexedField> {
        self.fields.get().unwrap()
    }

    pub fn set_fields(&self, fields: Vec<IndexedField>) -> Result<(), Vec<IndexedField>> {
        self.fields.set(fields)
    }

    pub fn methods(&self) -> &Vec<IndexedMethod> {
        self.methods.get().unwrap()
    }

    pub fn set_methods(&self, methods: Vec<IndexedMethod>) -> Result<(), Vec<IndexedMethod>> {
        self.methods.set(methods)
    }
    pub fn access_flags(&self) -> u16 {
        self.access_flags
    }
}

#[derive(Readable, Writable, Debug)]
pub struct IndexedField {
    name_index: u32,
    access_flags: u16,
    field_signature: IndexedSignatureType,
}

impl IndexedField {
    pub fn new(name_index: u32, access_flags: u16, field_signature: IndexedSignatureType) -> Self {
        Self {
            name_index,
            access_flags,
            field_signature,
        }
    }

    pub fn field_name<'b>(&self, constant_pool: &'b ClassIndexConstantPool) -> &'b AsciiStr {
        constant_pool
            .string_view_at(self.name_index)
            .into_ascii_string(constant_pool)
    }

    pub fn access_flags(&self) -> u16 {
        self.access_flags
    }

    pub fn field_signature(&self) -> &IndexedSignatureType {
        &self.field_signature
    }
}

#[derive(Readable, Writable, Debug)]
pub struct IndexedMethod {
    name_index: u32,
    access_flags: u16,
    method_signature: IndexedMethodSignature,
}

impl IndexedMethod {
    pub fn new(
        name_index: u32,
        access_flags: u16,
        method_signature: IndexedMethodSignature,
    ) -> Self {
        Self {
            name_index,
            access_flags,
            method_signature,
        }
    }

    pub fn method_name<'b>(&self, constant_pool: &'b ClassIndexConstantPool) -> &'b AsciiStr {
        constant_pool
            .string_view_at(self.name_index)
            .into_ascii_string(constant_pool)
    }

    pub fn access_flags(&self) -> u16 {
        self.access_flags
    }

    pub fn method_signature(&self) -> &IndexedMethodSignature {
        &self.method_signature
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
            .into_ascii_string(constant_pool)
    }

    pub fn package_name_with_parents_cmp(
        &self,
        constant_pool: &ClassIndexConstantPool,
        str: &AsciiStr,
    ) -> Ordering {
        let mut index = str.len() - 1;

        let mut current_package = self;
        let mut current_part = constant_pool.string_view_at(current_package.package_name_index);
        if str.is_empty() {
            return if current_part.is_empty() {
                Ordering::Equal
            } else {
                Ordering::Greater
            };
        }

        loop {
            for i in (0..current_part.len()).rev() {
                let compare = current_part
                    .byte_at(constant_pool, i)
                    .cmp(&str[index].as_byte());
                if compare != Ordering::Equal {
                    return compare;
                }

                if index == 0 {
                    return if i > 0 || current_package.previous_package_index != 0 {
                        Ordering::Greater
                    } else {
                        Ordering::Equal
                    };
                }
                index -= 1;
            }

            //If we do not end a slash, the package names don't match
            if str[index] != '/' {
                return Ordering::Less;
            } else {
                index -= 1;
            }

            if current_package.previous_package_index == 0 {
                break;
            }

            current_package = constant_pool.package_at(current_package.previous_package_index);
            current_part = constant_pool.string_view_at(current_package.package_name_index);
        }

        Ordering::Less
    }

    pub fn package_name_with_parents(&self, constant_pool: &ClassIndexConstantPool) -> AsciiString {
        let mut base = constant_pool
            .string_view_at(self.package_name_index)
            .into_ascii_string(constant_pool)
            .to_owned();

        let mut parent_index = self.previous_package_index;
        while parent_index != 0 {
            let parent_package = constant_pool.package_at(parent_index);
            base = parent_package.package_name(constant_pool).to_owned()
                + "/".as_ascii_str().unwrap()
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

    pub fn index(&self) -> u32 {
        self.index
    }
}

pub fn do_multi_threaded<I, O>(
    queue: Vec<I>,
    func: &'static (dyn Fn(&[I]) -> Vec<O> + Sync),
) -> Vec<O>
where
    O: std::marker::Send,
    I: Sync + std::marker::Send,
{
    do_multi_threaded_with_config(queue, num_cpus::get(), func)
}

pub fn do_multi_threaded_with_config<I, O>(
    queue: Vec<I>,
    thread_count: usize,
    func: &'static (dyn Fn(&[I]) -> Vec<O> + Sync),
) -> Vec<O>
where
    O: std::marker::Send,
    I: Sync + std::marker::Send,
{
    let mut threads = Vec::with_capacity(min(queue.len(), thread_count));

    let split_size = queue.len() / threads.capacity();
    let queue_arc = Arc::new(queue);
    for i in 0..threads.capacity() {
        let queue = Arc::clone(&queue_arc);

        let start = i * split_size;
        let end = if i == threads.capacity() - 1 {
            queue.len()
        } else {
            start + split_size
        };
        threads.push(
            std::thread::Builder::new()
                .name(format!("JIndex Thread {}", i))
                .spawn(move || func(&queue[start..end]))
                .unwrap(),
        )
    }

    threads
        .into_iter()
        .map(|t| t.join().unwrap())
        .reduce(|mut v1, v2| {
            v1.extend(v2);
            v1
        })
        .unwrap()
}

fn process_jar_worker(queue: &[String]) -> Vec<Vec<u8>> {
    let mut output = Vec::new();
    for file_name in queue.iter() {
        let file_path = Path::new(&file_name);
        if !file_path.exists() {
            continue;
        }

        let mut archive = ZipArchive::new(File::open(file_path).unwrap()).unwrap();

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

pub fn create_class_index_from_jars(jar_names: Vec<String>) -> ClassIndex {
    let now = Instant::now();
    let class_bytes = do_multi_threaded(jar_names, &process_jar_worker);

    println!(
        "read {} classes into ram in {}ms",
        class_bytes.len(),
        now.elapsed().as_millis()
    );

    create_class_index(class_bytes)
}

fn process_class_bytes_worker(bytes_queue: &[Vec<u8>]) -> Vec<ClassInfo> {
    let mut class_info_list = Vec::new();

    for bytes in bytes_queue.iter() {
        let thing =
            parse_class_with_options(&bytes[..], ParseOptions::default().parse_bytecode(false));

        if let Ok(class) = thing {
            let parsed_signature = if let Some(attr) = get_signature_attribute(&class.attributes) {
                RawClassSignature::from_str(match &attr.data {
                    AttributeData::Signature(s) => s,
                    _ => unreachable!(),
                })
                .expect("Invalid class signature")
            } else {
                RawClassSignature::new(
                    class
                        .super_class
                        .map(|s| RawSignatureType::Object(s.into_ascii_string().unwrap())),
                    Some(
                        class
                            .interfaces
                            .into_iter()
                            .map(|s| RawSignatureType::Object(s.into_ascii_string().unwrap()))
                            .collect(),
                    )
                    .filter(|v: &Vec<RawSignatureType>| !v.is_empty()),
                )
            };

            let full_class_name = class.this_class;
            let split_pair = full_class_name
                .rsplit_once('/')
                .unwrap_or(("", &full_class_name));

            let package_name = split_pair.0.into_ascii_string().unwrap();
            let class_name = split_pair.1.into_ascii_string().unwrap();

            class_info_list.push(ClassInfo {
                package_name,
                class_name,
                access_flags: class.access_flags,
                signature: parsed_signature,
                fields: class
                    .fields
                    .into_iter()
                    .filter_map(|m| {
                        let name = m.name.into_ascii_string();
                        if name.is_err() {
                            return None;
                        }

                        Some(FieldInfo {
                            field_name: name.unwrap(),
                            descriptor: SignatureType::from_str(&m.descriptor)
                                .expect("Invalid field signature"),
                            access_flags: m.access_flags,
                        })
                    })
                    .collect(),
                methods: class
                    .methods
                    .into_iter()
                    .filter_map(|m| {
                        let name = m.name.into_ascii_string();
                        if name.is_err() {
                            return None;
                        }

                        let signature =
                            get_signature_attribute(&m.attributes).map_or(&m.descriptor, |i| {
                                if let AttributeData::Signature(ref s) = i.data {
                                    return s;
                                }
                                unreachable!();
                            });

                        Some(MethodInfo {
                            method_name: name.unwrap(),
                            signature: RawMethodSignature::from_str(signature)
                                .expect("Invalid method descriptor"),
                            access_flags: m.access_flags,
                        })
                    })
                    .collect(),
            })
        }
    }

    class_info_list
}

fn get_signature_attribute<'a>(
    attributes: &'a [AttributeInfo<'a>],
) -> Option<&'a AttributeInfo<'a>> {
    attributes
        .iter()
        .find(|a| matches!(a.data, AttributeData::Signature(_)))
}

pub fn create_class_index(class_bytes: Vec<Vec<u8>>) -> ClassIndex {
    let mut now = Instant::now();
    let mut class_info_list: Vec<ClassInfo> =
        do_multi_threaded(class_bytes, &process_class_bytes_worker);

    //Removes duplicate classes
    class_info_list.sort_unstable_by(|a, b| {
        a.class_name
            .cmp(&b.class_name)
            .then_with(|| a.package_name.cmp(&b.package_name))
    });
    class_info_list
        .dedup_by(|a, b| a.class_name.eq(&b.class_name) && a.package_name.eq(&b.package_name));

    println!(
        "Reading {} classes took {:?}",
        class_info_list.len(),
        now.elapsed().as_nanos().div(1_000_000)
    );
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
