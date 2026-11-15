//! The Rust side of abi.md §7: the `Device` trait and the borrow-checked
//! lenses `Eval` and `React` over a call's frame. The lenses hold slices the
//! thunk has already cut to each cell's length (`cell_lens`, computed from the
//! shape entry), so nothing here is `unsafe`.

use std::fmt;
use std::ops::Deref;

pub use crate::raw::Pod;
use crate::raw::{Cell, List, Shape, pod_bytes, pod_mut, pod_ref, status};
use crate::shape::ShapeRef;
pub use crate::validate::Access;
use crate::validate::{CellBytes, cell_bytes};

/// A device written in Rust. `module::<D>` turns it into the C entry points.
pub trait Device: Sized + Send + 'static {
    /// The shape at default values (abi.md §2, `Module.prototype`).
    fn prototype() -> &'static Shape<'static>;
    /// An instance and its shape for these param values.
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault>;
    /// Runs function `k`.
    fn eval(&self, k: u32, cx: &mut Eval<'_>) -> Result<(), Fault>;
    /// Runs reaction `k`.
    fn react(&self, k: u32, cx: &mut React<'_>) -> Result<Flow, Fault>;
}

/// What a reaction asks of the analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Continue,
    /// End the analysis after this instant (status `FINISH`).
    Finish,
}

/// A device's error: status `FAULT` with `code` and `message` in the note, or
/// `RESHAPE` when these param values need another shape (abi.md §9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    pub code: u32,
    pub message: String,
    pub reshape: bool,
}

impl Fault {
    /// A `FAULT` with a device-chosen code.
    pub fn new(code: u32, message: impl Into<String>) -> Fault {
        Fault { code, message: message.into(), reshape: false }
    }

    /// A `RESHAPE`: these param values need another shape.
    pub fn reshape(message: impl Into<String>) -> Fault {
        Fault { code: 0, message: message.into(), reshape: true }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (code {})", self.message, self.code)
    }
}

impl std::error::Error for Fault {}

/// The initial param values `create` receives: `PARAM` pins of the prototype,
/// each in its layout.
#[derive(Clone, Debug, Default)]
pub struct Values<'a> {
    entries: Vec<(u32, &'a [u8])>,
}

impl<'a> Values<'a> {
    pub fn new(entries: Vec<(u32, &'a [u8])>) -> Values<'a> {
        Values { entries }
    }

    /// The bytes given for `pin`, if any.
    pub fn get(&self, pin: u32) -> Option<&'a [u8]> {
        self.entries.iter().find(|(p, _)| *p == pin).map(|(_, b)| *b)
    }

    /// The `F64` at byte 0 of the bytes given for `pin`, if any.
    pub fn f64(&self, pin: u32) -> Option<f64> {
        let b = self.get(pin)?;
        Some(f64::from_le_bytes(b.get(..8)?.try_into().ok()?))
    }
}

/// The byte length of each cell of `cells` under `access`, from the shape
/// entry that lists them: what a frame's buffer for that cell holds. `None`
/// when a cell breaks a rule `validate` checks.
pub fn cell_lens(shape: &Shape, cells: &List<Cell>, access: Access) -> Option<Vec<usize>> {
    cells
        .iter()
        .map(|c| {
            cell_bytes(shape, c, access).ok().map(|b| match b {
                CellBytes::F64 | CellBytes::I64 => 8,
                CellBytes::Byte => 1,
                CellBytes::Layout(l) => l.size as usize,
            })
        })
        .collect()
}

/// The read cells of a call, shared by both lenses.
#[derive(Debug)]
pub struct Reads<'a> {
    input: Vec<&'a [u8]>,
}

