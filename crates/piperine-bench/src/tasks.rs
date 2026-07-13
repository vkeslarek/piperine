//! Bench-only system tasks: effectful, need a [`SimSession`] to run. Each
//! is a unit struct implementing [`BenchTask`] — the same
//! `run(args, cx)` shape a plugin's bench task has (SPEC Part VI §6,
//! `PluginBenchTask`), with a different capability: a builtin's
//! [`BenchCtx`] hands it the full session, a plugin's `HostCtx` is
//! permission-gated. The shared pure registry
//! ([`piperine_lang::eval::Task`]) stays the fallback consulted by
//! [`crate::host::SimHost::syscall`].
//!
//! Analyses take a **config bundle** (piperine-bench/docs/SPEC.md §5.1, `Value::Record`
//! built from the prelude's `OpConfig`/`TranConfig`/`AcConfig`/
//! `NoiseConfig`) — configuration is an argument, never hidden state.
//! `$tran(stop, step)` positional form is kept as a convenience alias.

use std::collections::HashMap;

use piperine_lang::eval::{EvalError, Value};

use crate::objects::NetRef;
use crate::session::{SimSession, SolverConfig};

/// A bench-only system task (`$op`, `$tran`, `$ac`, `$noise`, `$write`).
/// Same shape as a plugin bench task — implement it, register it, call it
/// as `$name(...)`.
pub trait BenchTask {
    fn name(&self) -> &'static str;
    fn run(&self, args: Vec<Value>, cx: &mut BenchCtx) -> Result<Value, EvalError>;
}

/// What a builtin task runs against — the internal capability: the whole
/// [`SimSession`]. (Plugin tasks get a permission-gated `HostCtx` instead.)
pub struct BenchCtx<'a> {
    session: &'a SimSession,
}

impl<'a> BenchCtx<'a> {
    pub fn new(session: &'a SimSession) -> Self {
        Self { session }
    }

    pub fn session(&self) -> &SimSession {
        self.session
    }
}

// ─── Config-bundle field access ───────────────────────────────────────────────

/// A field of a config `Record`, or `None` when absent.
fn field(rec: &Value, name: &str) -> Option<Value> {
    match rec {
        Value::Record { fields, .. } => fields.borrow().get(name).cloned(),
        _ => None,
    }
}

/// A `Map`-typed config field (`ic`/`nodeset`), defaulting to an empty map
/// when absent or not a Map (piperine-bench/docs/SPEC.md §5.1).
fn field_opt_map(cfg: Option<&Value>, name: &str) -> Value {
    cfg.and_then(|c| field(c, name))
        .and_then(|v| if matches!(v, Value::Map(_)) { Some(v) } else { None })
        .unwrap_or_else(|| Value::Map(std::rc::Rc::new(std::cell::RefCell::new(vec![]))))
}

fn real_field(rec: &Value, name: &str) -> Result<Option<f64>, EvalError> {
    field(rec, name).map(|v| v.coerce_real()).transpose()
}

/// A required config field — absence is a fail-loud error naming it.
fn required_real(rec: &Value, bundle: &str, name: &str) -> Result<f64, EvalError> {
    real_field(rec, name)?
        .ok_or_else(|| EvalError::TypeMismatch(format!("`{bundle}` needs `.{name}`")))
}

/// The `solver : Solver` sub-bundle of a config, folded onto the defaults.
fn solver_config(cfg: Option<&Value>) -> Result<SolverConfig, EvalError> {
    let mut sc = SolverConfig::default();
    let Some(cfg) = cfg else { return Ok(sc) };
    let Some(solver) = field(cfg, "solver") else { return Ok(sc) };
    if let Some(t) = real_field(&solver, "temperature")? {
        sc.temperature = t;
    }
    if let Some(r) = real_field(&solver, "reltol")? {
        sc.reltol = r;
    }
    if let Some(a) = real_field(&solver, "abstol")? {
        sc.abstol = a;
    }
    if let Some(g) = real_field(&solver, "gmin")? {
        sc.gmin = g;
    }
    if let Some(m) = real_field(&solver, "max_iter")? {
        sc.max_iter = m as usize;
    }
    Ok(sc)
}

/// `scale : Scale` → is the sweep logarithmic? (`Oct` maps onto the
/// solver's logarithmic sweep — it distinguishes only lin/log.)
fn is_log_scale(cfg: &Value) -> bool {
    match field(cfg, "scale") {
        Some(Value::EnumVariant(_, v)) => v != "Lin",
        _ => true, // the prelude default is Dec
    }
}

