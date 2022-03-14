use crate::constant_pool::ClassIndexConstantPool;
use crate::signature::{SignaturePrimitive, SignatureType};
use ascii::{AsAsciiStr, AsciiChar, AsciiStr, AsciiString, IntoAsciiString};
use cafebabe::attributes::AttributeData;
use cafebabe::{
    parse_class_with_options, ClassAccessFlags, FieldAccessFlags, MethodAccessFlags, ParseOptions,
};
use jni::signature::{Primitive, TypeSignature};
use speedy::{Readable, Writable};
use std::cell::{Ref, RefCell, RefMut};
use std::cmp::{min, Ordering};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
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
            match a_name.cmp(b_name) {
                Ordering::Equal => constant_pool
                    .package_at(a.package_index)
                    .package_name_with_parents_cmp(
                        &constant_pool,
                        &constant_pool
                            .package_at(b.package_index)
                            .package_name_with_parents(&constant_pool),
                    ),
                o => o,
            }
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
            match a.class_name(&self.constant_pool()).cmp(class_name) {
                Ordering::Equal => self
                    .constant_pool()
                    .package_at(a.package_index)
                    .package_name_with_parents_cmp(&self.constant_pool(), package_name),
                o => o,
            }
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

        let mut time = 0u128;
        for class_info in vec.iter() {
            let t = Instant::now();
            let indexed_class = class_index
                .find_class(&class_info.package_name, &class_info.class_name)
                .unwrap()
                .1;
            time += t.elapsed().as_nanos();

            //Super class
            if let Some(super_class_name) = &class_info.super_class {
                let super_class_name = rsplit_once(super_class_name, AsciiChar::Slash);
                let index_or_none = class_index
                    .find_class(super_class_name.0, super_class_name.1)
                    .map(|s| s.0);
                if let Some(i) = index_or_none {
                    indexed_class.set_super_class_index(i);
                }
            }

            //Interfaces
            indexed_class.set_interfaces_indices(
                class_info
                    .interfaces
                    .iter()
                    .filter_map(|interface_name| {
                        let interface_name = rsplit_once(interface_name, AsciiChar::Slash);
                        class_index
                            .find_class(interface_name.0, interface_name.1)
                            .map(|s| s.0)
                    })
                    .collect(),
            );

            //Fields
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
                    field_info.access_flags.bits(),
                    ClassIndexBuilder::compute_signature_for_descriptor(
                        &field_info.descriptor,
                        &class_index,
                    ),
                ));
            }

            //Methods
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
                    IndexedMethodSignature::new(
                        /*method_info
                        .signature
                        .args
                        .iter()
                        .map(|arg| {
                            ClassIndexBuilder::compute_signature_for_descriptor(
                                arg,
                                &class_index,
                            )
                        })
                        .collect()*/
                        Vec::new(),
                        ClassIndexBuilder::compute_signature_for_descriptor(
                            &SignatureType::Primitive(SignaturePrimitive::Void),
                            &class_index,
                        ),
                    ),
                ));
            }
        }

        println!("Time spent in findClass {:?}", time.div(1_000_000));

        class_index
    }

    fn compute_signature_for_descriptor(
        signature_type: &SignatureType,
        class_index: &ClassIndex,
    ) -> IndexedSignature {
        match signature_type {
            SignatureType::Object(full_class_name) => {
                ClassIndexBuilder::compute_signature_for_object(full_class_name, class_index)
            }
            SignatureType::Primitive(p) => IndexedSignature::from_primitive_type(p),
            SignatureType::Array(t) => IndexedSignature::Array(Box::new(
                ClassIndexBuilder::compute_signature_for_descriptor(t, class_index),
            )),
            _ => unreachable!(),
        }
    }

    fn compute_signature_for_object(
        full_class_name: &AsciiStr,
        class_index: &ClassIndex,
    ) -> IndexedSignature {
        let split_pair = rsplit_once(full_class_name, AsciiChar::Slash);

        let package_name = split_pair.0;
        let class_name = split_pair.1;

        // let t = Instant::now();
        let option = class_index.find_class(package_name, class_name);
        // time += t.elapsed().as_nanos();
        if option.is_none() {
            IndexedSignature::Unresolved
        } else {
            IndexedSignature::Object(
                option
                    .unwrap_or_else(|| {
                        panic!("Field type not found {:?}, {:?}", package_name, class_name)
                    })
                    .0,
            )
        }
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

fn rsplit_once(str: &AsciiStr, separator: AsciiChar) -> (&AsciiStr, &AsciiStr) {
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
    pub super_class: Option<AsciiString>,
    pub interfaces: Vec<AsciiString>,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
}

pub struct FieldInfo {
    pub field_name: AsciiString,
    pub descriptor: SignatureType,
    pub access_flags: FieldAccessFlags,
}

pub struct MethodInfo {
    pub method_name: AsciiString,
    pub signature: TypeSignature,
    pub access_flags: MethodAccessFlags,
}

// #[derive(Readable, Writable)]
pub struct IndexedClass {
    package_index: u32,
    name_index: u32,
    access_flags: u16,
    //TODO: These should use IndexedSignatureType to support generic data, for
    // example 'implements Comparable<? extends Number>'. This would
    // be 'Ljava/lang/Object;Ljava/lang/Comparable<+Ljava/lang/Number>;'
    super_class_index: RefCell<Option<u32>>,
    interfaces_indices: RefCell<Option<Vec<u32>>>,
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
            super_class_index: RefCell::new(None),
            interfaces_indices: RefCell::new(None),
            fields: RefCell::new(fields),
            methods: RefCell::new(methods),
        }
    }

    pub fn class_name<'b>(&self, constant_pool: &'b ClassIndexConstantPool) -> &'b AsciiStr {
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

        package_name + "/".as_ascii_str().unwrap() + class_name
    }

    pub fn set_super_class_index(&self, super_class_index: u32) {
        self.super_class_index.replace(Some(super_class_index));
    }

    pub fn set_interfaces_indices(&self, interfaces_indices: Vec<u32>) {
        self.interfaces_indices.replace(Some(interfaces_indices));
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

    pub fn super_class_index(&self) -> Option<u32> {
        *self.super_class_index.borrow()
    }

    pub fn interface_indicies(&self) -> Ref<Option<Vec<u32>>> {
        self.interfaces_indices.borrow()
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

#[derive(Clone)]
pub enum IndexedSignature {
    Primitive(jni::signature::Primitive),
    Object(u32),
    Array(Box<IndexedSignature>),
    Void,
    Unresolved,
}

//TODO: Primitive const optimization? Signatures take a lot of RAM
/*static PRIMITIVE_SIG_BOOLEAN: IndexedSignature = IndexedSignature::Primitive(0);
static PRIMITIVE_SIG_BYTE :IndexedSignature = IndexedSignature::Primitive(1);
static PRIMITIVE_SIG_CHAR :IndexedSignature = IndexedSignature::Primitive(2);
static PRIMITIVE_SIG_DOUBLE :IndexedSignature = IndexedSignature::Primitive(3);
static PRIMITIVE_SIG_FLOAT :IndexedSignature = IndexedSignature::Primitive(4);
static PRIMITIVE_SIG_INT :IndexedSignature = IndexedSignature::Primitive(5);
static PRIMITIVE_SIG_LONG :IndexedSignature = IndexedSignature::Primitive(6);
static PRIMITIVE_SIG_SHORT :IndexedSignature = IndexedSignature::Primitive(7);
static PRIMITIVE_SIG_VOID :IndexedSignature = IndexedSignature::Primitive(8);
*/
impl IndexedSignature {
    fn from_primitive_type(t: &jni::signature::Primitive) -> Self {
        match t {
            Primitive::Void => IndexedSignature::Void,
            _ => IndexedSignature::Primitive(t.clone()),
        }
    }

    pub fn signature_string(&self, class_index: &ClassIndex) -> String {
        Self::signature_to_string(self, class_index)
    }

    fn signature_to_string(sig: &IndexedSignature, class_index: &ClassIndex) -> String {
        match sig {
            IndexedSignature::Primitive(i) => i.to_string(),
            IndexedSignature::Object(index) => {
                let mut result = String::from("L;");
                result.insert_str(
                    1,
                    class_index
                        .class_at_index(*index)
                        .class_name_with_package(&class_index.constant_pool())
                        .as_ref(),
                );
                result
            }
            IndexedSignature::Array(sig) => {
                String::from("[") + &IndexedSignature::signature_to_string(sig, class_index)
            }
            IndexedSignature::Void => String::from("V"),
            IndexedSignature::Unresolved => String::from(""),
        }
    }
}

#[derive(Readable, Writable, Clone)]
pub struct IndexedMethodSignature {
    //TODO: Parameter names
    params: Option<Vec<IndexedSignature>>,
    return_type: IndexedSignature,
}

impl IndexedMethodSignature {
    pub fn new(params: Vec<IndexedSignature>, return_type: IndexedSignature) -> Self {
        Self {
            params: Some(params).filter(|v| !v.is_empty()),
            return_type,
        }
    }

    pub fn params(&self) -> Option<&Vec<IndexedSignature>> {
        self.params.as_ref()
    }

    pub fn return_type(&self) -> &IndexedSignature {
        &self.return_type
    }
}

#[derive(Readable, Writable)]
pub struct IndexedField {
    name_index: u32,
    access_flags: u16,
    field_signature: IndexedSignature,
}

impl IndexedField {
    pub fn new(name_index: u32, access_flags: u16, field_signature: IndexedSignature) -> Self {
        Self {
            name_index,
            access_flags,
            field_signature,
        }
    }

    pub fn field_name<'b>(&self, constant_pool: &'b ClassIndexConstantPool) -> &'b AsciiStr {
        constant_pool
            .string_view_at(self.name_index)
            .to_ascii_string(constant_pool)
    }

    pub fn access_flags(&self) -> u16 {
        self.access_flags
    }

    pub fn field_signature(&self) -> &IndexedSignature {
        &self.field_signature
    }
}

