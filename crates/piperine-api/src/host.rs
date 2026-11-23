//! Host actions of a session (HII-12 to HII-14, X8, Q2 (a)): the host drives a top's discrete
//! `in` ports, sets a param at a time and narrows the record streams, from the library only.
//! Each action is validated when it is given, so a refusal changes nothing (`E0801` to `E0806`,
//! `docs/spec/part_viii_host.md` §23.1). An action at `at = 0` is applied at once, before the
//! analysis, in the order given; one at `at > 0` waits for the transient, which lands on its
//! time (`TransientSolver::schedule_set` and `schedule_write`).

use piperine_ir::{CarrierRole, ValueType};
use piperine_solver::abi::Value;
use piperine_solver::prelude::ParamValue;

use piperine_solver::prelude::CircuitInstance;

use crate::params::{Changed, Derive, Params};

use crate::failure::Failure;
use crate::request::Analysis;
use crate::session::{ParamPath, ParamTarget, Session};

/// A `set` the host scheduled at `at > 0`, as it gave it.
#[derive(Debug, Clone)]
pub(crate) struct TimedSet {
    pub path: ParamPath,
    pub value: f64,
    pub at: f64,
}

/// One param a timed `set` changes at `at`: the set's own, or one derived from it. `written` is
/// whether a device pin holds the param (a pass-through param only the table knows is not).
#[derive(Debug, Clone)]
pub(crate) struct TimedChange {
    pub change: Changed,
    pub at: f64,
    pub written: bool,
}

impl TimedChange {
    /// The write the transient performs after the step to `at`.
    pub fn action(&self) -> HostAction {
        HostAction::Set { path: ParamPath { instance: self.change.instance.clone(), param: self.change.param.clone() }, value: self.change.scheduled(), at: self.at }
    }
}

/// A host action due after the start of a transient.
#[derive(Debug, Clone)]
pub(crate) enum HostAction {
    /// The discrete net `net` of the plan takes `value`.
    Drive { net: String, value: ParamValue, at: f64 },
    /// The param takes `value`.
    Set { path: ParamPath, value: Value, at: f64 },
}

impl Session {
    /// Drives the discrete `in` port `port` of the top (as written or relative to the top) with
    /// `value` at time `at`: before the run (`at = 0`) the port's initial value, later a commit
    /// at microstep 0 of `at`, so a reaction on its change fires then. A port never driven stays
    /// unknown. `E0801` for a path that names nothing, `E0804` for a carrier that is not a
    /// discrete `in` port of the top, `E0802` for a value that is not the port's type, `E0806`
    /// for a negative `at` or one past the transient's stop; each leaves the session as it was.
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

    /// Sets the param `path` (`<instance>.<param>`) to `value` at time `at`: before the run
    /// (`at = 0`) as a `set` before the analysis, later after the step to `at` is accepted, so a
    /// node its behavior reads keeps its old value at `at`. `E0801` for a path that names no
    /// param, `E0803` for a structural param (at any `at`: the body is elaborated), `E0802` for a value its `where`
    /// refuses, `E0806` for a negative `at` or one past the transient's stop.
    pub fn set_at(&mut self, path: &str, value: f64, at: f64) -> Result<(), Failure> {
        let action = format!("the set of `{path}`");
        Session::not_before(at, &action)?;
        let target = ParamTarget::resolve(self.ir(), self.ir().top().path(), path)?;
        let param = target.param.clone();
        param.refuse_structural(self.ir())?;
        self.ready_for(at, &action)?;
        if let Some(leaf) = &target.leaf {
            // a leaf of a composite param is written whole, before the run: a timed write of one leaf
            // would need the schedule to hold the bytes of the rest
            if at > 0.0 {
                return Err(Failure::Host { code: "E0806", message: format!("{action} is scheduled at {at:e} s: a leaf of a composite param is set before the run (at = 0)") });
            }
            let circuit = self.circuit.as_mut().ok_or_else(Session::ran)?;
            return Session::set_leaf(&mut self.params, circuit, &param, leaf, value);
        }
        if at > 0.0 {
            // the values a timed set derives depend on every other set due before it, whatever the
            // order the host gave them: the whole schedule is derived again with this one in it,
            // and the call is refused when any of it is
            self.timed_sets.push(TimedSet { path: param, value, at });
            let circuit = self.circuit.as_ref().ok_or_else(Session::ran)?;
            let derive = self.compiled.as_ref().map(|c| c as &dyn Derive);
            let checked = Session::timed_changes(&self.params, derive, circuit, &self.timed_sets);
            if let Err(refused) = checked {
                self.timed_sets.pop();
                return Err(refused);
            }
            return Ok(());
        }
        let derive = self.compiled.as_ref().map(|c| c as &dyn Derive);
        let circuit = self.circuit.as_mut().ok_or_else(Session::ran)?;
        Session::set_param(&mut self.params, derive, circuit, &param, value)
    }

