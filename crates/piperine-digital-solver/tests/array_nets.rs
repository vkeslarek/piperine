//! Array nets (composite-values CV-23 to CV-27, part II §5.1.8, abi.md §3.1 P4a): an array of carriers is one net
//! whose store holds only the elements written, an element never written reads the array's initial value and its
//! `PREVIOUS` is that value, and a rewind restores the map. T67 adds windows and element commits to this target.

use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::store::Store;

/// The arrays this target builds: one net of one-byte elements, `count` of them.
struct Arrays;

impl Arrays {
    /// A store holding one array net `lines` of `count` one-byte elements whose initial bytes are `init`.
    fn store(count: u64, init: &[u8]) -> (Store, NetId) {
        let spec = NetSpec { init: init.to_vec(), ..NetSpec::array("lines", 1, 1, count) };
        (Store::new(&[spec]), NetId(0))
    }
}

#[test]
fn a_sparse_array_stores_only_written_elements_and_rewinds() {
    // CV-27 AC 1, §5.1.8: `1 << 32` elements and 1 000 distinct writes hold 1 000 entries, not 2^32.
    let (mut store, net) = Arrays::store(1 << 32, &[0]);
    for k in 0..1000u64 {
        let element = k * 4_000_000;
        assert!(store.write_element(net, element, 0, &[1]), "element {element} changes from its initial 0");
    }
    assert_eq!(store.array(net).expect("an array net").written_len(), 1000, "storage is what was written");
    assert_eq!(store.element_value(net, 4_000_000), [1]);
    assert_eq!(store.element_value(net, 4_000_001), [0], "a neighbour never written reads the initial value");
    // a write of the value an element already holds is no change and stores nothing
    assert!(!store.write_element(net, 7, 0, &[0]));
    assert_eq!(store.array(net).expect("an array net").written_len(), 1000);

    // R6: a rewind restores the map.
    let mark = store.mark();
    store.begin_microstep();
    assert!(store.write_element(net, 7, 0, &[1]), "a new element");
    assert!(store.write_element(net, 4_000_000, 0, &[0]), "an existing element changes back");
    assert_eq!(store.array(net).expect("an array net").written_len(), 1001);
    store.rewind(&mark);
    assert_eq!(store.array(net).expect("an array net").written_len(), 1000, "the new element left the map");
    assert_eq!(store.element_value(net, 7), [0]);
    assert_eq!(store.element_value(net, 4_000_000), [1], "the changed element has its value back");
    assert_eq!(store.element_previous(net, 4_000_000), [0], "and the previous it had");
    store.release(mark);
}

#[test]
fn an_unwritten_element_reads_its_initial_value_and_previous() {
    // §5.1.8: an element never written reads the array's initial value for that element, and `prev()` is the same.
    // `[x; K]`: one element's bytes for every element.
    let (store, net) = Arrays::store(8, &[5]);
    for element in [0, 3, 7] {
        assert_eq!(store.element_value(net, element), [5], "element {element} of [5; 8]");
        assert_eq!(store.element_previous(net, element), [5], "its previous is the same");
        assert!(!store.element_changed(net, element), "nothing was committed");
    }
    // `[a, b, c]`: every element's bytes.
    let (each, net) = Arrays::store(3, &[1, 2, 3]);
    assert_eq!([0, 1, 2].map(|element| each.element_value(net, element)[0]), [1, 2, 3]);
    // no init: all zero bytes (abi.md §3.1 P3)
    let (zero, net) = Arrays::store(4, &[]);
    assert_eq!(zero.element_value(net, 2), [0]);

    // part IV §13 rule 6: the initial commit is a change of every element, in the microstep it lands in only.
    let (mut store, net) = Arrays::store(8, &[5]);
    store.stamp_initial_array(net);
    store.begin_microstep();
    assert!(store.element_changed(net, 3), "every element changes in the initial microstep");
    assert_eq!(store.array(net).expect("an array net").written_len(), 0, "and none is materialized");
    store.begin_microstep();
    assert!(!store.element_changed(net, 3), "the next microstep it is quiet again");
    // a written element changes in its own microstep and keeps the old value as its previous
    store.begin_microstep();
    assert!(store.write_element(net, 3, 0, &[9]));
    assert!(store.element_changed(net, 3));
    assert!(!store.element_changed(net, 4), "a neighbour did not change");
    assert_eq!((store.element_value(net, 3), store.element_previous(net, 3)), (&[9u8][..], &[5u8][..]));
}

