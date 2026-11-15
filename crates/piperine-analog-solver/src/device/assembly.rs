//! One solve's evaluation (design `device::assembly`, abi.md F3, A4): the
//! assembly keeps, per analog function, the bytes it last read and the
//! outputs and partials it returned. A function whose read bytes are
//! unchanged is not called and its outputs are reused (F3). The discrete pins
//! analog functions read are the assembly's own byte slots, written by
//! `set_discrete` (abi.md §10). A non-finite value or seeded partial that the
//! solve reads fails it, naming instance, cell and origin (A4).

use piperine_abi::raw::{ambient, op};

use super::device_error;
use super::flow::{direct, stamp_flow};
use super::plan::{AnalogPlan, FnPlan, InstancePlan, Source};
use crate::error::Error;
use crate::math::linear::Stamp;
use crate::netlist::AnalogReference;

/// The coordinates of one solve (abi.md §4.2, ambient inputs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ambient {
    /// The time of the solve; 0 where time does not exist.
    pub time: f64,
    /// The sweep point's frequency, else 0.
    pub freq: f64,
    /// An equilibrium solve.
    pub is_static: bool,
    /// The continuation variables; at rest (the session `gmin`, 1.0) in the
    /// last solve of a sequence.
    pub gmin: f64,
    pub source_scale: f64,
}

/// What the solver holds for one held input (abi.md §4.2, §4.3).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Held {
    /// `VALUE`: for a `LIMIT`, the iterate `x̂` held for this iteration.
    pub value: f64,
    /// `LAST`: the value held.
    pub last: f64,
    /// `AGE`: 0 none held, 1 from the last accepted solution, 2 from the
    /// previous iteration.
    pub age: u8,
}

/// The last call of one function.
#[derive(Debug)]
pub(crate) struct FnState {
    /// The bytes of every read cell at the last call, concatenated.
    read: Vec<u8>,
    /// Whether `out` and `jac` hold a call's results.
    valid: bool,
    /// One buffer per written cell.
    pub out: Vec<Vec<u8>>,
    /// One entry per declared partial.
    pub jac: Vec<f64>,
}

impl FnState {
    /// The `F64` at byte 0 of write `w`.
    pub fn f64(&self, w: usize) -> f64 {
        f64::from_le_bytes(self.out[w][..8].try_into().expect("an F64 cell"))
    }
}

/// The mutable side of one instance.
#[derive(Debug)]
pub(crate) struct InstanceState {
    pub fns: Vec<FnState>,
    pub held: Vec<Held>,
}

/// The device assembly of one circuit: its plan, the discrete slots its
/// functions read, and every function's last call.
#[derive(Debug)]
pub struct Assembly {
    pub(crate) plan: AnalogPlan,
    slots: Vec<Vec<u8>>,
    previous: Vec<Vec<u8>>,
    pub(crate) state: Vec<InstanceState>,
    scratch: Vec<u8>,
}

impl Assembly {
    /// An assembly over `plan`: every slot all zero (P3), no function called.
    pub fn new(plan: AnalogPlan) -> Assembly {
        let slots: Vec<Vec<u8>> = plan.slots.iter().map(|n| vec![0; *n]).collect();
        let state = plan
            .instances
            .iter()
            .map(|inst| InstanceState {
                fns: inst
                    .functions
                    .iter()
                    .map(|f| FnState {
                        read: Vec::new(),
                        valid: false,
                        out: f.write_lens.iter().map(|n| vec![0; *n]).collect(),
                        jac: vec![0.0; f.seed.len()],
                    })
                    .collect(),
                held: vec![Held::default(); inst.inputs.len()],
            })
            .collect();
        Assembly { previous: slots.clone(), slots, plan, state, scratch: Vec::new() }
    }

    /// The plan this assembly runs.
    pub fn plan(&self) -> &AnalogPlan {
        &self.plan
    }

    /// Writes discrete slot `slot` (a boundary event). When the bytes differ,
    /// the old ones become its `PREVIOUS` and every function that reads it
    /// runs again in the next solve. Panics when `bytes` is not the slot's size.
    pub fn set_discrete(&mut self, slot: u32, bytes: &[u8]) {
        let s = slot as usize;
        assert_eq!(bytes.len(), self.slots[s].len(), "discrete slot {slot} holds {} bytes", self.slots[s].len());
        if self.slots[s] != bytes {
            std::mem::swap(&mut self.slots[s], &mut self.previous[s]);
            self.slots[s].copy_from_slice(bytes);
        }
    }

    /// Evaluates, at iterate `x`, every analog function a Newton solve needs
    /// (those writing terms or limits) whose read bytes changed since its
    /// last call, then checks every value and seeded partial the solve reads.
    pub fn evaluate(&mut self, x: &[f64], amb: Ambient) -> Result<(), Error> {
        for (inst, st) in self.plan.instances.iter().zip(self.state.iter_mut()) {
            for (fi, f) in inst.functions.iter().enumerate() {
                if f.rows {
                    let ctx = Reads { x, amb, slots: &self.slots, previous: &self.previous, held: &st.held };
                    run(inst, f, &mut st.fns[fi], &ctx, &mut self.scratch)?;
                }
            }
            check_finite(inst, st)?;
        }
        Ok(())
    }
}

