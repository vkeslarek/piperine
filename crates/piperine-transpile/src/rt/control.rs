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
//! `Step`, `Outcome` and `Unrolled` run a loop under unknown control as its unrolling (§3.3.3,
//! §3.3.16): each iteration answers with a `Step`; a decision that may be unknown answers with the
//! union of both of its outcomes, and the exits of every path merge into the one state the code
//! after the loop sees.
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

/// Indices checked against a length (§1.5.25): outside the length, a panic naming
/// the index and where the access was written. An index that may be unknown names any element, so a
/// read merges the elements and a write merges the value into each (§3.3.13).
pub struct Index;

impl Index {
    /// The position `index` names in `length` elements; outside them the panic naming the index and `origin`.
    pub fn of(index: i128, length: usize, origin: &str) -> usize {
        if index < 0 || index as u128 >= length as u128 {
            Panic::raise(format!("{origin}: the index {index} is out of range for a length of {length}"));
        }
        index as usize
    }

    /// `elements[index]` for a known index.
    pub fn at<T: Copy>(elements: &[T], index: i128, origin: &str) -> T {
        elements[Index::of(index, elements.len(), origin)]
    }

    /// `elements[index]` where the index may be unknown (`None`): the merge of every element it may name.
    pub fn read<T: Merge + Copy>(elements: &[T], index: Option<i128>, origin: &str) -> T {
        match index {
            Some(index) => Index::at(elements, index, origin),
            None => match elements.split_first() {
                Some((first, rest)) => rest.iter().fold(*first, |merged, element| merged.merge(*element)),
                None => Panic::raise(format!("{origin}: an index into no elements")),
            },
        }
    }

    /// `elements[index] = value` where the index may be unknown (`None`): every element it may name
    /// becomes the merge of what it held and `value`.
    pub fn write<T: Merge + Copy>(elements: &mut [T], index: Option<i128>, value: T, origin: &str) {
        match index {
            Some(index) => elements[Index::of(index, elements.len(), origin)] = value,
            None => {
                if elements.is_empty() {
                    Panic::raise(format!("{origin}: an index into no elements"));
                }
                for element in elements.iter_mut() {
                    *element = element.merge(value);
                }
            }
        }
    }
}

/// What one iteration of a loop under unknown control does, or what a part of an iteration does
/// so far (§3.3.3). `S` is the tuple of locals the loop writes, `B` the value a `break` carries and
/// `R` the value a `return` carries.
pub enum Step<S, B, R> {
    /// The part before an unknown decision ended without leaving: the rest of the iteration still
    /// runs, on this state (`Step::then`). Only inside an iteration; the iteration itself ends in
    /// `Next`.
    Fall(S),
    /// The iteration ended (or `continue`): the next one starts on this state.
    Next(S),
    /// `break`: the loop is left with this state and value.
    Exit(S, B),
    /// `return`: the function is left with this value.
    Returned(R),
    /// A decision that may be unknown: control took both outcomes.
    Either(Box<Step<S, B, R>>, Box<Step<S, B, R>>),
}

impl<S, B, R> Step<S, B, R> {
    /// This step with `rest` run on every `Fall` of it: the rest of an iteration after an unknown
    /// decision, run once per path that reached it. Any other outcome is kept as it is.
    pub fn then(self, mut rest: impl FnMut(S) -> Step<S, B, R>) -> Step<S, B, R> {
        self.each_fall(&mut rest)
    }

    /// `then`, over the tree of paths.
    fn each_fall(self, rest: &mut impl FnMut(S) -> Step<S, B, R>) -> Step<S, B, R> {
        match self {
            Step::Fall(state) => rest(state),
            Step::Either(a, b) => Step::Either(Box::new(a.each_fall(rest)), Box::new(b.each_fall(rest))),
            settled => settled,
        }
    }

    /// Every path of this step, one by one, and whether it was a union: an unknown decision.
    fn paths(self, into: &mut Vec<Step<S, B, R>>) -> bool {
        match self {
            Step::Either(a, b) => {
                a.paths(into);
                b.paths(into);
                true
            }
            path => {
                into.push(path);
                false
            }
        }
    }
}

impl<S: Merge, B: Merge, R: Merge> Merge for Step<S, B, R> {
    /// The two outcomes of a decision that may be unknown, kept apart until the loop's exits merge.
    fn merge(self, other: Self) -> Self {
        Step::Either(Box::new(self), Box::new(other))
    }

    /// A path that panicked (§1 rule 13): it may have continued, left the loop or returned, with
    /// any state.
    fn unknown() -> Self {
        Step::Either(
            Box::new(Step::Either(Box::new(Step::Fall(S::unknown())), Box::new(Step::Exit(S::unknown(), B::unknown())))),
            Box::new(Step::Returned(R::unknown())),
        )
    }
}

