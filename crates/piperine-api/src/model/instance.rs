//! [`InstanceView`] — the terminal sub-view over one instance (CLA-17): its
//! connectivity ([`InstanceView::terminal_connections`]), terminal readouts
//! (`v`/`i`), computed operating-point variables (`opvar`/`opvars`), and the
//! static reflection catalogs (`model`/`terminals`/`observables`/`param`/
//! `params`).
//!
//! Two shapes share the one type (MD-13: one owner per operation):
//!
//! - an **introspection view**, what [`OpResult::instance`] hands out — the
//!   opvar snapshot + catalogs, cloned out of the result (reflection, not a
//!   hot path). Terminal readouts and connectivity need a result and a
//!   resolver, so on this shape they fail loud;
//! - a **bound view** ([`InstanceView::new_op`]/[`InstanceView::new_trace`]) —
//!   the full surface: `v`/`i` read the parent result over the nets the
//!   instance's ports connect to, resolved against the authored POM by
//!   [`InstanceResolver`]. Scalars over an [`OpResult`], [`Waveform`]s over a
//!   [`Trace`] — the uniform shape (MD-22) the Python `_InstanceView` spells
//!   dynamically.

use std::rc::Rc;

use piperine_lang::pom::node::Node as PomNode;
use piperine_solver::prelude::{
    ModelDescriptor, ObservableDescriptor, ParamDescriptor, TerminalDescriptor,
};

use crate::error::Error;
use crate::model::Terminal;
use crate::results::{NetRef, OpResult};
use crate::waveform::{Trace, Waveform};

/// The parent result a bound [`InstanceView`] projects.
enum InstanceParent {
    Op(Rc<OpResult>),
    Trace(Rc<Trace>),
}

/// What a terminal readout returns: a scalar when the parent is an
/// [`OpResult`], a [`Waveform`] when the parent is a [`Trace`] — the two
/// shapes Python's `_InstanceView.v/.i` return as `float`/`_Waveform`.
#[derive(Debug)]
pub enum InstanceReadout {
    Scalar(f64),
    Waveform(Waveform),
}

/// A per-instance view (HOST-07/09 + PY-13): one device's operating-point
/// variables, static reflection catalogs, terminal connectivity, and
/// terminal readouts, addressed by instance label.
pub struct InstanceView {
    label: String,
    opvars: Vec<(String, f64)>,
    model: ModelDescriptor,
    terminals: Vec<TerminalDescriptor>,
    observables: Vec<ObservableDescriptor>,
    params: Vec<ParamDescriptor>,
    parent: Option<InstanceParent>,
    resolver: Option<InstanceResolver>,
}

impl std::fmt::Debug for InstanceView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceView").field("label", &self.label).finish()
    }
}

impl InstanceView {
    /// The introspection-only shape [`OpResult::instance`] hands out: the
    /// opvar snapshot + catalogs, cloned out of the result.
    pub(crate) fn catalog(
        label: &str,
        opvars: &[(String, f64)],
        model: ModelDescriptor,
        terminals: Vec<TerminalDescriptor>,
        observables: Vec<ObservableDescriptor>,
        params: Vec<ParamDescriptor>,
    ) -> Self {
        Self {
            label: label.to_string(),
            opvars: opvars.to_vec(),
            model,
            terminals,
            observables,
            params,
            parent: None,
            resolver: None,
        }
    }

    /// A bound view over an op() snapshot (PY-13): the full surface, with
    /// scalar terminal readouts. Fails loud when the op result carries no
    /// device labeled `label`.
    pub fn new_op(inner: Rc<OpResult>, resolver: InstanceResolver, label: &str) -> Result<Self, Error> {
        let mut view = inner.instance(label)?;
        view.parent = Some(InstanceParent::Op(inner));
        view.resolver = Some(resolver);
        Ok(view)
    }