impl<'a> Reads<'a> {
    fn cell(&self, i: usize) -> &'a [u8] {
        match self.input.get(i) {
            Some(b) => b,
            None => panic!("read {i} does not exist: the call has {} reads", self.input.len()),
        }
    }

    fn fixed<const N: usize>(&self, i: usize, what: &str) -> [u8; N] {
        let b = self.cell(i);
        match b.get(..N) {
            Some(head) => head.try_into().expect("N bytes"),
            None => panic!("read {i} holds {} bytes, too few for {what}", b.len()),
        }
    }

    /// The number of reads.
    pub fn reads(&self) -> usize {
        self.input.len()
    }

    /// The bytes of read `i`.
    pub fn bytes(&self, i: usize) -> &'a [u8] {
        self.cell(i)
    }

    /// The `F64` at byte 0 of read `i`. Panics (a `FAULT`) when it is shorter.
    pub fn f64(&self, i: usize) -> f64 {
        f64::from_le_bytes(self.fixed(i, "an F64"))
    }

    /// The byte 0 of read `i`.
    pub fn u8(&self, i: usize) -> u8 {
        self.fixed::<1>(i, "a U8")[0]
    }

    /// Whether byte 0 of read `i` is nonzero.
    pub fn bool(&self, i: usize) -> bool {
        self.u8(i) != 0
    }

    /// The `I64` at byte 0 of read `i`.
    pub fn i64(&self, i: usize) -> i64 {
        i64::from_le_bytes(self.fixed(i, "an I64"))
    }

    /// Read `i` as a `T`; `None` when `T` does not fit its bytes.
    pub fn get<T: Pod>(&self, i: usize) -> Option<&'a T> {
        pod_ref(self.input.get(i)?)
    }
}

/// The lens of `eval`.
#[derive(Debug)]
pub struct Eval<'a> {
    reads: Reads<'a>,
    output: Vec<&'a mut [u8]>,
    seed: &'a [u8],
    jac: &'a mut [f64],
}

impl<'a> Eval<'a> {
    /// A lens over already-cut cells. `seed` and `jac` have one entry per
    /// declared partial.
    pub fn new(input: Vec<&'a [u8]>, output: Vec<&'a mut [u8]>, seed: &'a [u8], jac: &'a mut [f64]) -> Eval<'a> {
        assert_eq!(seed.len(), jac.len(), "one seed per partial");
        Eval { reads: Reads { input }, output, seed, jac }
    }

    fn out_cell(&mut self, j: usize) -> &mut [u8] {
        let n = self.output.len();
        match self.output.get_mut(j) {
            Some(b) => b,
            None => panic!("write {j} does not exist: the call has {n} writes"),
        }
    }

    /// The buffer of write `j`, pre-filled with its current bytes.
    pub fn out_bytes(&mut self, j: usize) -> &mut [u8] {
        self.out_cell(j)
    }

    /// Write `j` as a `T`; `None` when `T` does not fit its bytes.
    pub fn out<T: Pod>(&mut self, j: usize) -> Option<&mut T> {
        pod_mut(self.output.get_mut(j)?)
    }

    /// Writes `value` at byte 0 of write `j`. Panics (a `FAULT`) when it does not fit.
    pub fn set<T: Pod>(&mut self, j: usize, value: T) {
        let bytes = pod_bytes(&value);
        let cell = self.out_cell(j);
        let len = cell.len();
        match cell.get_mut(..bytes.len()) {
            Some(head) => head.copy_from_slice(bytes),
            None => panic!("write {j} holds {len} bytes, too few for {} bytes", bytes.len()),
        }
    }

    /// Whether partial `p` is seeded.
    pub fn wanted(&self, p: usize) -> bool {
        self.seed.get(p).is_some_and(|s| *s != 0)
    }

    /// The partials, one per declared entry, pre-zeroed.
    pub fn jac(&mut self) -> &mut [f64] {
        self.jac
    }
}

impl<'a> Deref for Eval<'a> {
    type Target = Reads<'a>;
    fn deref(&self) -> &Reads<'a> {
        &self.reads
    }
}

/// Queues one write of the calling reaction: `(statement, bytes, after)` to a
/// status.
pub type CommitSink<'a> = dyn FnMut(u32, &[u8], f64) -> u32 + 'a;

/// The lens of `react`.
pub struct React<'a> {
    reads: Reads<'a>,
    commit: &'a mut CommitSink<'a>,
}

impl fmt::Debug for React<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("React").field("reads", &self.reads).finish_non_exhaustive()
    }
}

