use anyhow::{anyhow, Result};
use ascii::{AsciiChar, AsciiStr};
use speedy::{Context, Readable, Reader, Writable};

/// Length-prefixed ASCII strings. Every entry is validated as ASCII when a string is added and
/// when a snapshot is read. Length prefixes themselves may exceed 127, so a view still checks the
/// bytes it exposes as text; offsets stored elsewhere in a snapshot are not verified to be entry
/// starts.
#[derive(Writable)]
pub struct ClassIndexConstantPool {
    string_data: Vec<u8>, //Holds Ascii Strings prefixed with their length
}

impl<'a, C> Readable<'a, C> for ClassIndexConstantPool
where
    C: Context,
{
    fn read_from<R: Reader<'a, C>>(reader: &mut R) -> std::result::Result<Self, C::Error> {
        let string_data: Vec<u8> = reader.read_value()?;
        ClassIndexConstantPool::validate(&string_data).map_err(speedy::Error::custom)?;
        Ok(Self { string_data })
    }
}

impl ClassIndexConstantPool {
    pub(crate) fn entry_offsets(&self) -> rustc_hash::FxHashSet<u32> {
        let mut entries = rustc_hash::FxHashSet::default();
        let mut offset = 0;
        while offset < self.string_data.len() {
            entries.insert(offset as u32);
            offset += 1 + usize::from(self.string_data[offset]);
        }
        entries
    }

    fn validate(string_data: &[u8]) -> std::result::Result<(), &'static str> {
        let mut offset = 0;
        while let Some(length) = string_data.get(offset) {
            let start = offset + 1;
            let end = start + usize::from(*length);
            let Some(entry) = string_data.get(start..end) else {
                return Err("Constant pool string lengths do not match its size");
            };
            if !entry.is_ascii() {
                return Err("Constant pool contains non-ASCII data");
            }
            offset = end;
        }
        Ok(())
    }

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
        // Offsets can originate in a snapshot and select a length prefix instead of an entry, so
        // never fabricate invalid AsciiChar values. Comparisons and searches use `as_bytes`.
        AsciiStr::from_ascii(self.as_bytes(constant_pool))
            .expect("Constant pool offset does not select an ASCII string")
    }

    /// The raw entry bytes, for comparisons and searches that do not need typed characters.
    pub fn as_bytes<'a>(&self, constant_pool: &'a ClassIndexConstantPool) -> &'a [u8] {
        &constant_pool.string_data[self.index as usize + 1..][..self.len as usize]
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
        search_bytes(self.as_bytes(constant_pool), query, options)
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

/// Searches `query` in the candidate bytes and returns the first matching position. Candidate
/// bytes need no ASCII cast; only the query is typed.
pub(crate) fn search_bytes(
    value: &[u8],
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
        query
            .as_bytes()
            .iter()
            .copied()
            .enumerate()
            .all(|(offset, expected)| {
                let actual = value[start + offset];
                match options.match_mode {
                    MatchMode::MatchCase => actual == expected,
                    MatchMode::IgnoreCase => actual.eq_ignore_ascii_case(&expected),
                    MatchMode::MatchCaseFirstCharOnly if offset == 0 => actual == expected,
                    MatchMode::MatchCaseFirstCharOnly => actual.eq_ignore_ascii_case(&expected),
                }
            })
    })
}

#[cfg(test)]
mod tests {
    use super::{search_bytes, ClassIndexConstantPool, MatchMode, SearchMode, SearchOptions};
    use ascii::AsAsciiStr;
    use speedy::{Readable, Writable};

    #[test]
    fn snapshot_pools_are_validated_when_read() {
        for string_data in [vec![1, 255], vec![2, b'a'], vec![0, 1]] {
            let bytes = ClassIndexConstantPool { string_data }
                .write_to_vec()
                .unwrap();
            assert!(ClassIndexConstantPool::read_from_buffer(&bytes).is_err());
        }
        let mut string_data = vec![0, 1, b'a', 200];
        string_data.extend_from_slice(&[b'b'; 200]);
        let bytes = ClassIndexConstantPool { string_data }
            .write_to_vec()
            .unwrap();
        let pool = ClassIndexConstantPool::read_from_buffer(&bytes).unwrap();
        assert_eq!("a", pool.string_view_at(1).as_ascii_str(&pool));
        assert_eq!(200, pool.string_view_at(3).as_ascii_str(&pool).len());
    }

    #[test]
    fn stale_offsets_never_produce_invalid_ascii() {
        let mut pool = ClassIndexConstantPool::new(0);
        pool.add_string(b"x").unwrap();
        pool.add_string(&[b'a'; 200]).unwrap();
        // Offset 1 selects the byte 'x' as a length, so the view spans the next length prefix.
        let view = pool.string_view_at(1);
        assert!(std::panic::catch_unwind(|| view.as_ascii_str(&pool).len()).is_err());
    }

    #[test]
    fn longer_queries_do_not_match() {
        for search_mode in [SearchMode::Prefix, SearchMode::Contains] {
            for match_mode in [
                MatchMode::IgnoreCase,
                MatchMode::MatchCase,
                MatchMode::MatchCaseFirstCharOnly,
            ] {
                assert_eq!(
                    None,
                    search_bytes(
                        b"Object",
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
    fn pool_views_preserve_maximum_length_and_empty_values() {
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
    fn oversized_unicode_error_does_not_panic() {
        assert!(ClassIndexConstantPool::new(0)
            .add_string("Ä".repeat(256).as_bytes())
            .is_err());
    }

    #[test]
    fn searches_ascii_with_every_match_mode() {
        let value = b"java/lang/String";

        assert_eq!(
            Some(10),
            search_bytes(
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
            search_bytes(
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
            search_bytes(
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
            search_bytes(
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
        let value = b"java/lang/String";
        let query = "lang".as_ascii_str().unwrap();

        assert_eq!(
            None,
            search_bytes(
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
            search_bytes(
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
