//! Generic items run from MSL source on the Rust backend (feature `generics`, GN-01 to GN-06): one
//! test per run-time criterion, each model written in `tests/generics/`, elaborated by the front
//! end, compiled on the transpiled backend and run on the solver. Every expected value is computed
//! here by Rust from the rule the test cites, with Rust's own structs and methods, never read back
//! from the model.
#![cfg(unix)]

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;
#[allow(dead_code)]
#[path = "source/bytes.rs"]
mod bytes;
#[allow(dead_code)]
#[path = "source/operators.rs"]
mod operators;
#[allow(dead_code)]
#[path = "source/run.rs"]
mod run;

use bytes::CarrierBytes;
use run::SourceRun;

/// The model files of `tests/generics/`.
const RUN: SourceRun = SourceRun::in_directory("tests/generics");

/// `struct Point { x: f64, y: f64 }` of the model `methods.msl`, with the methods its `impl` gives.
#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

impl Point {
    /// `const SIDES: u8 = 4;`
    const SIDES: u8 = 4;

    fn new(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn norm2(self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    fn scaled(self, k: f64) -> Point {
        Point { x: self.x * k, y: self.y * k }
    }

    fn dot(self, other: Point) -> f64 {
        self.x * other.x + self.y * other.y
    }
}

/// `struct Sq` of the models.
#[derive(Clone, Copy)]
struct Sq {
    a: f64,
}

#[test]
fn methods_associated_functions_and_constants_run_with_rusts_meaning() {
    // GN-03 AC 1 (§3.1), GN-04 AC 1 (§3.7): `Point::new` and `p.norm2()`, a method on the result of a method, a
    // method with another receiver as an argument, an associated constant, a method a trait impl gives
    let p = Point::new(1.0, 2.0);
    RUN.assert_commits(
        "methods.msl",
        &["m0", "m1", "m2", "m3", "m4"],
        &[
            CarrierBytes::real(Point::new(3.0, 4.0).norm2()),
            CarrierBytes::real(p.scaled(3.0).norm2()),
            CarrierBytes::real(p.dot(Point::new(3.0, 4.0))),
            CarrierBytes::byte(Point::SIDES * 2),
            CarrierBytes::real(Area::area(Sq { a: 3.0 })),
        ],
    );
}

/// `trait Shape` of the model `dispatch.msl`, with its default body and the `impl`s that give or override it.
trait Area {
    fn area(self) -> f64;

    fn doubled(self) -> f64
    where
        Self: Sized + Copy,
    {
        2.0 * self.area()
    }
}

#[derive(Clone, Copy)]
struct Rect {
    w: f64,
    h: f64,
}

impl Area for Sq {
    fn area(self) -> f64 {
        self.a * self.a
    }
}

impl Area for Rect {
    fn area(self) -> f64 {
        self.w * self.h
    }

    fn doubled(self) -> f64 {
        100.0 + self.area()
    }
}

/// `fn total<S: Shape>(shape: S) -> f64` of the model.
fn total<S: Area + Copy>(shape: S) -> f64 {
    shape.area() + shape.doubled()
}

#[test]
fn trait_methods_dispatch_statically_to_their_impl_default_or_blanket() {
    // GN-04 AC 1 to 3 (§3.7): a trait's own method, its default body, an impl that overrides the default, a call through
    // a `S: Shape` bound for two types, and a blanket `impl<T: Shape> Describe for T` read through a `D: Describe` bound
    let (square, rectangle) = (Sq { a: 3.0 }, Rect { w: 2.0, h: 5.0 });
    RUN.assert_commits(
        "dispatch.msl",
        &["m0", "m1", "m2", "m3", "m4", "m5"],
        &[
            CarrierBytes::real(Area::area(square)),
            CarrierBytes::real(Area::doubled(square)),
            CarrierBytes::real(rectangle.doubled()),
            CarrierBytes::real(total(square)),
            CarrierBytes::real(total(rectangle)),
            CarrierBytes::byte(7 + 1),
        ],
    );
}

/// `fn twice<F: Fn(f64) -> f64>(f: F, x: f64) -> f64` of the model `callables.msl`.
fn twice<F: Fn(f64) -> f64>(f: F, x: f64) -> f64 {
    f(f(x))
}

/// `fn halve(x: f64) -> f64` of the model.
fn halve(x: f64) -> f64 {
    0.5 * x
}

#[test]
fn a_capturing_closure_a_plain_closure_and_a_function_name_pass_for_an_fn_bound() {
    // GN-01 AC 4 (§3.8.6, D9): one generic function called with a plain closure, a closure that captures a local by
    // value and a function's name, each its own instance
    let gain = 4.0;
    RUN.assert_commits(
        "callables.msl",
        &["m0", "m1", "m2"],
        &[CarrierBytes::real(twice(|v| v + 1.0, 3.0)), CarrierBytes::real(twice(|v| v * gain, 2.0)), CarrierBytes::real(twice(halve, 10.0))],
    );
}

/// `struct Pair<T> { a: T, b: T }` of the model `types.msl`, with the method of its `impl<T>`.
#[derive(Clone, Copy)]
struct Pair<T> {
    a: T,
    b: T,
}

impl<T> Pair<T> {
    fn swapped(self) -> Pair<T> {
        Pair { a: self.b, b: self.a }
    }
}

/// `enum Slot<T> { Empty, Full(T) }` of the model.
enum Slot<T> {
    Empty,
    Full(T),
}

/// `fn load(s: Slot<f64>) -> f64` of the model.
fn load(slot: Slot<f64>) -> f64 {
    match slot {
        Slot::Empty => -1.0,
        Slot::Full(value) => value,
    }
}

/// `struct Window<T: PartialOrd>` of the model, with the method of its `impl<T>`.
struct Window<T: PartialOrd> {
    lo: T,
    hi: T,
}

impl<T: PartialOrd> Window<T> {
    fn holds(self, x: T) -> bool {
        self.lo <= x && x <= self.hi
    }
}

#[test]
fn generic_structs_and_enums_instantiate_per_argument_list() {
    // GN-06 AC 1 (§3.8.9): `Pair<f64>`, `Pair<u8>` and `Pair<Pair<f64>>` are three types; a method of `impl<T> Pair<T>`,
    // a function over `Pair<f64>`, a `match` over `Slot<f64>`, a nested instance and `Window<T: PartialOrd>`
    let nested = Pair { a: Pair { a: 1.0, b: 2.0 }, b: Pair { a: 3.0, b: 4.0 } };
    let in_window = |v: f64| u8::from(Window { lo: 3.0, hi: 3.6 }.holds(v));
    RUN.assert_commits(
        "types.msl",
        &["m0", "m1", "m2", "m3", "m4", "m5", "m6", "m7"],
        &[
            CarrierBytes::real(Pair { a: 1.0, b: 2.0 }.swapped().a),
            CarrierBytes::byte(Pair { a: 3u8, b: 4u8 }.swapped().b),
            CarrierBytes::real(1.5 + 2.5),
            CarrierBytes::real(load(Slot::Full(2.5))),
            CarrierBytes::real(load(Slot::<f64>::Empty)),
            CarrierBytes::real(nested.b.a),
            CarrierBytes::byte(in_window(3.3)),
            CarrierBytes::byte(in_window(4.0)),
        ],
    );
}

/// `enum Phase { A, B, C, D }` of the model `repr.msl`: one level per variant, written by hand.
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    A,
    B,
    C,
    D,
}

impl Phase {
    /// The levels `encode` of the model gives: the variant's own level is high.
    fn encode(self) -> [bool; 4] {
        [self == Phase::A, self == Phase::B, self == Phase::C, self == Phase::D]
    }

