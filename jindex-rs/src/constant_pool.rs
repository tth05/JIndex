use crate::class_index::IndexedPackage;
use anyhow::{anyhow, Result};
use ascii::{AsciiChar, AsciiStr};
use speedy::{Readable, Writable};

#[derive(Readable, Writable)]
pub struct ClassIndexConstantPool {
    string_data: Vec<u8>, //Holds Ascii Strings prefixed with their length
    //TODO: Remove packages from constant pool, don't know why they're here anyway
    indexed_packages: Vec<IndexedPackage>,
}

impl ClassIndexConstantPool {
    pub fn new(capacity: u32) -> Self {
        Self {
            string_data: Vec::with_capacity(capacity as usize),
            indexed_packages: vec![IndexedPackage::new(0, 0)],
        }
    }

    pub fn package_at(&self, index: u32) -> &IndexedPackage {
        self.indexed_packages.get(index as usize).unwrap()
    }

    pub fn package_at_mut(&mut self, index: u32) -> &mut IndexedPackage {
        self.indexed_packages.get_mut(index as usize).unwrap()
    }

    pub fn get_or_add_package_index(&mut self, name: &AsciiStr) -> u32 {
        self.get_or_add_package_index0(0, name)
    }

    /// This may be the most disgusting method I've ever written, but I suck at Rust too much to fix it
    fn get_or_add_package_index0(&mut self, indexed_package_index: u32, name: &AsciiStr) -> u32 {
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
                    .package_name(self)
                    .eq(sub_name)
            })
            .map(|p| *p.1);

        if let Some(index) = possible_index {
            if let Some(dot_index) = slash_index_or_none {
                self.get_or_add_package_index0(index, &name[dot_index + 1..])
            } else {
                index
            }
        } else {
            let name_index = self.add_string(sub_name.as_bytes()).unwrap();
            let new_index = self.indexed_packages.len();
            self.indexed_packages
                .push(IndexedPackage::new(name_index, indexed_package_index));
            self.indexed_packages
                .get_mut(indexed_package_index as usize)
                .unwrap()
                .add_sub_package(new_index as u32);

            if let Some(index) = slash_index_or_none {
                self.get_or_add_package_index0(new_index as u32, &name[index + 1..])
            } else {
                new_index as u32
            }
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
    pub fn into_ascii_string(self, constant_pool: &ClassIndexConstantPool) -> &AsciiStr {
        unsafe {
            AsciiStr::from_ascii_unchecked(
                &constant_pool.string_data[(self.index + self.start as u32) as usize
                    ..(self.index + self.end as u32) as usize],
            )
        }
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
        match_mode: MatchMode,
    ) -> bool {
        self.starts_with_at(constant_pool, other, 0, match_mode)
    }

    pub fn starts_with_at(
        &self,
        constant_pool: &ClassIndexConstantPool,
        other: &AsciiStr,
        start_index: u8,
        match_mode: MatchMode,
    ) -> bool {
        //Checks if the operation is possible
        if (start_index as usize + other.len()) > self.len() as usize {
            return false;
        }
        //Every string starts with empty string
        if other.is_empty() {
            return true;
        }

        let mut start = start_index;
        let offset = start;
        let end = start_index + other.len() as u8;
        let ignore_case = match match_mode {
            MatchMode::MatchCaseFirstCharOnly => {
                //If the first char is not the same, then it is not the same
                if self.byte_at(constant_pool, start_index) != other[0] {
                    return false;
                }

                //We don't want to check the first char again
                start += 1;
                true
            }
            MatchMode::MatchCase => false,
            MatchMode::IgnoreCase => true,
        };

        for i in start..end {
            let current_byte = self.byte_at(constant_pool, i);
            let current_char = other[(i - offset) as usize];
            if current_byte != current_char
                && (!ignore_case || current_byte != switch_ascii_char_case(current_char))
            {
                return false;
            }
        }

        true
    }

    /// Searches for the given `query` using the given `options` and returns the matched position of
    /// there is one.
    pub fn search(
        &self,
        constant_pool: &ClassIndexConstantPool,
        query: &AsciiStr,
        options: SearchOptions,
    ) -> Option<usize> {
        match options.search_mode {
            SearchMode::Prefix => {
                if self.starts_with(constant_pool, query, options.match_mode) {
                    Some(0)
                } else {
                    None
                }
            }
            SearchMode::Contains => {
                //We need to do this check because we cast the query length to a u8
                if query.len() > self.len() as usize {
                    return None;
                }

                for i in 0..=(self.len() - query.len() as u8) {
                    if self.starts_with_at(constant_pool, query, i, options.match_mode) {
                        return Some(i as usize);
                    }
                }

                None
            }
        }
    }

    pub fn len(&self) -> u8 {
        self.end - self.start
    }
}

#[derive(Clone, Copy)]
pub enum SearchMode {
    Prefix,
    Contains,
}

#[derive(Clone, Copy)]
pub enum MatchMode {
    IgnoreCase,
    MatchCase,
    MatchCaseFirstCharOnly,
}

#[derive(Clone, Copy)]
pub struct SearchOptions {
    pub limit: usize,
    pub search_mode: SearchMode,
    pub match_mode: MatchMode,
}

impl Default for SearchOptions {
    fn default() -> Self {
        SearchOptions {
            limit: usize::MAX,
            search_mode: SearchMode::Prefix,
            match_mode: MatchMode::IgnoreCase,
        }
    }
}

fn switch_ascii_char_case(char: AsciiChar) -> AsciiChar {
    if char.is_uppercase() {
        char.to_ascii_lowercase()
    } else {
        char.to_ascii_uppercase()
    }
}