// ─── Windows and element commits (T67) ──────────────────────────────────────

use std::sync::{Arc, OnceLock};

use piperine_abi::caller::Handle;
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, BYTE};
use piperine_abi::raw::{Cell, Module, Shape, Statement, WHOLE, cell, part, role};
use piperine_abi::shape::{PinSpec, ReactionSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;
use piperine_digital_solver::engine::{Digital, Horizon, Mode};
use piperine_digital_solver::landing::TimePointRule;
use piperine_digital_solver::limits::DigitalLimits;
use piperine_digital_solver::observe::Observation;
use piperine_digital_solver::plan::{Binding, DigitalPlan, InstancePlan};
use piperine_digital_solver::tick::{Resolution, Tick};

/// What the devices and engines of this target share.
struct Kit;

impl Kit {
    /// A statement of one byte on pin `pin`.
    fn statement(pin: u32) -> Statement {
        Statement { pin, offset: 0, len: 1, flags: 0, min_after: 0.0 }
    }

    /// An instance of `D` named `path` whose pins are bound as `pins`.
    fn instance<D: Device>(path: &str, library: &'static OnceLock<Module>, name: &'static str, pins: Vec<Binding>) -> InstancePlan {
        let module = library.get_or_init(|| module::<D>(name));
        InstancePlan { path: path.to_string(), handle: Handle::create(module, &[]).expect("a fixture device creates"), pins }
    }

    /// An engine over `nets` and `instances`, started.
    fn engine(nets: Vec<NetSpec>, instances: Vec<InstancePlan>) -> Digital {
        let plan = DigitalPlan::bind(nets, instances, Resolution::default()).expect("the fixture plan binds");
        let mut engine = Digital::new(plan, DigitalLimits::default(), TimePointRule::default());
        engine.start().expect("the engine starts");
        engine
    }

    /// Runs the instant at `at` to its end.
    fn instant(engine: &mut Digital, at: Tick) {
        engine.run_instant(at, &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).expect("the instant runs");
    }
}

/// Writes `1` to its scalar pin when `go` fires: bound to one element of an array net, a write of that element.
struct Poker;
static POKER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static POKER: OnceLock<Module> = OnceLock::new();

impl Device for Poker {
    fn prototype() -> &'static Shape<'static> {
        POKER_SHAPE.get_or_init(Poker::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Poker, Arc::clone(POKER_SHAPE.get_or_init(Poker::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit(0, &[1], 0.0)?;
        Ok(Flow::Continue)
    }
}

impl Poker {
    /// pins: `bit` (0, `BIT`), `go` (1, event); reaction 0 on `go` writes statement 0 on `bit`.
    fn shape() -> ShapeRef {
        let mut buffer = ShapeBuf::new();
        let bit = buffer.pin(PinSpec::discrete("bit", role::PORT, &BIT));
        let go = buffer.pin(PinSpec::event("go", role::PRIVATE));
        buffer.reaction(ReactionSpec { name: "poke", on: vec![go], writes: vec![Kit::statement(bit)], ..ReactionSpec::default() });
        buffer.freeze()
    }
}

/// On `go`, commits to elements of its array pin `mem` of four elements: element 2 twice in one microstep (`1` then
/// `0`), element 3 once, and elements 1 and 0 each once, `1 ns` later.
struct Banker;
static BANKER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static BANKER: OnceLock<Module> = OnceLock::new();

impl Device for Banker {
    fn prototype() -> &'static Shape<'static> {
        BANKER_SHAPE.get_or_init(Banker::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Banker, Arc::clone(BANKER_SHAPE.get_or_init(Banker::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        cx.commit_element(0, 2, &[1], 0.0)?;
        cx.commit_element(0, 2, &[0], 0.0)?;
        cx.commit_element(0, 3, &[1], 0.0)?;
        cx.commit_element(0, 1, &[1], 1e-9)?;
        cx.commit_element(0, 0, &[1], 1e-9)?;
        Ok(Flow::Continue)
    }
}

impl Banker {
    /// pins: `mem` (0, `BIT`, 4 elements), `go` (1, event).
    fn shape() -> ShapeRef {
        let mut buffer = ShapeBuf::new();
        let mem = buffer.pin(PinSpec { count: 4, ..PinSpec::discrete("mem", role::PORT, &BIT) });
        let go = buffer.pin(PinSpec::event("go", role::PRIVATE));
        buffer.reaction(ReactionSpec { name: "bank", on: vec![go], writes: vec![Kit::statement(mem)], ..ReactionSpec::default() });
        buffer.freeze()
    }
}

/// On `go`, reads its whole array pin `mem` and commits to `sum` how many elements are `1`.
struct Summer;
static SUMMER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SUMMER: OnceLock<Module> = OnceLock::new();

impl Device for Summer {
    fn prototype() -> &'static Shape<'static> {
        SUMMER_SHAPE.get_or_init(Summer::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Summer, Arc::clone(SUMMER_SHAPE.get_or_init(Summer::shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let view = cx.array(0);
        let ones = (0..view.count).filter(|element| view.element(*element, part::VALUE) == Some(&[1][..])).count();
        cx.commit(0, &[ones as u8], 0.0)?;
        Ok(Flow::Continue)
    }
}

impl Summer {
    /// pins: `mem` (0, `BIT`, `count` elements, read whole), `go` (1, event), `sum` (2, `BYTE`).
    fn shape() -> ShapeRef {
        let mut buffer = ShapeBuf::new();
        let mem = buffer.pin(PinSpec { count: 8, ..PinSpec::discrete("mem", role::PORT, &BIT) });
        let go = buffer.pin(PinSpec::event("go", role::PRIVATE));
        let sum = buffer.pin(PinSpec::discrete("sum", role::PORT, &BYTE));
        buffer.reaction(ReactionSpec {
            name: "sum",
            on: vec![go],
            reads: vec![Cell { kind: cell::PIN, index: mem, part: part::VALUE }],
            elements: vec![WHOLE],
            writes: vec![Kit::statement(sum)],
            ..ReactionSpec::default()
        });
        buffer.freeze()
    }
}

#[test]
fn an_element_window_reads_and_writes_one_element() {
    // CV-23: a scalar pin on one element writes and reads that element only; an array pin on a window reads its run
    // of elements whole. Nets: 0 `lines` (16 elements), 1 `go_poke`, 2 `go_all`, 3 `go_high`, 4 `all`, 5 `high`.
    let nets = vec![
        NetSpec::array("lines", 1, 1, 16),
        NetSpec::event("go_poke"),
        NetSpec::event("go_all"),
        NetSpec::event("go_high"),
        NetSpec::discrete("all", 1, 1),
        NetSpec::discrete("high", 1, 1),
    ];
    let lines = NetId(0);
    let instances = vec![
        Kit::instance::<Poker>("poker", &POKER, "poker", vec![Binding::ArrayElement { net: lines, element: 5 }, Binding::Net(NetId(1))]),
        // `Summer` declares 8 elements: `all` sums elements 0..8 of the net, `high` elements 6..14
        Kit::instance::<Summer>("all", &SUMMER, "summer", vec![Binding::ArrayWindow { net: lines, first: 0, length: 8 }, Binding::Net(NetId(2)), Binding::Net(NetId(4))]),
        Kit::instance::<Summer>("high", &SUMMER, "summer", vec![Binding::ArrayWindow { net: lines, first: 6, length: 8 }, Binding::Net(NetId(3)), Binding::Net(NetId(5))]),
    ];
    let mut engine = Kit::engine(nets, instances);
    Kit::instant(&mut engine, Tick(0));
    assert_eq!(engine.read_element(lines, 5), [0], "element 5 starts at its initial value");
    engine.fire(NetId(1), Tick(1)).expect("go_poke is an event net");
    Kit::instant(&mut engine, Tick(1));
    for element in 0..16u64 {
        let expected = u8::from(element == 5);
        assert_eq!(engine.read_element(lines, element), [expected], "only element 5 was written (element {element})");
    }
    engine.fire(NetId(2), Tick(2)).expect("go_all is an event net");
    Kit::instant(&mut engine, Tick(2));
    assert_eq!(engine.read(NetId(4)), [1], "the whole-array read of window 0..8 saw exactly one element at 1");
    engine.fire(NetId(3), Tick(2)).expect("go_high is an event net");
    Kit::instant(&mut engine, Tick(2));
    assert_eq!(engine.read(NetId(5)), [0], "window 6..14 does not hold element 5");
    // a host commit to another element is seen by the next whole read
    engine.host_commit_element(lines, 0, 0, &[1], Tick(3)).expect("element 0 is below the count");
    engine.fire(NetId(2), Tick(3)).expect("go_all");
    Kit::instant(&mut engine, Tick(3));
    assert_eq!(engine.read(NetId(4)), [2], "the whole-array read follows the elements");
    assert!(engine.host_commit_element(lines, 16, 0, &[1], Tick(4)).is_err(), "element 16 of 16 is past the end");
    engine.host_commit_element(lines, 6, 0, &[1], Tick(4)).expect("element 6");
    engine.fire(NetId(3), Tick(4)).expect("go_high");
    Kit::instant(&mut engine, Tick(4));
    assert_eq!(engine.read(NetId(5)), [1], "element 6 is element 0 of window 6..14");
}

#[test]
fn element_commits_keep_the_last_write_per_level_and_transport_per_element() {
    // abi.md §6 C2 per element: two commits of one microstep to element 2 keep the last; C3 per element: a commit
    // to element 0 does not remove the pending commit of the same statement to element 1.
    let nets = vec![NetSpec::array("mem", 1, 1, 4), NetSpec::event("go")];
    let mem = NetId(0);
    let instances = vec![Kit::instance::<Banker>("bank", &BANKER, "banker", vec![Binding::Net(mem), Binding::Net(NetId(1))])];
    let mut engine = Kit::engine(nets, instances);
    Kit::instant(&mut engine, Tick(0));
    engine.fire(NetId(1), Tick(1)).expect("go is an event net");
    engine.drain_observations();
    Kit::instant(&mut engine, Tick(1));
    assert_eq!(engine.read_element(mem, 2), [0], "C2: the last write to element 2 wins");
    assert_eq!(engine.read_element(mem, 3), [1], "element 3 was committed once");
    let changed: Vec<String> = engine
        .drain_observations()
        .into_iter()
        .filter_map(|o| match o {
            Observation::Change { net, .. } => Some(net),
            _ => None,
        })
        .collect();
    assert!(changed.contains(&"mem[3]".to_string()) && !changed.contains(&"mem[2]".to_string()), "element 2 never changed: {changed:?}");
    // 1 ns later, at resolution 1 fs: both delayed commits apply, none transported away by the other
    let due = engine.next_due().expect("the delayed commits are queued");
    Kit::instant(&mut engine, due);
    assert_eq!([engine.read_element(mem, 0)[0], engine.read_element(mem, 1)[0]], [1, 1], "C3 per element");
}
