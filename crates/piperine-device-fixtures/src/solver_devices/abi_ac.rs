//! The devices of the solver target `abi_ac`, one type per device.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::raw::{Module, Shape, Statement, global, role};
use piperine_abi::shape::{PinSpec, ReactionSpec, ShapeBuf, ShapeRef};
use piperine_abi::layout::BYTE;
use piperine_abi::thunk::module;

/// The reactions `Spy` has run in this process.
pub static REACTIONS: AtomicUsize = AtomicUsize::new(0);

/// A device with a reaction on `START` that counts its runs: pins start=0 (GLOBAL START),
/// log=1 (PRIVATE BYTE); reaction 0 on [start] writes [S0 {log}].
pub struct Spy;
static SPY_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SPY_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for Spy {
    fn prototype() -> &'static Shape<'static> {
        SPY_SHAPE.get_or_init(AbiAcShapes::spy_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Spy, Arc::clone(SPY_SHAPE.get_or_init(AbiAcShapes::spy_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        REACTIONS.fetch_add(1, Ordering::SeqCst);
        cx.commit(0, &[1], 0.0)?;
        Ok(Flow::Continue)
    }
}
impl Spy {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        SPY_MODULE.get_or_init(|| module::<Spy>("Spy"))
    }
}

/// The shapes the `abi_ac` devices are built from.
pub struct AbiAcShapes;

impl AbiAcShapes {
    /// The shape of `Spy`, built once and shared by every instance.
    pub fn spy_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
        let log = s.pin(PinSpec::discrete("log", role::PRIVATE, &BYTE));
        s.reaction(ReactionSpec {
            name: "run",
            origin: "abi_ac.rs",
            on: vec![start],
            writes: vec![Statement { pin: log, offset: 0, len: 1, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
        s.freeze()
    }
}
