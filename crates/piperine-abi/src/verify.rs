//! The finite-difference verifier of declared partials (abi.md §8): purity
//! (F1) lets a function be re-run at perturbed inputs, so each declared
//! partial is compared with a central difference. It calls nothing but
//! `Handle::eval`.

use crate::caller::{CallError, Handle};
use crate::device::Access;
use crate::raw::{cell, part};

/// One declared partial that disagrees with its finite difference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mismatch {
    /// The partial's index in the function's `partials`.
    pub partial: u32,
    pub declared: f64,
    pub finite: f64,
}

/// The verifier's answer for one function at one point.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// Every declared partial agrees with its finite difference.
    Exact,
    /// The function writes limit input `limit` and its `LIMITED` differs from
    /// `PROPOSED` here: the point is skipped and nothing is compared (§8).
    Skipped { limit: u32 },
    Mismatch(Vec<Mismatch>),
}

impl Verdict {
    /// The little-endian `f64` at byte `at` of `bytes`.
    fn f64_at(bytes: &[u8], at: u32) -> f64 {
        let at = at as usize;
        f64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"))
    }

    /// Verifies every declared partial of function `k` of `handle` at the point
    /// `input` (the bytes of each read cell). A partial passes when
    /// `|declared − finite| ≤ rel_tol · max(|declared|, |finite|)` plus the
    /// central difference's own round-off, `4 ε · max(|f(x+h)|, |f(x−h)|) / 2h`.
    /// The step is `h = 1e-6 · max(1, |x|)`. A non-finite declared value or
    /// finite difference is a mismatch, never a pass.
    /// A function that faults at the point returns its `CallError`.
    /// Kept for the parity host that verifies a device's declared partials (`piperine-parity` `tests/source_tables.rs`).
    pub fn of_partials(handle: &Handle, k: u32, input: &[Vec<u8>], rel_tol: f64) -> Result<Verdict, CallError> {
        let shape = handle.shape();
        let f = shape.functions.get(k as usize).ok_or_else(|| CallError {
            status: crate::raw::status::FAULT,
            code: 0,
            message: format!("function {k} does not exist"),
        })?;
        let writes = shape.cell_lens(&f.writes, Access::FunctionWrite).unwrap_or_default();
        let n = f.partials.len();

        // The analytic call: every partial seeded.
        let run = |input: &[Vec<u8>], seed: &[u8]| -> Result<(Vec<Vec<u8>>, Vec<f64>), CallError> {
            let mut out: Vec<Vec<u8>> = writes.iter().map(|len| vec![0u8; *len]).collect();
            let mut jac = vec![0.0; n];
            let reads: Vec<&[u8]> = input.iter().map(Vec::as_slice).collect();
            let mut outs: Vec<&mut [u8]> = out.iter_mut().map(Vec::as_mut_slice).collect();
            handle.eval(k, &reads, &mut outs, seed, &mut jac)?;
            Ok((out, jac))
        };
        let (out, jac) = run(input, &vec![1u8; n])?;

        // §8: only at points where every limit the function writes has LIMITED = PROPOSED.
        let written: Vec<_> = f.writes.iter().copied().collect();
        for (j, c) in written.iter().enumerate() {
            if c.kind != cell::INPUT || c.part != part::PROPOSED {
                continue;
            }
            let limited = written.iter().position(|w| w.kind == cell::INPUT && w.index == c.index && w.part == part::LIMITED);
            if let Some(l) = limited
                && Self::f64_at(&out[j], 0) != Self::f64_at(&out[l], 0)
            {
                return Ok(Verdict::Skipped { limit: c.index });
            }
        }

        let zero_seed = vec![0u8; n];
        let mut mismatches = Vec::new();
        for (i, p) in f.partials.iter().enumerate() {
            let x = Self::f64_at(&input[p.read as usize], p.at);
            let h = 1e-6 * x.abs().max(1.0);
            let at = |v: f64| -> Vec<Vec<u8>> {
                let mut moved = input.to_vec();
                let a = p.at as usize;
                moved[p.read as usize][a..a + 8].copy_from_slice(&v.to_le_bytes());
                moved
            };
            let (xp, xm) = (x + h, x - h);
            let (plus, _) = run(&at(xp), &zero_seed)?;
            let (minus, _) = run(&at(xm), &zero_seed)?;
            let (fp, fm) = (Self::f64_at(&plus[p.write as usize], p.to), Self::f64_at(&minus[p.write as usize], p.to));
            let finite = (fp - fm) / (xp - xm);
            let declared = jac[i];
            let roundoff = 4.0 * f64::EPSILON * fp.abs().max(fm.abs()) / (xp - xm);
            let off = (declared - finite).abs() > rel_tol * declared.abs().max(finite.abs()) + roundoff;
            if off || !declared.is_finite() || !finite.is_finite() {
                mismatches.push(Mismatch { partial: i as u32, declared, finite });
            }
        }
        Ok(if mismatches.is_empty() { Verdict::Exact } else { Verdict::Mismatch(mismatches) })
    }
}
