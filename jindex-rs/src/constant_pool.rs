use anyhow::{anyhow, Result};
use ascii::AsciiStr;
use std::cmp::min;

pub struct ClassIndexConstantPool {
    string_data: Vec<u8>,  //Holds Ascii Strings prefixed with their length
    method_data: Vec<u32>, //Holds string_data indexes for method names
}

impl ClassIndexConstantPool {
    pub fn new(capacity: u32) -> Self {
        Self {
            string_data: Vec::with_capacity(capacity as usize),
            method_data: Vec::new(),
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

    pub fn add_methods(&mut self, method_indexes: &[u32]) -> u32 {
        let index = self.method_data.len();
        self.method_data.extend(method_indexes.iter());
        index as u32
    }

    pub fn get_methods_at(&self, index: u32, length: u16) -> &[u32] {
        &self.method_data[index as usize..(index + length as u32) as usize]
    }
}

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

impl Eq for ConstantPoolStringView {}

impl ConstantPoolStringView {
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

    pub fn starts_with(&self, constant_pool: &ClassIndexConstantPool, other: &AsciiStr) -> bool {
        for i in 0..min(self.end - self.start, other.len() as u8) {
            if self.byte_at(constant_pool, i) != other[i as usize] {
                return false;
            }
        }

        true
    }
}
