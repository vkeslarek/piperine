//! `Panic`, `Trips`, `Depth` and `Index`: how a value-level panic is raised and the bounds of a
//! model's own values (MSL §1 rule 11).
//!
//! A panic is a run fault: the thunk's `catch_unwind` turns it into a fault naming the message.
//!
//! `Trips` and `Depth` are the bounds of Q5 (b): a loop that runs more than 2^24 iterations in one
//! evaluation, or a call depth over 1 024, is a panic of the model's own values (§1 rule 11), so a
//! model's bug fails the analysis and never hangs or aborts the process.

use std::panic::resume_unwind;

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

/// The iteration count of one loop in one evaluation, bounded by Q5 (b).
pub struct Trips {
    /// Where the loop was written, named by the panic.
    origin: &'static str,
    /// The iterations counted so far.
    count: u64,
}

impl Trips {
    /// The most iterations one loop may run in one evaluation: 2^24 (Q5 (b), §1 rule 11).
    pub const LIMIT: u64 = 1 << 24;

    /// A count of zero for the loop written at `origin`.
    pub fn new(origin: &'static str) -> Trips {
        Trips { origin, count: 0 }
    }

    /// Counts one iteration; the iteration past [`Trips::LIMIT`] is the panic naming the loop.
    pub fn tick(&mut self) {
        self.count += 1;
        if self.count > Trips::LIMIT {
            Panic::raise(format!("{}: the loop ran more than {} iterations in one evaluation", self.origin, Trips::LIMIT));
        }
    }
}

/// The call depth of a recursive function, carried as its trailing argument (Q5 (b)).
#[derive(Clone, Copy)]
pub struct Depth(u32);

impl Depth {
    /// The deepest a chain of recursive calls may go: 1 024 (Q5 (b), §1 rule 11).
    pub const LIMIT: u32 = 1024;

    /// The depth of a call made from outside the recursion.
    pub fn root() -> Depth {
        Depth(0)
    }

    /// The depth of a call made from this one, to `function`; past [`Depth::LIMIT`] it is the
    /// panic naming the function.
    pub fn deeper(self, function: &str) -> Depth {
        if self.0 >= Depth::LIMIT {
            Panic::raise(format!("{function}: the recursion went deeper than {} calls", Depth::LIMIT));
        }
        Depth(self.0 + 1)
    }
}

/// Indices checked against a length (§1.5.25): outside the length, a panic naming the index and
/// where the access was written.
pub struct Index;

impl Index {
    /// The position `index` names in `length` elements; outside them the panic naming the index and `origin`.
    pub fn of(index: i128, length: usize, origin: &str) -> usize {
        if index < 0 || index as u128 >= length as u128 {
            Panic::raise(format!("{origin}: the index {index} is out of range for a length of {length}"));
        }
        index as usize
    }

    /// `elements[index]`.
    pub fn at<T: Copy>(elements: &[T], index: i128, origin: &str) -> T {
        elements[Index::of(index, elements.len(), origin)]
    }
}

#[cfg(test)]
mod tests {
    use super::{Depth, Index, Panic, Trips};

    #[test]
    fn an_index_out_of_range_panics_naming_it() {
        // §1.5.25: an index at or past the length or a negative one is a panic naming the index and
        // where it was written
        let elements = [10u8, 20, 30];
        assert_eq!(Index::at(&elements, 2, "a.msl:3:5"), 30);
        let message = |body: fn() -> u8| {
            let payload = std::panic::catch_unwind(body).expect_err("the access panics");
            payload.downcast_ref::<String>().cloned().expect("a text payload")
        };
        assert_eq!(message(|| Index::at(&[10u8, 20, 30], 3, "a.msl:3:5")), "a.msl:3:5: the index 3 is out of range for a length of 3");
        assert_eq!(message(|| Index::at(&[10u8, 20, 30], -1, "a.msl:4:5")), "a.msl:4:5: the index -1 is out of range for a length of 3");
    }

    #[test]
    fn a_raised_panic_reaches_the_thunk_as_the_text_a_panic_has() {
        let payload = std::panic::catch_unwind(|| Panic::raise("level 9 of a u8".to_string())).expect_err("raise unwinds");
        assert_eq!(payload.downcast_ref::<String>().map(String::as_str), Some("level 9 of a u8"));
    }

    /// Associated test helpers of `control`, grouped on one fixture type.
    struct ControlFixture;

    impl ControlFixture {
        /// The text a caught panic carries.
        fn message(payload: Box<dyn std::any::Any + Send>) -> String {
            payload.downcast_ref::<String>().cloned().expect("a raised panic carries its text")
        }
    }

    #[test]
    fn trips_panic_past_two_to_the_twenty_four() {
        // Q5 (b): 2^24 iterations of one loop in one evaluation are allowed, the next is a panic
        // naming the loop
        let mut trips = Trips::new("loop @ spin.msl:3:5");
        for _ in 0..Trips::LIMIT {
            trips.tick();
        }
        let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| trips.tick())).expect_err("iteration 2^24 + 1 panics");
        let text = ControlFixture::message(payload);
        assert!(text.contains("loop @ spin.msl:3:5") && text.contains("16777216"), "{text}");
        assert_eq!(Trips::LIMIT, 16_777_216);
    }

    #[test]
    fn depth_panics_past_one_thousand_twenty_four() {
        // Q5 (b): a chain of 1 024 recursive calls is allowed, the 1 025th is a panic naming the
        // function
        let mut depth = Depth::root();
        for _ in 0..Depth::LIMIT {
            depth = depth.deeper("fact");
        }
        let payload = std::panic::catch_unwind(|| depth.deeper("fact")).err().expect("call 1025 panics");
        let text = ControlFixture::message(payload);
        assert!(text.contains("fact") && text.contains("1024"), "{text}");
    }
}