/// How the paths of a loop left it, each kind merged into one.
pub struct Outcome<S, B, R> {
    /// The merged state and `break` value of the paths that left the loop.
    exit: Option<(S, B)>,
    /// The merged value of the paths that returned from the function.
    returned: Option<R>,
}

impl<S: Merge, B: Merge, R: Merge> Outcome<S, B, R> {
    /// The loop left through its exit: the merged state and value, or unknown when no path
    /// exited. A loop that cannot return (`R` is `()`) never has a returned path.
    pub fn exit(self) -> (S, B) {
        self.exit.unwrap_or_else(|| (S::unknown(), B::unknown()))
    }

    /// The loop's exit, or the value the function returns: when every path returned it is that
    /// value; when only some did, the function's value is unknown, which is the merge of what it
    /// returned with a result the paths that left the loop go on to compute (§3.3.3).
    pub fn finish(self) -> Result<(S, B), R> {
        match (self.exit, self.returned) {
            (Some(exit), None) => Ok(exit),
            (None, Some(returned)) => Err(returned),
            (Some(_), Some(_)) => Err(R::unknown()),
            (None, None) => Ok((S::unknown(), B::unknown())),
        }
    }

    /// Adds a path that left the loop.
    fn leave(&mut self, state: S, value: B) {
        self.exit = Some(match self.exit.take() {
            Some((s, b)) => (s.merge(state), b.merge(value)),
            None => (state, value),
        });
    }

    /// Adds a path that returned.
    fn give(&mut self, value: R) {
        self.returned = Some(match self.returned.take() {
            Some(r) => r.merge(value),
            None => value,
        });
    }
}

/// A loop under unknown control as its unrolling: one iteration at a time, an unknown decision
/// splitting into the path that leaves and the path that goes on, the paths that leave merged as
/// they do (§3.3.3, §3.3.16).
pub struct Unrolled<S, B, R> {
    /// How the paths that ended so far left the loop.
    outcome: Outcome<S, B, R>,
    /// The state the next iteration starts on; `None` when no path goes on.
    live: Option<S>,
    /// The loop's iteration count, bounded as any loop is.
    trips: Trips,
    /// The iterations run since the first decision that was unknown.
    unknown_iterations: u32,
}

impl<S: Merge, B: Merge, R: Merge> Unrolled<S, B, R> {
    /// The most iterations a path may run after an unknown decision before it is cut: 64
    /// (Appendix C.2). A path of known decisions is bounded by `Trips` alone.
    pub const LIMIT: u32 = 64;

    /// The unrolling of the loop written at `origin`, starting on `state`.
    pub fn start(origin: &'static str, state: S) -> Unrolled<S, B, R> {
        Unrolled { outcome: Outcome { exit: None, returned: None }, live: Some(state), trips: Trips::new(origin), unknown_iterations: 0 }
    }

    /// Whether a decision so far was unknown: the iterations that follow are paths that may not
    /// run.
    pub fn is_unknown(&self) -> bool {
        self.unknown_iterations > 0
    }

    /// Runs one iteration with `body`; whether another follows. A path still running
    /// [`Unrolled::LIMIT`] iterations after an unknown decision is cut: it leaves with every local
    /// unknown (§3.3.3, §3.3.16).
    pub fn advance(&mut self, body: impl FnOnce(S) -> Step<S, B, R>) -> bool {
        let Some(state) = self.live.take() else { return false };
        if self.unknown_iterations >= Self::LIMIT {
            drop(state);
            self.outcome.leave(S::unknown(), B::unknown());
            return false;
        }
        self.trips.tick();
        let mut paths = Vec::new();
        let split = body(state).paths(&mut paths);
        if split || self.unknown_iterations > 0 {
            self.unknown_iterations += 1;
        }
        for path in paths {
            match path {
                Step::Next(state) | Step::Fall(state) => {
                    self.live = Some(match self.live.take() {
                        Some(other) => other.merge(state),
                        None => state,
                    })
                }
                Step::Exit(state, value) => self.outcome.leave(state, value),
                Step::Returned(value) => self.outcome.give(value),
                Step::Either(..) => {}
            }
        }
        self.live.is_some()
    }

    /// How the loop ended, once no path goes on.
    pub fn finish(self) -> Outcome<S, B, R> {
        self.outcome
    }

