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
    /// currents — as one unified `Net` list. A design's discrete nets are the
    /// bound plan's (`bound().names.nets`).
    pub fn nets(&self) -> Vec<crate::core::net::Net> {
        crate::core::net::Net::analog_nets(&self.netlist)
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
            bound.set_temperature(ctx.tolerances.temperature);
        }
        self.is_set_up = true;
        Ok(())
    }

    /// Sets the run temperature (ABI-20): the `TEMPERATURE` global holds `t`
    /// from the next analysis on, and a device that reads it recomputes.
    pub fn set_temperature(&mut self, t: f64) {
        if let Some(bound) = self.abi.as_mut() {
            bound.set_temperature(t);
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
            shared: recipe.shared,
            session: recipe.session,
            netlist: &mut netlist,
            resolution: recipe.resolution,
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
        Self { title: title.into(), netlist, abi: None, recipe: None, is_set_up: false }
    }
}
