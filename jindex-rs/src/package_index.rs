use crate::constant_pool::ClassIndexConstantPool;
use anyhow::{anyhow, Result};
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
        name: &str,
    ) -> Result<u32> {
        let mut parent_index = 0;
        for component in name.split('/') {
            let existing_index = self.indexed_packages[parent_index as usize]
                .sub_packages_indices()
                .iter()
                .copied()
                .find(|index| {
                    self.indexed_packages[*index as usize]
                        .package_name(constant_pool)
                        .eq(component)
                });
            if let Some(index) = existing_index {
                parent_index = index;
                continue;
            }

            let component_index = constant_pool.add_string(component.as_bytes())?;
            let new_index = u32::try_from(self.indexed_packages.len())
                .map_err(|_| anyhow!("Package index exceeds the supported u32 range"))?;
            self.indexed_packages
                .push(IndexedPackage::new(component_index, parent_index));
            self.indexed_packages[parent_index as usize].add_sub_package(new_index);
            parent_index = new_index;
        }

        Ok(parent_index)
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
            .into_ascii_str(constant_pool)
    }

    pub fn package_name_with_parents_cmp(
        &self,
        package_index: &PackageIndex,
        constant_pool: &ClassIndexConstantPool,
        str: &AsciiStr,
    ) -> Ordering {
        let mut current_package = self;
        let mut current_part = constant_pool.string_view_at(current_package.package_name_index);
        if str.is_empty() {
            return if current_part.is_empty() {
                Ordering::Equal
            } else {
                Ordering::Greater
            };
        }

        let mut index = str.len() - 1;

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
                .into_ascii_str(constant_pool),
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

    pub fn sub_classes_indices(&self) -> AtomicRef<'_, Vec<u32>> {
        self.sub_classes_indices.borrow()
    }

    pub fn package_name_index(&self) -> u32 {
        self.package_name_index
    }

    pub fn previous_package_index(&self) -> u32 {
        self.previous_package_index
    }
}

#[cfg(test)]
mod audit_tests {
    use super::*;

    #[test]
    fn audit_default_package_comparison() {
        let mut pool = ClassIndexConstantPool::new(0);
        pool.add_string(b"").unwrap();
        let packages = PackageIndex::new();
        assert_eq!(
            Ordering::Equal,
            packages.package_at(0).package_name_with_parents_cmp(
                &packages,
                &pool,
                "".as_ascii_str().unwrap()
            )
        );
    }
}