    /// The whole loop: `body` once per iteration until no path goes on.
    pub fn run(origin: &'static str, state: S, mut body: impl FnMut(S) -> Step<S, B, R>) -> Outcome<S, B, R> {
        let mut unrolled = Unrolled::start(origin, state);
        while unrolled.advance(&mut body) {}
        unrolled.finish()
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

    use super::{Attempt, Decision, Depth, Index, Panic, Step, Trips, Unrolled};
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
    fn an_unknown_index_merges_the_elements_it_may_name() {
        // §3.3.13: an unknown index may name any element: a read is the merge of all of them (known
        // only where they agree), a write merges the value into each
        let n = |v: u64| -> U8 { k(UInt::<8>::new(v)) };
        let elements = [n(1), n(1), n(2)];
        assert_eq!(Index::read(&elements, Some(2), "a.msl:1:1"), n(2));
        assert_eq!(Index::read(&elements, None, "a.msl:1:1"), u());
        assert_eq!(Index::read(&[n(3), n(3)], None, "a.msl:1:1"), n(3));
        let mut written = [n(1), n(1), n(2)];
        Index::write(&mut written, None, n(1), "a.msl:2:1");
        assert_eq!(written, [n(1), n(1), u()], "equal levels stay known, the rest become unknown");
        let mut known = [n(1), n(1)];
        Index::write(&mut known, Some(1), n(9), "a.msl:3:1");
        assert_eq!(known, [n(1), n(9)]);
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

    /// The state of the loops below: the iteration count and a tag the loop never changes.
    type State = (U8, U8);
    type Move = Step<State, (), U8>;

    fn state(count: u64, tag: u64) -> State {
        (k(UInt::<8>::new(count)), k(UInt::<8>::new(tag)))
    }

    fn count_of((count, _): State) -> u64 {
        count.known_or(UInt::<8>::new(0)).get()
    }

    #[test]
    fn unrolled_merges_exit_by_exit() {
        // §3.3.3: a decision that may be unknown at iteration 3 leaves with the state it has and
        // goes on; the loop then leaves for sure at iteration 5. The exits merge level by level:
        // the count differs (3 and 5), the tag, 7 on both, stays known
        let body = |(count, tag): State| -> Move {
            let n = count_of((count, tag));
            let next = || state(n + 1, 7);
            match n {
                3 => Step::Either(Box::new(Step::Exit(state(3, 7), ())), Box::new(Step::Next(next()))),
                5 => Step::Exit(state(5, 7), ()),
                _ => Step::Next(next()),
            }
        };
        let ((count, tag), ()) = Unrolled::run("loop @ merge.msl:1:1", state(0, 7), body).exit();
        assert_eq!(count, u());
        assert_eq!(tag, k(UInt::<8>::new(7)));

        // known decisions only: the loop is Rust's, and past 64 iterations it is not cut
        let known = |(count, tag): State| -> Move {
            let n = count_of((count, tag));
            if n == 100 { Step::Exit(state(n, 7), ()) } else { Step::Next(state(n + 1, 7)) }
        };
        let (state_100, ()) = Unrolled::run("loop @ known.msl:1:1", state(0, 7), known).exit();
        assert_eq!(state_100, state(100, 7));
    }

    #[test]
    fn unrolled_cuts_at_sixty_four() {
        // Appendix C.2: an exit that is never known keeps a path running; after 64 iterations of
        // unknown decisions it is cut and every local it writes is unknown, so the merge of the
        // exits is unknown in the count and in the tag the cut path no longer vouches for
        let calls = Cell::new(0u32);
        let body = |(count, _): State| -> Move {
            calls.set(calls.get() + 1);
            let n = count.known_or(UInt::<8>::new(0)).get();
            Step::Either(Box::new(Step::Exit(state(n, 7), ())), Box::new(Step::Next(state(n + 1, 7))))
        };
        let (exit, ()) = Unrolled::run("loop @ never.msl:1:1", state(0, 7), body).exit();
        assert_eq!(calls.get(), 64);
        assert_eq!(exit, (u(), u()));
    }

    #[test]
    fn a_return_ends_the_function_and_a_partial_one_leaves_its_value_unknown() {
        // every path returns: the value; some return and some leave: unknown (§3.3.3)
        let only = Unrolled::<State, (), U8>::run("loop @ r.msl:1:1", state(0, 7), |_| Step::Returned(k(UInt::new(9)))).finish();
        assert_eq!(only.err(), Some(k(UInt::new(9))));
        let some = Unrolled::<State, (), U8>::run("loop @ r.msl:2:1", state(0, 7), |s| {
            Step::Either(Box::new(Step::Returned(k(UInt::new(9)))), Box::new(Step::Exit(s, ())))
        })
        .finish();
        assert_eq!(some.err(), Some(u()));
    }

    #[test]
    fn then_runs_the_rest_of_an_iteration_on_every_path_that_fell_through() {
        // an unknown decision whose arms fall through runs the rest on both; an arm that left does not
        let split: Move = Step::Either(Box::new(Step::Fall(state(1, 7))), Box::new(Step::Exit(state(2, 7), ())));
        let mut ran = Vec::new();
        let done = split.then(|s| {
            ran.push(count_of(s));
            Step::Next(s)
        });
        assert_eq!(ran, [1]);
        assert!(matches!(done, Step::Either(..)));
    }
}
