//! The linear-term rule on compiled code (feature `ir-rust`, RS-06 AC 3 and 4; checks C36, C38):
//! a term affine in its continuous reads is written as `c + Σ partial·read`, folded from `0.0`
//! in declared partial order, so the compiled resistor's value has the bits of `g*vp - g*vn`
//! and the assembly's right side, `-(v - Σ g·x)`, is exactly 0 over 1000 points; the Vsource's
//! `eq` term over 2000 points, half of them wide, leaves a right side within one ulp of
//! `max(|c|, |Σ|)` of `-c`, exactly 0 at `dc = 0`, with inexact points present.
#![cfg(unix)]

use piperine_abi::raw::{Shape, ambient};

use crate::both;

use both::{Both, Rng, Written};

/// Associated test helpers of `residue`, grouped on one fixture type.
struct ResidueFixture;

impl ResidueFixture {
    fn reals(values: &[f64]) -> Vec<Vec<u8>> {
        values.iter().map(|v| v.to_le_bytes().to_vec()).collect()
    }

    /// The index of the `terms` function of a body of `shape`.
    fn terms(shape: &Shape<'_>) -> u32 {
        shape.functions.iter().position(|f| f.name.as_str() == "terms").expect("a terms function") as u32
    }
}

// RS-06 AC 3: Resistor `terms` reads [p, n, r].
#[test]
fn the_compiled_resistor_matches_the_hand_written_products_bit_for_bit() {
    let both = Both::five();
    let (_, handle) = both.instances("Resistor");
    let k = ResidueFixture::terms(handle.shape());
    let mut rng = Rng(0x0123_4567_89ab_cdef);
    for _ in 0..1000 {
        let (vp, vn) = (rng.range(-100.0, 100.0), rng.range(-100.0, 100.0));
        let r = 10f64.powf(rng.range(-3.0, 9.0));
        let out = Written::eval(&handle, k, &ResidueFixture::reals(&[vp, vn, r]), None).expect("terms");
        let g = 1.0 / r;
        let value = out.real(0);
        // the assembly's right side, -(v - Σ g_j·x_j) folded from 0.0, is exactly zero
        assert_eq!(value - (0.0 + g * vp + (-g) * vn), 0.0, "residue at ({vp}, {vn}, {r})");
        assert_eq!(value.to_bits(), (g * vp - g * vn).to_bits(), "({vp}, {vn}, {r})");
        assert_eq!(out.jac, [g, -g]);
    }
}

// RS-06 AC 4: the assembly stamps `-(v - dot)` with `v = c ⊕ dot`. With `c = 0` the residue is
// exactly 0; with `c ≠ 0` it is `(c ⊕ dot) ⊖ dot`, within one ulp of the row's magnitude
// `max(|c|, |dot|)` of `c` (no float `v` gives `c` back exactly at every point).
// Vsource `terms` reads [p, n, b0.flow, stim VALUE, dc, ac, SOURCE_SCALE].
#[test]
fn a_compiled_affine_constant_leaves_at_most_one_ulp() {
    let both = Both::five();
    let (_, handle) = both.instances("Vsource");
    let shape = handle.shape();
    let k = ResidueFixture::terms(shape);
    let f = shape.functions.get(k as usize).expect("terms");
    assert!(f.reads.iter().any(|c| c.kind == piperine_abi::raw::cell::AMBIENT && c.index == ambient::SOURCE_SCALE), "the sweep's last read is SOURCE_SCALE");
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let mut inexact = 0;
    for i in 0..2000 {
        let wide = i % 2 == 1;
        let (vp, vn) = if wide { (rng.range(-1e6, 1e6), rng.range(-20.0, 20.0)) } else { (rng.range(-20.0, 20.0), rng.range(-20.0, 20.0)) };
        let (dc, scale) = if wide { (rng.range(-1e-3, 1e3), rng.next()) } else { (rng.range(-10.0, 10.0), rng.next()) };
        let out = Written::eval(&handle, k, &ResidueFixture::reals(&[vp, vn, 0.0, 0.0, dc, 1.0, scale]), None).expect("terms");
        // the assembly's fold over b0.eq's written partials (p, n, the stim at 0), from 0.0
        let dot = out.jac[1..].iter().zip([vp, vn, 0.0]).fold(0.0, |s, (g, x)| s + g * x);
        let c = -(dc * scale);
        let residue = out.real(1) - dot;
        let top = c.abs().max(dot.abs());
        let ulp = f64::from_bits(top.to_bits() + 1) - top;
        assert!((residue - c).abs() <= ulp, "residue {residue:e} vs c = {c:e} at ({vp}, {vn}, {dc}, {scale}): ulp {ulp:e}");
        inexact += usize::from(residue != c);

        let out = Written::eval(&handle, k, &ResidueFixture::reals(&[vp, vn, 0.0, 0.0, 0.0, 1.0, scale]), None).expect("terms");
        let dot = out.jac[1..].iter().zip([vp, vn, 0.0]).fold(0.0, |s, (g, x)| s + g * x);
        assert_eq!(out.real(1) - dot, 0.0, "c = 0 leaves no residue at ({vp}, {vn})");
    }
    assert!(inexact > 0, "the sweep holds points where c ⊕ dot ⊖ dot is not c");
}
