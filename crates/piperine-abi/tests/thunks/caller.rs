//! The solver-side caller (feature `solver-abi-core`, SAC-08): `Handle`
//! builds the frames of abi.md §3, passes them to the module's pointers and
//! returns the status as a `Result`; `FAULT` and `RESHAPE` are errors with
//! code and message; dropping a handle destroys its instance; `Handle` is
//! `Send`.

use std::cell::Cell as StdCell;
use std::sync::OnceLock;

use piperine_abi::caller::{Commit, Handle};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, F64, HIGH};
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Partial, Shape, Statement, cell, op, part, role, status};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;

thread_local! {
    static DROPS: StdCell<usize> = const { StdCell::new(0) };
}

struct Amplifier;

impl Drop for Amplifier {
    fn drop(&mut self) {
        DROPS.with(|d| d.set(d.get() + 1));
    }
}

impl Device for Amplifier {
    fn prototype() -> &'static Shape<'static> {
        static PROTO: OnceLock<ShapeRef> = OnceLock::new();
        PROTO.get_or_init(AmplifierDevice::amplifier_shape).shape()
    }
    fn create(_values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Amplifier, AmplifierDevice::amplifier_shape()))
    }
    fn eval(&self, k: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        match k {
            0 => {
                let (x, g) = (cx.f64(0), cx.f64(1));
                cx.set(0, g * x);
                if cx.wanted(0) {
                    cx.jac()[0] = g;
                }
                if cx.wanted(1) {
                    cx.jac()[1] = x;
                }
                Ok(())
            }
            1 => Err(Fault::reshape("needs another shape")),
            _ => Err(Fault::new(41, "out of range")),
        }
    }
    fn react(&self, k: u32, cx: &mut React<'_>) -> Result<Flow, Fault> {
        if k == 1 {
            return Ok(Flow::Finish);
        }
        if cx.f64(0) > 0.0 {
            cx.commit(0, &HIGH, 2e-9)?;
        }
        Ok(Flow::Continue)
    }
}

#[test]
fn handle_builds_the_frames_and_returns_results() {
    let g = 4.0f64.to_le_bytes();
    let mut h = Handle::create(AmplifierDevice::amplifier_module(), &[(0, &g)]).expect("create");
    assert_eq!(h.shape().functions.len(), 3);
    assert_eq!(h.module().name().as_str(), "amp");

    let x = 1.5f64.to_le_bytes();
    let mut out = [0u8; 8];
    let mut jac = [0.0; 2];
    h.eval(0, &[&x, &g], &mut [&mut out], &[1, 0], &mut jac).expect("eval");
    assert_eq!(f64::from_le_bytes(out), 6.0);
    assert_eq!(jac, [4.0, 0.0], "only the seeded partial is written");

    let mut seen: Vec<(u32, u32, Vec<u8>, f64)> = Vec::new();
    let mut sink = |c: Commit<'_>| {
        seen.push((c.statement, c.offset, c.bytes.to_vec(), c.after));
        status::OK
    };
    assert_eq!(h.react(0, &[&x], &mut sink), Ok(Flow::Continue));
    assert_eq!(h.react(1, &[], &mut sink), Ok(Flow::Finish));
    assert_eq!(seen, [(0, 0, HIGH.to_vec(), 2e-9)]);

    // A frame that does not match the function is refused before any call.
    let err = h.eval(0, &[&x], &mut [&mut out], &[1, 1], &mut jac).expect_err("one read missing");
    assert_eq!(err.status, status::FAULT);
    let short = [0u8; 4];
    assert!(h.eval(0, &[&short, &g], &mut [&mut out], &[1, 1], &mut jac).is_err(), "a read shorter than its cell");
    assert!(h.eval(9, &[], &mut [], &[], &mut []).is_err(), "no function 9");
}

#[test]
fn handle_reports_fault_and_reshape_as_errors() {
    let h = Handle::create(AmplifierDevice::amplifier_module(), &[]).expect("create");
    let reshape = h.eval(1, &[], &mut [], &[], &mut []).expect_err("RESHAPE");
    assert_eq!(reshape.status, status::RESHAPE);
    assert_eq!(reshape.message, "needs another shape");
    let fault = h.eval(2, &[], &mut [], &[], &mut []).expect_err("FAULT");
    assert_eq!((fault.status, fault.code, fault.message.as_str()), (status::FAULT, 41, "out of range"));

    let err = Handle::create(AmplifierDevice::amplifier_module(), &[(1, &[0u8; 8])]).expect_err("a value for a port");
    assert_eq!(err.status, status::FAULT);
    assert!(err.message.contains("pin 1"), "{}", err.message);
}

#[test]
fn dropping_a_handle_destroys_the_instance() {
    let before = DROPS.with(StdCell::get);
    let h = Handle::create(AmplifierDevice::amplifier_module(), &[]).expect("create");
    assert_eq!(DROPS.with(StdCell::get), before);
    drop(h);
    assert_eq!(DROPS.with(StdCell::get), before + 1);
}

#[test]
fn handle_is_send() {
    AmplifierDevice::assert_send::<Handle>();
    let h = Handle::create(AmplifierDevice::amplifier_module(), &[]).expect("create");
    let functions = std::thread::spawn(move || h.shape().functions.len()).join().unwrap();
    assert_eq!(functions, 3);
}

