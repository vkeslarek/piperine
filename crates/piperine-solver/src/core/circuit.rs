//! `CircuitInstance` — the built, simulatable circuit: owns the bound device
//! instances behind four contracted responsibilities (circuit state, analysis
//! entry, live mutation; construction stays in `CircuitBuilder`).
use crate::analyses::noise::NoiseAnalysisOptions;
use crate::analyses::tf::TransferFunctionAnalysisOptions;
use crate::analyses::transient::TransientAnalysisOptions;
use piperine_analog_solver::error::{Error, SolverDomain};
use piperine_analog_solver::netlist::Netlist;
use crate::analyses::Context;
use crate::analyses::ac::AcSolver;
use crate::analyses::dc::DcSolver;
use crate::analyses::noise::NoiseSolver;
use crate::analyses::tf::TransferFunctionSolver;
use crate::analyses::transient::TransientSolver;


// ---------------------------------------------------------------------------
// CircuitInstance — the instantiated, ready-to-simulate circuit
// ---------------------------------------------------------------------------

/// The instantiated, ready-to-simulate circuit.
///
/// `CircuitInstance` has exactly four jobs (design §6b), and every method
/// below sits under one of them — the impl is grouped into four contracted
/// sections:
///
/// 1. **Circuit state** — read-only views of the built circuit's structure.
/// 2. **Analysis entry** — hand a driver a borrow of the circuit + a
///    [`Context`]; uniform shape, one line each.
/// 3. **Live mutation** — host writes to the bound device instances.
/// 4. **Construction** — stays in [`CircuitBuilder`](crate::core::builder::CircuitBuilder);
///    this type grows no ad-hoc constructor beyond
///    [`from_netlist`](Self::from_netlist) (the
///    builder's output) and documented re-entry.
pub struct CircuitInstance {
    pub title: String,
    pub netlist: Netlist,
    /// The bound device instances (the device contract); `Some` from `build` on.
    pub(crate) abi: Option<crate::core::bind::Bound>,
    /// What the device instances were bound from (a circuit of devices).
    pub(crate) recipe: Option<crate::core::bind::Recipe>,
    /// The name each node was given to `CircuitBuilder::node` (ground excluded).
    pub(crate) node_names: Vec<(String, piperine_analog_solver::netlist::AnalogReference)>,
    is_set_up: bool,
}

impl CircuitInstance {
    // ── Circuit state ────────────────────────────────────────────────────────
    //
    // Read-only views of the built circuit's structure: the analog netlist,
    // the unified net naming layer and the bound device instances.

    pub fn netlist(&self) -> &Netlist { &self.netlist }

    /// The bound device instances, when the circuit was built from devices.
    pub fn bound(&self) -> Option<&crate::core::bind::Bound> { self.abi.as_ref() }
    /// A run error with its unknowns named by the pins and branches of this
    /// circuit; any other error passes through.
    pub(crate) fn name_error(&self, error: piperine_analog_solver::error::Error) -> piperine_analog_solver::error::Error {
        match &self.abi {
            Some(bound) => bound.name(error),
            None => error,
        }
    }
    pub fn bound_mut(&mut self) -> Option<&mut crate::core::bind::Bound> { self.abi.as_mut() }

    /// The values the device instance at `path` is created with, by param
    /// name: what `CircuitBuilder::device` was given (or a later structural
    /// `set` wrote), which bind hands to the module's `create`. A param absent
    /// here keeps its prototype's default. `None` when no instance has `path`.
    pub fn given(&self, path: &str) -> Option<&[(String, crate::core::bind::ParamValue)]> {
        let recipe = self.recipe.as_ref()?;
        recipe.specs.iter().find(|s| s.path == path).map(|s| s.values.as_slice())
    }

    /// Every solved signal in the circuit — analog nodes and analog branch
    /// currents — as one unified `Net` list. A node given a name to
    /// `CircuitBuilder::node` is labelled `v(<name>)`. A design's discrete nets
    /// are the bound plan's (`bound().names.nets`).
    pub fn nets(&self) -> Vec<crate::core::net::Net> {
        let mut nets = crate::core::net::Net::analog_nets(&self.netlist);
        for net in &mut nets {
            let named = self.node_names.iter().find(|(_, r)| net.analog_variable() == Some(r.variable()));
            if let Some((name, _)) = named {
                net.relabel(format!("v({name})"));
            }
        }
        nets
    }

