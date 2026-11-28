//! The text of a panic a closure raises, shared by the unit tests of the runtime files.

/// The text of the panic a closure raises.
pub struct PanicText;

impl PanicText {
    /// Runs `f` and returns its panic message, `None` when it does not panic.
    pub fn panic_of(f: impl FnOnce() + std::panic::UnwindSafe) -> Option<String> {
        std::panic::catch_unwind(f).err().map(|e| {
            e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
        })
    }
}