#[derive(Readable, Writable, Clone)]
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
            .to_ascii_string(constant_pool)
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
            .to_ascii_string(constant_pool)
    }

    pub fn package_name_with_parents_equals(
        &self,
        constant_pool: &ClassIndexConstantPool,
        str: &AsciiStr,
    ) -> bool {
        self.package_name_with_parents_cmp(constant_pool, str) == Ordering::Equal
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
                    return Ordering::Equal;
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
            .to_ascii_string(constant_pool)
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
            //<T:Ljava/lang/Throwable;>Ljava/lang/Object;Ljava/lang/Cloneable;
            //<A:LMain;B:LMain;:Ljava/lang/Comparable;>([TA;)TB;
            let option = class
                .attributes
                .iter()
                .find(|a| matches!(a.data, AttributeData::Signature(_)));

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
                super_class: class.super_class.map(|s| s.into_ascii_string().unwrap()),
                interfaces: class
                    .interfaces
                    .into_iter()
                    .map(|i| i.into_ascii_string().unwrap())
                    .collect(),
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

                        Some(MethodInfo {
                            method_name: name.unwrap(),
                            signature: TypeSignature::from_str(&m.descriptor)
                                .expect("Not a method descriptor"),
                            access_flags: m.access_flags,
                        })
                    })
                    .collect(),
            })
        }
    }

    class_info_list
}

pub fn create_class_index(class_bytes: Vec<Vec<u8>>) -> ClassIndex {
    let mut now = Instant::now();
    let class_info_list: Vec<ClassInfo> =
        do_multi_threaded(class_bytes, &process_class_bytes_worker);

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