    /// Sets the param `path` to `text`, read as its type reads it (PI-01 to PI-03, D24), before the run: an
    /// integer literal of the type and range, the name of a variant of a unit enum, `None` or a value of
    /// `T` for an `Option<T>`. `E0801` for a path that names no param, `E0803` for a structural param,
    /// `E0802` for a text that is no value of its type, naming the param and listing an enum's variants.
    pub fn set_text(&mut self, path: &str, text: &str) -> Result<(), Failure> {
        let action = format!("the set of `{path}`");
        let param = ParamPath::resolve(self.ir(), self.ir().top().path(), path)?;
        param.refuse_structural(self.ir())?;
        self.ready_for(0.0, &action)?;
        let ir = self.ir.clone();
        let derive = self.compiled.as_ref().map(|c| c as &dyn Derive);
        let circuit = self.circuit.as_mut().ok_or_else(Session::ran)?;
        Session::set_text_param(&mut self.params, derive, circuit, &ir, &param, text)
    }

    /// What every timed `set` of `sets` changes, in time order (the order given between equal
    /// times): each is derived against `table` as it is at its time, the sets due before it
    /// applied, and each changed param is checked against the param's `where` on `circuit`
    /// (`E0802`). The first set's own param comes first among its changes.
    pub(crate) fn timed_changes(table: &Params, derive: Option<&dyn Derive>, circuit: &CircuitInstance, sets: &[TimedSet]) -> Result<Vec<TimedChange>, Failure> {
        let mut ordered: Vec<&TimedSet> = sets.iter().collect();
        ordered.sort_by(|a, b| a.at.total_cmp(&b.at));
        let mut working = table.clone();
        let mut scheduled = Vec::new();
        for set in ordered {
            let changes = working.changes(&set.path.instance, &set.path.param, set.value, derive)?;
            let forwarded = !working.plan().readers(&set.path.instance, &set.path.param).is_empty();
            for change in &changes {
                let written = !Session::unwritten(circuit, change, forwarded);
                if written {
                    let refused = |error| Failure::Host { code: "E0802", message: format!("`{}` refuses the value {}: {error}", change.label(), change.value) };
                    circuit.check_set(&change.instance, &change.param, &change.param_value()).map_err(refused)?;
                }
                scheduled.push(TimedChange { change: change.clone(), at: set.at, written });
            }
            working.apply(&changes);
        }
        Ok(scheduled)
    }

    /// Narrows the session's record streams to those `streams` name (a stream's name, or one
    /// stream's full path), before the run: only their records reach the sink. `E0801` for a
    /// name that matches no stream, `E0805` once the session has run.
    pub fn subscribe(&mut self, streams: &[&str]) -> Result<(), Failure> {
        if self.circuit.is_none() {
            return Err(Failure::Host { code: "E0805", message: format!("the session has run: it cannot subscribe to {} now", streams.join(", ")) });
        }
        let entries: Vec<String> = streams.iter().map(|s| s.to_string()).collect();
        self.streams.keep(&entries)
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

    /// An action at `at > 0` waits for a transient to land on it, at a time up to the run's stop
    /// (`E0806` past it: the run never reaches it); any other analysis has no such time, and the
    /// session has not run.
    fn ready_for(&self, at: f64, action: &str) -> Result<(), Failure> {
        if self.circuit.is_none() {
            return Err(Session::ran());
        }
        match &self.spec().analysis {
            Analysis::Transient { stop, .. } if at > *stop => Err(Failure::Host { code: "E0806", message: format!("{action} is scheduled at {at:e} s, after the run's stop at {stop:e} s: the run never reaches it") }),
            Analysis::Transient { .. } => Ok(()),
            other if at > 0.0 => Err(Failure::Host { code: "P0010", message: format!("{action} is scheduled at {at:e} s: only a `transient` run has a time to land on, this run is `{}`", other.name()) }),
            _ => Ok(()),
        }
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
