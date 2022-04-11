//! Memory allocation error.

use std::alloc::Layout;
use std::collections::TryReserveError;
use std::error::Error;
use std::fmt;
use std::panic::{PanicInfo, UnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};

/// The error type for allocation failure.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct AllocError(Layout);

impl AllocError {
    /// Creates a new `AllocError`.
    #[must_use]
    #[inline]
    pub const fn new(layout: Layout) -> Self {
        AllocError(layout)
    }

    /// Returns the memory layout of the `AllocError`.
    #[must_use]
    #[inline]
    pub const fn layout(self) -> Layout {
        self.0
    }
}

impl fmt::Debug for AllocError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AllocError")
            .field("size", &self.0.size())
            .field("align", &self.0.align())
            .finish()
    }
}

impl fmt::Display for AllocError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to allocate memory by required layout {{size: {}, align: {}}}",
            self.0.size(),
            self.0.align()
        )
    }
}

impl Error for AllocError {}

impl From<TryReserveError> for AllocError {
    #[inline]
    fn from(e: TryReserveError) -> Self {
        use std::collections::TryReserveErrorKind;
        match e.kind() {
            TryReserveErrorKind::AllocError { layout, .. } => AllocError::new(layout),
            TryReserveErrorKind::CapacityOverflow => {
                unreachable!("unexpected capacity overflow")
            }
        }
    }
}

fn alloc_error_hook(layout: Layout) {
    std::panic::panic_any(AllocError(layout))
}

type PanicHook = Box<dyn Fn(&PanicInfo<'_>) + 'static + Sync + Send>;

fn panic_hook(panic_info: &PanicInfo<'_>) {
    // panic abort except alloc error
    if !panic_info.payload().is::<AllocError>() {
        std::process::abort();
    }
}

/// Invokes a closure, capturing the panic of memory allocation error if one occurs.
///
/// This function will return `Ok` with the closure's result if the closure
/// does not panic, and will return `AllocError` if allocation error occurs. The
/// process will abort if other panics occur.
///
/// Notes that this function will set panic hook and alloc error hook.
#[inline]
pub fn catch_alloc_error<F: FnOnce() -> R + UnwindSafe, R>(f: F) -> Result<R, AllocError> {
    static SET_HOOK: AtomicBool = AtomicBool::new(false);
    if !SET_HOOK.load(Ordering::Acquire) {
        let hook: PanicHook =
            Box::try_new(panic_hook).map_err(|_| AllocError::new(Layout::new::<PanicHook>()))?;
        std::panic::set_hook(hook);
        std::alloc::set_alloc_error_hook(alloc_error_hook);
        SET_HOOK.store(true, Ordering::Release);
    }

    let result = std::panic::catch_unwind(f);
    match result {
        Ok(r) => Ok(r),
        Err(e) => match e.downcast_ref::<AllocError>() {
            None => {
                unreachable!()
            }
            Some(e) => Err(*e),
        },
    }
}
