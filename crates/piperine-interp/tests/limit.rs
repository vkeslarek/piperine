//! The function of a `limit` site (feature `ir-interp`, INT-10 AC 1 to 3): `PROPOSED` is the
//! site's input `x` with its partials against the pins, `LIMITED` is `f(x, old)`; `old` is the
//! seed (or `x` with no seed) when `AGE` is 0 or `STATIC` and `AGE` is 1, else `LAST` (abi.md
//! A2); starting from the seed, a limited value below it is raised to it (§18 rule 2).

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "support/models.rs"]
mod models;

use models::{In, Models, View};

/// The physical constants `vt()` folds (`std/analog.msl`).
const K_B: f64 = 1.380649e-23;
const Q: f64 = 1.602176634e-19;

/// The diode's numbers, written as `devices.rs` builds them.
struct Junction {
    is: f64,
    n: f64,
    t: f64,
}

impl Junction {
    fn nvt(&self) -> f64 {
        self.n * (K_B * self.t / Q)
    }

    fn vcrit(&self) -> f64 {
        let nvt = self.nvt();
        nvt * (nvt / (2.0_f64.sqrt() * self.is)).ln()
    }

    /// `pnjlim` of `std/analog.msl`, in plain Rust.
    fn pnjlim(&self, new: f64, old: f64) -> f64 {
        let (nvt, vcrit) = (self.nvt(), self.vcrit());
        if new > vcrit && (new - old).abs() > 2.0 * nvt {
            if old > 0.0 {
                let arg = (new - old) / nvt;
                if arg > 0.0 { old + nvt * (2.0 + (arg - 2.0).ln()) } else { old - nvt * (2.0 + (2.0 - arg).ln()) }
            } else {
                nvt * (new / nvt).ln()
            }
        } else if new < 0.0 {
            let arg = if old > 0.0 { -old - 1.0 } else { 2.0 * old - 1.0 };
            if new < arg { arg } else { new }
        } else {
            new
        }
    }

    /// `limit:vd` reads `[a, k, is, n, temperature, temperature given, LAST, AGE, STATIC]`; the run set its temperature.
    fn reads(&self, va: f64, vk: f64, last: f64, age: u8, is_static: bool) -> Vec<In> {
        vec![
            In::F64(va),
            In::F64(vk),
            In::F64(self.is),
            In::F64(self.n),
            In::F64(self.t),
            In::Byte(1),
            In::F64(last),
            In::Byte(age),
            In::Byte(u8::from(is_static)),
        ]
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * a.abs().max(b.abs())
    }
}

/// The index of function `name` of `body`.
struct Fns;

impl Fns {
    fn index(m: &Models, body: &str, name: &str) -> u32 {
        let names = View { shape: m.module(body).prototype() }.function_names().iter().map(|s| s.to_string()).collect::<Vec<_>>();
        names.iter().position(|n| n == name).unwrap_or_else(|| panic!("{body} has no {name}")) as u32
    }
}

#[test]
fn a_limit_writes_proposed_and_limited() {
    let m = Models::all();
    let d = m.instance("Diode");
    let k = Fns::index(&m, "Diode", "limit:vd");
    let j = Junction { is: 1e-14, n: 1.0, t: 300.0 };
    // A step past the knee from LAST = 0.7: pnjlim bounds it. A small step is left as it is.
    for (va, vk, last) in [(0.95, 0.05, 0.7), (0.4, 0.1, 0.29), (1.5, 0.2, 0.75), (0.1, 0.6, 0.2)] {
        let out = Models::eval(&d, k, &j.reads(va, vk, last, 2, false), None).expect("limit");
        assert_eq!(out.values[0], va - vk, "PROPOSED is v(a, k)");
        let expected = j.pnjlim(va - vk, last);
        assert!(Junction::close(out.values[1], expected), "LIMITED at {}: {} vs {expected}", va - vk, out.values[1]);
        assert_eq!(out.jac, [1.0, -1.0], "PROPOSED against a and k");
    }
    // the first case is limited: LIMITED differs from PROPOSED
    let out = Models::eval(&d, k, &j.reads(0.95, 0.05, 0.7, 2, false), None).expect("limit");
    assert!(out.values[1] < 0.9 - 0.05, "{}", out.values[1]);
}

#[test]
fn old_is_seed_or_last_by_age_and_static() {
    // Hold's closure returns `old`, so LIMITED is the `old` the site took.
    let (x, last, seed) = (0.9 - 0.3, -0.4, 0.25);
    let reads = |age: u8, is_static: bool| vec![In::F64(0.9), In::F64(0.3), In::F64(last), In::Byte(age), In::Byte(u8::from(is_static))];
    let seeded = Models::hold(Some(seed), false);
    let h = seeded.instance("Hold");
    let no_seed = Models::hold(None, false);
    let g = no_seed.instance("Hold");
    for (age, is_static, from_seed) in
        [(0, false, true), (0, true, true), (1, true, true), (1, false, false), (2, false, false), (2, true, false)]
    {
        let out = Models::eval(&h, 0, &reads(age, is_static), None).expect("limit");
        assert_eq!(out.values[0], 0.9 - 0.3);
        let want = if from_seed { seed } else { last };
        assert_eq!(out.values[1], want, "AGE {age}, STATIC {is_static}, seeded");
        let out = Models::eval(&g, 0, &reads(age, is_static), None).expect("limit");
        let want = if from_seed { x } else { last };
        assert_eq!(out.values[1], want, "AGE {age}, STATIC {is_static}, no seed: x");
    }
}

#[test]
fn the_seed_is_kept_when_the_limited_value_falls_below() {
    let m = Models::all();
    let d = m.instance("Diode");
    let k = Fns::index(&m, "Diode", "limit:vd");
    let j = Junction { is: 1e-14, n: 1.0, t: 300.15 };
    let vcrit = j.vcrit();
    assert!(vcrit > 0.6 && vcrit < 0.8, "vcrit {vcrit}");
    // x below vcrit on the first iteration: pnjlim(x, vcrit) is x, below the seed: LIMITED is vcrit.
    for x in [0.3, 0.0, -0.5] {
        assert!(j.pnjlim(x, vcrit) < vcrit);
        for (age, is_static) in [(0, false), (1, true)] {
            let out = Models::eval(&d, k, &j.reads(x, 0.0, 0.0, age, is_static), None).expect("limit");
            assert!(Junction::close(out.values[1], vcrit), "x {x}, AGE {age}: {} vs vcrit {vcrit}", out.values[1]);
        }
        // from LAST the seed is not read, nothing is raised
        let out = Models::eval(&d, k, &j.reads(x, 0.0, 0.31, 2, false), None).expect("limit");
        assert_eq!(out.values[1], j.pnjlim(x, 0.31), "x {x} from LAST");
    }
    // a limited value at or above the seed is kept as it is
    let out = Models::eval(&d, k, &j.reads(1.2, 0.0, 0.0, 0, false), None).expect("limit");
    let expected = j.pnjlim(1.2, vcrit);
    assert!(expected > vcrit);
    assert!(Junction::close(out.values[1], expected));
}