    /// `decode` of the model: the lowest high level names the variant; none high is no variant.
    fn decode(levels: [bool; 4]) -> Option<Phase> {
        [Phase::A, Phase::B, Phase::C, Phase::D].into_iter().zip(levels).find(|(_, high)| *high).map(|(phase, _)| phase)
    }
}

/// `struct Count { n: u8 }` of the model `repr.msl`: a two-level Gray code.
#[derive(Clone, Copy)]
struct Count {
    n: u8,
}

impl Count {
    /// The levels `encode` of the model gives: the Gray code of `n`, level `k` being bit `k`.
    fn encode(self) -> [bool; 2] {
        let gray = self.n ^ (self.n >> 1);
        [gray & 1 == 1, gray & 2 == 2]
    }
}

/// The byte of a carrier of a hand-written layout: level `k` is bit `k` (the narrowest unsigned integer of the levels).
struct LevelByte;

impl LevelByte {
    /// The byte whose bit `k` is `levels[k]`.
    fn of(levels: &[bool]) -> Vec<u8> {
        CarrierBytes::byte(levels.iter().enumerate().fold(0u8, |byte, (k, high)| byte | (u8::from(*high) << k)))
    }
}

#[test]
fn a_hand_written_repr_carries_its_own_layout() {
    // GN-05 AC 1: a one-hot `Bit<Phase>` commits the 4 levels its `encode` gives and a Gray-coded `Bit<Count>` the
    // 2 levels of its code, each as the byte whose bit `k` is level `k`
    RUN.assert_commits(
        "repr.msl",
        &["p0", "p1", "p2", "g0", "g1", "g2"],
        &[
            LevelByte::of(&Phase::B.encode()),
            LevelByte::of(&Phase::C.encode()),
            LevelByte::of(&Phase::D.encode()),
            LevelByte::of(&Count { n: 1 }.encode()),
            LevelByte::of(&Count { n: 2 }.encode()),
            LevelByte::of(&Count { n: 3 }.encode()),
        ],
    );
}

#[test]
fn a_decode_returning_none_is_returned_to_the_caller() {
    // GN-05 AC 2 (§2 rule 9 as MD-78 amends it): `decode` of no high level is `None`, of the third level `Some(C)`
    let none = Phase::decode([false; 4]).map_or(0, |_| 1);
    let some = Phase::decode([false, false, true, false]).map_or(0, |phase| u8::from(phase == Phase::C));
    RUN.assert_commits("repr.msl", &["d0", "d1"], &[CarrierBytes::byte(none), CarrierBytes::byte(some)]);
}

#[test]
fn a_generic_repr_instance_is_laid_out_per_section_2() {
    // GN-06 AC 2 (§2 rule 3): `Pair<u8>` derives `Repr` through its argument, so a `Bit<Pair<u8>>` carries the leaves of
    // its fields in order, one byte each
    let pair = Pair { a: 3u8, b: 4u8 };
    RUN.assert_commits("types.msl", &["m8"], &[[CarrierBytes::byte(pair.a), CarrierBytes::byte(pair.b)].concat()]);
}

/// `struct Pt { x: f64, y: f64 }` of the model `functions.msl`.
#[derive(Clone, Copy)]
struct Pt {
    x: f64,
    y: f64,
}

/// `fn pick<T>(flag: bool, a: T, b: T) -> T` of the model.
fn pick<T>(flag: bool, a: T, b: T) -> T {
    if flag { a } else { b }
}

/// `fn count<const N: usize>() -> u8` of the model: one step for each of `N`, with the loop variable unread.
fn count<const N: usize>() -> u8 {
    (0..N).fold(0u8, |n, _| n + 1)
}

/// `fn table_total<const N: usize>(xs: [f64; N]) -> f64` of the model.
fn table_total<const N: usize>(xs: [f64; N]) -> f64 {
    xs.iter().sum()
}

#[test]
fn generic_functions_run_with_explicit_and_inferred_arguments() {
    // GN-01 AC 1, GN-02: `pick` at `f64`, `u8` and a struct, each with the argument inferred and written
    // (`pick::<f64>`), and `table_total` over a table of three: the Independent Test of phase A
    let (a, b) = (Pt { x: 1.0, y: 2.0 }, Pt { x: 3.0, y: 4.0 });
    RUN.assert_commits(
        "functions.msl",
        &["r0", "r1", "r2", "r3", "r4", "r5", "r9"],
        &[
            CarrierBytes::real(pick(true, 1.5, 2.5)),
            CarrierBytes::real(pick::<f64>(false, 1.5, 2.5)),
            CarrierBytes::byte(pick(false, 3u8, 4u8)),
            CarrierBytes::byte(pick::<u8>(true, 3u8, 4u8)),
            CarrierBytes::real(pick(false, a, b).y),
            CarrierBytes::real(pick::<Pt>(true, a, b).x),
            CarrierBytes::real(table_total([1.0, 2.0, 4.0])),
        ],
    );
}

#[test]
fn a_const_generic_of_zero_runs_its_loops_zero_times() {
    // GN-01 edge case: `count::<0>()` takes no step and `table_total` of an empty table is 0.0; `count::<3>()` takes three
    let empty: [f64; 0] = [];
    RUN.assert_commits("functions.msl", &["r6", "r7", "r8"], &[CarrierBytes::byte(count::<0>()), CarrierBytes::byte(count::<3>()), CarrierBytes::real(table_total(empty) + 7.0)]);
}

#[test]
fn a_const_context_reads_the_const_generic_arguments() {
    // GN-14 (the const evaluator): a `const` that calls a generic function with a const argument runs its loops and its
    // array repeats `[x; N]` for that argument: `count::<3>()` is 3 and `filled::<4>()`, four elements of 1.5, sums to 6.0
    RUN.assert_commits("functions.msl", &["r10", "r11"], &[CarrierBytes::byte(count::<3>()), CarrierBytes::real(table_total([1.5; 4]))]);
}