// ─── Analyses ─────────────────────────────────────────────────────────────────

struct Op;
impl BenchTask for Op {
    fn name(&self) -> &'static str {
        "op"
    }
    fn run(&self, args: Vec<Value>, cx: &mut BenchCtx) -> Result<Value, EvalError> {
        // `$op()` or `$op(OpConfig { .solver = …, .nodeset = Map { … } })`.
        let cfg = args.first();
        let nodeset = field_opt_map(cfg, "nodeset");
        let sc = solver_config(cfg)?;
        let result = cx.session().run_op(&sc, &nodeset).map_err(EvalError::from)?;
        Ok(Value::Object(std::rc::Rc::new(result)))
    }
}

struct Tran;
impl BenchTask for Tran {
    fn name(&self) -> &'static str {
        "tran"
    }
    fn run(&self, args: Vec<Value>, cx: &mut BenchCtx) -> Result<Value, EvalError> {
        // `$tran(TranConfig { .stop = …, [.step], [.start], [.solver] })`, or
        // the positional convenience `$tran(stop, step)`.
        let (stop, step, start, cfg) = match args.first() {
            Some(rec @ Value::Record { .. }) => {
                let stop = required_real(rec, "TranConfig", "stop")?;
                let step = match real_field(rec, "step")? {
                    Some(s) if s > 0.0 => Some(s),
                    _ => None, // 0.0 = "auto" → adaptive stepping
                };
                let start = real_field(rec, "start")?.unwrap_or(0.0);
                (stop, step, start, solver_config(args.first())?)
            }
            _ => {
                let stop = args.first().ok_or_else(|| {
                    EvalError::TypeMismatch("$tran needs a TranConfig or (stop, step)".into())
                })?.coerce_real()?;
                let step = args.get(1).ok_or_else(|| {
                    EvalError::TypeMismatch("positional $tran needs (stop, step)".into())
                })?.coerce_real()?;
                (stop, Some(step), 0.0, SolverConfig::default())
            }
        };
        let ic = field_opt_map(args.first(), "ic");
        let trace = cx.session().run_tran(stop, step, start, &cfg, &ic).map_err(EvalError::from)?;
        Ok(Value::Object(std::rc::Rc::new(trace)))
    }
}

struct Ac;
impl BenchTask for Ac {
    fn name(&self) -> &'static str {
        "ac"
    }
    fn run(&self, args: Vec<Value>, cx: &mut BenchCtx) -> Result<Value, EvalError> {
        // `$ac(AcConfig { .fstart = …, .fstop = …, [.points, .scale, .solver] })`.
        let cfg = args.first().ok_or_else(|| {
            EvalError::TypeMismatch("$ac needs an AcConfig { .fstart, .fstop, … }".into())
        })?;
        let fstart = required_real(cfg, "AcConfig", "fstart")?;
        let fstop = required_real(cfg, "AcConfig", "fstop")?;
        let points = real_field(cfg, "points")?.unwrap_or(100.0) as usize;
        let trace = cx.session()
            .run_ac(fstart, fstop, points, is_log_scale(cfg), &solver_config(Some(cfg))?)
            .map_err(EvalError::from)?;
        Ok(Value::Object(std::rc::Rc::new(trace)))
    }
}

struct Noise;
impl BenchTask for Noise {
    fn name(&self) -> &'static str {
        "noise"
    }
    fn run(&self, args: Vec<Value>, cx: &mut BenchCtx) -> Result<Value, EvalError> {
        // `$noise(NoiseConfig { .out = Net | (Net, Net), .fstart = …, .fstop
        // = …, … })`. The spec's `out : Branch` config field (piperine-bench/docs/SPEC.md
        // §5.1) is the output branch: a bare `Net` means `(net, gnd)`, a
        // `(Net, Net)` pair means `(plus, minus)`. The deprecated positional
        // alias `$noise(out, cfg)` is kept for one release.
        let (cfg, out_value) = match args.as_slice() {
            [rec @ Value::Record { .. }] => {
                let out = field(rec, "out").ok_or_else(|| {
                    EvalError::TypeMismatch("NoiseConfig needs `.out = Net | (Net, Net)`".into())
                })?;
                (rec, out)
            }
            [Value::Object(obj), cfg @ Value::Record { .. }]
                if obj.as_any().downcast_ref::<NetRef>().is_some() =>
            {
                let out_name = obj.as_any().downcast_ref::<NetRef>().unwrap().name.clone();
                (cfg, Value::Object(std::rc::Rc::new(NetRef { name: out_name })))
            }
            _ => return Err(EvalError::TypeMismatch(
                "$noise needs a NoiseConfig { .out = …, .fstart, .fstop, … } (or the deprecated $noise(out, cfg) alias)".into(),
            )),
        };
        let (out, reference) = branch_nets(&out_value)?;
        let fstart = required_real(cfg, "NoiseConfig", "fstart")?;
        let fstop = required_real(cfg, "NoiseConfig", "fstop")?;
        let points = real_field(cfg, "points")?.unwrap_or(100.0) as usize;
        let trace = cx.session()
            .run_noise(&out, &reference, fstart, fstop, points, is_log_scale(cfg), &solver_config(Some(cfg))?)
            .map_err(EvalError::from)?;
        Ok(Value::Object(std::rc::Rc::new(trace)))
    }
}

