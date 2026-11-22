//! The cost of an assembly (feature `transient-performance`, TP-07): once an `Assembly` has
//! evaluated a circuit, evaluating it again at another iterate allocates nothing, and stamping a
//! transient stage into a buffer that already holds the stage's shape allocates nothing. The
//! circuit is one instance of a hand-written module whose `eval` does nothing, so a count is the
//! assembly's own (the in-tree device thunk cuts its own frame). The allocator is counted per
//! thread, so the other tests' threads never add to a count.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell as StdCell;
use std::sync::OnceLock;

use piperine_abi::caller::Handle;
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, Created, DEFAULT, Eval, GROUND, Instance, List, Module, Note, Partial, React, Str, Value, cell, op, part, role, status};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_analog_solver::device::AnalogBinding::{Discrete, Unknown};
use piperine_analog_solver::device::{Ambient, AnalogInstance, AnalogPlan, Assembly};
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

/// pins: g=0 (PARAM F64), x=1 (PORT continuous); term T0 FLOW {x, GROUND};
/// fn 0 "out": reads [x, g] writes [T0] partials (0←0) (0←1).
fn gain_shape() -> ShapeRef {
    let mut shape = ShapeBuf::new();
    let g = shape.pin(PinSpec::discrete("g", role::PARAM, &F64));
    let x = shape.pin(PinSpec::continuous("x", role::PORT));
    let t0 = shape.term(TermSpec { name: "out", origin: "", op: op::FLOW, plus: x, minus: GROUND, abstol: DEFAULT, max: DEFAULT });
    let pin = |index| Cell { kind: cell::PIN, index, part: part::VALUE };
    shape.function(FunctionSpec {
        name: "out",
        reads: vec![pin(x), pin(g)],
        writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
        partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
        ..FunctionSpec::default()
    });
    shape.freeze()
}

/// The shape every instance of `gain_module` has, built once.
fn gain_prototype() -> &'static ShapeRef {
    static PROTOTYPE: OnceLock<ShapeRef> = OnceLock::new();
    PROTOTYPE.get_or_init(gain_shape)
}

/// `create` of a module that has nothing but a shape: the instance is a marker no call reads.
unsafe extern "C" fn create(_module: *const Module, _values: List<'_, Value<'_>>, _note: *mut Note) -> Created {
    Created { status: status::OK, instance: std::ptr::NonNull::<Instance>::dangling().as_ptr(), shape: gain_prototype().shape() }
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

fn gain_module() -> &'static Module {
    static MODULE: OnceLock<Module> = OnceLock::new();
    MODULE.get_or_init(|| {
        // SAFETY: the prototype is a static shape, and the four functions keep the contracts of
        // abi.md §2 for the process: they read nothing and the instance is never dereferenced.
        unsafe {
            Module::from_raw_parts(Str::from_static("gain"), gain_prototype().shape(), std::ptr::null(), create, destroy, eval, react)
        }
    })
}

/// An assembly of one `gain` instance on node 0, and the node count.
fn assembly() -> Assembly {
    let mut netlist = Netlist::new();
    let node = netlist.connect_node(NodeIdentifier::Anonymous(0));
    let handle = Handle::create(gain_module(), &[]).expect("create gain");
    let instance = AnalogInstance { path: "gain".into(), handle, pins: vec![Discrete(0), Unknown(node)] };
    Assembly::new(AnalogPlan::bind(vec![instance], &netlist).expect("bind"))
}

fn ambient() -> Ambient {
    Ambient { time: 0.0, freq: 0.0, is_static: false, gmin: 1e-12, source_scale: 1.0 }
}

/// The allocations the calling thread makes while `work` runs.
fn allocations_during(work: impl FnOnce()) -> usize {
    let before = ALLOCATIONS.with(StdCell::get);
    work();
    ALLOCATIONS.with(StdCell::get) - before
}

#[test]
fn the_second_evaluation_at_a_changed_iterate_allocates_nothing() {
    let mut assembly = assembly();
    assembly.evaluate(&[0.5], ambient()).expect("the first evaluation");

    let allocations = allocations_during(|| {
        assembly.evaluate(&[0.75], ambient()).expect("the second evaluation");
    });

    assert_eq!(allocations, 0, "the second evaluation allocated");
}
