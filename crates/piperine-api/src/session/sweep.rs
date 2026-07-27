//! Sweeps over a compiled [`Session`]: a single knob ([`Sweep`] yielding
//! [`SweepPoint`]s, HOST-18) and a named multi-axis grid ([`Grid`] mapping
//! into a [`Nested`] tree, HOST-19).

use crate::error::Error;

use super::entry::Session;

// ─── Sweep / SweepPoint (HOST-18) ───────────────────────────────────────────

/// A `Session` view at one sweep coordinate (HOST-18): `Deref`/`DerefMut` to
/// [`Session`], so `point.op(...)`/`point.tran(...)`/… — every analysis —
/// runs directly on it, at the knob value the [`Sweep`] just restamped
/// (or rebuilt) onto the held circuit.
pub struct SweepPoint<'a> {
    session: &'a mut Session,
    /// The knob value this point was set to.
    pub value: f64,
    /// This point's position in the sweep's `values` slice.
    pub index: usize,
}

impl std::ops::Deref for SweepPoint<'_> {
    type Target = Session;
    fn deref(&self) -> &Session {
        self.session
    }
}

impl std::ops::DerefMut for SweepPoint<'_> {
    fn deref_mut(&mut self) -> &mut Session {
        self.session
    }
}

/// A fluent single-knob sweep (HOST-18, [`Session::sweep`]): a streaming
/// (lending) iterator — `next(&mut self) -> Option<Result<SweepPoint<'_>, Error>>`
/// instead of `std::iter::Iterator`, since each yielded [`SweepPoint`]
/// mutably borrows the sweep's own `Session` and Rust's stable `Iterator`
/// trait cannot express an item borrowing from the iterator itself. Drive it
/// with `while let Some(point) = sweep.next() { let point = point?; … }`.
pub struct Sweep<'a> {
    session: &'a mut Session,
    label: String,
    param: String,
    values: Vec<f64>,
    idx: usize,
}

impl Sweep<'_> {
    /// The number of points in this sweep.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// `true` when the sweep has no points.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Restamp (or rebuild, for a structural knob) the session onto the next
    /// sweep value and yield the resulting [`SweepPoint`]; `None` once every
    /// value has been visited. A structural knob transparently rebuilds the
    /// circuit and counts it in [`Session::rebuilds`] (HOST-18) rather than
    /// failing loud the way a bare [`Session::set`] does.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Result<SweepPoint<'_>, Error>> {
        if self.idx >= self.values.len() {
            return None;
        }
        let value = self.values[self.idx];
        let index = self.idx;
        self.idx += 1;
        if let Err(e) = self.session.set_or_rebuild(&self.label, &self.param, value) {
            return Some(Err(e));
        }
        Some(Ok(SweepPoint { session: self.session, value, index }))
    }
}

// ─── Grid / Nested (HOST-19) ────────────────────────────────────────────────

/// A nested (axis-shaped) result tree (HOST-19): [`Grid::map`]'s return
/// shape — `Leaf` at the deepest axis, `Branch` at every outer axis. Mirrors
/// a numpy ndarray's shape without pulling an `ndarray`/ad hoc flat-index
/// dependency into a generic-`R` result type; the tree's depth equals the
/// grid's axis count and each `Branch`'s length equals that axis's value
/// count (i.e. `Grid::shape()`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nested<R> {
    Leaf(R),
    Branch(Vec<Nested<R>>),
}

/// A named multi-axis sweep grid (HOST-19, [`Session::sweep_grid`]): each
/// axis is `(label, param, values)`; [`Grid::map`] visits the cartesian
/// product in row-major (outer-axis-first) order, restamping (or
/// rebuilding, per axis write — same [`Session::set_or_rebuild`] escape
/// hatch [`Sweep`] uses) each axis's value before calling the mapped
/// function, and collects the results into a [`Nested`] tree shaped like
/// [`Grid::shape`].
pub struct Grid<'a> {
    session: &'a mut Session,
    axes: Vec<(String, String, Vec<f64>)>,
}

impl Session {
    /// A fluent single-knob sweep over `label.param` (HOST-18): iterate with
    /// `while let Some(point) = sweep.next() { ... }` — each `point` is a
    /// [`SweepPoint`], a `Session` view at that knob value (`Deref`/
    /// `DerefMut` to `Session`, so every analysis method is callable
    /// directly on it). A non-structural value restamps on the one
    /// compilation (MD-18); a structural value rebuilds the circuit in
    /// place and increments [`Self::rebuilds`] — see [`Sweep::next`].
    pub fn sweep<'a>(&'a mut self, label: &str, param: &str, values: &[f64]) -> Sweep<'a> {
        Sweep { session: self, label: label.to_string(), param: param.to_string(), values: values.to_vec(), idx: 0 }
    }

    /// A named multi-axis sweep grid (HOST-19): `axes` is
    /// `[(label, param, values), ...]`, outer axis first. Iterate with
    /// [`Grid::map`].
    pub fn sweep_grid<'a>(&'a mut self, axes: &[(&str, &str, &[f64])]) -> Grid<'a> {
        Grid {
            session: self,
            axes: axes.iter().map(|&(l, p, v)| (l.to_string(), p.to_string(), v.to_vec())).collect(),
        }
    }
}

impl Grid<'_> {
    /// The grid's shape — one length per axis, outer axis first.
    pub fn shape(&self) -> Vec<usize> {
        self.axes.iter().map(|(_, _, v)| v.len()).collect()
    }

    /// The total number of grid points (product of [`Self::shape`]).
    pub fn len(&self) -> usize {
        self.axes.iter().map(|(_, _, v)| v.len()).product()
    }

    /// `true` when any axis has no values (an empty grid).
    pub fn is_empty(&self) -> bool {
        self.axes.iter().any(|(_, _, v)| v.is_empty())
    }

    /// Visit every combination in the grid (row-major, outer axis first),
    /// restamping (or rebuilding) each axis's value on the held session
    /// before calling `f` with the session and this point's coordinates
    /// (one value per axis, outer axis first), and collect the results into
    /// a [`Nested`] tree shaped like [`Self::shape`]. A `f` error
    /// propagates with the failing combination's coordinates prefixed (the
    /// spec's edge case: a sweep-point failure surfaces with the point's
    /// coordinates, not a bare error).
    pub fn map<R>(
        &mut self,
        mut f: impl FnMut(&mut Session, &[f64]) -> Result<R, Error>,
    ) -> Result<Nested<R>, Error> {
        let axes = self.axes.clone();
        let mut coord = Vec::with_capacity(axes.len());
        Self::map_axis(self.session, &axes, 0, &mut coord, &mut f)
    }

    fn map_axis<R>(
        session: &mut Session,
        axes: &[(String, String, Vec<f64>)],
        depth: usize,
        coord: &mut Vec<f64>,
        f: &mut impl FnMut(&mut Session, &[f64]) -> Result<R, Error>,
    ) -> Result<Nested<R>, Error> {
        if depth == axes.len() {
            let value = f(session, coord)
                .map_err(|e| Error::Measurement(format!("sweep_grid at {coord:?}: {e}")))?;
            return Ok(Nested::Leaf(value));
        }
        let (label, param, values) = &axes[depth];
        let mut branch = Vec::with_capacity(values.len());
        for &v in values {
            session.set_or_rebuild(label, param, v).map_err(|e| {
                Error::Measurement(format!("sweep_grid at {coord:?} + [{label}.{param}={v}]: {e}"))
            })?;
            coord.push(v);
            branch.push(Self::map_axis(session, axes, depth + 1, coord, f)?);
            coord.pop();
        }
        Ok(Nested::Branch(branch))
    }
}
