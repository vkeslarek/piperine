//! `Panic` and `Attempt`: how a value-level panic is raised and how a path taken under unknown
//! control survives one (MSL §1 rule 11, §1 rule 13, §3.3.3).
//!
//! A panic under known control is a run fault: the thunk's `catch_unwind` turns it into a fault
//! naming the message. A panic on a path taken only because a condition is unknown is no fault:
//! the path may not have run, so `Attempt` catches it and the merge makes every effect of that
//! path unknown (§1 rule 13).
//!
//! `Decision` and `Flagged<bool>::choose` are the merge of pure code: an `if` whose condition
//! may be unknown (§3.3.3).
//!
//! `Trips` and `Depth` are the bounds of Q5 (b): a loop that runs more than 2^24 iterations in one
//! evaluation, or a call depth over 1 024, is a panic of the model's own values (§1 rule 11), so a
//! model's bug fails the analysis and never hangs or aborts the process.

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use super::flag::{Flagged, Merge};

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

/// What a condition lets control do (§3.3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// The condition is known: control takes exactly the arm it names.
    Known(bool),
    /// The condition is unknown: control takes both arms and merges their effects.
    Unknown,
}

impl Decision {
    /// The decision a condition gives.
    pub fn of(condition: Flagged<bool>) -> Decision {
        if condition.is_known() { Decision::Known(condition.known_or(false)) } else { Decision::Unknown }
    }
}

impl Flagged<bool> {
    /// `if self { then } else { otherwise }` in code that commits nothing: the arm a known
    /// condition names; for an unknown one both arms, each under an `Attempt`, and the merge of
    /// their values (§3.3.3). An arm that panicked makes the merge `Unknown` (§1 rule 13), where
    /// under a known condition its panic is the run's fault.
    pub fn choose<T: Merge>(self, then: impl FnOnce() -> T, otherwise: impl FnOnce() -> T) -> T {
        match Decision::of(self) {
            Decision::Known(true) => then(),
            Decision::Known(false) => otherwise(),
            Decision::Unknown => match (Attempt::of(then), Attempt::of(otherwise)) {
                (Some(a), Some(b)) => a.merge(b),
                _ => T::unknown(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{Attempt, Decision, Depth, Panic, Trips};
    use crate::rt::flag::Flagged;
    use crate::rt::int::UInt;

    type B = Flagged<bool>;
    type U8 = Flagged<UInt<8>>;

    fn k<T: Copy + Default>(v: T) -> Flagged<T> {
        Flagged::known(v)
    }

    fn u<T: Copy + Default>() -> Flagged<T> {
        Flagged::unknown()
    }

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

    #[test]
    fn choose_runs_one_arm_or_merges_both() {
        let (then_ran, otherwise_ran) = (Cell::new(0), Cell::new(0));
        let run = |c: B, a: U8, b: U8| {
            then_ran.set(0);
            otherwise_ran.set(0);
            let r = c.choose(
                || {
                    then_ran.set(then_ran.get() + 1);
                    a
                },
                || {
                    otherwise_ran.set(otherwise_ran.get() + 1);
                    b
                },
            );
            (r, then_ran.get(), otherwise_ran.get())
        };
        let (five, six) = (k(UInt::<8>::new(5)), k(UInt::<8>::new(6)));
        // §3.3.3: a known condition takes exactly one arm
        assert_eq!(run(k(true), five, six), (five, 1, 0));
        assert_eq!(run(k(false), five, six), (six, 0, 1));
        // an unknown one takes both, and the value is the merge: equal known values are kept,
        // anything else is unknown (US-04 AC 4, AC 10)
        assert_eq!(run(u(), five, five), (five, 1, 1));
        assert_eq!(run(u(), five, six), (u(), 1, 1));
        assert_eq!(run(u(), five, u()), (u(), 1, 1));
        // a statement-like arm yields nothing to merge, and still runs on both paths
        let ran = Cell::new(0);
        u::<bool>().choose(|| ran.set(ran.get() + 1), || ran.set(ran.get() + 10));
        assert_eq!(ran.get(), 11);
        assert_eq!(Decision::of(k(false)), Decision::Known(false));
        assert_eq!(Decision::of(k(true)), Decision::Known(true));
        assert_eq!(Decision::of(u()), Decision::Unknown);
    }

    #[test]
    fn a_panicking_arm_makes_the_choice_unknown() {
        // §1 rule 13: under an unknown condition a path that panics is no fault, and every
        // effect of the merge is unknown, whichever arm it was and even if the other agrees
        let five = k(UInt::<8>::new(5));
        let fails = || -> U8 { Panic::raise("division by zero".to_string()) };
        assert_eq!(u::<bool>().choose(fails, || five), u());
        assert_eq!(u::<bool>().choose(|| five, fails), u());
        assert_eq!(u::<bool>().choose(fails, fails), u());
        // under a known condition the arm that panics is the run's fault, and the arm not taken
        // is not run at all
        assert_eq!(k(false).choose(fails, || five), five);
        assert!(std::panic::catch_unwind(|| k(true).choose(fails, || five)).is_err());
    }

    /// The text a caught panic carries.
    fn message(payload: Box<dyn std::any::Any + Send>) -> String {
        payload.downcast_ref::<String>().cloned().expect("a raised panic carries its text")
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
        let text = message(payload);
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
        let text = message(payload);
        assert!(text.contains("fact") && text.contains("1024"), "{text}");
    }
}
