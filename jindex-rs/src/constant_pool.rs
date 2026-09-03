use anyhow::{anyhow, Result};
use ascii::{AsciiChar, AsciiStr};
use speedy::{Readable, Writable};

#[derive(Readable, Writable)]
pub struct ClassIndexConstantPool {
    string_data: Vec<u8>, //Holds Ascii Strings prefixed with their length
}

impl ClassIndexConstantPool {
    pub(crate) fn new(capacity: u32) -> Self {
        Self {
            string_data: Vec::with_capacity(capacity as usize),
        }
    }

    pub(crate) fn add_string(&mut self, str: &[u8]) -> Result<u32> {
        let index = self.string_data.len();
        let length = str.len();
        if length > u8::MAX as usize {
            return Err(anyhow!(
                "The string {} exceeds the maximum size of {}",
                String::from_utf8_lossy(str),
                u8::MAX
            ));
        }

        AsciiStr::from_ascii(str)?;
        self.string_data.try_reserve(1 + str.len())?;
        self.string_data.push(length as u8);
        self.string_data.extend_from_slice(str);

        Ok(index as u32)
    }

    pub fn string_view_at(&self, index: u32) -> ConstantPoolStringView {
        ConstantPoolStringView {
            index,
            len: *self.string_data.get(index as usize).unwrap(),
        }
    }
}

#[derive(Debug, Eq)]
pub struct ConstantPoolStringView {
    index: u32,
    len: u8,
}

impl PartialEq for ConstantPoolStringView {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.len == other.len
    }
}

impl ConstantPoolStringView {
    pub fn into_ascii_str(self, constant_pool: &ClassIndexConstantPool) -> &AsciiStr {
        self.as_ascii_str(constant_pool)
    }

    pub fn as_ascii_str<'a>(&self, constant_pool: &'a ClassIndexConstantPool) -> &'a AsciiStr {
        unsafe {
            AsciiStr::from_ascii_unchecked(
                &constant_pool.string_data[(self.index + 1) as usize..][..self.len as usize],
            )
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn byte_at(&self, constant_pool: &ClassIndexConstantPool, index: u8) -> u8 {
        *constant_pool
            .string_data
            .get(self.index as usize + 1 + index as usize)
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

    /// Searches for the given `query` using the given `options` and returns its first matching
    /// position.
    pub fn search(
        &self,
        constant_pool: &ClassIndexConstantPool,
        query: &AsciiStr,
        options: SearchOptions,
    ) -> Option<usize> {
        search_ascii(self.as_ascii_str(constant_pool), query, options)
    }

    pub fn len(&self) -> u8 {
        self.len
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

pub(crate) fn search_ascii(
    value: &AsciiStr,
    query: &AsciiStr,
    options: SearchOptions,
) -> Option<usize> {
    if query.len() > value.len() {
        return None;
    }
    let last_start = match options.search_mode {
        SearchMode::Prefix => 0,
        SearchMode::Contains => value.len().checked_sub(query.len())?,
    };

    (0..=last_start).find(|&start| {
        query.chars().enumerate().all(|(offset, expected)| {
            let actual = value[start + offset];
            match options.match_mode {
                MatchMode::MatchCase => actual == expected,
                MatchMode::IgnoreCase => {
                    actual == expected || actual == switch_ascii_char_case(expected)
                }
                MatchMode::MatchCaseFirstCharOnly if offset == 0 => actual == expected,
                MatchMode::MatchCaseFirstCharOnly => {
                    actual == expected || actual == switch_ascii_char_case(expected)
                }
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{search_ascii, ClassIndexConstantPool, MatchMode, SearchMode, SearchOptions};
    use ascii::AsAsciiStr;

    #[test]
    fn audit_longer_queries_do_not_match() {
        for search_mode in [SearchMode::Prefix, SearchMode::Contains] {
            for match_mode in [
                MatchMode::IgnoreCase,
                MatchMode::MatchCase,
                MatchMode::MatchCaseFirstCharOnly,
            ] {
                assert_eq!(
                    None,
                    search_ascii(
                        "Object".as_ascii_str().unwrap(),
                        "Objects".as_ascii_str().unwrap(),
                        SearchOptions {
                            search_mode,
                            match_mode,
                            limit: 1
                        }
                    )
                );
            }
        }
    }

    #[test]
    fn audit_pool_views_preserve_maximum_length_and_empty_values() {
        let mut pool = ClassIndexConstantPool::new(0);
        pool.add_string(b"").unwrap();
        let one = pool.add_string(b"x").unwrap();
        let empty = pool.add_string(b"").unwrap();
        let maximum = pool.add_string(&[b'x'; 255]).unwrap();
        assert!(!pool.string_view_at(one).is_empty());
        assert!(pool.string_view_at(empty).is_empty());
        let view = pool.string_view_at(maximum);
        assert_eq!(255, view.len());
        assert_eq!(255, view.as_ascii_str(&pool).len());
    }

    #[test]
    fn audit_oversized_unicode_error_does_not_panic() {
        assert!(ClassIndexConstantPool::new(0)
            .add_string("Ä".repeat(256).as_bytes())
            .is_err());
    }

    #[test]
    fn searches_ascii_with_every_match_mode() {
        let value = "java/lang/String".as_ascii_str().unwrap();

        assert_eq!(
            Some(10),
            search_ascii(
                value,
                "string".as_ascii_str().unwrap(),
                SearchOptions {
                    limit: 1,
                    search_mode: SearchMode::Contains,
                    match_mode: MatchMode::IgnoreCase,
                },
            )
        );
        assert_eq!(
            None,
            search_ascii(
                value,
                "string".as_ascii_str().unwrap(),
                SearchOptions {
                    limit: 1,
                    search_mode: SearchMode::Contains,
                    match_mode: MatchMode::MatchCase,
                },
            )
        );
        assert_eq!(
            Some(10),
            search_ascii(
                value,
                "String".as_ascii_str().unwrap(),
                SearchOptions {
                    limit: 1,
                    search_mode: SearchMode::Contains,
                    match_mode: MatchMode::MatchCaseFirstCharOnly,
                },
            )
        );
        assert_eq!(
            None,
            search_ascii(
                value,
                "string".as_ascii_str().unwrap(),
                SearchOptions {
                    limit: 1,
                    search_mode: SearchMode::Contains,
                    match_mode: MatchMode::MatchCaseFirstCharOnly,
                },
            )
        );
    }

    #[test]
    fn distinguishes_prefix_from_contains() {
        let value = "java/lang/String".as_ascii_str().unwrap();
        let query = "lang".as_ascii_str().unwrap();

        assert_eq!(
            None,
            search_ascii(
                value,
                query,
                SearchOptions {
                    limit: 1,
                    search_mode: SearchMode::Prefix,
                    match_mode: MatchMode::IgnoreCase,
                },
            )
        );
        assert_eq!(
            Some(5),
            search_ascii(
                value,
                query,
                SearchOptions {
                    limit: 1,
                    search_mode: SearchMode::Contains,
                    match_mode: MatchMode::IgnoreCase,
                },
            )
        );
    }
}
