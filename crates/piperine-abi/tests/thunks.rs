//! The generic `extern "C"` thunks over a Rust `Device` (feature
//! `solver-abi-core`, SAC-04, SAC-05, SAC-06): the pointers of `module::<D>`
//! call the device, `destroy` drops it and releases its shape, a panic or a
//! `Fault` is a `FAULT` with its message in the note and no unwinding,
//! `cx.commit` reaches `Host.commit` once, `Flow::Finish` is `FINISH`, a value
//! for a non-param pin is refused, and `library` fills its header. The
//! raw-pointer calls here stand in for a solver.

use std::cell::{Cell as StdCell, RefCell};
use std::ffi::c_void;
use std::sync::{Arc, OnceLock, Weak};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, F64, HIGH, LOW};
use piperine_abi::raw::{
    self, Bytes, Cell, Created, DEFAULT, GROUND, Host, List, MINOR, Module, NOTE_TEXT, Note, Partial, Shape, Statement,
    Value, cell, op, part, pin_flag, role, status,
};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::{library, module};

thread_local! {
    static EVALS: StdCell<usize> = const { StdCell::new(0) };
    static DROPS: StdCell<usize> = const { StdCell::new(0) };
    static LAST_SHAPE: RefCell<Weak<ShapeBuf>> = const { RefCell::new(Weak::new()) };
}

const GAIN_DEFAULT: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0x40]; // 2.0f64

/// pins: gain=0 (PARAM F64), x=1 (PORT continuous), q=2 (PORT BIT), e=3 (PRIVATE event)
/// term T0 FLOW {x, GROUND}
/// fn 0 "out": reads [x, gain] writes [T0] partials (0←0) (0←1)
/// fn 1 "boom": panics.   fn 2 "fault": returns a Fault with code 77.
/// reaction 0 "drive": on [e] reads [x] writes [S0 {q}]
/// reaction 1 "tick":  on [e] writes [S0 {e}] (an event statement)
/// reaction 2 "stop":  on [e] returns Finish
fn probe_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let gain = s.pin(PinSpec {
        flags: pin_flag::HAS_INIT,
        init: &GAIN_DEFAULT,
        origin: "probe:1:1",
        ..PinSpec::discrete("gain", role::PARAM, &F64)
    });
    let x = s.pin(PinSpec { origin: "probe:2:1", ..PinSpec::continuous("x", role::PORT) });
    let q = s.pin(PinSpec { origin: "probe:3:1", ..PinSpec::discrete("q", role::PORT, &BIT) });
    let e = s.pin(PinSpec { origin: "probe:4:1", ..PinSpec::event("e", role::PRIVATE) });
    let t0 = s.term(TermSpec {
        name: "out",
        origin: "probe:5:1",
        op: op::FLOW,
        plus: x,
        minus: GROUND,
        abstol: DEFAULT,
        max: DEFAULT,
    });
    let pin = |index, part| Cell { kind: cell::PIN, index, part };
    s.function(FunctionSpec {
        name: "out",
        origin: "probe:5:1",
        reads: vec![pin(x, part::VALUE), pin(gain, part::VALUE)],
        writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
        partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
        ..FunctionSpec::default()
    });
    for name in ["boom", "fault"] {
        s.function(FunctionSpec { name, origin: "probe:6:1", ..FunctionSpec::default() });
    }
    s.reaction(ReactionSpec {
        name: "drive",
        origin: "probe:7:1",
        on: vec![e],
        reads: vec![pin(x, part::VALUE)],
        writes: vec![Statement { pin: q, offset: 0, len: 1, flags: 0, min_after: 0.0 }],
        ..ReactionSpec::default()
    });
    s.reaction(ReactionSpec {
        name: "tick",
        origin: "probe:8:1",
        on: vec![e],
        writes: vec![Statement { pin: e, offset: 0, len: 0, flags: 0, min_after: 0.0 }],
        ..ReactionSpec::default()
    });
    s.reaction(ReactionSpec { name: "stop", origin: "probe:9:1", on: vec![e], ..ReactionSpec::default() });
    s.freeze()
}

/// A message of 300 bytes whose 2-byte `é` straddles byte 248.
fn long_message() -> String {
    format!("{}é{}", "a".repeat(247), "b".repeat(51))
}

struct Probe {
    gain: f64,
}

impl Drop for Probe {
    fn drop(&mut self) {
        DROPS.with(|d| d.set(d.get() + 1));
    }
}

