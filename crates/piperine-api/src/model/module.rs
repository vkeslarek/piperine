//! [`Module`] — a navigable view of one named module in a shared
//! [`Design`](crate::model::Design) (CLA-17): the reflected children
//! (ports/nets/instances/params/behaviors), the analysis menu, staging
//! (`set`), and `compile` into a live [`Session`].
//!
//! The view stores `(Rc<Design>, name)` and re-resolves the module on each
//! call, so no accessor holds a POM borrow open for the view's lifetime.
//!
//! Staged overrides live in an isolated map and replay onto a fresh
//! [`SessionBuilder`] fork per analysis call — the held parent design is
//! never mutated, and no state carries between analyses. A re-stage of the
//! same `(label, param)` overwrites (last write wins).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use piperine_lang::Value;

use crate::error::Error;
use crate::model::{Behavior, Instance, Net, Param, Port};
use crate::results::{DistoResult, OpResult, PssResult, PzResult, SParamResult, SensResult};
use crate::session::{Session, SolverConfig};
use crate::waveform::{AcTrace, NoiseTrace, Trace, Waveform};

/// A navigable view of a named module.
///
/// **Navigation walks the authored hierarchy** (MD-25): [`Module::instances`]
/// yields the instances the author wrote — one entry per authored instance, not
/// the leaf-only splice codegen's flattened side artifact carries. Descending is
/// `instance.module()` → [`Design::module`](crate::model::Design::module), and
/// the tree stays walkable to any depth.
#[derive(Clone)]
pub struct Module {
    design: Rc<piperine_lang::Design>,
    name: String,
    /// `(instance label, param name) → staged value`. Isolated from the
    /// parent design so the user's [`Design`](crate::model::Design) is never
    /// mutated; replayed onto each analysis's fork before solving.
    staged: RefCell<HashMap<(String, String), Value>>,
}

impl std::fmt::Debug for Module {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Module").field("name", &self.name).finish()
    }
}

impl Module {
    /// Build a view of `name` in `design`. Constructed by
    /// [`Design::module`](crate::model::Design::module)/`top`/`modules`.
    pub(crate) fn new(design: Rc<piperine_lang::Design>, name: String) -> Self {
        Self { design, name, staged: RefCell::new(HashMap::new()) }
    }

    /// Re-resolve the live module from the shared POM — the **authored** map,
    /// never the flattened one (MD-25).
    fn pom(&self) -> Result<&piperine_lang::Module, Error> {
        self.design
            .module(&self.name)
            .ok_or_else(|| Error::NotFound(format!("module `{}` is no longer present", self.name)))
    }

    /// The module's declared name (re-resolved against the live POM).
    pub fn name(&self) -> Result<&str, Error> {
        Ok(self.pom()?.name())
    }

    /// The module's ports.
    pub fn ports(&self) -> Result<Vec<Port>, Error> {
        Ok(self.pom()?.ports().iter().map(Port::of).collect())
    }

    /// The module's nets (its `wire` declarations).
    pub fn nets(&self) -> Result<Vec<Net>, Error> {
        Ok(self.pom()?.wires().iter().map(Net::of).collect())
    }

    /// The module's submodule instances, as the author wrote them (MD-25).
    pub fn instances(&self) -> Result<Vec<Instance>, Error> {
        Ok(self.pom()?.instances().iter().map(Instance::of).collect())
    }

    /// The module's params.
    pub fn params(&self) -> Result<Vec<Param>, Error> {
        Ok(self.pom()?.params().iter().map(Param::of).collect())
    }

    /// The module's `analog`/`digital` behavior blocks.
    pub fn behaviors(&self) -> Result<Vec<Behavior>, Error> {
        Ok(self.pom()?.behaviors().iter().map(Behavior::of).collect())
    }

    /// Compile a fresh [`Session`] for one analysis: hand every staged
    /// override to a [`SessionBuilder`](crate::SessionBuilder), which forks
    /// the parent design and replays them onto the fork. Each analysis call
    /// gets its own session + fork, so results never leak between calls.
    fn session(&self) -> Result<Session, Error> {
        self.session_with_disto(false)
    }

    /// [`Self::session`], plus the `.disto` 2nd/3rd-derivative kernels.
    /// `disto` is the only analysis that reads them and they are opt-in
    /// (a many-branch device overruns the JIT backend compiling them), so it
    /// is the only caller.
    fn session_with_disto(&self, disto: bool) -> Result<Session, Error> {
        let mut builder = Session::builder(&self.design, &self.name).disto(disto);
        for ((label, param), value) in self.staged.borrow().iter() {
            builder = builder.stage(label, param, value.clone());
        }
        builder.compile()
    }

    /// `None` maps to the solver defaults — the `solver=None` arm of the
    /// Python facade.
    fn solver_config(config: Option<&SolverConfig>) -> SolverConfig {
        config.cloned().unwrap_or_default()
    }

    /// Run a DC operating-point analysis: the solved node voltages + branch
    /// currents as an [`OpResult`]. `nodeset` seeds the Newton initial guess.
    pub fn op(
        &self,
        nodeset: Option<&HashMap<String, f64>>,
        config: Option<&SolverConfig>,
    ) -> Result<OpResult, Error> {
        let mut session = self.session()?;
        session.op(&Self::solver_config(config), nodeset)
    }

