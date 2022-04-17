use crate::constant_pool::{ClassIndexConstantPool, MatchMode, SearchMode, SearchOptions};
use crate::signature::indexed_signature::ToIndexedType;
use crate::signature::{
    IndexedClassSignature, IndexedMethodSignature, IndexedSignatureType, RawClassSignature,
    RawMethodSignature, RawSignatureType, SignatureType,
};
use ascii::{AsAsciiStr, AsciiChar, AsciiStr, AsciiString, IntoAsciiString};
use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use cafebabe::attributes::{AttributeData, AttributeInfo, InnerClassEntry};
use cafebabe::constant_pool::NameAndType;
use cafebabe::{
    parse_class_with_options, ClassAccessFlags, ClassFile, FieldAccessFlags, MethodAccessFlags,
    ParseOptions,
};
use speedy::{Readable, Writable};
use std::borrow::Cow;
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
    constant_pool: AtomicRefCell<ClassIndexConstantPool>,
    class_prefix_range_map: HashMap<u8, Range<u32>>,
    classes: Vec<IndexedClass>,
}

impl ClassIndex {
    pub fn new(constant_pool: ClassIndexConstantPool, mut classes: Vec<IndexedClass>) -> Self {
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
            constant_pool: AtomicRefCell::new(constant_pool),
            classes,
            class_prefix_range_map: range_map,
        }
    }

    pub fn find_classes(
        &self,
        name: &AsciiStr,
        options: SearchOptions,
    ) -> Vec<(u32, &IndexedClass)> {
        if name.is_empty() {
            return Vec::default();
        }

        let mut iters = Vec::with_capacity(2);
        match options.search_mode {
            SearchMode::Prefix => match options.match_mode {
                MatchMode::IgnoreCase => {
                    iters.push(self.class_iter_for_char(
                        name.get_ascii(0).unwrap().to_ascii_lowercase().as_byte(),
                    ));
                    iters.push(self.class_iter_for_char(
                        name.get_ascii(0).unwrap().to_ascii_uppercase().as_byte(),
                    ));
                }
                MatchMode::MatchCase | MatchMode::MatchCaseFirstCharOnly => {
                    iters.push(self.class_iter_for_char(name.get_ascii(0).unwrap().as_byte()));
                }
            },
            SearchMode::Contains => {
                //We have to search all classes in contains mode
                iters.push((0..self.classes.len() as u32, &self.classes[..]));
            }
        }

        let mut result: Vec<(usize, (u32, &IndexedClass))> = Vec::new();

        for x in iters {
            let mut index = 0;
            x.1.iter()
                .filter_map(|class| {
                    let result = self
                        .constant_pool()
                        .string_view_at(class.name_index)
                        .search(&self.constant_pool(), name, options)
                        .map(|r| (r, (x.0.start + index, class)));

                    index += 1;
                    result
                })
                .take(options.limit.saturating_sub(result.len()))
                .for_each(|el| result.push(el))
        }

        result.sort_by_key(|el| el.0);
        result.into_iter().map(|el| el.1).collect()
    }

    ///TODO: 1. Abstract the prefix_range_map into its own type
    /// 2. Use that type to fast access all root packages
    /// 3. Utilize find_package (which uses that new type) and then a binary search on the found package class_indices to make this whole find_class even faster
    /// For example, when searching for 'java/lang/S', we perform a binary search on a slice with 12k elements.
    /// Instead we could find java/lang extremely fast and then binary search ~200 classes.
    pub fn find_class(
        &self,
        package_name: &AsciiStr,
        class_name: &AsciiStr,
    ) -> Option<(u32, &IndexedClass)> {
        if class_name.is_empty() {
            return Option::None;
        }

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

    pub fn find_packages(&self, name: &AsciiStr) -> Vec<AtomicRef<IndexedPackage>> {
        if name.is_empty() {
            return Vec::default();
        }

        let pool = self.constant_pool();
        let split_index = rsplit_once(name, AsciiChar::Slash);

        let base_package = if split_index.0.is_empty() {
            // :/
            Some(AtomicRef::map(self.constant_pool(), |p| p.package_at(0)))
        } else {
            self.find_package(split_index.0)
        };

        match base_package {
            Some(p) => {
                let mut results = Vec::new();
                for sub_index in p.sub_packages_indices() {
                    let sub_package =
                        AtomicRef::map(self.constant_pool(), |p| p.package_at(*sub_index));
                    if pool
                        .string_view_at(sub_package.package_name_index)
                        .starts_with(&pool, split_index.1, MatchMode::IgnoreCase)
                    {
                        results.push(sub_package);
                    }
                }

                results
            }
            None => Vec::default(),
        }
    }

    pub fn find_package(&self, name: &AsciiStr) -> Option<AtomicRef<IndexedPackage>> {
        let pool = self.constant_pool();
        for sub_index in pool.package_at(0).sub_packages_indices() {
            let result = self.find_package_starting_at(name, *sub_index);
            if result.is_some() {
                return result.map(|t| t.1);
            }
        }

        None
    }

    fn find_package_starting_at(
        &self,
        name: &AsciiStr,
        start_package_index: u32,
    ) -> Option<(u32, AtomicRef<IndexedPackage>)> {
        let package = AtomicRef::map(self.constant_pool(), |pool| {
            pool.package_at(start_package_index)
        });
        let split_index = name
            .chars()
            .position(|ch| ch == AsciiChar::Slash)
            .unwrap_or(name.len());
        let part = &name[0..split_index];

        if package.package_name(&self.constant_pool()) != part {
            return None;
        }

        if split_index >= name.len() {
            //We've found it!
            Some((start_package_index, package))
        } else {
            let remaining_name = &name[split_index + 1..];
            for sub_index in package.sub_packages_indices() {
                let result = self.find_package_starting_at(remaining_name, *sub_index);
                if result.is_some() {
                    return result;
                }
            }

            None
        }
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
                    .starts_with(&self.constant_pool(), name, MatchMode::MatchCase)
            })
            .take(limit)
            .collect();
        Ok(res)
    }

    pub fn class_at_index(&self, index: u32) -> &IndexedClass {
        self.classes().get(index as usize).unwrap()
    }

    pub fn classes(&self) -> &Vec<IndexedClass> {
        &self.classes
    }

    pub fn constant_pool(&self) -> AtomicRef<ClassIndexConstantPool> {
        self.constant_pool.borrow()
    }

    pub fn constant_pool_mut(&self) -> AtomicRefMut<ClassIndexConstantPool> {
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
            let package_index = constant_pool.get_or_add_package_index(&class_info.package_name);
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
            let (indexed_class_index, indexed_class) = class_index
                .find_class(&class_info.package_name, &class_info.class_name)
                .unwrap();
            time += t.elapsed().as_nanos();

            //Add class to its package
            class_index
                .constant_pool_mut()
                .package_at_mut(indexed_class.package_index)
                .add_class(indexed_class_index);

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
    str.chars()
        .enumerate()
        .rev()
        .find(|(_, c)| *c == separator)
        .map(|(i, _)| (&str[0..i], &str[(i + 1)..]))
        .unwrap_or_else(|| (unsafe { "".as_ascii_str_unchecked() }, str))
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
    pub enclosing_type: Option<EnclosingTypeInfo>,
    pub inner_classes: Option<Vec<AsciiString>>,
    pub signature: RawClassSignature,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
}

