//! A counting allocator for the tests that bound a run's memory (feature `long-run`, LR-04,
//! LR-12). The target that uses it installs one `#[global_allocator]` static of `Counting` and
//! calls `begin` before the run it measures and `peak_since` after it.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

/// `System` that counts the bytes live and the most that were live since `begin`.
pub struct Counting {
    live: AtomicUsize,
    peak: AtomicUsize,
}

impl Counting {
    pub const fn new() -> Counting {
        Counting { live: AtomicUsize::new(0), peak: AtomicUsize::new(0) }
    }

    /// Starts a measurement: the peak restarts from what is live now, which is returned.
    pub fn begin(&self) -> usize {
        let live = self.live.load(Ordering::SeqCst);
        self.peak.store(live, Ordering::SeqCst);
        live
    }

    /// The most live bytes since `begin`, less the `base` that `begin` returned.
    pub fn peak_since(&self, base: usize) -> usize {
        self.peak.load(Ordering::SeqCst).saturating_sub(base)
    }

    fn grew(&self, by: usize) {
        let now = self.live.fetch_add(by, Ordering::SeqCst) + by;
        self.peak.fetch_max(now, Ordering::SeqCst);
    }
}

// SAFETY: every call forwards to `System`; the counters only observe the sizes.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            self.grew(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            self.grew(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        self.live.fetch_sub(layout.size(), Ordering::SeqCst);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            if new_size >= layout.size() {
                self.grew(new_size - layout.size());
            } else {
                self.live.fetch_sub(layout.size() - new_size, Ordering::SeqCst);
            }
        }
        new
    }
}
