//! The cost of a function call (feature `transient-performance`, TP-07 AC 1): a `Handle` computes
//! the cell lengths of each function once, at `create`, and keeps the pointer lists of a frame
//! between calls, so the second `eval` of a function allocates nothing, and a buffer shorter than
//! its cell is still refused with the text it always had. The module is hand-written with an
//! `eval` that does nothing, so a count is the `Handle`'s own. The allocator is counted per
//! thread, so the other tests' threads never add to a count.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell as StdCell;
use std::sync::OnceLock;

use piperine_abi::caller::Handle;
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, Created, DEFAULT, Eval, GROUND, Instance, List, Module, Note, Partial, React, Str, Value, cell, op, part, role, status};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};

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

/// The hand-written `gain` module of this target.
///
/// pins: g=0 (PARAM F64), x=1 (PORT continuous); term T0 FLOW {x, GROUND};
/// fn 0 "out": reads [x, g] writes [T0] partials (0<-0) (0<-1).
struct Gain;

impl Gain {
    /// The shape of the module.
    fn shape() -> ShapeRef {
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

    /// The shape every instance of the module has, built once.
    fn prototype() -> &'static ShapeRef {
        static PROTOTYPE: OnceLock<ShapeRef> = OnceLock::new();
        PROTOTYPE.get_or_init(Gain::shape)
    }

    /// `create` of a module that has nothing but a shape: the instance is a marker no call reads.
    unsafe extern "C" fn create(_module: *const Module, _values: List<'_, Value<'_>>, _note: *mut Note) -> Created {
        Created { status: status::OK, instance: std::ptr::NonNull::<Instance>::dangling().as_ptr(), shape: Gain::prototype().shape() }
    }

    /// `destroy` of the marker instance: nothing to free.
    unsafe extern "C" fn destroy(_instance: *mut Instance) {}

    /// `eval` that does nothing and allocates nothing, so a count is the `Handle`'s own. The in-tree
    /// device thunk cuts its own frame (generated side, not part of TP-07).
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
            unsafe { Module::from_raw_parts(Str::from_static("gain"), Gain::prototype().shape(), std::ptr::null(), Gain::create, Gain::destroy, Gain::eval, Gain::react) }
        })
    }
}

/// What the tests of this target read of a run.
struct Probe;

impl Probe {
    /// The allocations the calling thread makes while `work` runs.
    fn allocations_during(work: impl FnOnce()) -> usize {
        let before = ALLOCATIONS.with(StdCell::get);
        work();
        ALLOCATIONS.with(StdCell::get) - before
    }
}

#[test]
fn the_second_eval_of_a_function_allocates_nothing() {
    let g = 4.0f64.to_le_bytes();
    let handle = Handle::create(Gain::module(), &[(0, &g)]).expect("create");
    let x = 1.5f64.to_le_bytes();
    let mut out = [0u8; 8];
    let mut jac = [0.0; 2];
    handle.eval(0, &[&x, &g], &mut [&mut out], &[1, 0], &mut jac).expect("the first eval");

    let allocations = Probe::allocations_during(|| {
        handle.eval(0, &[&x, &g], &mut [&mut out], &[1, 0], &mut jac).expect("the second eval");
    });

    assert_eq!(allocations, 0, "the second eval allocated");
}

#[test]
fn a_buffer_shorter_than_its_cell_is_refused_with_the_text_it_always_had() {
    let g = 4.0f64.to_le_bytes();
    let handle = Handle::create(Gain::module(), &[(0, &g)]).expect("create");
    let mut out = [0u8; 8];
    let mut jac = [0.0; 2];
    let short = [0u8; 4];

    let shorter = handle.eval(0, &[&short, &g], &mut [&mut out], &[1, 1], &mut jac).expect_err("a short read");
    let fewer = handle.eval(0, &[&g], &mut [&mut out], &[1, 1], &mut jac).expect_err("a missing read");

    assert_eq!(shorter.message, "read 0 has 4 bytes, its cell 8");
    assert_eq!(fewer.message, "1 read buffers for 2 read cells");
}