impl Device for Probe {
    fn prototype() -> &'static Shape<'static> {
        static PROTO: OnceLock<ShapeRef> = OnceLock::new();
        PROTO.get_or_init(probe_shape).shape()
    }

    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        let gain = values.f64(0).unwrap_or(2.0);
        if gain < 0.0 {
            panic!("negative gain {gain}");
        }
        let shape = probe_shape();
        LAST_SHAPE.with(|w| *w.borrow_mut() = Arc::downgrade(&shape));
        Ok((Probe { gain }, shape))
    }

    fn eval(&self, k: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        EVALS.with(|n| n.set(n.get() + 1));
        match k {
            0 => {
                let (x, g) = (cx.f64(0), cx.f64(1));
                assert_eq!(g, self.gain, "the solver seeds the param net with the created value");
                cx.set(0, g * x);
                cx.jac().copy_from_slice(&[g, x]);
                Ok(())
            }
            1 => panic!("probe panicked on purpose"),
            _ => Err(Fault::new(77, long_message())),
        }
    }

    fn react(&self, k: u32, cx: &mut React<'_>) -> Result<Flow, Fault> {
        match k {
            0 => {
                let level = if cx.f64(0) > 0.0 { HIGH } else { LOW };
                cx.commit(0, &level, 1e-9)?;
                Ok(Flow::Continue)
            }
            1 => {
                cx.commit(0, &[], 0.0)?;
                Ok(Flow::Continue)
            }
            _ => Ok(Flow::Finish),
        }
    }
}

fn empty_note() -> Note {
    Note { code: 0, len: 0, text: [0; NOTE_TEXT] }
}

fn note_text(note: &Note) -> &str {
    std::str::from_utf8(&note.text[..note.len as usize]).expect("the note is UTF-8")
}

fn create_with(m: &Module, values: &[Value], note: &mut Note) -> Created {
    // SAFETY: `m` comes from `module::<Probe>`, `values` and `note` live for the call.
    unsafe { (m.create())(m, List::from_slice(values), note) }
}

/// Calls function `k` with the given reads; returns (status, output, jac).
fn eval_with(m: &Module, inst: &Created, k: u32, reads: &[&[u8]], note: &mut Note) -> (u32, [u8; 8], [f64; 2]) {
    let input: Vec<*const u8> = reads.iter().map(|r| r.as_ptr()).collect();
    let mut out = [0u8; 8];
    let output = [out.as_mut_ptr()];
    let seed = [1u8, 1];
    let mut jac = [0.0f64; 2];
    let frame = raw::Eval {
        size: size_of::<raw::Eval>() as u32,
        input: input.as_ptr(),
        output: output.as_ptr(),
        seed: seed.as_ptr(),
        jac: jac.as_mut_ptr(),
        note,
    };
    // SAFETY: the frame holds one pointer per read and write of function `k`, each to its cell's bytes.
    let s = unsafe { (m.eval())(inst.instance, k, &frame) };
    (s, out, jac)
}

/// What the recording host saw: (statement, offset, len, bytes, after).
type Seen = Vec<(u32, u32, u32, Vec<u8>, f64)>;

struct Recorder {
    seen: Seen,
    answer: u32,
}

unsafe extern "C" fn record(data: *mut c_void, statement: u32, offset: u32, len: u32, bytes: *const u8, after: f64) -> u32 {
    // SAFETY: `data` is the `Recorder` passed as `host_data` by `react_with`.
    let rec = unsafe { &mut *data.cast::<Recorder>() };
    // SAFETY: abi.md §2 entry 6: `bytes` holds `len` bytes.
    let b = if len == 0 { Vec::new() } else { unsafe { std::slice::from_raw_parts(bytes, len as usize) }.to_vec() };
    rec.seen.push((statement, offset, len, b, after));
    rec.answer
}

unsafe extern "C" fn refuse_element(_: *mut c_void, _: u32, _: u64, _: u32, _: u32, _: *const u8, _: f64) -> u32 {
    status::FAULT
}

