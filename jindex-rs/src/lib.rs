#![recursion_limit = "40"]
#![feature(once_cell)]

mod class_index;
mod constant_pool;
mod io;
mod signature;

pub mod jni;

#[cfg(test)]
mod test {
}
