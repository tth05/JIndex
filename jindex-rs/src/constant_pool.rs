use crate::class_index::IndexedPackage;
use anyhow::{anyhow, Result};
use ascii::{AsciiChar, AsciiStr};
use speedy::{Readable, Writable};
use std::cmp::min;

#[derive(Readable, Writable)]
pub struct ClassIndexConstantPool {
    string_data: Vec<u8>, //Holds Ascii Strings prefixed with their length
    indexed_packages: Vec<IndexedPackage>,
}

impl ClassIndexConstantPool {
    pub fn new(capacity: u32) -> Self {
        Self {
            string_data: Vec::with_capacity(capacity as usize),
            indexed_packages: vec![IndexedPackage::new(0, 0, 0)],
        }
    }

    pub fn package_at(&self, index: u32) -> &IndexedPackage {
        self.indexed_packages.get(index as usize).unwrap()
    }

    pub fn get_or_add_package(&mut self, name: &AsciiStr) -> Result<&IndexedPackage> {
        self.get_or_add_package0(0, name)
    }

    /// This may be the most disgusting method I've ever written, but I suck at Rust too much to fix it
    fn get_or_add_package0(
        &mut self,
        indexed_package_index: u32,
        name: &AsciiStr,
    ) -> Result<&IndexedPackage> {
        let slash_index_or_none = name.chars().position(|char| char == '/');
        let sub_name = match slash_index_or_none {
            Some(dot_index) => &name[..dot_index],
            None => name,
        };

        let possible_index = self
            .indexed_packages
            .get(indexed_package_index as usize)
            .unwrap()
            .sub_packages_indexes()
            .iter()
            .enumerate()
            .find(|p| {
                self.indexed_packages
                    .get(*p.1 as usize)
                    .unwrap()
                    .package_name(self)
                    .eq(sub_name)
            })
            .map(|p| *p.1);

        if let Some(index) = possible_index {
            if let Some(dot_index) = slash_index_or_none {
                self.get_or_add_package0(index, &name[dot_index + 1..])
            } else {
                Ok(self.indexed_packages.get(index as usize).unwrap())
            }
        } else {
            let name_index = self.add_string(sub_name.as_bytes()).unwrap();
            let new_index = self.indexed_packages.len();
            self.indexed_packages.push(IndexedPackage::new(
                new_index as u32,
                name_index,
                indexed_package_index,
            ));
            self.indexed_packages
                .get_mut(indexed_package_index as usize)
                .unwrap()
                .add_sub_package(new_index as u32);

            if let Some(index) = slash_index_or_none {
                self.get_or_add_package0(new_index as u32, &name[index + 1..])
            } else {
                Ok(self.indexed_packages.last().unwrap())
            }
        }
    }

    pub fn clear_sub_packages(&mut self) {
        for p in self.indexed_packages.iter_mut() {
            p.clear_sub_packages();
        }
    }

    pub fn add_string(&mut self, str: &[u8]) -> Result<u32> {
        let index = self.string_data.len();
        let length = str.len();
        if length > u8::MAX as usize {
            return Err(anyhow!(
                "The string {} exceeds the maximum size of {}",
                AsciiStr::from_ascii(str).unwrap(),
                u8::MAX
            ));
        }

        self.string_data.try_reserve(1 + str.len())?;
        self.string_data.push(length as u8);
        self.string_data.extend_from_slice(str);

        Ok(index as u32)
    }

    pub fn string_view_at(&self, index: u32) -> ConstantPoolStringView {
        ConstantPoolStringView {
            index,
            start: 1,
            end: 1 + /* Add the length */ self.string_data.get(index as usize).unwrap(),
        }
    }
}

#[derive(Debug, Eq)]
pub struct ConstantPoolStringView {
    index: u32,
    start: u8,
    end: u8,
}

impl PartialEq for ConstantPoolStringView {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.start == other.start && self.end == other.end
    }
}

impl ConstantPoolStringView {
    pub fn new(index: u32, start: u8, end: u8) -> Self {
        Self { index, start, end }
    }

    pub fn to_ascii_string(self, constant_pool: &ClassIndexConstantPool) -> &AsciiStr {
        AsciiStr::from_ascii(
            &constant_pool.string_data[(self.index + self.start as u32) as usize
                ..(self.index + self.end as u32) as usize],
        )
        .unwrap()
    }

    pub fn substring_to_end(&self, start: u8) -> Result<ConstantPoolStringView> {
        self.substring(min(start, self.end - self.start), self.end - self.start)
    }

    pub fn substring(&self, start: u8, end: u8) -> Result<ConstantPoolStringView> {
        if start > end || end > self.end - self.start {
            return Err(anyhow::Error::msg("Parameters out of range"));
        }

        Ok(ConstantPoolStringView {
            index: self.index,
            start: self.start + start,
            end: self.start + end,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.end == self.start
    }

    pub fn byte_at(&self, constant_pool: &ClassIndexConstantPool, index: u8) -> u8 {
        *constant_pool
            .string_data
            .get(self.index as usize + self.start as usize + index as usize)
            .unwrap()
    }

    pub fn starts_with(
        &self,
        constant_pool: &ClassIndexConstantPool,
        other: &AsciiStr,
        ignore_case: bool,
    ) -> bool {
        if other.len() > self.len() as usize {
            return false;
        }

        for i in 0..min(self.len(), other.len() as u8) {
            let current_byte = self.byte_at(constant_pool, i);
            let current_char = other[i as usize];
            if current_byte != current_char
                && (!ignore_case || current_byte != switch_ascii_char_case(current_char))
            {
                return false;
            }
        }

        true
    }

    pub fn equals_ascii(
        &self,
        constant_pool: &ClassIndexConstantPool,
        other: &AsciiStr
    ) -> bool {
        if other.len() != self.len() as usize {
            return false;
        }

        for i in 0..self.len() {
            let current_byte = self.byte_at(constant_pool, i);
            let current_char = other[i as usize];
            if current_byte != current_char {
                return false;
            }
        }

        true
    }

    pub fn len(&self) -> u8 {
        self.end - self.start
    }

    pub fn index(&self) -> u32 {
        self.index
    }
    pub fn start(&self) -> u8 {
        self.start
    }
    pub fn end(&self) -> u8 {
        self.end
    }
}

fn switch_ascii_char_case(char: AsciiChar) -> AsciiChar {
    if char.is_uppercase() {
        char.to_ascii_lowercase()
    } else {
        char.to_ascii_uppercase()
    }
}