/// Resolve a `Branch` config value (piperine-bench/docs/SPEC.md §5.1 `NoiseConfig.out`)
/// to a `(plus, minus)` net-name pair. A bare `Net` is `(net, gnd)`; a
/// `(Net, Net)` tuple is `(plus, minus)`.
fn branch_nets(v: &Value) -> Result<(String, String), EvalError> {
    fn net_name(v: &Value) -> Result<String, EvalError> {
        match v {
            Value::Object(obj) => obj
                .as_any()
                .downcast_ref::<NetRef>()
                .map(|n| n.name.clone())
                .ok_or_else(|| {
                    EvalError::TypeMismatch(format!("$noise `.out` must be a Net, got {}", obj.type_name()))
                }),
            other => Err(EvalError::TypeMismatch(format!(
                "$noise `.out` must be a Net, got {}",
                other.type_name()
            ))),
        }
    }
    match v {
        Value::Tuple(items) if items.len() == 2 => {
            let plus = net_name(&items[0])?;
            let minus = net_name(&items[1])?;
            Ok((plus, minus))
        }
        other => Ok((net_name(other)?, "gnd".to_string())),
    }
}

// ─── Artifacts ────────────────────────────────────────────────────────────────

struct Write;
impl BenchTask for Write {
    fn name(&self) -> &'static str {
        "write"
    }
    fn run(&self, args: Vec<Value>, _cx: &mut BenchCtx) -> Result<Value, EvalError> {
        let Some(Value::Str(path)) = args.first() else {
            return Err(EvalError::TypeMismatch("$write needs (path, value)".into()));
        };
        let value = args
            .get(1)
            .ok_or_else(|| EvalError::TypeMismatch("$write needs (path, value)".into()))?;
        let text = csv_of(value);
        std::fs::write(path, text).map_err(|e| EvalError::Host(format!("$write `{path}`: {e}")))?;
        Ok(Value::Unit)
    }
}

/// CSV rendering: a list becomes one row per element (tuples split into
/// columns), anything else a single line.
fn csv_of(v: &Value) -> String {
    fn cell(v: &Value) -> String {
        v.to_string().trim_matches('"').to_string()
    }
    match v {
        Value::List(items) => {
            let mut out = String::new();
            for item in items.borrow().iter() {
                let row = match item {
                    Value::Tuple(cols) => cols.iter().map(cell).collect::<Vec<_>>().join(","),
                    other => cell(other),
                };
                out.push_str(&row);
                out.push('\n');
            }
            out
        }
        Value::Tuple(cols) => {
            let mut row = cols.iter().map(cell).collect::<Vec<_>>().join(",");
            row.push('\n');
            row
        }
        other => format!("{}\n", cell(other)),
    }
}

// ─── Registry ─────────────────────────────────────────────────────────────────

/// The bench-only task registry.
pub struct BenchTaskRegistry(HashMap<&'static str, Box<dyn BenchTask>>);

impl BenchTaskRegistry {
    pub fn with_builtins() -> Self {
        let tasks: Vec<Box<dyn BenchTask>> =
            vec![Box::new(Op), Box::new(Tran), Box::new(Ac), Box::new(Noise), Box::new(Write)];
        Self(tasks.into_iter().map(|t| (t.name(), t)).collect())
    }

    pub fn lookup(&self, name: &str) -> Option<&dyn BenchTask> {
        self.0.get(name).map(|b| b.as_ref())
    }
}
