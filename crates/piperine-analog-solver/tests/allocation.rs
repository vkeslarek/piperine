//! The cost of an assembly (feature `transient-performance`, TP-07): once an `Assembly` has
//! evaluated a circuit, evaluating it again at another iterate allocates nothing, and stamping a
//! transient stage into a buffer that already holds the stage's shape allocates nothing (TP-07
//! AC 2), in both stages of a step. The circuit is one instance of a hand-written module with a
//! `FLOW`, a `DDT` and a `DELAY` term whose `eval` does nothing, so a count is the assembly's own
//! (the in-tree device thunk cuts its own frame). The allocator is counted per
//! thread, so the other tests' threads never add to a count.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell as StdCell;
use std::sync::OnceLock;

use piperine_abi::caller::Handle;
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, Created, DEFAULT, Eval, GROUND, Instance, List, Module, Note, Partial, React, Str, Value, cell, op, part, role, status};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_analog_solver::device::AnalogBinding::{Discrete, Unknown};
use piperine_analog_solver::device::{Ambient, AnalogInstance, AnalogPlan, Assembly, JumpPosition, Stage};
use piperine_analog_solver::math::integration::TrBdf2Phase;
use piperine_analog_solver::netlist::{Netlist, NodeIdentifier};

thread_local! {
    static ALLOCATIONS: StdCell<usize> = const { StdCell::new(0) };
}

/// The system allocator, counting each allocation of the calling thread.
struct Counting;

// SAFETY: every call is forwarded to `System`, which upholds the `GlobalAlloc` contract; the
// counter is a thread-local `Cell` that never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|count| count.set(count.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// The hand-written module of this target: one term of each operator the transient stage stamps.
///
/// pins: g=0, max=1 (PARAM F64); x=2, y=3, z=4 (PORT continuous);
/// terms: T0 FLOW {x, GROUND}, T1 DDT {y, GROUND}, T2 DELAY {z, GROUND} bounded by `max`;
/// fn 0 "out": reads [x, y, z, g] writes [T0, T1, T2, T2.TIME] partials (0<-0) (1<-1) (2<-2) (0<-3).
struct Terms;

impl Terms {
    /// The shape of the module.
    fn shape() -> ShapeRef {
        let mut shape = ShapeBuf::new();
        let g = shape.pin(PinSpec::discrete("g", role::PARAM, &F64));
        let max = shape.pin(PinSpec::discrete("max", role::PARAM, &F64));
        let [x, y, z] = ["x", "y", "z"].map(|name| shape.pin(PinSpec::continuous(name, role::PORT)));
        let term = |name, op, plus, max| TermSpec { name, origin: "", op, plus, minus: GROUND, abstol: DEFAULT, max };
        let t0 = shape.term(term("out", op::FLOW, x, DEFAULT));
        let t1 = shape.term(term("charge", op::DDT, y, DEFAULT));
        let t2 = shape.term(term("late", op::DELAY, z, max));
        let pin = |index| Cell { kind: cell::PIN, index, part: part::VALUE };
        let value = |index| Cell { kind: cell::TERM, index, part: part::VALUE };
        shape.function(FunctionSpec {
            name: "out",
            reads: vec![pin(x), pin(y), pin(z), pin(g)],
            writes: vec![value(t0), value(t1), value(t2), Cell { kind: cell::TERM, index: t2, part: part::TIME }],
            partials: vec![
                Partial { write: 0, to: 0, read: 0, at: 0 },
                Partial { write: 1, to: 0, read: 1, at: 0 },
                Partial { write: 2, to: 0, read: 2, at: 0 },
                Partial { write: 0, to: 0, read: 3, at: 0 },
            ],
            ..FunctionSpec::default()
        });
        shape.freeze()
    }

