use crate::class_index::ClassIndex;
use crate::constant_pool::ClassIndexConstantPool;
use crate::package_index::PackageIndex;
use crate::signature::{
    IndexedClassSignature, IndexedEnclosingTypeInfo, IndexedMethodSignature, IndexedSignatureType,
};
use ascii::{AsAsciiStr, AsciiStr, AsciiString};
use atomic_refcell::{AtomicRef, AtomicRefCell};
use cafebabe::MethodAccessFlags;
use speedy::{Readable, Writable};
use std::lazy::OnceCell;

pub struct IndexedClass {
    index: OnceCell<u32>,
    package_index: u32,
    name_index: u32,
    name_start_index: u8,
    access_flags: u16,
    signature: OnceCell<IndexedClassSignature>,
    enclosing_type_info: OnceCell<IndexedEnclosingTypeInfo>,
    member_classes: AtomicRefCell<Vec<u32>>,
    fields: OnceCell<Vec<IndexedField>>,
    methods: OnceCell<Vec<IndexedMethod>>,
}

#[macro_export]
macro_rules! all_direct_super_types {
    ($ref: ident) => {
        $ref.signature()
            .super_class()
            .into_iter()
            .chain($ref.signature().interfaces().iter().flat_map(|v| v.iter()))
    };
}

impl IndexedClass {
    pub(crate) fn new(
        package_index: u32,
        class_name_index: u32,
        class_name_start_index: u8,
        access_flags: u16,
    ) -> Self {
        Self {
            index: OnceCell::new(),
            package_index,
            name_index: class_name_index,
            name_start_index: class_name_start_index,
            access_flags,
            signature: OnceCell::new(),
            enclosing_type_info: OnceCell::new(),
            member_classes: AtomicRefCell::default(),
            fields: OnceCell::new(),
            methods: OnceCell::new(),
        }
    }

    pub fn class_name<'b>(&self, constant_pool: &'b ClassIndexConstantPool) -> &'b AsciiStr {
        constant_pool
            .string_view_at(self.name_index)
            .into_ascii_string(constant_pool)
    }

    pub fn class_name_with_package(
        &self,
        package_index: &PackageIndex,
        constant_pool: &ClassIndexConstantPool,
    ) -> AsciiString {
        let package_name = package_index
            .package_at(self.package_index)
            .package_name_with_parents(package_index, constant_pool);
        let class_name = constant_pool
            .string_view_at(self.name_index)
            .into_ascii_string(constant_pool);

        if package_name.is_empty() {
            class_name.to_ascii_string()
        } else {
            package_name + unsafe { "/".as_ascii_str_unchecked() } + class_name
        }
    }

    pub(crate) fn add_member_class(&self, class: u32) {
        self.member_classes.borrow_mut().push(class);
    }

    pub fn enclosing_class<'a>(&self, class_index: &'a ClassIndex) -> Option<&'a IndexedClass> {
        self.enclosing_type_info()
            .filter(|info| info.class_name().is_some())
            .map(|info| class_index.class_at_index(*info.class_name().unwrap()))
    }

    pub fn is_direct_sub_type_of(&self, other_class: u32) -> bool {
        all_direct_super_types!(self)
            .filter_map(|s| s.extract_base_object_type())
            .any(|o| o == other_class)
    }

    pub fn index(&self) -> u32 {
        *self.index.get().unwrap()
    }

    pub(crate) fn set_index(&self, index: u32) {
        self.index.set(index).unwrap();
    }

    pub(crate) fn set_signature(&self, signature: IndexedClassSignature) {
        self.signature.set(signature).unwrap();
    }

    pub(crate) fn set_enclosing_type_info(&self, info: IndexedEnclosingTypeInfo) {
        self.enclosing_type_info.set(info).unwrap();
    }

    pub fn class_name_index(&self) -> u32 {
        self.name_index
    }

    pub fn class_name_start_index(&self) -> u8 {
        self.name_start_index
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

    pub fn enclosing_type_info(&self) -> Option<&IndexedEnclosingTypeInfo> {
        self.enclosing_type_info.get()
    }

    pub fn fields(&self) -> &Vec<IndexedField> {
        self.fields.get().unwrap()
    }

    pub(crate) fn set_fields(&self, fields: Vec<IndexedField>) -> Result<(), Vec<IndexedField>> {
        self.fields.set(fields)
    }

    pub fn methods(&self) -> &Vec<IndexedMethod> {
        self.methods.get().unwrap()
    }

    pub(crate) fn set_methods(
        &self,
        methods: Vec<IndexedMethod>,
    ) -> Result<(), Vec<IndexedMethod>> {
        self.methods.set(methods)
    }

    pub fn member_classes(&self) -> AtomicRef<Vec<u32>> {
        self.member_classes.borrow()
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
    pub(crate) fn new(
        name_index: u32,
        access_flags: u16,
        field_signature: IndexedSignatureType,
    ) -> Self {
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
    pub(crate) fn new(
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

    pub fn overrides(&self, base_method: &IndexedMethod) -> bool {
        // If the target method is private, we can't override it
        if MethodAccessFlags::PRIVATE.bits() & base_method.access_flags != 0 {
            return false;
        }

        self.name_index == base_method.name_index
            && self.method_signature.parameter_count()
                == base_method.method_signature.parameter_count()
            && self
                .method_signature
                .parameters()
                .map(|a| {
                    a.iter()
                        // Safety: We know that the parameter count is equal, so we know that the
                        //  other method must have parameters
                        .zip(base_method.method_signature.parameters().unwrap().iter())
                        .all(|(a, b)| a.eq_erased(b))
                })
                // If we don't have any parameters, the count is 0, so we automatically match
                .unwrap_or(true)
    }

    pub fn method_name_index(&self) -> u32 {
        self.name_index
    }

    pub fn access_flags(&self) -> u16 {
        self.access_flags
    }

    pub fn method_signature(&self) -> &IndexedMethodSignature {
        &self.method_signature
    }
}