fn react_with(m: &Module, inst: &Created, k: u32, reads: &[&[u8]], answer: u32, note: &mut Note) -> (u32, Seen) {
    let input: Vec<*const u8> = reads.iter().map(|r| r.as_ptr()).collect();
    let host = Host { size: size_of::<Host>() as u32, commit: record, commit_element: refuse_element };
    let mut rec = Recorder { seen: Vec::new(), answer };
    let frame = raw::React {
        size: size_of::<raw::React>() as u32,
        input: input.as_ptr(),
        host: &host,
        host_data: (&mut rec as *mut Recorder).cast::<c_void>(),
        note,
    };
    // SAFETY: the frame holds one pointer per read of reaction `k`; `host` and `rec` outlive the call.
    let s = unsafe { (m.react())(inst.instance, k, &frame) };
    (s, rec.seen)
}

fn destroy(m: &Module, inst: &Created) {
    // SAFETY: `inst` came from `m.create` and is destroyed once.
    unsafe { (m.destroy())(inst.instance) }
}

fn gain_value(bytes: &'static [u8; 8]) -> Value<'static> {
    Value { pin: 0, bytes: Bytes::from_static(bytes) }
}

static GAIN_3: [u8; 8] = [0, 0, 0, 0, 0, 0, 0x08, 0x40]; // 3.0f64
static GAIN_NEG: [u8; 8] = [0, 0, 0, 0, 0, 0, 0xF0, 0xBF]; // -1.0f64

#[test]
fn module_pointers_call_the_device() {
    assert_eq!(f64::from_le_bytes(GAIN_3), 3.0);
    let m = module::<Probe>("probe");
    assert_eq!(m.name().as_str(), "probe");
    assert_eq!(m.prototype_ptr(), Probe::prototype() as *const Shape);
    assert!(m.data().is_null());

    let mut note = empty_note();
    let inst = create_with(&m, &[gain_value(&GAIN_3)], &mut note);
    assert_eq!(inst.status, status::OK, "{}", note_text(&note));
    assert!(!inst.instance.is_null() && !inst.shape.is_null());
    // SAFETY: a created shape lives until the instance is destroyed.
    let shape = unsafe { &*inst.shape };
    assert_eq!(shape.functions.len(), 3);

    let before = EVALS.with(StdCell::get);
    let x = 2.0f64.to_le_bytes();
    let (s, out, jac) = eval_with(&m, &inst, 0, &[&x, &GAIN_3], &mut note);
    assert_eq!(s, status::OK);
    assert_eq!(f64::from_le_bytes(out), 6.0, "gain * x");
    assert_eq!(jac, [3.0, 2.0], "d/dx = gain, d/dgain = x");
    assert_eq!(EVALS.with(StdCell::get), before + 1, "one call reaches the device once");

    let drops = DROPS.with(StdCell::get);
    destroy(&m, &inst);
    assert_eq!(DROPS.with(StdCell::get), drops + 1);
}

#[test]
fn destroy_drops_the_device_and_releases_the_shape() {
    let m = module::<Probe>("probe");
    let mut note = empty_note();
    let drops = DROPS.with(StdCell::get);
    let inst = create_with(&m, &[], &mut note);
    assert_eq!(inst.status, status::OK);
    let shape = LAST_SHAPE.with(|w| w.borrow().clone());
    assert_eq!(shape.strong_count(), 1, "the instance holds its shape");
    assert_eq!(shape.upgrade().map(|s| s.as_raw()), Some(inst.shape));
    destroy(&m, &inst);
    assert_eq!(DROPS.with(StdCell::get), drops + 1, "destroy drops the device");
    assert_eq!(shape.strong_count(), 0, "and releases the shape");
    assert!(shape.upgrade().is_none());
}

#[test]
fn a_panic_is_a_fault_with_its_message() {
    let m = module::<Probe>("probe");
    let mut note = empty_note();
    let inst = create_with(&m, &[], &mut note);
    let (s, _, _) = eval_with(&m, &inst, 1, &[], &mut note);
    assert_eq!(s, status::FAULT);
    assert!(note_text(&note).contains("probe panicked on purpose"), "{}", note_text(&note));

    // A panic in `create` is a FAULT too, with no instance.
    let mut note = empty_note();
    let failed = create_with(&m, &[gain_value(&GAIN_NEG)], &mut note);
    assert_eq!(failed.status, status::FAULT);
    assert!(failed.instance.is_null() && failed.shape.is_null());
    assert!(note_text(&note).contains("negative gain -1"), "{}", note_text(&note));
    destroy(&m, &inst);
}

#[test]
fn a_fault_carries_its_code_and_a_cut_message() {
    let m = module::<Probe>("probe");
    let mut note = empty_note();
    let inst = create_with(&m, &[], &mut note);
    let msg = long_message();
    assert_eq!(msg.len(), 300);
    assert!(!msg.is_char_boundary(248), "the cut falls inside `é`");
    let (s, _, _) = eval_with(&m, &inst, 2, &[], &mut note);
    assert_eq!(s, status::FAULT);
    assert_eq!(note.code, 77);
    assert_eq!(note.len, 247, "cut back to the character boundary below 248");
    assert_eq!(note_text(&note), "a".repeat(247));
    destroy(&m, &inst);
}

#[test]
fn commit_reaches_the_host_once() {
    let m = module::<Probe>("probe");
    let mut note = empty_note();
    let inst = create_with(&m, &[], &mut note);
    let x = 0.5f64.to_le_bytes();
    let (s, seen) = react_with(&m, &inst, 0, &[&x], status::OK, &mut note);
    assert_eq!(s, status::OK);
    assert_eq!(seen, [(0, 0, 1, HIGH.to_vec(), 1e-9)], "statement 0, offset 0, one byte, after");
    destroy(&m, &inst);
}

#[test]
fn an_event_statement_commits_zero_bytes() {
    let m = module::<Probe>("probe");
    let mut note = empty_note();
    let inst = create_with(&m, &[], &mut note);
    let (s, seen) = react_with(&m, &inst, 1, &[], status::OK, &mut note);
    assert_eq!(s, status::OK);
    assert_eq!(seen, [(0, 0, 0, vec![], 0.0)]);
    destroy(&m, &inst);
}

#[test]
fn a_refused_commit_is_an_error() {
    let m = module::<Probe>("probe");
    let mut note = empty_note();
    let inst = create_with(&m, &[], &mut note);
    let x = (-1.0f64).to_le_bytes();
    let (s, seen) = react_with(&m, &inst, 0, &[&x], status::FAULT, &mut note);
    assert_eq!(seen.len(), 1, "the host was asked once");
    assert_eq!(s, status::FAULT, "the reaction propagated the refusal");
    assert_eq!(note.code, status::FAULT, "the note carries the host's status");
    assert!(note_text(&note).contains("statement 0"), "{}", note_text(&note));
    destroy(&m, &inst);
}

#[test]
fn finish_is_a_status() {
    let m = module::<Probe>("probe");
    let mut note = empty_note();
    let inst = create_with(&m, &[], &mut note);
    let (s, seen) = react_with(&m, &inst, 2, &[], status::OK, &mut note);
    assert_eq!(s, status::FINISH);
    assert!(seen.is_empty());
    destroy(&m, &inst);
}

#[test]
fn a_value_for_a_non_param_pin_is_refused() {
    let m = module::<Probe>("probe");
    for pin in [1u32, 99] {
        let mut note = empty_note();
        let value = Value { pin, bytes: Bytes::from_static(&GAIN_3) };
        let created = create_with(&m, &[value], &mut note);
        assert_eq!(created.status, status::FAULT);
        assert!(created.instance.is_null());
        assert!(note_text(&note).contains(&format!("pin {pin}")), "{}", note_text(&note));
    }
}

#[test]
fn library_fills_its_header() {
    static MODULES: OnceLock<[Module; 1]> = OnceLock::new();
    let modules = MODULES.get_or_init(|| [module::<Probe>("probe")]);
    let lib = library("probes", "test", modules);
    assert_eq!(lib.size as usize, size_of::<piperine_abi::raw::Library>());
    assert_eq!(lib.minor, MINOR);
    assert_eq!((lib.name.as_str(), lib.build.as_str()), ("probes", "test"));
    assert_eq!(lib.modules.iter().map(|m| m.name().as_str()).collect::<Vec<_>>(), ["probe"]);
}

#[test]
fn an_unreadable_value_list_is_a_fault() {
    let m = module::<Probe>("probe");
    let values = [gain_value(&GAIN_3)];
    // An older host compiled `Value` smaller: its stride is below this one's.
    // SAFETY: the pointer is live for the call; the list is refused, never read.
    let short = unsafe { List::from_raw_parts(values.as_ptr(), 1, 8) };
    let mut note = empty_note();
    // SAFETY: `m` comes from `module::<Probe>`; `note` lives for the call.
    let created = unsafe { (m.create())(&m, short, &mut note) };
    assert_eq!(created.status, status::FAULT, "never created on its defaults with OK");
    assert!(created.instance.is_null());
    assert!(note_text(&note).contains("value list"), "{}", note_text(&note));
}
