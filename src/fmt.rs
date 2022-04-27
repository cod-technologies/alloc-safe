//! Utilities for formatting and printing strings.

use std::fmt::{self, Write};

#[repr(transparent)]
struct StrBuf(String);

impl Write for StrBuf {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.try_reserve(s.len()).map_err(|_| fmt::Error)?;
        self.0.push_str(s);
        Ok(())
    }
}

/// The `try_format` function takes an `Arguments` struct and returns the resulting
/// formatted string.
#[inline]
pub fn try_format(args: fmt::Arguments<'_>) -> Result<String, fmt::Error> {
    let capacity = args.estimated_capacity();
    let mut output = String::new();
    output.try_reserve(capacity).map_err(|_| fmt::Error)?;

    let mut buf = StrBuf(output);
    buf.write_fmt(args)?;
    Ok(buf.0)
}

/// Creates a `String` using interpolation of runtime expressions.
///
/// The first argument `try_format!` receives is a format string. This must be a string
/// literal. The power of the formatting string is in the `{}`s contained.
///
/// Additional parameters passed to `try_format!` replace the `{}`s within the
/// formatting string in the order given unless named or positional parameters
/// are used; see [`std::fmt`] for more information.
///
/// A common use for `try_format!` is concatenation and interpolation of strings.
#[macro_export]
macro_rules! try_format {
    ($($arg:tt)*) => {{
        let res = $crate::try_format(format_args!($($arg)*));
        res
    }}
}