    /// Run a transient analysis. `step = None` (or `Some(0.0)`) selects the
    /// adaptive stepper; `start` is the earliest recorded time. `ic` is an
    /// optional per-node initial-condition map. `record_device_state` opts
    /// into per-step device runtime-bank recording; `probe` names
    /// `"instance.opvar_name"` observables to record selectively.
    #[allow(clippy::too_many_arguments)]
    pub fn tran(
        &self,
        stop: f64,
        step: Option<f64>,
        start: f64,
        ic: Option<&HashMap<String, f64>>,
        config: Option<&SolverConfig>,
        record_device_state: bool,
        probe: &[&str],
    ) -> Result<Trace<Waveform>, Error> {
        let mut session = self.session()?;
        session.tran(stop, step, start, &Self::solver_config(config), ic, record_device_state, probe)
    }

    /// Run an AC small-signal sweep.
    pub fn ac(
        &self,
        fstart: f64,
        fstop: f64,
        points: usize,
        logarithmic: bool,
        config: Option<&SolverConfig>,
    ) -> Result<AcTrace, Error> {
        let mut session = self.session()?;
        session.ac(fstart, fstop, points, logarithmic, &Self::solver_config(config))
    }

    /// Run an output-referred noise analysis. `reference` is the reference
    /// net (`"gnd"` for the single-net form).
    pub fn noise(
        &self,
        out: &str,
        reference: &str,
        frange: (f64, f64),
        points: usize,
        logarithmic: bool,
        config: Option<&SolverConfig>,
    ) -> Result<NoiseTrace, Error> {
        let mut session = self.session()?;
        session.noise(out, reference, frange, points, logarithmic, &Self::solver_config(config))
    }

    /// Run a DC sensitivity analysis (`.sens`): `∂V(output)/∂(param)` at the
    /// operating point for each `(label, param)` pair, by central finite
    /// difference.
    pub fn sens(
        &self,
        outputs: &[&str],
        params: &[(String, String)],
        dp_rel: f64,
        config: Option<&SolverConfig>,
    ) -> Result<SensResult, Error> {
        let mut session = self.session()?;
        session.sens(outputs, params, dp_rel, &Self::solver_config(config))
    }

    /// Run a periodic-steady-state analysis (single shooting): one converged
    /// period as a transient trace plus the shooting stats. The drive period
    /// is user-supplied; non-periodic circuits fail loud.
    pub fn pss(
        &self,
        period: f64,
        tstab: f64,
        config: Option<&SolverConfig>,
    ) -> Result<PssResult, Error> {
        let mut session = self.session()?;
        session.pss(period, tstab, &Self::solver_config(config))
    }

    /// Run a pole-zero analysis (`.pz`): poles and transmission zeros of the
    /// linearized input→output transfer function at the DC operating point.
    /// `input_source` is the driving voltage source's instance label;
    /// `output` is the measured net, optionally differential against
    /// `output_ref`.
    pub fn pz(
        &self,
        input_source: &str,
        output: &str,
        output_ref: Option<&str>,
        config: Option<&SolverConfig>,
    ) -> Result<PzResult, Error> {
        let mut session = self.session()?;
        session.pz(input_source, output, output_ref, &Self::solver_config(config))
    }

    /// Run a distortion analysis (`.disto`): small-signal Volterra
    /// distortion at the DC operating point. Single-tone (`f2 = None`)
    /// reports `hd2`/`hd3`; two-tone (`f2` given) reports `im2`/`im3`.
    /// `amplitude` scales every AC stimulus magnitude in the circuit.
    pub fn disto(
        &self,
        f1: f64,
        f2: Option<f64>,
        amplitude: f64,
        output: &str,
        output_ref: Option<&str>,
        config: Option<&SolverConfig>,
    ) -> Result<DistoResult, Error> {
        let mut session = self.session_with_disto(true)?;
        session.disto(f1, f2, amplitude, output, output_ref, &Self::solver_config(config))
    }

    /// Run an N-port S-parameter analysis (`.sp`): the scattering matrix
    /// over a frequency sweep for every node carrying an `@rfport(num, z0)`
    /// attribute in this module.
    pub fn sp(
        &self,
        fstart: f64,
        fstop: f64,
        points: usize,
        logarithmic: bool,
        config: Option<&SolverConfig>,
    ) -> Result<SParamResult, Error> {
        let mut session = self.session()?;
        session.sp(fstart, fstop, points, logarithmic, &Self::solver_config(config))
    }

    /// Stage a parameter override on `label`'s `param`: the next analysis on
    /// this module uses `value`. Staging is pure — the held
    /// [`Design`](crate::model::Design) is never mutated; overrides live in
    /// an isolated map and replay onto each analysis's fork. A re-stage of
    /// the same `(label, param)` overwrites.
    pub fn set(&self, label: &str, param: &str, value: f64) {
        self.staged
            .borrow_mut()
            .insert((label.to_string(), param.to_string()), Value::Real(value));
    }

    /// Compile this module **once** into a live [`Session`]: the returned
    /// session holds the elaborated design and the JIT-compiled circuit, so
    /// [`Session::set`] + re-run analyses never recompile (MD-18). Currently
    /// staged overrides are baked into the compilation (the same replay as
    /// the per-analysis path); the parent design stays untouched.
    pub fn compile(&self) -> Result<Session, Error> {
        self.session()
    }
}
