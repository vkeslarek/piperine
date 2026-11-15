//! The operating point of abi.md §10 over the two sides: start the discrete
//! side, solve the equilibrium, run the instant; while the instant changed a
//! value the analog side reads, deliver it and solve again (a round, bounded:
//! `E0506` names the nets still changing); then fire `SETTLE` once.

use piperine_analog_solver::error::{Error, Result, SolverDomain};

use super::sides::{AnalogSide, AnalysisKind, Boundary, DiscreteSide, InstantMode, Restart};
use super::transient::{Ended, Protocol};

/// The `E0506` error: the rounds of an operating point exceeded `max`.
fn too_many_rounds(max: u32, changing: &[Boundary]) -> Error {
    let slots: Vec<String> = changing.iter().map(|b| b.slot.to_string()).collect();
    Error::simple(
        SolverDomain::Dc,
        format!(
            "E0506: the operating point did not settle in {max} rounds; nets still changing \
             (boundary slots): {}",
            slots.join(", ")
        ),
    )
}

impl<D: DiscreteSide, A: AnalogSide> Protocol<D, A> {
    /// Solves the operating point at the current time (abi.md §10, steps 1 to 4).
    /// A reaction that returns `FINISH` there ends the run: the result is
    /// `Ended::Finish` and a later `transient` does nothing.
    pub fn operating_point(&mut self) -> Result<Ended> {
        let at = self.tick(self.now);
        let analysis = AnalysisKind::OperatingPoint;
        let unknowns = self.analog.has_unknowns();
        self.discrete.start()?;
        self.started = true;
        self.collect(analysis);
        let mut rounds = 0u32;
        let mut finish = false;
        let mut samples = Vec::new();
        let quiet = InstantMode { is_static: true, settle: false };
        loop {
            if unknowns {
                rounds += 1;
                let accepted = self.analog.equilibrium()?;
                let analog = &mut self.analog;
                self.delivery.deliver_now(analysis, |out| analog.drain_traces(out));
                for net in &accepted.fired {
                    self.discrete.fire(*net, at)?;
                }
                samples = accepted.samples;
            }
            let outcome = self.run_whole_instant(at, &samples, at, quiet)?;
            finish |= outcome.finish;
            self.collect(analysis);
            if !unknowns || outcome.boundary.is_empty() || finish {
                break;
            }
            if rounds >= self.options.max_rounds {
                return Err(too_many_rounds(self.options.max_rounds, &outcome.boundary));
            }
            self.analog.deliver(&outcome.boundary, Restart::None);
        }
        let settled = self.run_whole_instant(at, &samples, at, InstantMode { is_static: true, settle: true })?;
        if unknowns && !settled.boundary.is_empty() {
            // A settle commit is read by the solve that arrives next (abi.md §10 step 4).
            self.analog.deliver(&settled.boundary, Restart::None);
        }
        finish |= settled.finish;
        self.samples = samples;
        self.collect(analysis);
        self.delivery.flush();
        if finish {
            self.finished = Some(self.now);
            return Ok(Ended::Finish(self.now));
        }
        Ok(Ended::StopTime)
    }
}