#[test]
fn a_shared_handle_is_one_instance_destroyed_with_its_last_share() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CREATES: AtomicUsize = AtomicUsize::new(0);
    static DESTROYS: AtomicUsize = AtomicUsize::new(0);
    struct Counted;
    impl piperine_abi::device::Device for Counted {
        fn prototype() -> &'static piperine_abi::raw::Shape<'static> {
            static S: std::sync::OnceLock<piperine_abi::shape::ShapeRef> = std::sync::OnceLock::new();
            S.get_or_init(|| piperine_abi::shape::ShapeBuf::new().freeze()).shape()
        }
        fn create(_: &piperine_abi::device::Values<'_>) -> Result<(Self, piperine_abi::shape::ShapeRef), piperine_abi::device::Fault> {
            CREATES.fetch_add(1, Ordering::SeqCst);
            Ok((Counted, std::sync::Arc::new(piperine_abi::shape::ShapeBuf::new())))
        }
        fn eval(&self, _: u32, _: &mut piperine_abi::device::Eval) -> Result<(), piperine_abi::device::Fault> {
            Ok(())
        }
        fn react(&self, _: u32, _: &mut piperine_abi::device::React) -> Result<piperine_abi::device::Flow, piperine_abi::device::Fault> {
            Ok(piperine_abi::device::Flow::Continue)
        }
    }
    impl Drop for Counted {
        fn drop(&mut self) {
            DESTROYS.fetch_add(1, Ordering::SeqCst);
        }
    }
    static MODULE: std::sync::OnceLock<piperine_abi::raw::Module> = std::sync::OnceLock::new();
    let module = MODULE.get_or_init(|| piperine_abi::thunk::module::<Counted>("counted"));
    let a = Handle::create(module, &[]).expect("create");
    let b = a.share();
    assert_eq!(CREATES.load(Ordering::SeqCst), 1, "sharing creates nothing");
    drop(a);
    assert_eq!(DESTROYS.load(Ordering::SeqCst), 0, "a share keeps the instance alive");
    drop(b);
    assert_eq!(DESTROYS.load(Ordering::SeqCst), 1);
}

#[test]
fn a_commit_reaches_the_owner() {
    // abi.md §2 entry 6: `commit` reaches the owner's closure with the statement, the bytes and
    // the delay; it is the host's one entry
    let mut h = Handle::create(AmplifierDevice::amplifier_module(), &[]).expect("create");
    let x = 1.5f64.to_le_bytes();
    let mut seen: Vec<(u32, Vec<u8>, f64)> = Vec::new();
    let mut sink = |c: Commit<'_>| {
        seen.push((c.statement, c.bytes.to_vec(), c.after));
        status::OK
    };
    assert_eq!(h.react(0, &[&x], &mut sink), Ok(Flow::Continue));
    assert_eq!(seen, [(0, HIGH.to_vec(), 2e-9)]);
}

/// The amplifier device the caller tests drive.
pub struct AmplifierDevice;

impl AmplifierDevice {
    /// pins: g=0 (PARAM F64), x=1 (PORT continuous), q=2 (PORT BIT), e=3 (PRIVATE event)
    /// term T0 FLOW {x, GROUND}
    /// fn 0 "out": reads [x, g] writes [T0] partials (0←0) (0←1)
    /// fn 1 "reshape": returns RESHAPE.  fn 2 "fault": returns a Fault with code 41.
    /// reaction 0 "drive": on [e] reads [x] writes [S0 {q}]: commits HIGH after 2 ns
    /// reaction 1 "stop": on [e] returns Finish
    fn amplifier_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let g = s.pin(PinSpec::discrete("g", role::PARAM, &F64));
        let x = s.pin(PinSpec::continuous("x", role::PORT));
        let q = s.pin(PinSpec::discrete("q", role::PORT, &BIT));
        let e = s.pin(PinSpec::event("e", role::PRIVATE));
        let t0 = s.term(TermSpec { name: "out", origin: "", op: op::FLOW, plus: x, minus: GROUND, abstol: DEFAULT, max: DEFAULT });
        let pin = |index| Cell { kind: cell::PIN, index, part: part::VALUE };
        s.function(FunctionSpec {
            name: "out",
            reads: vec![pin(x), pin(g)],
            writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
            partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec { name: "reshape", ..FunctionSpec::default() });
        s.function(FunctionSpec { name: "fault", ..FunctionSpec::default() });
        s.reaction(ReactionSpec {
            name: "drive",
            on: vec![e],
            reads: vec![pin(x)],
            writes: vec![Statement { pin: q, offset: 0, len: 1, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
        s.reaction(ReactionSpec { name: "stop", on: vec![e], ..ReactionSpec::default() });
        s.freeze()
    }

    /// The amp module helper `amp_module`.
    fn amplifier_module() -> &'static Module {
        static MODULE: OnceLock<Module> = OnceLock::new();
        MODULE.get_or_init(|| module::<Amplifier>("amp"))
    }

    /// The assert send helper `assert_send`.
    fn assert_send<T: Send>() {}
}
