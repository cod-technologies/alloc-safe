//! Safe memory allocation, no panic!

#![feature(allocator_api)]
#![feature(alloc_error_hook)]
#![feature(try_reserve_kind)]
#![feature(fmt_internals)]

mod sealed {
    pub trait Sealed {}
}

mod alloc;
mod fmt;
mod string;
mod vec;

pub use crate::alloc::{allocator::Alloc, catch_alloc_error, AllocError};
pub use crate::fmt::try_format;
pub use crate::string::TryToString;
pub use crate::vec::{VecAllocExt, VecExt};
