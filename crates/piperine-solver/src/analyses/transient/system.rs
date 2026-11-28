//! The nonlinear system of one transient stage: the device assembly, the solver's shunts and the UIC hold clamps.

use ndarray::ArrayView1;
use piperine_analog_solver::device::{Ambient, Stage};
use piperine_analog_solver::math::circular_array::CircularArrayBuffer2;
use piperine_analog_solver::math::integration::TrBdf2Phase;
use piperine_analog_solver::math::linear::Stamp;
use piperine_analog_solver::math::newton_raphson::NonLinearSystem;
use piperine_analog_solver::device::JumpPosition;
use piperine_analog_solver::netlist::AnalogReference;

use crate::analyses::Context;
use crate::core::circuit::CircuitInstance;

// UIC hold clamps (ngspice `CKTsetIC` analog): an `@initial` branch seed
// is enforced during the t=0 solve and held through the first accepted
// step by a large conductance across the seeded branch carrying `G·ic` —
// the seed value becomes the *consistent* t=0 solution (the rest of the
// circuit solves against it), not just a Newton guess overlaid on an
// inconsistent operating point.

/// One seeded branch clamp: hold `V(plus) − V(minus) ≈ ic`.
#[derive(Debug, Clone)]
pub struct UicClamp {
    pub plus: AnalogReference,
    pub minus: Option<AnalogReference>,
    pub ic: f64,
}

impl UicClamp {
    /// Clamp conductance: large enough to pin the branch against any circuit
    /// admittance, small enough to keep the matrix conditioned.
    pub const G: f64 = 1.0e12;

    /// Stamp `G·(v − ic)`: conductance across the branch plus the `G·ic`
    /// offset current that pins the branch voltage to `ic`.
    pub fn stamp(&self, stamps: &mut Vec<Stamp<AnalogReference, f64>>) {
        stamps.push(Stamp::Matrix(self.plus.clone(), self.plus.clone(), Self::G));
        stamps.push(Stamp::Rhs(self.plus.clone(), Self::G * self.ic));
        if let Some(minus) = &self.minus {
            stamps.push(Stamp::Matrix(minus.clone(), minus.clone(), Self::G));
            stamps.push(Stamp::Matrix(self.plus.clone(), minus.clone(), -Self::G));
            stamps.push(Stamp::Matrix(minus.clone(), self.plus.clone(), -Self::G));
            stamps.push(Stamp::Rhs(minus.clone(), -Self::G * self.ic));
        }
    }
}

/// The nonlinear system of one transient stage: the circuit, the shared context, the stage's time
/// and TR-BDF2 phase, and the UIC hold clamps.
pub struct TransientSystem<'a> {
    pub circuit: &'a mut CircuitInstance,
    pub context: Context,
    /// Absolute time at the point this phase solves (t_{n+γ} for the TR stage,
    /// t_{n+1} for the BDF2 stage).
    pub time: f64,
    /// Which TR-BDF2 sub-step the next assemble stamps.
    pub phase: TrBdf2Phase,
    /// Full step size `h = t_{n+1} − t_n`.
    pub h: f64,
    /// Last accepted step size, so the TR stage can re-derive the previous
    /// capacitor current. Updated after each accepted BDF2 phase.
    pub prev_h: f64,
    /// Where the next step stands relative to a jump (§15.2.6): set by `device_attempt`.
    pub jump: JumpPosition,
    /// Whether the last accepted step read a jump: the step after it is `JumpPosition::After`.
    /// Time-loop state, kept by `AcceptUndo` and `AnalogResume`.
    pub last_read_jump: bool,
    pub tfinal: f64,
    /// UIC hold clamps (ngspice `CKTsetIC`): `@initial` branch seeds pinned
    /// through the t=0 solve and the first accepted step.
    pub uic_clamps: Vec<UicClamp>,
    /// While true, the clamps stamp — released after the first accepted step.
    pub uic_hold: bool,
    /// An error a Newton callback met (they return nothing): the driver
    /// takes it after the solve that raised it.
    pub(super) callback_error: Option<piperine_analog_solver::error::Error>,
}