    /// A bound view over a tran() snapshot (PY-13): terminal readouts return
    /// waveforms. The opvar/catalog surface is op-side only and fails loud
    /// here (recorded observables use [`Trace::opvar`] instead, HOST-08).
    pub fn new_trace(inner: Rc<Trace>, resolver: InstanceResolver, label: &str) -> Self {
        Self {
            label: label.to_string(),
            opvars: Vec::new(),
            model: ModelDescriptor::default(),
            terminals: Vec::new(),
            observables: Vec::new(),
            params: Vec::new(),
            parent: Some(InstanceParent::Trace(inner)),
            resolver: Some(resolver),
        }
    }

    /// The instance label this view projects.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Loud when the accessor is op-side only and this view is trace-bound.
    fn require_op_side(&self, what: &str) -> Result<(), Error> {
        if matches!(self.parent, Some(InstanceParent::Trace(_))) {
            return Err(Error::Measurement(format!("{what} is not available on a trace view")));
        }
        Ok(())
    }

    /// The resolver a bound view carries; an introspection-only view has
    /// none, and connectivity/readouts fail loud rather than guessing.
    fn resolver(&self) -> Result<&InstanceResolver, Error> {
        self.resolver.as_ref().ok_or_else(|| {
            Error::Measurement(format!(
                "instance view `{}` is introspection-only (no design bound) — \
                 terminal connectivity and readouts need `InstanceView::new_op`/`new_trace`",
                self.label
            ))
        })
    }

    /// The parent result a bound view projects.
    fn parent(&self) -> Result<&InstanceParent, Error> {
        self.parent.as_ref().ok_or_else(|| {
            Error::Measurement(format!(
                "instance view `{}` is introspection-only (no result bound) — \
                 terminal readouts need `InstanceView::new_op`/`new_trace`",
                self.label
            ))
        })
    }

    /// The instance's terminal connectivity: `(port, net)` pairs in
    /// port-declaration order — the port on the instance's module declaration
    /// and the parent-scope net it is wired to.
    pub fn terminal_connections(&self) -> Result<Vec<Terminal>, Error> {
        Ok(self
            .resolver()?
            .terminal_nets(&self.label)?
            .into_iter()
            .map(|(port, net)| Terminal::new(port, net))
            .collect())
    }

    /// Translate a `(port_a, port_b?)` pair into the connected nets; `port_b`
    /// defaults to the implicit ground reference.
    fn resolve_pair(&self, port_a: &str, port_b: Option<&str>) -> Result<(NetRef, Option<NetRef>), Error> {
        let resolver = self.resolver()?;
        let net_a = resolver.terminal_net(&self.label, port_a)?;
        let net_b = port_b.map(|p| resolver.terminal_net(&self.label, p)).transpose()?;
        Ok((NetRef::from(net_a), net_b.map(NetRef::from)))
    }

    /// Terminal voltage at `port_a` minus `port_b` (ground-referenced when
    /// `port_b` is `None`) — the voltage at the connected net(s). A scalar
    /// over an op result, a waveform over a trace (uniform shape).
    pub fn v(&self, port_a: &str, port_b: Option<&str>) -> Result<InstanceReadout, Error> {
        let (net_a, net_b) = self.resolve_pair(port_a, port_b)?;
        match self.parent()? {
            InstanceParent::Op(op) => Ok(InstanceReadout::Scalar(match net_b {
                Some(b) => op.v((net_a, b))?,
                None => op.v(net_a)?,
            })),
            InstanceParent::Trace(trace) => Ok(InstanceReadout::Waveform(match net_b {
                Some(b) => trace.v((net_a, b))?,
                None => trace.v(net_a)?,
            })),
        }
    }

