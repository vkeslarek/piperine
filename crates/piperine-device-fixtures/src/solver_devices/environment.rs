//! The devices of the solver target `environment`, one type per device.

use std::cell::RefCell;
use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Shape, Statement, ambient, cell, global, op, part, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

const ORIGIN: &str = "environment.rs";

thread_local! {
    /// `(temperature, gmin, source_scale)` as each reaction call read them.
    pub static READ: RefCell<Vec<[f64; 3]>> = const { RefCell::new(Vec::new()) };
}

/// Reads the environment.
/// pins: t=0 (GLOBAL TEMPERATURE F64); fg=1, fs=2, rt=3, rg=4, rs=5 (PORT F64);
///       start=6 (GLOBAL START); go=7 (PORT event)
/// fn 0 "env": reads [GMIN, SOURCE_SCALE] writes [fg, fs]
/// reaction 0 "env": on [start, go] reads [t, GMIN, SOURCE_SCALE] writes [S0 {rt}, S1 {rg}, S2 {rs}]
pub struct EnvProbe;
static PROBE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static PROBE: OnceLock<Module> = OnceLock::new();

impl Device for EnvProbe {
    fn prototype() -> &'static Shape<'static> {
        PROBE_SHAPE.get_or_init(EnvironmentShapes::probe_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((EnvProbe, Arc::clone(PROBE_SHAPE.get_or_init(EnvironmentShapes::probe_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (g, s) = (cx.f64(0), cx.f64(1));
        cx.set(0, g);
        cx.set(1, s);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let read = [cx.f64(0), cx.f64(1), cx.f64(2)];
        READ.with(|r| r.borrow_mut().push(read));
        for (k, v) in read.into_iter().enumerate() {
            cx.commit(k as u32, &v.to_le_bytes(), 0.0)?;
        }
        Ok(Flow::Continue)
    }
}

/// Draws `gmin · 1e9` amperes out of `p`, `gmin` its ambient `GMIN`: into 1 kΩ, v(p) = gmin · 1e12.
/// pins: p=0 (PORT continuous);  terms: T0 FLOW{p, GROUND} "i";  fn 0: reads [p, GMIN] writes [T0]
pub struct GminLoad;
static LOAD_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static LOAD: OnceLock<Module> = OnceLock::new();

impl Device for GminLoad {
    fn prototype() -> &'static Shape<'static> {
        LOAD_SHAPE.get_or_init(EnvironmentShapes::load_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((GminLoad, Arc::clone(LOAD_SHAPE.get_or_init(EnvironmentShapes::load_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let gmin = cx.f64(1);
        cx.set(0, -gmin * 1.0e9);
        Ok(())
    }
}

// ─── The analysis flags of an operating point (FIX-11) ──────────────────────

thread_local! {
    /// `[dc, tran, ac, noise]` as each `START` reaction of `FlagProbe` read its global pins.
    pub static FLAGS: RefCell<Vec<[bool; 4]>> = const { RefCell::new(Vec::new()) };
}

/// Reads the analysis globals in a reaction.
/// pins: dc=0, tran=1, ac=2, noise=3 (GLOBAL BIT); start=4 (GLOBAL START)
/// reaction 0 "flags": on [start] reads [dc, tran, ac, noise]
pub struct FlagProbe;
static FLAG_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static FLAG_PROBE: OnceLock<Module> = OnceLock::new();

impl Device for FlagProbe {
    fn prototype() -> &'static Shape<'static> {
        FLAG_SHAPE.get_or_init(EnvironmentShapes::flag_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((FlagProbe, Arc::clone(FLAG_SHAPE.get_or_init(EnvironmentShapes::flag_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let read = [0, 1, 2, 3].map(|i| cx.bytes(i) == [1]);
        FLAGS.with(|f| f.borrow_mut().push(read));
        Ok(Flow::Continue)
    }
}
impl EnvProbe {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        PROBE.get_or_init(|| module::<EnvProbe>("EnvProbe"))
    }
}

impl FlagProbe {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        FLAG_PROBE.get_or_init(|| module::<FlagProbe>("FlagProbe"))
    }
}

impl GminLoad {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        LOAD.get_or_init(|| module::<GminLoad>("GminLoad"))
    }
}

/// The shapes the `environment` devices are built from.
pub struct EnvironmentShapes;

impl EnvironmentShapes {
    /// The shape of `EnvProbe`, built once and shared by every instance.
    pub fn probe_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let t = s.pin(PinSpec { global: global::TEMPERATURE, ..PinSpec::discrete("t", role::GLOBAL, &F64) });
        let [fg, fs, rt, rg, rs] = ["fg", "fs", "rt", "rg", "rs"].map(|n| s.pin(PinSpec::discrete(n, role::PORT, &F64)));
        let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
        let go = s.pin(PinSpec::event("go", role::PORT));
        let amb = |index| Cell { kind: cell::AMBIENT, index, part: part::VALUE };
        let pin = |index| Cell { kind: cell::PIN, index, part: part::VALUE };
        s.function(FunctionSpec {
            name: "env",
            origin: ORIGIN,
            reads: vec![amb(ambient::GMIN), amb(ambient::SOURCE_SCALE)],
            writes: vec![pin(fg), pin(fs)],
            ..FunctionSpec::default()
        });
        let st = |pin| Statement { pin, offset: 0, len: 8, flags: 0, min_after: 0.0 };
        s.reaction(ReactionSpec {
            name: "env",
            origin: ORIGIN,
            on: vec![start, go],
            reads: vec![pin(t), amb(ambient::GMIN), amb(ambient::SOURCE_SCALE)],
            writes: vec![st(rt), st(rg), st(rs)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The shape of `GminLoad`, built once and shared by every instance.
    pub fn load_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: GROUND, abstol: DEFAULT, max: DEFAULT });
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![Cell { kind: cell::PIN, index: p, part: part::VALUE }, Cell { kind: cell::AMBIENT, index: ambient::GMIN, part: part::VALUE }],
            writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The shape of `FlagProbe`, built once and shared by every instance.
    pub fn flag_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let flags = [("dc", global::DC), ("tran", global::TRAN), ("ac", global::AC), ("noise", global::NOISE)]
            .map(|(name, kind)| s.pin(PinSpec { global: kind, ..PinSpec::discrete(name, role::GLOBAL, &piperine_abi::layout::BIT) }));
        let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
        let pin = |index| Cell { kind: cell::PIN, index, part: part::VALUE };
        s.reaction(ReactionSpec { name: "flags", origin: ORIGIN, on: vec![start], reads: flags.map(pin).to_vec(), ..ReactionSpec::default() });
        s.freeze()
    }
}
