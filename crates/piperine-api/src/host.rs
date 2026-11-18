//! Host actions of a session (HII-12 to HII-14, X8, Q2 (a)): the host drives a top's discrete
//! `in` ports, sets a param at a time and narrows the record streams, from the library only.
//! Each action is validated when it is given, so a refusal changes nothing (`E0801` to `E0806`,
//! `docs/spec/part_viii_host.md` §23.1). An action at `at = 0` is applied at once, before the
//! analysis, in the order given; one at `at > 0` waits for the transient, which lands on its
//! time (`TransientSolver::schedule_set` and `schedule_write`).

use piperine_ir::{CarrierRole, ValueType};
use piperine_solver::prelude::ParamValue;

use crate::failure::Failure;
use crate::request::Analysis;
use crate::session::Session;

/// A host action due after the start of a transient.
#[derive(Debug, Clone)]
pub(crate) enum HostAction {
    /// The discrete net `net` of the plan takes `value`.
    Drive { net: String, value: ParamValue, at: f64 },
}

impl Session {
    /// Drives the discrete `in` port `port` of the top (as written or relative to the top) with
    /// `value` at time `at`: before the run (`at = 0`) the port's initial value, later a commit
    /// at microstep 0 of `at`, so a reaction on its change fires then. A port never driven stays
    /// unknown. `E0801` for a path that names nothing, `E0804` for a carrier that is not a
    /// discrete `in` port of the top, `E0802` for a value that is not the port's type, `E0806`
    /// for a negative `at`; each leaves the session as it was.
    pub fn drive(&mut self, port: &str, value: ParamValue, at: f64) -> Result<(), Failure> {
        Session::not_before(at, &format!("the drive of `{port}`"))?;
        let (net, ty) = self.top_input(port)?;
        let fits = match (&ty, &value) {
            (ValueType::F64, ParamValue::Real(_)) | (ValueType::Bool, ParamValue::Bool(_)) => true,
            (ty, ParamValue::Int(n)) => ty.integer_range().is_some_and(|(low, high)| (low..=high).contains(&i128::from(*n))),
            _ => false,
        };
        if !fits {
            return Err(Failure::Host { code: "E0802", message: format!("`{port}` is a port of type {ty}: the value {value:?} is not of that type") });
        }
        self.ready_for(at, &format!("the drive of `{port}`"))?;
        if at > 0.0 {
            self.actions.push(HostAction::Drive { net, value, at });
            return Ok(());
        }
        let circuit = self.circuit.as_mut().ok_or_else(Session::ran)?;
        circuit.write_net(&net, value).map_err(|error| Failure::Backend(format!("the drive of `{port}` is refused: {error}")))
    }

    fn ran() -> Failure {
        Failure::Backend("this session has already run: open another".into())
    }

    /// `E0806` for an `at` before the start of a run.
    fn not_before(at: f64, action: &str) -> Result<(), Failure> {
        if at >= 0.0 {
            return Ok(());
        }
        Err(Failure::Host { code: "E0806", message: format!("{action} is scheduled at {at:e} s, a time already past") })
    }

    /// An action at `at > 0` waits for a transient to land on it; any other analysis has no such
    /// time, and the session has not run.
    fn ready_for(&self, at: f64, action: &str) -> Result<(), Failure> {
        if self.circuit.is_none() {
            return Err(Session::ran());
        }
        if at > 0.0 && !matches!(self.spec().analysis, Analysis::Transient { .. }) {
            return Err(Failure::Host { code: "P0010", message: format!("{action} is scheduled at {at:e} s: only a `transient` run has a time to land on, this run is `{}`", self.spec().analysis.name()) });
        }
        Ok(())
    }

    /// The plan's net and the type of the discrete `in` port of the top `port` names.
    fn top_input(&self, port: &str) -> Result<(String, ValueType), Failure> {
        let ir = self.ir();
        let top = ir.top().path();
        let short = port.strip_prefix(&format!("{top}.")).unwrap_or(port);
        let carrier = ir.top().body().carriers().iter().find(|c| c.name == short);
        match carrier {
            Some(c) if c.role == CarrierRole::In => {
                let path = format!("{top}.{short}");
                let net = ir.net_at(&path).ok_or_else(|| Failure::Backend(format!("the top's port `{short}` has no net")))?;
                Ok((path, net.ty().clone()))
            }
            Some(c) => Err(Failure::Host { code: "E0804", message: format!("`{port}` is {} of the top, not an `in` port: the host holds no drive of it", if c.role == CarrierRole::Out { "an `out` port" } else { "a signal" }) }),
            None if [port.to_string(), format!("{top}.{port}")].iter().any(|p| ir.net_at(p).is_some() || ir.node_at(p).is_some()) => {
                Err(Failure::Host { code: "E0804", message: format!("`{port}` is not a discrete `in` port of the top `{top}`: the host holds no drive of it") })
            }
            None => Err(Failure::Host { code: "E0801", message: format!("`{port}` names no carrier of the design") }),
        }
    }
}