    /// Branch current from `port_a` to `port_b` (ground-referenced when
    /// `port_b` is `None`) through this instance — the current in the branch
    /// the instance's two terminals define. A scalar over an op result, a
    /// waveform over a trace.
    pub fn i(&self, port_a: &str, port_b: Option<&str>) -> Result<InstanceReadout, Error> {
        let (net_a, net_b) = self.resolve_pair(port_a, port_b)?;
        match self.parent()? {
            InstanceParent::Op(op) => Ok(InstanceReadout::Scalar(match net_b {
                Some(b) => op.i((net_a, b))?,
                None => op.i(net_a)?,
            })),
            InstanceParent::Trace(trace) => Ok(InstanceReadout::Waveform(match net_b {
                Some(b) => trace.i((net_a, b))?,
                None => trace.i(net_a)?,
            })),
        }
    }

    /// The device's computed operating-point variable `name` (ABI-30). Fails
    /// loud — never `None`/`NaN` — when the device declares no such opvar,
    /// naming the instance and listing the opvars it does have.
    pub fn opvar(&self, name: &str) -> Result<f64, Error> {
        self.require_op_side("opvar()")?;
        self.opvars.iter().find(|(n, _)| n == name).map(|(_, v)| *v).ok_or_else(|| {
            Error::Measurement(format!(
                "instance `{}` has no opvar `{name}`; available: {}",
                self.label,
                self.opvars.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ")
            ))
        })
    }

    /// Every opvar this device declared, as `(name, value)` pairs. Empty on
    /// a trace-bound view (a device with no op-side snapshot) — the same
    /// ABI-30 default an opvarless device reports.
    pub fn opvars(&self) -> Vec<(String, f64)> {
        self.opvars.clone()
    }

    /// The device's model identity (ABI-46 / HOST-09): `type_id` and
    /// `version`. An author-declared `@model(type, version)` populates both;
    /// absent `@model` falls back to the module name as `type_id` with empty
    /// version — never panics.
    pub fn model(&self) -> &ModelDescriptor {
        &self.model
    }

    /// The device's declared terminals with their kind (ABI-27 / HOST-09):
    /// name, domain, direction, and `TerminalKind` (external/internal/
    /// auxiliary). For port→net connectivity, use
    /// [`Self::terminal_connections`].
    pub fn terminals(&self) -> &[TerminalDescriptor] {
        &self.terminals
    }

    /// The device's observable catalog (ABI-32 / HOST-09): each entry is
    /// `(name, kind, cost)` — what CAN be probed via `probe=["inst.name"]`.
    pub fn observables(&self) -> &[ObservableDescriptor] {
        &self.observables
    }

    /// The parameter descriptor for `name` (HOST-12): `bounds`, `unit`,
    /// `scope`, `invalidation`. Fails loud when the device declares no such
    /// parameter, naming the instance and listing available params.
    pub fn param(&self, name: &str) -> Result<&ParamDescriptor, Error> {
        self.require_op_side("param()")?;
        self.params.iter().find(|p| p.name == name).ok_or_else(|| {
            Error::Measurement(format!(
                "instance `{}` has no param `{name}`; available: {}",
                self.label,
                self.params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ")
            ))
        })
    }

    /// The device's parameter descriptor catalog (HOST-12): `bounds`/`unit`/
    /// `scope`/`invalidation` for each declared parameter.
    pub fn params(&self) -> &[ParamDescriptor] {
        &self.params
    }
}

/// Bridge between a hierarchical/dot-notation instance path the user types
/// and the POM instance it names. Carries a shared design + the parent module
/// name (the module the analysis ran on); result objects use it to detect
/// instance paths and resolve them to terminal info.
#[derive(Clone)]
pub struct InstanceResolver {
    design: Rc<piperine_lang::Design>,
    module_name: String,
}

impl std::fmt::Debug for InstanceResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceResolver").field("module_name", &self.module_name).finish()
    }
}

impl InstanceResolver {
    /// Bind a resolver to `module_name` in `design`.
    pub fn new(design: Rc<piperine_lang::Design>, module_name: String) -> Self {
        Self { design, module_name }
    }