impl Assembly {
    /// The stamps of an equilibrium solve at iterate `x`: every `FLOW` term
    /// in its Norton form.
    pub fn assemble_dc(&mut self, x: &[f64], amb: Ambient) -> Result<Vec<Stamp<AnalogReference, f64>>, Error> {
        self.evaluate(x, amb)?;
        let mut out = Vec::new();
        for (inst, st) in self.plan.instances.iter().zip(&self.state) {
            for t in &inst.terms {
                if t.op == op::FLOW {
                    stamp_flow(t, &direct(t, st), x, &mut out);
                }
            }
        }
        Ok(out)
    }
}

/// Everything a read cell's bytes come from.
pub(crate) struct Reads<'a> {
    pub x: &'a [f64],
    pub amb: Ambient,
    pub slots: &'a [Vec<u8>],
    pub previous: &'a [Vec<u8>],
    pub held: &'a [Held],
}

impl Reads<'_> {
    /// Appends the bytes of one read cell of `len` bytes to `buf`.
    fn push(&self, source: Source, len: usize, buf: &mut Vec<u8>) {
        let f = |v: f64| v.to_le_bytes();
        match source {
            Source::Unknown(i) => buf.extend_from_slice(&f(self.x[i])),
            Source::Zero => buf.resize(buf.len() + len, 0),
            Source::Slot(s) => buf.extend_from_slice(&self.slots[s]),
            Source::Previous(s) => buf.extend_from_slice(&self.previous[s]),
            Source::Held(i) => buf.extend_from_slice(&f(self.held[i].value)),
            Source::Last(i) => buf.extend_from_slice(&f(self.held[i].last)),
            Source::Age(i) => buf.push(self.held[i].age),
            Source::Ambient(ambient::STATIC) => buf.push(u8::from(self.amb.is_static)),
            Source::Ambient(k) => {
                let v = match k {
                    ambient::TIME => self.amb.time,
                    ambient::FREQ => self.amb.freq,
                    ambient::GMIN => self.amb.gmin,
                    _ => self.amb.source_scale,
                };
                buf.extend_from_slice(&f(v));
            }
        }
    }
}

/// Calls `f` unless its read bytes are those of its last call (F3).
pub(crate) fn run(
    inst: &InstancePlan,
    f: &FnPlan,
    st: &mut FnState,
    ctx: &Reads<'_>,
    scratch: &mut Vec<u8>,
) -> Result<(), Error> {
    scratch.clear();
    for (source, len) in &f.reads {
        ctx.push(*source, *len, scratch);
    }
    if st.valid && st.read == *scratch {
        return Ok(());
    }
    let mut at = 0;
    let input: Vec<&[u8]> = f
        .reads
        .iter()
        .map(|(_, len)| {
            let cell = &scratch[at..at + len];
            at += len;
            cell
        })
        .collect();
    st.jac.fill(0.0);
    let mut output: Vec<&mut [u8]> = st.out.iter_mut().map(Vec::as_mut_slice).collect();
    st.valid = false;
    inst.handle.eval(f.k, &input, &mut output, &f.seed, &mut st.jac).map_err(|e| {
        device_error(format!("instance `{}`: function `{}` ({}): {e}", inst.path, f.name, f.origin))
    })?;
    st.read.clear();
    st.read.extend_from_slice(scratch);
    st.valid = true;
    Ok(())
}

/// A4: every term value, limit iterate and seeded partial a Newton solve
/// reads is finite.
fn check_finite(inst: &InstancePlan, st: &InstanceState) -> Result<(), Error> {
    let fail = |cell: String, origin: &str, v: f64| {
        Err(device_error(format!(
            "instance `{}`: {cell} ({origin}) is not finite: {v} (abi.md A4; LRM §7.3.2.1: an analog \
             expression shall not evaluate to NaN or infinity)",
            inst.path
        )))
    };
    for t in &inst.terms {
        let Some((fi, w)) = t.value else { continue };
        let fs = &st.fns[fi];
        let v = fs.f64(w);
        if !v.is_finite() {
            return fail(format!("term `{}`", t.name), &t.origin, v);
        }
        for p in t.direct.iter().map(|(p, _)| p).chain(t.via.iter().map(|(p, _)| p)) {
            if !fs.jac[*p].is_finite() {
                let f = &inst.functions[fi];
                return fail(format!("partial {p} of term `{}` in function `{}`", t.name, f.name), &t.origin, fs.jac[*p]);
            }
        }
    }
    for x in &inst.inputs {
        for (what, at) in [("PROPOSED", x.proposed), ("LIMITED", x.limited)] {
            let Some((fi, w)) = at else { continue };
            let v = st.fns[fi].f64(w);
            if !v.is_finite() {
                return fail(format!("input `{}`.{what}", x.name), &x.origin, v);
            }
        }
        if let Some((fi, _)) = x.proposed {
            for (p, _) in &x.p {
                let v = st.fns[fi].jac[*p];
                if !v.is_finite() {
                    return fail(format!("partial {p} of input `{}`.PROPOSED", x.name), &x.origin, v);
                }
            }
        }
    }
    Ok(())
}