impl<'a> React<'a> {
    /// A lens over already-cut read cells and the host's commit.
    pub fn new(input: Vec<&'a [u8]>, commit: &'a mut CommitSink<'a>) -> React<'a> {
        React { reads: Reads { input }, commit }
    }

    /// Commits `bytes` through statement `statement`, due `after` seconds
    /// later (abi.md §6). A refusal is a `Fault` carrying the status as its code.
    pub fn commit(&mut self, statement: u32, bytes: &[u8], after: f64) -> Result<(), Fault> {
        match (self.commit)(statement, bytes, after) {
            status::OK => Ok(()),
            s => Err(Fault::new(s, format!("commit of statement {statement} refused with status {s}"))),
        }
    }
}

impl<'a> Deref for React<'a> {
    type Target = Reads<'a>;
    fn deref(&self) -> &Reads<'a> {
        &self.reads
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eight bytes aligned for a `u64`, as abi.md §12 promises a buffer of a cell.
    #[repr(C, align(8))]
    struct Aligned([u8; 8]);

    #[test]
    fn reads_and_writes_through_the_eval_lens() {
        let v = 2.5f64.to_le_bytes();
        let flag = [1u8];
        let tick = (-7i64).to_le_bytes();
        let bit = [1u8, 1];
        let mut out0 = [0u8; 8];
        let mut out1 = Aligned([0u8; 8]);
        let seed = [1u8, 0];
        let mut jac = [0.0; 2];
        let mut cx = Eval::new(vec![&v, &flag, &tick, &bit], vec![&mut out0, &mut out1.0], &seed, &mut jac);
        assert_eq!(cx.f64(0), 2.5);
        assert_eq!(cx.u8(1), 1);
        assert!(cx.bool(1));
        assert_eq!(cx.i64(2), -7);
        assert_eq!(cx.bytes(3), [1, 1]);
        assert_eq!(cx.reads(), 4);
        cx.set(0, 4.0f64);
        *cx.out::<u64>(1).expect("8 aligned bytes hold a u64") = 9;
        assert!(cx.wanted(0) && !cx.wanted(1) && !cx.wanted(2));
        cx.jac().copy_from_slice(&[1.5, -1.5]);
        cx.out_bytes(1)[7] = 0;
        drop(cx);
        assert_eq!(f64::from_le_bytes(out0), 4.0);
        assert_eq!(u64::from_le_bytes(out1.0), 9);
        assert_eq!(jac, [1.5, -1.5]);
    }

    #[test]
    fn typed_access_that_does_not_fit_is_none() {
        let small = [0u8; 4];
        let exact = 3.0f64.to_le_bytes();
        let mut out = [0u8; 2];
        let mut jac = [];
        let mut cx = Eval::new(vec![&small, &exact], vec![&mut out], &[], &mut jac);
        assert!(cx.get::<f64>(0).is_none(), "8 bytes do not fit a 4-byte cell");
        assert!(cx.get::<[u8; 5]>(0).is_none());
        assert_eq!(cx.get::<[u8; 4]>(0), Some(&[0u8; 4]), "a type of the cell's size fits");
        assert_eq!(cx.get::<u8>(0), Some(&0u8));
        assert!(cx.get::<u8>(9).is_none(), "a read that does not exist");
        assert!(cx.out::<u32>(0).is_none(), "4 bytes do not fit a 2-byte write");
        assert!(cx.out::<u16>(3).is_none(), "a write that does not exist");
        assert!(cx.out::<[u8; 2]>(0).is_some());
    }

    #[test]
    fn react_lens_reads_and_commits() {
        let d = [1u8, 1];
        let mut seen = Vec::new();
        let mut sink = |s: u32, b: &[u8], after: f64| {
            seen.push((s, b.to_vec(), after));
            if s == 9 { status::FAULT } else { status::OK }
        };
        let mut cx = React::new(vec![&d], &mut sink);
        assert_eq!(cx.bytes(0), [1, 1]);
        let q = cx.bytes(0);
        assert_eq!(cx.commit(0, q, 0.0), Ok(()));
        assert_eq!(cx.commit(1, &[], 1e-9), Ok(()));
        let err = cx.commit(9, &[0], 0.0).expect_err("a refused commit");
        assert_eq!(err.code, status::FAULT);
        drop(cx);
        assert_eq!(seen, [(0, vec![1, 1], 0.0), (1, vec![], 1e-9), (9, vec![0], 0.0)]);
    }
}
