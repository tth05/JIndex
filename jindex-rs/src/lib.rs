#![recursion_limit = "40"]
#![feature(once_cell)]
#![feature(try_blocks)]

use ascii::{AsAsciiStr, AsciiChar, AsciiStr};
use mimalloc::MiMalloc;

pub mod builder;
pub mod class_index;
pub mod class_index_members;
pub mod constant_pool;
pub mod io;
pub mod package_index;
pub mod signature;

pub mod jni;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub(crate) fn rsplit_once(str: &AsciiStr, separator: AsciiChar) -> (&AsciiStr, &AsciiStr) {
    str.chars()
        .enumerate()
        .rev()
        .find(|(_, c)| *c == separator)
        .map(|(i, _)| (&str[0..i], &str[(i + 1)..]))
        .unwrap_or_else(|| (unsafe { "".as_ascii_str_unchecked() }, str))
}

#[cfg(test)]
mod test {
}