#[derive(Debug)]
pub struct EnclosingTypeInfo {
    pub class_name: AsciiString,
    pub method_name: Option<AsciiString>,
    pub method_descriptor: Option<RawMethodSignature>,
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
    package_name_index: u32,
    sub_packages_indices: Vec<u32>,
    sub_classes_indices: Vec<u32>,
    previous_package_index: u32,
}

impl IndexedPackage {
    pub fn new(package_name_index: u32, previous_package_index: u32) -> Self {
        Self {
            package_name_index,
            sub_packages_indices: Vec::new(),
            sub_classes_indices: Vec::new(),
            previous_package_index,
        }
    }

    pub fn add_class(&mut self, class_index: u32) {
        self.sub_classes_indices.push(class_index);
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
        let mut parts = Vec::with_capacity(3);
        parts.push(
            constant_pool
                .string_view_at(self.package_name_index)
                .into_ascii_string(constant_pool),
        );

        let mut total_length = parts.first().unwrap().len();
        let mut parent_index = self.previous_package_index;
        while parent_index != 0 {
            let parent_package = constant_pool.package_at(parent_index);
            let package_name = parent_package.package_name(constant_pool);
            total_length += package_name.len();

            parts.push(package_name);
            parent_index = parent_package.previous_package_index;
        }

        let mut result = AsciiString::with_capacity(total_length);
        parts.iter().rev().enumerate().for_each(|(i, part)| {
            //Add separator if we're not the last part
            if i != 0 {
                unsafe { result.push_str("/".as_ascii_str_unchecked()) }
            }

            result.push_str(part)
        });

        result
    }