impl<'a> NonLinearSystem<AnalogReference, f64> for TransientSystem<'a> {
    fn assemble(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
    ) -> crate::core::result::Result<Vec<Stamp<AnalogReference, f64>>> {
        let mut stamps = Vec::new();
        self.assemble_device(state, &mut stamps)?;
        Ok(stamps)
    }

    fn assemble_into(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        out: &mut Vec<Stamp<AnalogReference, f64>>,
    ) -> crate::core::result::Result<()> {
        self.assemble_device(state, out)
    }

    fn netlist(&self) -> &piperine_analog_solver::netlist::Netlist {
        self.circuit.netlist()
    }

    fn any_limiting_report(&self) -> bool {
        !self.circuit.abi.analog.limiting_active().is_empty()
    }

    /// Every Newton iteration but the first ends here (abi.md A2).
    fn before_iter_callback(&mut self, _state: &CircularArrayBuffer2<f64>, iteration_number: usize) {
        if iteration_number > 0 {
            self.circuit.abi.analog.end_iteration();
        }
    }

    fn update_sources(&mut self, _state: &mut CircularArrayBuffer2<f64>) {}

    /// A converged stage gains a row of the Newton buffer: the assembly's term
    /// histories gain theirs, at the same point and in the same call.
    fn convergence_success_callback(
        &mut self,
        _state: &CircularArrayBuffer2<f64>,
        converged: &ArrayView1<f64>,
    ) {
        let amb = self.ambient();
        let bound = &mut self.circuit.abi;
        bound.analog.end_iteration();
        if let Err(e) = bound.analog.push_stage(&converged.to_vec(), amb) {
            self.callback_error.get_or_insert(e);
        }
    }
}

impl TransientSystem<'_> {
    /// The coordinates of the stage being solved (abi.md §4.2).
    pub(super) fn ambient(&self) -> Ambient {
        Ambient {
            time: self.time,
            freq: 0.0,
            is_static: false,
            gmin: self.context.tolerances.gmin,
            source_scale: 1.0,
        }
    }

    /// Fills `out` with the stamps of one stage of a circuit of devices: `FLOW` terms in their
    /// Norton form, `DDT` terms through the TR-BDF2 companion over their own
    /// history, `DELAY` terms from theirs, then the solver's shunts and clamps.
    pub(super) fn assemble_device(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        out: &mut Vec<Stamp<AnalogReference, f64>>,
    ) -> crate::core::result::Result<()> {
        let owned;
        let x: &[f64] = match state.latest().and_then(|row| row.to_slice()) {
            Some(row) => row,
            None => {
                owned = state.latest().map_or_else(|| vec![0.0; state.size()], |row| row.to_vec());
                &owned
            }
        };
        let stage = Stage { phase: self.phase, h: self.h, prev_h: self.prev_h, jump: self.jump };
        let amb = self.ambient();
        let bound = &mut self.circuit.abi;
        bound.analog.assemble_tran_into(x, amb, stage, out)?;
        if let Some(err) = piperine_analog_solver::stamp_validation::StampValidation::non_finite_error("devices", out) {
            return Err(err);
        }
        let gshunt = self.context.tolerances.gshunt;
        if gshunt > 0.0 {
            for r in self.circuit.netlist().all_references() {
                if r.variable().is_node() && !r.variable().is_ground() {
                    out.push(Stamp::Matrix(r.clone(), r.clone(), gshunt));
                }
            }
        }
        if self.uic_hold {
            for clamp in &self.uic_clamps {
                clamp.stamp(out);
            }
        }
        Ok(())
    }

    /// The error a callback met during the solve just run, if any.
    pub(super) fn take_callback_error(&mut self) -> crate::core::result::Result<()> {
        self.callback_error.take().map_or(Ok(()), Err)
    }
}
