//! String extensions.

use crate::alloc::AllocError;
use std::borrow::Cow;

/// A trait for converting a value to a `String`.
pub trait TryToString {
    /// Converts the given value to a `String`.
    fn try_to_string(&self) -> Result<String, AllocError>;
}

impl TryToString for str {
    #[inline]
    fn try_to_string(&self) -> Result<String, AllocError> {
        let mut s = String::new();
        s.try_reserve_exact(self.len())?;
        s.push_str(self);
        Ok(s)
    }
}

impl TryToString for Cow<'_, str> {
    #[inline]
    fn try_to_string(&self) -> Result<String, AllocError> {
        self.as_ref().try_to_string()
    }
}

impl TryToString for String {
    #[inline]
    fn try_to_string(&self) -> Result<String, AllocError> {
        self.as_str().try_to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_to_string() {
        assert_eq!("abc".try_to_string().unwrap(), "abc");
    }
}
