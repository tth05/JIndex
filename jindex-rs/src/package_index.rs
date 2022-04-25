use crate::class_index::IndexedPackage;
use crate::constant_pool::ClassIndexConstantPool;
use ascii::AsciiStr;
use speedy::{Readable, Writable};

#[derive(Readable, Writable)]
pub struct PackageIndex {
    indexed_packages: Vec<IndexedPackage>,
}

impl PackageIndex {
    pub fn new() -> PackageIndex {
        PackageIndex {
            indexed_packages: vec![IndexedPackage::new(0, 0)],
        }
    }

    pub fn get_or_add_package_index(
        &mut self,
        constant_pool: &mut ClassIndexConstantPool,
        name: &AsciiStr,
    ) -> u32 {
        self.get_or_add_package_index0(0, constant_pool, name)
    }

    /// This may be the most disgusting method I've ever written, but I suck at Rust too much to fix it
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
