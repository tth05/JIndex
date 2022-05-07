use crate::constant_pool::ClassIndexConstantPool;
use ascii::{AsAsciiStr, AsciiStr, AsciiString};
use atomic_refcell::{AtomicRef, AtomicRefCell};
use speedy::{Readable, Writable};
use std::cmp::Ordering;

#[derive(Readable, Writable)]
pub struct PackageIndex {
    indexed_packages: Vec<IndexedPackage>,
}

impl PackageIndex {
    pub(crate) fn new() -> PackageIndex {
        PackageIndex {
            indexed_packages: vec![IndexedPackage::new(0, 0)],
        }
    }

    pub(crate) fn get_or_add_package_index(
        &mut self,
        constant_pool: &mut ClassIndexConstantPool,
        name: &AsciiStr,
    ) -> u32 {
        self.get_or_add_package_index0(0, constant_pool, name)
    }

    /// This may be the most disgusting method I've ever written, but I suck at
    /// Rust too much to fix it
    fn get_or_add_package_index0(
        &mut self,
        indexed_package_index: u32,
        constant_pool: &mut ClassIndexConstantPool,
        name: &AsciiStr,
    ) -> u32 {
        let slash_index_or_none = name.chars().position(|char| char == '/');
        let sub_name = match slash_index_or_none {
            Some(dot_index) => &name[..dot_index],
            None => name,
        };

        let possible_index = self
            .indexed_packages
            .get(indexed_package_index as usize)
            .unwrap()
            .sub_packages_indices()
            .iter()
            .enumerate()
            .find(|p| {
                self.indexed_packages
                    .get(*p.1 as usize)
                    .unwrap()
                    .package_name(constant_pool)
                    .eq(sub_name)
            })
            .map(|p| *p.1);

        if let Some(index) = possible_index {
            if let Some(dot_index) = slash_index_or_none {
                self.get_or_add_package_index0(index, constant_pool, &name[dot_index + 1..])
            } else {
                index
            }
        } else {
            let name_index = constant_pool.add_string(sub_name.as_bytes()).unwrap();
            let new_index = self.indexed_packages.len();
            self.indexed_packages
                .push(IndexedPackage::new(name_index, indexed_package_index));
            self.indexed_packages
                .get_mut(indexed_package_index as usize)
                .unwrap()
                .add_sub_package(new_index as u32);

            if let Some(index) = slash_index_or_none {
                self.get_or_add_package_index0(new_index as u32, constant_pool, &name[index + 1..])
            } else {
                new_index as u32
            }
        }
    }

    pub fn package_at(&self, index: u32) -> &IndexedPackage {
        self.indexed_packages.get(index as usize).unwrap()
    }
}

pub struct IndexedPackage {
    package_name_index: u32,
    sub_packages_indices: Vec<u32>,
    sub_classes_indices: AtomicRefCell<Vec<u32>>,
    previous_package_index: u32,
}

impl IndexedPackage {
    pub(crate) fn new(package_name_index: u32, previous_package_index: u32) -> Self {
        Self {
            package_name_index,
            sub_packages_indices: Vec::new(),
            sub_classes_indices: AtomicRefCell::default(),
            previous_package_index,
        }
    }

    pub(crate) fn add_class(&self, class_index: u32) {
        self.sub_classes_indices.borrow_mut().push(class_index);
    }

    pub fn package_name<'a>(&self, constant_pool: &'a ClassIndexConstantPool) -> &'a AsciiStr {
        constant_pool
            .string_view_at(self.package_name_index)
            .into_ascii_string(constant_pool)
    }

    pub fn package_name_with_parents_cmp(
        &self,
        package_index: &PackageIndex,
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

            current_package = package_index.package_at(current_package.previous_package_index);
            current_part = constant_pool.string_view_at(current_package.package_name_index);
        }

        Ordering::Less
    }

    pub fn package_name_with_parents(
        &self,
        package_index: &PackageIndex,
        constant_pool: &ClassIndexConstantPool,
    ) -> AsciiString {
        let mut parts = Vec::with_capacity(3);
        parts.push(
            constant_pool
                .string_view_at(self.package_name_index)
                .into_ascii_string(constant_pool),
        );

        let mut total_length = parts.first().unwrap().len();
        let mut parent_index = self.previous_package_index;
        while parent_index != 0 {
            let parent_package = package_index.package_at(parent_index);
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

    pub(crate) fn add_sub_package(&mut self, index: u32) {
        self.sub_packages_indices.push(index);
    }

    pub fn sub_packages_indices(&self) -> &[u32] {
        &self.sub_packages_indices[..]
    }

    pub fn sub_classes_indices(&self) -> AtomicRef<Vec<u32>> {
        self.sub_classes_indices.borrow()
    }

    pub fn package_name_index(&self) -> u32 {
        self.package_name_index
    }

    pub fn previous_package_index(&self) -> u32 {
        self.previous_package_index
    }
}
