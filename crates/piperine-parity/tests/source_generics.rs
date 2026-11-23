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

/// `trait Shape { fn area(self) -> f64; }` and `struct Sq`.
trait Shape {
    fn area(self) -> f64;
}

#[derive(Clone, Copy)]
struct Sq {
    a: f64,
}

impl Shape for Sq {
    fn area(self) -> f64 {
        self.a * self.a
    }
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
            CarrierBytes::real(Sq { a: 3.0 }.area()),
        ],
    );
}
