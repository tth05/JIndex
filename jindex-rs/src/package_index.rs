use crate::constant_pool::ClassIndexConstantPool;
use anyhow::{anyhow, Result};
use ascii::{AsciiChar, AsciiStr, AsciiString};
use atomic_refcell::{AtomicRef, AtomicRefCell};
use speedy::{Readable, Writable};

#[derive(Readable, Writable)]
pub struct PackageIndex {
    indexed_packages: Vec<IndexedPackage>,
}

impl PackageIndex {
    pub(crate) fn validate_snapshot(
        &self,
        pool: &ClassIndexConstantPool,
        entries: &rustc_hash::FxHashSet<u32>,
        classes: &[crate::class_index_members::IndexedClass],
    ) -> Result<()> {
        use anyhow::ensure;
        ensure!(
            !self.indexed_packages.is_empty(),
            "Snapshot has no root package"
        );
        let mut seen_packages = vec![false; self.indexed_packages.len()];
        let mut seen_classes = vec![false; classes.len()];
        seen_packages[0] = true;
        for (index, package) in self.indexed_packages.iter().enumerate() {
            ensure!(
                entries.contains(&package.package_name_index),
                "Invalid package name offset"
            );
            if index == 0 {
                ensure!(
                    package.package_name(pool).is_empty() && package.previous_package_index == 0,
                    "Invalid root package"
                );
            } else {
                ensure!(
                    (package.previous_package_index as usize) < index,
                    "Invalid package parent index"
                );
            }
            let mut names = rustc_hash::FxHashSet::default();
            for child in &package.sub_packages_indices {
                let child = *child as usize;
                ensure!(
                    child < self.indexed_packages.len() && !seen_packages[child],
                    "Invalid or duplicate child package index"
                );
                let child_package = &self.indexed_packages[child];
                ensure!(
                    child_package.previous_package_index as usize == index,
                    "Package parent/child mismatch"
                );
                ensure!(
                    entries.contains(&child_package.package_name_index),
                    "Invalid child package name offset"
                );
                ensure!(
                    names.insert(child_package.package_name(pool)),
                    "Duplicate child package name"
                );
                seen_packages[child] = true;
            }
            for class in package.sub_classes_indices.borrow().iter().copied() {
                let class = class as usize;
                ensure!(
                    class < classes.len() && !seen_classes[class],
                    "Invalid or duplicate package class index"
                );
                ensure!(
                    classes[class].package_index() as usize == index,
                    "Class/package mismatch"
                );
                seen_classes[class] = true;
            }
        }
        ensure!(
            seen_packages.iter().all(|seen| *seen) && seen_classes.iter().all(|seen| *seen),
            "Snapshot contains orphan packages or classes"
        );
        Ok(())
    }

    pub(crate) fn new(constant_pool: &mut ClassIndexConstantPool) -> Result<Self> {
        Ok(Self {
            indexed_packages: vec![IndexedPackage::new(constant_pool.add_string(b"")?, 0)],
        })
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
            let mut package = IndexedPackage::new(component_index, parent_index);
            package.full_name = self.child_name(parent_index, package.package_name(constant_pool));
            self.indexed_packages.push(package);
            self.indexed_packages[parent_index as usize].add_sub_package(new_index);
            parent_index = new_index;
        }

        Ok(parent_index)
    }

    // Cached names are derived, not persisted. Parents always precede children in the snapshot.
    pub(crate) fn rebuild_full_names(&mut self, constant_pool: &ClassIndexConstantPool) {
        for index in 1..self.indexed_packages.len() {
            let package = &self.indexed_packages[index];
            assert!(
                (package.previous_package_index as usize) < index,
                "Invalid package parent index"
            );
            let name = self.child_name(
                package.previous_package_index,
                package.package_name(constant_pool),
            );
            self.indexed_packages[index].full_name = name;
        }
    }

    fn child_name(&self, parent: u32, component: &AsciiStr) -> AsciiString {
        let parent = self.package_at(parent).full_name();
        let mut name = AsciiString::with_capacity(
            parent.len() + usize::from(!parent.is_empty()) + component.len(),
        );
        name.push_str(parent);
        if !parent.is_empty() {
            name.push(AsciiChar::Slash);
        }
        name.push_str(component);
        name
    }

    pub fn package_at(&self, index: u32) -> &IndexedPackage {
        self.indexed_packages.get(index as usize).unwrap()
    }
}

pub struct IndexedPackage {
    package_name_index: u32,
    full_name: AsciiString,
    sub_packages_indices: Vec<u32>,
    sub_classes_indices: AtomicRefCell<Vec<u32>>,
    previous_package_index: u32,
}

impl IndexedPackage {
    pub(crate) fn new(package_name_index: u32, previous_package_index: u32) -> Self {
        Self {
            package_name_index,
            full_name: AsciiString::new(),
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

    pub fn full_name(&self) -> &AsciiStr {
        &self.full_name
    }

    pub(crate) fn add_sub_package(&mut self, index: u32) {
        self.sub_packages_indices.push(index);
    }

    pub fn sub_packages_indices(&self) -> &[u32] {
        &self.sub_packages_indices
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
mod tests {
    use super::*;

    #[test]
    fn cached_package_names_include_empty_and_nested_packages() {
        let mut pool = ClassIndexConstantPool::new(0);
        pool.add_string(b"existing").unwrap();
        let mut packages = PackageIndex::new(&mut pool).unwrap();
        assert_eq!("", packages.package_at(0).package_name(&pool));
        for name in ["java/lang", "", "az", "ba", "java/lang/invoke"] {
            let index = packages.get_or_add_package_index(&mut pool, name).unwrap();
            assert_eq!(name, packages.package_at(index).full_name());
        }
        let mut loaded = PackageIndex::read_from_buffer(&packages.write_to_vec().unwrap()).unwrap();
        loaded.rebuild_full_names(&pool);
        for (expected, actual) in packages
            .indexed_packages
            .iter()
            .zip(&loaded.indexed_packages)
        {
            assert_eq!(expected.full_name(), actual.full_name());
        }
    }
}