    /// The shape every instance has, built once.
    fn prototype() -> &'static ShapeRef {
        static PROTOTYPE: OnceLock<ShapeRef> = OnceLock::new();
        PROTOTYPE.get_or_init(Terms::shape)
    }

    /// `create` of a module that has nothing but a shape: the instance is a marker no call reads.
    unsafe extern "C" fn create(_module: *const Module, _values: List<'_, Value<'_>>, _note: *mut Note) -> Created {
        Created { status: status::OK, instance: std::ptr::NonNull::<Instance>::dangling().as_ptr(), shape: Terms::prototype().shape() }
    }

    /// `destroy` of the marker instance: nothing to free.
    unsafe extern "C" fn destroy(_instance: *mut Instance) {}

    /// `eval` that does nothing and allocates nothing: every written cell keeps its zero bytes.
    unsafe extern "C" fn eval(_instance: *mut Instance, _function: u32, _frame: *const Eval) -> u32 {
        status::OK
    }

    /// `react` of a module with no reactions.
    unsafe extern "C" fn react(_instance: *mut Instance, _reaction: u32, _frame: *const React) -> u32 {
        status::OK
    }

    /// The module.
    fn module() -> &'static Module {
        static MODULE: OnceLock<Module> = OnceLock::new();
        MODULE.get_or_init(|| {
            // SAFETY: the prototype is a static shape, and the four functions keep the contracts of
            // abi.md §2 for the process: they read nothing and the instance is never dereferenced.
            unsafe { Module::from_raw_parts(Str::from_static("terms"), Terms::prototype().shape(), std::ptr::null(), Terms::create, Terms::destroy, Terms::eval, Terms::react) }
        })
    }

    /// An assembly of one instance of the module on three nodes.
    fn assembly() -> Assembly {
        let mut netlist = Netlist::new();
        let nodes: Vec<_> = (0..3).map(|index| netlist.connect_node(NodeIdentifier::Anonymous(index))).collect();
        let handle = Handle::create(Terms::module(), &[]).expect("create terms");
        let pins = [Discrete(0), Discrete(1)].into_iter().chain(nodes.into_iter().map(Unknown)).collect();
        let instance = AnalogInstance { path: "terms".into(), handle, pins };
        Assembly::new(AnalogPlan::bind(vec![instance], &netlist).expect("bind"))
    }
}

/// What the tests of this target read of a run.
struct Probe;

impl Probe {
    /// The ambient of every evaluation.
    fn ambient() -> Ambient {
        Ambient { time: 0.0, freq: 0.0, is_static: false, gmin: 1e-12, source_scale: 1.0 }
    }

    /// The allocations the calling thread makes while `work` runs.
    fn allocations_during(work: impl FnOnce()) -> usize {
        let before = ALLOCATIONS.with(StdCell::get);
        work();
        ALLOCATIONS.with(StdCell::get) - before
    }
}

#[test]
fn the_second_evaluation_at_a_changed_iterate_allocates_nothing() {
    let mut assembly = Terms::assembly();
    assembly.evaluate(&[0.5; 3], Probe::ambient()).expect("the first evaluation");

    let allocations = Probe::allocations_during(|| {
        assembly.evaluate(&[0.75; 3], Probe::ambient()).expect("the second evaluation");
    });

    assert_eq!(allocations, 0, "the second evaluation allocated");
}

#[test]
fn the_second_stamping_of_each_stage_into_a_kept_buffer_allocates_nothing() {
    for phase in [TrBdf2Phase::Trapezoidal, TrBdf2Phase::Bdf2] {
        let mut assembly = Terms::assembly();
        assembly.push_stage(&[0.25; 3], Ambient { time: 0.0, ..Probe::ambient() }).expect("a row of history");
        let stage = Stage { phase, h: 1e-6, prev_h: 1e-6, jump: JumpPosition::None };
        let ambient = Ambient { time: 1e-6, ..Probe::ambient() };
        let mut stamps = Vec::new();
        assembly.assemble_tran_into(&[0.5; 3], ambient, stage, &mut stamps).expect("the first stamping");
        let first = stamps.len();

        let allocations = Probe::allocations_during(|| {
            assembly.assemble_tran_into(&[0.75; 3], ambient, stage, &mut stamps).expect("the second stamping");
        });

        assert_eq!(allocations, 0, "the second stamping of the {phase:?} stage allocated");
        assert_eq!(stamps.len(), first, "the same stage has the same stamps");
        assert!(first >= 4, "the three terms and the parameter's partial stamp: {first}");
    }
}