    // Dot-notation (`buck.r1`) is accepted and translated to the POM
    // selector grammar (`/`-separated steps) internally — the translation is
    // a private concern of the model.
    fn to_selector_path(key: &str) -> String {
        let trimmed = key.trim_start_matches('/');
        let body = trimmed.replace('.', "/");
        format!("/{body}")
    }

    /// Whether `key` looks like an instance reference (not a plain net
    /// name): a path separator is present, OR `key` matches an instance
    /// label in the parent module.
    pub fn looks_like_instance(&self, key: &str) -> bool {
        if key.contains('.') || key.contains('/') {
            return true;
        }
        let Some(module) = self.design.module(&self.module_name) else {
            return false;
        };
        module.instances().iter().any(|i| i.name() == key)
    }

    /// Resolve `key` to a single leaf instance label that exists in the POM.
    /// One-segment keys (no separator) are looked up directly; multi-segment
    /// dot-paths are translated to selector grammar and resolved via
    /// `Design::select`. [`Error::NotFound`] for zero matches,
    /// [`Error::Measurement`] for an ambiguous match — both fail loud.
    pub fn resolve_label(&self, key: &str) -> Result<String, Error> {
        if !key.contains('.') && !key.contains('/') {
            let module = self.design.module(&self.module_name).ok_or_else(|| {
                Error::NotFound(format!("module `{}` not found", self.module_name))
            })?;
            if module.instances().iter().any(|i| i.name() == key) {
                return Ok(key.to_string());
            }
            return Err(Error::NotFound(format!(
                "`{key}` is not a net or instance of `{}`",
                self.module_name
            )));
        }
        let sel_path = Self::to_selector_path(key);
        let selection = self
            .design
            .select(&sel_path)
            .map_err(|e| Error::NotFound(format!("`{key}` did not resolve: {e}")))?;
        let labels: Vec<String> = selection
            .iter()
            .filter_map(|n| match n {
                PomNode::Instance(inst) => Some(inst.name().to_string()),
                _ => None,
            })
            .collect();
        match labels.as_slice() {
            [one] => Ok(one.clone()),
            [] => Err(Error::NotFound(format!("`{key}` did not resolve to an instance"))),
            many => Err(Error::Measurement(format!(
                "`{key}` resolved to {} instances; expected one",
                many.len()
            ))),
        }
    }

    /// Map `label`'s port names to their connected parent-scope net names by
    /// walking the authored POM. Returns `(port_name, net_name)` pairs in
    /// port-declaration order. [`Error::NotFound`] when the instance or its
    /// module is not found (fail loud).
    pub fn terminal_nets(&self, label: &str) -> Result<Vec<(String, String)>, Error> {
        let module = self.design.module(&self.module_name).ok_or_else(|| {
            Error::NotFound(format!("module `{}` not found", self.module_name))
        })?;
        let inst = module
            .instances()
            .iter()
            .find(|i| i.name() == label)
            .ok_or_else(|| {
                Error::NotFound(format!(
                    "instance `{label}` not found in module `{}`",
                    self.module_name
                ))
            })?;
        let child = self.design.module(inst.module_name()).ok_or_else(|| {
            Error::NotFound(format!("child module `{}` not found", inst.module_name()))
        })?;
        inst.ports()
            .iter()
            .zip(child.ports().iter())
            .map(|(binding, port)| Ok((port.name().to_string(), binding.net().to_string())))
            .collect()
    }

    /// Resolve a single `port_name` to its connected parent-scope net name.
    fn terminal_net(&self, label: &str, port_name: &str) -> Result<String, Error> {
        let pairs = self.terminal_nets(label)?;
        pairs
            .into_iter()
            .find(|(p, _)| p == port_name)
            .map(|(_, n)| n)
            .ok_or_else(|| {
                Error::NotFound(format!("port `{port_name}` not found on instance `{label}`"))
            })
    }
}