    pub fn add_sub_package(&mut self, index: u32) {
        self.sub_packages_indices.push(index);
    }

    pub fn sub_packages_indices(&self) -> &[u32] {
        &self.sub_packages_indices[..]
    }

    pub fn sub_classes_indices(&self) -> &[u32] {
        &self.sub_classes_indices[..]
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

macro_rules! get_attribute_info {
    ($attributes: expr, $match: pat_param) => {
        $attributes.iter().find(|a| matches!(&a.data, $match))
    };
}

macro_rules! get_attribute_data {
    ($attributes: expr, $info_match: pat_param, $data_var: expr, $default: expr) => {
        get_attribute_info!($attributes, $info_match).map_or($default, |a| {
            if let $info_match = &a.data {
                return $data_var;
            }
            unreachable!();
        })
    };
}

fn process_class_bytes_worker(bytes_queue: &[Vec<u8>]) -> Vec<ClassInfo> {
    let mut class_info_list = Vec::new();

    for bytes in bytes_queue.iter() {
        let parsed_class =
            parse_class_with_options(&bytes[..], ParseOptions::default().parse_bytecode(false));

        if let Ok(class) = parsed_class {
            let (full_class_name, enclosing_type, inner_classes) =
                convert_enclosing_type_and_inner_classes(
                    class.this_class,
                    get_attribute_data!(
                        &class.attributes,
                        AttributeData::EnclosingMethod { class_name, method },
                        Option::Some((class_name, method)),
                        Option::None
                    ),
                    get_attribute_data!(
                        &class.attributes,
                        AttributeData::InnerClasses(vec),
                        Option::Some(vec),
                        Option::None
                    ),
                );

            let parsed_signature =
                parse_class_signature(&class.attributes, class.super_class, class.interfaces);

            let (package_name, class_name) = rsplit_once(&full_class_name, AsciiChar::Slash);
            let package_name = package_name.into_ascii_string().unwrap();
            let class_name = class_name.into_ascii_string().unwrap();

            class_info_list.push(ClassInfo {
                package_name,
                class_name,
                access_flags: class.access_flags,
                signature: parsed_signature,
                enclosing_type,
                inner_classes,
                fields: class
                    .fields
                    .into_iter()
                    .filter_map(|f| {
                        let name = f.name.into_ascii_string();
                        if name.is_err() {
                            return None;
                        }

                        let signature = get_attribute_data!(
                            &f.attributes,
                            AttributeData::Signature(s),
                            s,
                            &f.descriptor
                        );

                        Some(FieldInfo {
                            field_name: name.unwrap(),
                            descriptor: SignatureType::from_str(signature)
                                .expect("Invalid field signature"),
                            access_flags: f.access_flags,
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

                        let signature = get_attribute_data!(
                            &m.attributes,
                            AttributeData::Signature(s),
                            s,
                            &m.descriptor
                        );

                        Some(MethodInfo {
                            method_name: name.unwrap(),
                            signature: RawMethodSignature::from_str(signature)
                                .expect("Invalid method signature"),
                            access_flags: m.access_flags,
                        })
                    })
                    .collect(),
            })
        }
    }

    class_info_list
}
fn convert_enclosing_type_and_inner_classes(
    this_name: Cow<str>,
    enclosing_method_data: Option<(&Cow<str>, &Option<NameAndType>)>,
    inner_class_data: Option<&Vec<InnerClassEntry>>,
) -> (
    AsciiString,
    Option<EnclosingTypeInfo>,
    Option<Vec<AsciiString>>,
) {
    //TODO: How to handle inner class access flags? "java/lang/ApplicationShutdownHooks$1" has FINAL | SUPER but inner class access flags are STATIC

    let mut new_this_name = this_name.to_owned().into_ascii_string().unwrap();
    let mut enclosing_type_info = None;
    let mut inner_classes = None;
    let mut skip_first_inner_class = false;

    //This blocks checks the first inner class entry which can represent this class. If so, we
    // extract the inner and outer class names from it.
    if let Some(vec) = inner_class_data {
        if let Some(first) = vec.first() {
            if first.inner_class_info.as_ref() == this_name {
                let (outer_name, inner_name) = extract_outer_and_inner_name(first);

                new_this_name = inner_name;
                enclosing_type_info = Some(EnclosingTypeInfo {
                    class_name: outer_name,
                    method_name: None,
                    method_descriptor: None,
                });

                skip_first_inner_class = true
            }
        }
    }
    if let Some((class_name, method_data)) = enclosing_method_data {
        let (method_name, method_descriptor) = match method_data {
            Some(NameAndType { name, descriptor }) => (
                Some(name.to_owned().into_ascii_string().unwrap()),
                Some(RawMethodSignature::from_str(descriptor).unwrap()),
            ),
            None => (None, None),
        };

        //NOTE: Anonymous class are allowed to have inner classes, but it's easier to just ignore them for now
        enclosing_type_info = Some(EnclosingTypeInfo {
            class_name: class_name.to_owned().into_ascii_string().unwrap(),
            method_name,
            method_descriptor,
        });
    } else if let Some(vec) = inner_class_data {
        inner_classes = Some(
            vec.iter()
                .skip(skip_first_inner_class as usize)
                .filter_map(|e| {
                    e.inner_name
                        .as_ref()
                        .map(|inner_name| inner_name.to_owned().into_ascii_string().unwrap())
                })
                .collect(),
        );
    }

    (new_this_name, enclosing_type_info, inner_classes)
}

fn extract_outer_and_inner_name(e: &InnerClassEntry) -> (AsciiString, AsciiString) {
    e.inner_name
        .as_ref()
        .filter(|n| !n.is_empty())
        .filter(|n| e.outer_class_info.is_some())
        .map(|n| {
            (
                e.outer_class_info
                    .as_ref()
                    .unwrap()
                    .to_owned()
                    .into_ascii_string()
                    .unwrap(),
                n.to_owned().into_ascii_string().unwrap(),
            )
        })
        .unwrap_or_else(|| {
            //If we don't have an inner name, we usually have an anonymous class like java/lang/Object$1.
            match &e.outer_class_info {
                //There might be an outer name which we can use to extract the inner name
                Some(outer_name) => (
                    outer_name.to_owned().into_ascii_string().unwrap(),
                    e.inner_class_info[outer_name.len() + 1..]
                        .to_owned()
                        .into_ascii_string()
                        .unwrap(),
                ),
                //Otherwise we trust the inner name info and split on '$'
                None => {
                    let split_pair = e.inner_class_info.rsplit_once('$').unwrap();
                    (
                        split_pair.0.to_owned().into_ascii_string().unwrap(),
                        split_pair.1.to_owned().into_ascii_string().unwrap(),
                    )
                }
            }
        })
}

fn parse_class_signature(
    attributes: &[AttributeInfo],
    super_class: Option<Cow<str>>,
    interfaces: Vec<Cow<str>>,
) -> RawClassSignature {
    if let Some(attr) = get_attribute_info!(attributes, AttributeData::Signature(_)) {
        RawClassSignature::from_str(match &attr.data {
            AttributeData::Signature(s) => s,
            _ => unreachable!(),
        })
        .expect("Invalid class signature")
    } else {
        RawClassSignature::new(
            super_class.map(|s| RawSignatureType::Object(s.into_ascii_string().unwrap())),
            Some(
                interfaces
                    .into_iter()
                    .map(|s| RawSignatureType::Object(s.into_ascii_string().unwrap()))
                    .collect(),
            )
            .filter(|v: &Vec<RawSignatureType>| !v.is_empty()),
        )
    }
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
