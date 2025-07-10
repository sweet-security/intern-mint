pub mod borrow;
#[cfg(feature = "bstr")]
pub mod bstr;
pub mod interned;
mod pool;
#[cfg(feature = "serde")]
pub mod serde;
#[cfg(test)]
mod tests;
