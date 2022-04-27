//! Safe memory allocation, no panic!

#![feature(allocator_api)]
#![feature(alloc_error_hook)]
#![feature(try_reserve_kind)]

mod sealed {
    pub trait Sealed {}
}

mod alloc;
mod vec;

pub use crate::alloc::{catch_alloc_error, AllocError};
pub use crate::vec::{VecAllocExt, VecExt};
