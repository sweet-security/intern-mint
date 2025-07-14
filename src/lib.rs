#![doc = include_str!("../README.md")]

pub mod borrow;
#[cfg(feature = "bstr")]
pub mod bstr;
#[cfg(feature = "databuf")]
pub mod databuf;
pub mod interned;
pub mod pool;
#[cfg(feature = "serde")]
pub mod serde;
#[cfg(test)]
mod tests;