    /// The node given `name` to `CircuitBuilder::node`; `None` for a name the
    /// builder was not given (ground's names included).
    pub fn node(&self, name: &str) -> Option<&piperine_analog_solver::netlist::AnalogReference> {
        self.node_names.iter().find(|(n, _)| n == name).map(|(_, r)| r)
    }

    // ── Analysis entry ───────────────────────────────────────────────────────
    //
    // Hand a driver a borrow of the circuit plus a [`Context`]. Uniform shape,
    // one line each — the analysis itself lives in its driver under
    // `crate::analyses`.

    pub fn dc(&mut self, context: Context) -> crate::core::result::Result<DcSolver<'_>> {
        DcSolver::new(self, context)
    }

    pub fn ac(&mut self, context: Context) -> crate::core::result::Result<AcSolver<'_>> {
        AcSolver::new(self, context)
    }

    pub fn transient(
        &mut self,
        transient_options: TransientAnalysisOptions,
        context: Context,
    ) -> crate::core::result::Result<TransientSolver<'_>> {
        TransientSolver::new(self, transient_options, context)
    }

    pub fn noise(
        &mut self,
        options: NoiseAnalysisOptions,
        context: Context,
    ) -> crate::core::result::Result<NoiseSolver<'_>> {
        NoiseSolver::new(self, options, context)
    }

    pub fn transfer_function(
        &mut self,
        options: TransferFunctionAnalysisOptions,
        context: Context,
    ) -> crate::core::result::Result<TransferFunctionSolver<'_>> {
        TransferFunctionSolver::new(self, options, context)
    }

    /// DC sensitivity analysis (`.sens`): `∂(output)/∂(param)` at the
    /// operating point over the restamp path — see
    /// `SensSolver`.
    pub fn sens(
        &mut self,
        options: crate::analyses::sens::SensAnalysisOptions,
        context: Context,
    ) -> crate::core::result::Result<crate::analyses::sens::SensSolver<'_>> {
        crate::analyses::sens::SensSolver::new(self, options, context)
    }

    /// DC adjoint sensitivity (dv-gradients DVG-08): `∂(output)/∂(param)`
    /// at the operating point from **one** transpose solve of `Jᵀλ = ∂f/∂x`,
    /// combined with each parameter's `∂F/∂p` — see [`AdjointSolver`](crate::analyses::adjoint::AdjointSolver).
    pub fn adjoint(
        &mut self,
        options: crate::analyses::adjoint::AdjointOptions,
        context: Context,
    ) -> crate::core::result::Result<crate::analyses::adjoint::AdjointSolver<'_>> {
        crate::analyses::adjoint::AdjointSolver::new(self, options, context)
    }

    /// Periodic steady state via single shooting — see
    /// `PssSolver`.
    pub fn pss(
        &mut self,
        options: crate::analyses::pss::PssAnalysisOptions,
        context: Context,
    ) -> crate::core::result::Result<crate::analyses::pss::PssSolver<'_>> {
        crate::analyses::pss::PssSolver::new(self, options, context)
    }

    /// Pole-zero analysis (`.pz`): poles/zeros of the linearized
    /// input→output transfer function — see
    /// [`PoleZeroSolver`](crate::analyses::pz::PoleZeroSolver).
    pub fn pz(
        &mut self,
        options: crate::analyses::pz::PoleZeroOptions,
        context: Context,
    ) -> crate::core::result::Result<crate::analyses::pz::PoleZeroSolver<'_>> {
        crate::analyses::pz::PoleZeroSolver::new(self, options, context)
    }

    /// N-port S-parameter analysis (`.sp`): power-wave scattering matrix
    /// over a frequency sweep — see [`SpSolver`](crate::analyses::sp::SpSolver).
    pub fn sp(
        &mut self,
        options: crate::analyses::sp::SpOptions,
        context: Context,
    ) -> crate::core::result::Result<crate::analyses::sp::SpSolver<'_>> {
        crate::analyses::sp::SpSolver::new(self, options, context)
    }

    // ── Live mutation ────────────────────────────────────────────────────────
    //
    // Host writes to the bound device instances, and the per-run temperature.

    pub(crate) fn setup_all(&mut self, ctx: &Context) -> crate::core::result::Result<()> {
        if self.is_set_up { return Ok(()); }
        if let Some(bound) = self.abi.as_mut() {
            bound.set_temperature(ctx.tolerances.temperature, ctx.tolerances.temperature_given);
        }
        self.is_set_up = true;
        Ok(())
    }

    /// Sets the run temperature (ABI-20): the `TEMPERATURE` global holds `t`
    /// from the next analysis on, `TEMPERATURE_GIVEN` says the run set it, and a
    /// device that reads them recomputes.
    pub fn set_temperature(&mut self, t: f64) {
        if let Some(bound) = self.abi.as_mut() {
            bound.set_temperature(t, true);
        }
    }

    /// Sets `param` of the device instance at `path` to `value` between
    /// analyses (abi.md §10 "Host actions", P5, P6): the parameter cone runs on
    /// a shadow first. A false `check` refuses the write and nothing changes; a
    /// `RESHAPE`, or a `STRUCTURAL` param, re-creates and re-binds the instances
    /// with the new value (every param keeps its value); otherwise the write is
    /// one of the host's initial writes of every later analysis, and the
    /// analog copy reads it at once.
    pub fn set(
        &mut self,
        path: &str,
        param: &str,
        value: crate::core::bind::ParamValue,
    ) -> crate::core::result::Result<()> {
        use piperine_digital_solver::engine::Shadow;
        let bound = self.abi.as_mut().ok_or_else(|| {
            Error::simple(SolverDomain::Device, "`set` addresses device instances; this circuit has none")
        })?;
        let (net, bytes) = bound.param_bytes(path, param, &value)?;
        let shadow = bound
            .digital
            .shadow(net, 0, &bytes)
            .map_err(|e| Error::simple(SolverDomain::Digital, e.to_string()))?;
        let structural = bound.digital.plan().nets[net.0 as usize].structural;
        match shadow {
            Shadow::Refused { check } => {
                return Err(Error::simple(
                    SolverDomain::Device,
                    format!("`{path}.{param}`: the write is refused, check {check} is false"),
                ));
            }
            Shadow::Accept if !structural => {
                bound.host_writes.retain(|(n, _)| *n != net);
                if let Some(slot) = bound.names.slots.get(&net) {
                    bound.analog.set_discrete(*slot, &bytes);
                }
                // The idle engine holds the value too, so the next write's `where` reads it.
                if bound.digital.plan().nets[net.0 as usize].param {
                    bound.digital.seed_param(net, &bytes);
                }
                bound.host_writes.push((net, bytes));
            }
            Shadow::Accept | Shadow::Reshape { .. } => {}
        }
        let recipe = self.recipe.as_mut().expect("a circuit of devices keeps its recipe");
        let spec = recipe.specs.iter_mut().find(|s| s.path == path).expect("the instance was bound");
        spec.values.retain(|(n, _)| n != param);
        spec.values.push((param.to_string(), value));
        if structural || !matches!(shadow, Shadow::Accept) {
            self.rebind()?;
        }
        Ok(())
    }

    /// Whether a `set` of `param` of the instance at `path` to `value` would be accepted during a
    /// transient (HII-13), changing nothing: the value must fit the param's layout and pass its
    /// `where`, and the write must not need another shape. The refusal is the error.
    pub fn check_set(&self, path: &str, param: &str, value: &crate::core::bind::ParamValue) -> crate::core::result::Result<()> {
        use piperine_digital_solver::engine::Shadow;
        let bound = self.abi.as_ref().ok_or_else(|| {
            Error::simple(SolverDomain::Device, "`check_set` addresses device instances; this circuit has none")
        })?;
        let (net, bytes) = bound.param_bytes(path, param, value)?;
        match bound.digital.shadow(net, 0, &bytes).map_err(|e| Error::simple(SolverDomain::Digital, e.to_string()))? {
            Shadow::Accept => Ok(()),
            Shadow::Refused { check } => {
                Err(Error::simple(SolverDomain::Device, format!("`{path}.{param}`: the write is refused, check {check} is false")))
            }
            Shadow::Reshape { instance } => Err(Error::simple(
                SolverDomain::Device,
                format!("`{path}.{param}`: the write would need instance {instance} in another shape"),
            )),
        }
    }

    /// Whether the host has written the discrete net `net` (its name in the digital plan) before
    /// the run, so that the write is the net's initial value; `false` for a name that is no net.
    pub fn net_written(&self, net: &str) -> bool {
        let Some(bound) = self.abi.as_ref() else { return false };
        let Some(at) = bound.digital.plan().nets.iter().position(|n| n.name == net) else { return false };
        bound.host_writes.iter().any(|(id, _)| id.0 as usize == at)
    }

    /// Writes `value` to the discrete net `net` (its name in the digital plan) between analyses
    /// (abi.md §10 "Host actions"): the net's shadow checks it first, a refusal or a `RESHAPE`
    /// changes nothing, and an accepted write is one of the host's initial writes of every later
    /// analysis, which the analog copy reads at once. The value is encoded in the net's layout,
    /// its value bytes only.
    pub fn write_net(&mut self, net: &str, value: crate::core::bind::ParamValue) -> crate::core::result::Result<()> {
        use piperine_digital_solver::engine::Shadow;
        let bound = self.abi.as_mut().ok_or_else(|| {
            Error::simple(SolverDomain::Device, "`write_net` addresses device instances; this circuit has none")
        })?;
        let (id, bytes) = bound.net_bytes(net, &value)?;
        let shadow = bound
            .digital
            .shadow(id, 0, &bytes)
            .map_err(|e| Error::simple(SolverDomain::Digital, e.to_string()))?;
        match shadow {
            Shadow::Accept => {
                bound.host_writes.retain(|(n, _)| *n != id);
                if let Some(slot) = bound.names.slots.get(&id) {
                    bound.analog.set_discrete(*slot, &bytes);
                }
                bound.host_writes.push((id, bytes));
                Ok(())
            }
            Shadow::Refused { check } => {
                Err(Error::simple(SolverDomain::Device, format!("`{net}`: the write is refused, check {check} is false")))
            }
            Shadow::Reshape { instance } => Err(Error::simple(
                SolverDomain::Device,
                format!("`{net}`: the write would need instance {instance} in another shape"),
            )),
        }
    }

    /// Destroys every device instance and creates and binds them again from
    /// the recipe, over the circuit's own nodes.
    fn rebind(&mut self) -> crate::core::result::Result<()> {
        use piperine_analog_solver::netlist::AnalogVariable;
        let recipe = self.recipe.clone().expect("a circuit of devices keeps its recipe");
        // The nodes keep their indices: the builder allocated them before bind.
        let mut nodes: Vec<_> = self
            .netlist
            .all_references()
            .into_iter()
            .filter(|r| r.variable().is_node())
            .cloned()
            .collect();
        nodes.sort_by_key(|r| r.idx());
        let mut netlist = Netlist::new();
        for node in &nodes {
            let AnalogVariable::Node(id) = node.variable().as_ref() else { unreachable!("filtered to nodes") };
            if netlist.connect_node(id.clone()).idx() != node.idx() {
                return Err(Error::simple(SolverDomain::Device, "a node was allocated after bind; cannot bind again"));
            }
        }
        self.abi = None;
        let bound = crate::core::bind::bind(crate::core::bind::BindInput {
            specs: recipe.specs,
            arrays: recipe.arrays,
            shared: recipe.shared,
            session: recipe.session,
            netlist: &mut netlist,
            resolution: recipe.resolution,
            microsteps: recipe.microsteps,
            time_point_rule: recipe.time_point_rule,
            temperature: recipe.temperature,
        })?;
        self.netlist = netlist;
        self.abi = Some(bound);
        Ok(())
    }

    // ── Construction ─────────────────────────────────────────────────────────
    //
    // Construction stays in [`CircuitBuilder`](crate::core::builder::CircuitBuilder):
    // devices are built and wired there, and this type grows no ad-hoc
    // constructor beyond the builder's output below and documented re-entry
    // (analyses re-enter solve state via e.g.
    // [`TransientSolver::with_initial_state`](crate::analyses::transient::TransientSolver::with_initial_state);
    // the MD-18 restamp path re-enters via [`set`](Self::set)
    // + a re-run of the analysis — never via a new constructor).

    /// Build a `CircuitInstance` over a netlist; the builder binds the device
    /// instances into it.
    pub(crate) fn from_netlist(title: impl Into<String>, netlist: Netlist) -> Self {
        Self { title: title.into(), netlist, abi: None, recipe: None, node_names: Vec::new(), is_set_up: false }
    }
}
