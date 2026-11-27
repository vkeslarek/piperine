//! The devices of the solver target `lifecycle`, one type per device.

use std::result::Result;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use piperine_abi::device::{Device, Eval, Fault, Values};
use piperine_abi::raw::{Module, Shape, role};
use piperine_abi::shape::{PinSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;

/// The shape of `Counted` and `Failing`: one port and no param.
pub static SHAPE: OnceLock<ShapeRef> = OnceLock::new();

/// The creations `Counted` has seen in this process.
pub static CREATES: AtomicUsize = AtomicUsize::new(0);
/// The destructions `Counted` has seen in this process.
pub static DESTROYS: AtomicUsize = AtomicUsize::new(0);
static COUNTED: OnceLock<Module> = OnceLock::new();

/// Counts its creations and its destruction.
pub struct Counted;

impl Drop for Counted {
    fn drop(&mut self) {
        DESTROYS.fetch_add(1, Ordering::SeqCst);
    }
}

impl Device for Counted {
    fn prototype() -> &'static Shape<'static> {
        LifecycleShapes::shared().shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        CREATES.fetch_add(1, Ordering::SeqCst);
        Ok((Counted, Arc::clone(LifecycleShapes::shared())))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
}

/// The creations `Failing` has been asked for in this process.
pub static FAIL_CREATES: AtomicUsize = AtomicUsize::new(0);
static FAILING: OnceLock<Module> = OnceLock::new();

/// Fails at creation.
pub struct Failing;

impl Device for Failing {
    fn prototype() -> &'static Shape<'static> {
        LifecycleShapes::shared().shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        FAIL_CREATES.fetch_add(1, Ordering::SeqCst);
        Err(Fault::new(1, "test setup error"))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
}
impl Counted {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        COUNTED.get_or_init(|| module::<Counted>("Counted"))
    }
}

impl Failing {
    /// The module of the device, built once.
    pub fn module() -> &'static Module {
        FAILING.get_or_init(|| module::<Failing>("Failing"))
    }
}

/// The shapes the `lifecycle` devices are built from.
pub struct LifecycleShapes;

impl LifecycleShapes {
    /// The shape of the shape fixture, built once and shared by every instance.
    pub fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec::continuous("p", role::PORT));
        s.freeze()
    }

    /// The shape `build` makes, built the first time and kept in `slot` so every instance shares it.
    pub fn shared() -> &'static ShapeRef {
        SHAPE.get_or_init(LifecycleShapes::shape)
    }
}
