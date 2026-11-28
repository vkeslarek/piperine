//! The text a panic carries, read back from the work that raised it.

/// The message of a panic, for the tests that hold a failure to name its cause.
pub struct PanicMessage;

impl PanicMessage {
    /// The text `work` panics with; `work` must panic.
    pub fn of(work: impl FnOnce() + std::panic::UnwindSafe) -> String {
        let payload = std::panic::catch_unwind(work).expect_err("the work must panic");
        match payload.downcast::<String>() {
            Ok(text) => *text,
            Err(other) => other.downcast::<&str>().map(|text| text.to_string()).expect("a text panic"),
        }
    }
}
