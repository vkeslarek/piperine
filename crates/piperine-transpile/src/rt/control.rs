//! `Panic` and `Attempt`: how a value-level panic is raised and how a path taken under unknown
//! control survives one (MSL §1 rule 11, §1 rule 13, §3.3.3).
//!
//! A panic under known control is a run fault: the thunk's `catch_unwind` turns it into a fault
//! naming the message. A panic on a path taken only because a condition is unknown is no fault:
//! the path may not have run, so `Attempt` catches it and the merge makes every effect of that
//! path unknown (§1 rule 13).

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

/// A panic of the model's own values: a division by zero, a `level` out of range, an `unwrap`
/// of `None`.
pub struct Panic;

impl Panic {
    /// Unwinds with `message`. `resume_unwind` does not call the panic hook, so a panic that an
    /// `Attempt` catches prints nothing, and one that reaches the thunk still becomes the fault
    /// `panic: <message>` (the payload is the same text a `panic!` would carry).
    pub fn raise(message: String) -> ! {
        resume_unwind(Box::new(message))
    }
}

/// The run of a path that may not have run (§3.3.3).
pub struct Attempt;

impl Attempt {
    /// The result of `body`, or `None` when it panicked (§1 rule 13: the path then contributes
    /// unknown to every effect of its merge). The closure may leave its captures half-written;
    /// the merge never reads what a panicked path wrote, hence `AssertUnwindSafe`.
    pub fn of<T>(body: impl FnOnce() -> T) -> Option<T> {
        catch_unwind(AssertUnwindSafe(body)).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::{Attempt, Panic};

    #[test]
    fn a_raised_panic_is_caught_by_an_attempt() {
        // §1 rule 13: a path under unknown control that panics is no fault, its result is
        // missing; a path that does not panic gives its value
        assert_eq!(Attempt::of(|| 7), Some(7));
        let caught: Option<u8> = Attempt::of(|| Panic::raise("division by zero".to_string()));
        assert_eq!(caught, None);
        // under known control the payload reaches the thunk as the same text a `panic!` has
        let payload = std::panic::catch_unwind(|| Panic::raise("level 9 of a u8".to_string())).expect_err("raise unwinds");
        assert_eq!(payload.downcast_ref::<String>().map(String::as_str), Some("level 9 of a u8"));
    }
}
