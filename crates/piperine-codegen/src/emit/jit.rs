//! Ownership of a finalized `JITModule`'s code pages.
//!
//! `cranelift_jit` deliberately leaks on drop — its `Memory::drop` reads
//! *"leak memory to guarantee validity of function pointers"* and `mem::forget`s
//! every allocation, because it cannot know whether a raw `fn` pointer taken out
//! of the module is still live. Returning the pages is the caller's call, behind
//! the unsafe `JITModule::free_memory`.
//!
//! Piperine can make that call: every compiled kernel stores its function
//! pointers in the *same value* as its `JITModule`, and a device instance holds
//! the kernel behind an `Arc`. So the moment the kernel drops is the moment no
//! pointer into its code can be called again — and the pages are free to go.
//! Without this, a process that compiles many modules (a parameter sweep that
//! re-elaborates, a test binary running dozens of circuits) grows for the
//! lifetime of the process.

use cranelift_jit::JITModule;

/// A finalized [`JITModule`] whose code pages are returned when it drops.
///
/// Hold this instead of a bare `JITModule` **beside** the function pointers it
/// backs, never apart from them: the safety argument is precisely that they
/// share a lifetime.
pub struct OwnedJit(Option<JITModule>);

impl From<JITModule> for OwnedJit {
    fn from(jit: JITModule) -> Self {
        Self(Some(jit))
    }
}

impl Drop for OwnedJit {
    fn drop(&mut self) {
        if let Some(jit) = self.0.take() {
            // SAFETY: `free_memory` requires that no function from this module
            // is executing and none of its `fn` pointers are called afterwards.
            // Both hold: the pointers live in the struct that owns this value,
            // so they are being dropped in the same breath, and a kernel is
            // shared with its instances through an `Arc` — this runs only after
            // the last one is gone.
            unsafe { jit.free_memory() };
        }
    }
}
