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

    #[cfg(feature = "global-allocator")]
    allocator::ThreadPanic::set_panic();
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

    #[cfg(feature = "global-allocator")]
    allocator::ThreadPanic::try_reserve_mem()?;

    let result = std::panic::catch_unwind(f);
    match result {
        Ok(r) => Ok(r),
        Err(e) => {
            #[cfg(feature = "global-allocator")]
            allocator::ThreadPanic::unset_panic();

            match e.downcast_ref::<AllocError>() {
                None => unreachable!(),
                Some(e) => Err(*e),
            }
        }
    }
}

#[cfg(feature = "global-allocator")]
mod allocator {
    use crate::AllocError;
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::{Cell, RefCell};
    use std::ptr::NonNull;

    #[global_allocator]
    static GLOBAL: Alloc = Alloc;

    struct Alloc;

    unsafe impl GlobalAlloc for Alloc {
        #[inline]
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ptr = System.alloc(layout);

            if ptr.is_null() && ThreadPanic::is_in_panic() {
                if let Some(p) = ThreadPanic::take_mem(layout) {
                    return p.as_ptr();
                }
            }

            ptr
        }

        #[inline]
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            System.dealloc(ptr, layout)
        }
    }

    struct PanicMem {
        // See core::panic::BoxMeUp
        box_me_up: Option<NonNull<u8>>,

        // Panic handler doesn't alloc memory for Exception in Windows.
        #[cfg(not(target_os = "windows"))]
        exception: Option<NonNull<u8>>,
    }

    impl PanicMem {
        const BOX_ME_UP_LAYOUT: Layout = unsafe { Layout::from_size_align_unchecked(16, 8) };

        #[cfg(not(target_os = "windows"))]
        const EXCEPTION_LAYOUT: Layout = unsafe { Layout::from_size_align_unchecked(80, 8) };

        #[inline]
        const fn new() -> Self {
            PanicMem {
                box_me_up: None,

                #[cfg(not(target_os = "windows"))]
                exception: None,
            }
        }

        #[inline]
        fn try_reserve(&mut self) -> Result<(), AllocError> {
            if self.box_me_up.is_none() {
                let ptr = unsafe { System.alloc(PanicMem::BOX_ME_UP_LAYOUT) };
                if ptr.is_null() {
                    return Err(AllocError::new(PanicMem::BOX_ME_UP_LAYOUT));
                } else {
                    self.box_me_up = unsafe { Some(NonNull::new_unchecked(ptr)) };
                }
            }

            #[cfg(not(target_os = "windows"))]
            if self.exception.is_none() {
                let ptr = unsafe { System.alloc(PanicMem::EXCEPTION_LAYOUT) };
                if ptr.is_null() {
                    return Err(AllocError::new(PanicMem::EXCEPTION_LAYOUT));
                } else {
                    self.exception = unsafe { Some(NonNull::new_unchecked(ptr)) };
                }
            }

            Ok(())
        }

        #[inline]
        fn take_mem(&mut self, layout: Layout) -> Option<NonNull<u8>> {
            if layout == PanicMem::BOX_ME_UP_LAYOUT {
                return self.box_me_up.take();
            }

            #[cfg(not(target_os = "windows"))]
            if layout == PanicMem::EXCEPTION_LAYOUT {
                return self.exception.take();
            }

            None
        }
    }

    impl Drop for PanicMem {
        #[inline]
        fn drop(&mut self) {
            if let Some(mut ptr) = self.box_me_up.take() {
                unsafe { System.dealloc(ptr.as_mut(), PanicMem::BOX_ME_UP_LAYOUT) };
            }

            #[cfg(not(target_os = "windows"))]
            if let Some(mut ptr) = self.exception.take() {
                unsafe { System.dealloc(ptr.as_mut(), PanicMem::EXCEPTION_LAYOUT) };
            }
        }
    }

    thread_local! {
        static THREAD_PANIC_MEM: RefCell<PanicMem> = RefCell::new(PanicMem::new());
        static THREAD_IN_PANIC: Cell<bool> = Cell::new(false);
    }

    pub struct ThreadPanic;

    impl ThreadPanic {
        #[inline]
        pub fn try_reserve_mem() -> Result<(), AllocError> {
            THREAD_PANIC_MEM.with(|panic_mem| panic_mem.borrow_mut().try_reserve())
        }

        #[inline]
        pub fn take_mem(layout: Layout) -> Option<NonNull<u8>> {
            THREAD_PANIC_MEM.with(|panic_mem| panic_mem.borrow_mut().take_mem(layout))
        }

        #[inline]
        pub fn set_panic() {
            THREAD_IN_PANIC.with(|in_panic| in_panic.set(true))
        }

        #[inline]
        pub fn unset_panic() {
            THREAD_IN_PANIC.with(|in_panic| in_panic.set(false))
        }

        #[inline]
        pub fn is_in_panic() -> bool {
            THREAD_IN_PANIC.with(|in_panic| in_panic.get())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::catch_alloc_error;
    use std::alloc::{AllocError as StdAllocError, Allocator, Layout};
    use std::ptr::NonNull;

    struct NoMem;

    unsafe impl Allocator for NoMem {
        #[inline]
        fn allocate(&self, _layout: Layout) -> Result<NonNull<[u8]>, StdAllocError> {
            Err(StdAllocError)
        }

        #[inline]
        unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
            unreachable!()
        }
    }

    #[test]
    fn test_catch_alloc_error() {
        let result = catch_alloc_error(|| Vec::<u8, _>::with_capacity_in(10, NoMem));
        assert_eq!(
            result.unwrap_err().layout(),
            Layout::from_size_align(10, 1).unwrap()
        );
    }
}
