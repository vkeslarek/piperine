//! Running the discrete functions (feature `solver-abi-digital`, SAD-06,
//! SAD-07 and SAD-13; abi.md §4 F3, §3.1 P2, §9 D1): dirty-driven evaluation
//! in the levelized order, relaxation of cycles through `RELAX` nets, and a
//! device fault carried with its code, tick and instance.

use piperine_device_fixtures::digital::{And2, CallCounts, DataFlipFlop, BitFaulty, Inverter, DriveDisjunction, Successor};
use piperine_device_fixtures::binding::{Placed, PlanBinding};
use piperine_abi::layout::{HIGH, LOW};
use piperine_digital_solver::net::NetId;
use piperine_digital_solver::observe::{Obs, Observation};
use piperine_digital_solver::plan::DigitalPlan;
use piperine_digital_solver::run::{Ambient, DirtySet, FunctionRun};
use piperine_digital_solver::store::Store;
use piperine_digital_solver::tick::{Resolution, Tick};
use piperine_digital_solver::error::DigitalError;

/// A plan with the state `FunctionRun::run` needs, standing for a few microsteps of one instant.
struct Env {
    plan: DigitalPlan,
    store: Store,
    dirty: DirtySet,
    obs: Obs,
    step: u32,
}

impl Env {
    fn new(plan: DigitalPlan) -> Env {
        let store = Store::new(&plan.nets);
        let dirty = DirtySet::none(&plan);
        Env { plan, store, dirty, obs: Obs::new(), step: 0 }
    }

    fn all_dirty(mut self) -> Env {
        self.dirty = DirtySet::all(&self.plan);
        self
    }

    fn net(&self, name: &str) -> NetId {
        PlanBinding::net_named(&self.plan, name)
    }

    /// The host writes `bytes` to the net called `name` in the current microstep.
    fn write(&mut self, name: &str, bytes: &[u8]) {
        let id = self.net(name);
        FunctionRun::new(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs).write_net(id, 0, bytes, (Tick(5), self.step));
    }

    fn run(&mut self) -> Result<(), DigitalError> {
        let amb = Ambient::at(Tick(5), 1e-15);
        FunctionRun::new(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs).run((Tick(5), self.step), &amb)?;
        self.obs.flush();
        Ok(())
    }

    fn next_microstep(&mut self) {
        self.store.begin_microstep();
        self.step += 1;
    }

    fn value(&self, name: &str) -> Vec<u8> {
        self.store.value(self.net(name)).to_vec()
    }
}

#[test]
fn a_function_runs_when_a_read_net_changed() {
    // C11
    CallCounts::reset();
    let mut e = Env::new(FunctionCases::plan(vec![("inv", Inverter::handle(), vec![("a", "a"), ("y", "y")])]));
    e.write("a", &HIGH);
    e.run().unwrap();
    assert_eq!(CallCounts::of("inverter").0, 1);
    assert_eq!(e.value("y"), LOW, "a known high in, a known low out");
    e.next_microstep();
    e.write("a", &LOW);
    e.run().unwrap();
    assert_eq!(CallCounts::of("inverter").0, 2);
    assert_eq!(e.value("y"), HIGH);
}

#[test]
fn functions_run_after_every_writer_of_their_reads() {
    // a chain a -> inv1 -> m -> inv2 -> z listed with the sink first runs in one pass
    CallCounts::reset();
    let mut e = Env::new(FunctionCases::plan(vec![
        ("sink", Inverter::handle(), vec![("a", "m"), ("y", "z")]),
        ("src", Inverter::handle(), vec![("a", "a"), ("y", "m")]),
    ]))
    .all_dirty();
    e.write("a", &LOW);
    e.run().unwrap();
    assert_eq!(e.value("m"), HIGH);
    assert_eq!(e.value("z"), LOW, "the sink saw the new m in the same microstep");
    assert_eq!(CallCounts::of("inverter").0, 2, "each ran once");
}

#[test]
fn a_function_with_unchanged_inputs_is_not_called() {
    // C12
    CallCounts::reset();
    let mut e = Env::new(FunctionCases::plan(vec![("inv", Inverter::handle(), vec![("a", "a"), ("y", "y")])]));
    e.write("a", &HIGH);
    e.run().unwrap();
    e.next_microstep();
    e.run().unwrap();
    assert_eq!(CallCounts::of("inverter").0, 1, "nothing changed in the second microstep");
    e.next_microstep();
    e.write("a", &HIGH); // the same bytes: not a change
    e.run().unwrap();
    assert_eq!(CallCounts::of("inverter").0, 1);
}

#[test]
fn every_function_runs_once_at_the_first_instant() {
    CallCounts::reset();
    let mut e = Env::new(FunctionCases::plan(vec![
        ("inv", Inverter::handle(), vec![("a", "a"), ("y", "y")]),
        ("and", And2::handle(), vec![("a", "a"), ("b", "b"), ("y", "z")]),
    ]))
    .all_dirty();
    e.run().unwrap();
    assert_eq!(CallCounts::of("inverter").0, 1);
    assert_eq!(CallCounts::of("and2").0, 1);
    assert_eq!(e.value("y"), HIGH, "the inverter of a zero input is high");
    e.next_microstep();
    e.run().unwrap();
    assert_eq!((CallCounts::of("inverter").0, CallCounts::of("and2").0), (1, 1), "and no more");
}

#[test]
fn an_absent_optional_pin_reads_zero_bytes() {
    // d1 of the resolver is OPTIONAL and unbound: it reads as zero bytes
    let mut e = Env::new(FunctionCases::plan(vec![("r", DriveDisjunction::handle(), vec![("d0", "d0"), ("v", "v")])]));
    e.write("d0", &HIGH);
    e.run().unwrap();
    assert_eq!(e.value("v"), HIGH, "d0 alone decides");
    e.next_microstep();
    e.write("d0", &LOW);
    e.run().unwrap();
    assert_eq!(e.value("v"), LOW, "neither drive is high");
}

#[test]
fn a_relax_group_settles_within_its_bound() {
    // C16: one RELAX net in the cycle, so at most two passes
    CallCounts::reset();
    let mut e = FunctionCases::keeper(&HIGH);
    e.run().unwrap();
    assert_eq!(e.value("v"), HIGH);
    assert_eq!(e.value("d"), HIGH);
    assert_eq!(CallCounts::of("resolver").0, 2, "one pass that moves, one that does not");
    // the net `d` changed once for the run, from nothing to high
    let changes: Vec<_> = e
        .obs
        .list()
        .iter()
        .filter(|o| matches!(o, Observation::Change { net, .. } if net == "d"))
        .collect();
    assert_eq!(changes.len(), 1, "one change of d, not one per pass: {changes:?}");
}

#[test]
fn a_moving_loop_is_e0511_not_a_value_made_up() {
    // C17, abi.md §9 D1: v = d | LOW, d = not v never settles; a loop that still moves after the
    // bounded passes is `E0511`, never a value made up
    let mut e = Env::new(FunctionCases::plan(vec![
        ("res", DriveDisjunction::handle(), vec![("d0", "d"), ("d1", "e"), ("v", "v")]),
        ("inv", Inverter::handle(), vec![("a", "v"), ("y", "d")]),
    ]))
    .all_dirty();
    e.write("e", &LOW);
    let error = e.run().expect_err("a loop that does not settle is E0511");
    assert!(error.to_string().contains("E0511") && error.to_string().contains("did not settle"), "{error}");
}

#[test]
fn a_counter_that_always_moves_is_e0511() {
    // C17: a counter field that always moves has nothing to fall back to: the loop is `E0511`
    let mut e = Env::new(FunctionCases::plan(vec![("s", Successor::handle(), vec![("a", "x"), ("y", "x")])])).all_dirty();
    match e.run() {
        Err(DigitalError::Unsettled { code: "E0511", drives, .. }) => assert_eq!(drives, ["x"], "the loop's drive is named"),
        other => panic!("expected E0511, got {other:?}"),
    }
}

#[test]
fn a_function_fault_carries_code_tick_and_instance() {
    // C25: BitFaulty sits behind an inverter, outside the parameter cone
    let mut e = Env::new(FunctionCases::plan(vec![
        ("front", Inverter::handle(), vec![("a", "x"), ("y", "m")]),
        ("f", BitFaulty::handle(), vec![("a", "m"), ("y", "out")]),
    ]))
    .all_dirty();
    e.write("x", &LOW); // m = high: a FAULT
    match e.run() {
        Err(DigitalError::Fault { code, message, tick, instance }) => {
            assert_eq!((code, message.as_str(), tick, instance.as_str()), (7, "boom", 5, "f"));
        }
        other => panic!("expected a fault, got {other:?}"),
    }
    // RESHAPE outside the cone is the same error
    let mut e = Env::new(FunctionCases::plan(vec![
        ("front", Inverter::handle(), vec![("a", "x"), ("y", "m")]),
        ("f", BitFaulty::handle(), vec![("a", "m"), ("y", "out")]),
    ]))
    .all_dirty();
    e.write("x", &HIGH); // m = low: a RESHAPE
    match e.run() {
        Err(DigitalError::Fault { tick, instance, message, .. }) => {
            assert_eq!((tick, instance.as_str()), (5, "f"));
            assert!(message.contains("another shape"), "{message}");
        }
        other => panic!("expected a fault, got {other:?}"),
    }
}

#[test]
fn changed_is_zero_one_microstep_later() {
    // the flip-flop's edge function reads CHANGED and fires `rise`
    let mut e = Env::new(FunctionCases::plan(vec![("ff", DataFlipFlop::handle(), vec![("clk", "clk"), ("d", "d"), ("q", "q")])]));
    let rise = e.net("ff.rise");
    e.write("clk", &LOW); // zero to low is no change, so no edge
    e.run().unwrap();
    assert!(!e.store.fired(rise));
    e.next_microstep();
    e.write("clk", &HIGH);
    e.run().unwrap();
    assert!(e.store.fired(rise), "a rising edge in the microstep of the change");
    e.next_microstep();
    assert!(!e.store.changed(e.net("clk")));
    e.dirty = DirtySet::all(&e.plan); // force the function to run again
    e.run().unwrap();
    assert!(!e.store.fired(rise), "the same clk, CHANGED read 0: no edge");
}

// ─── The environment (feature `solver-crossings`, ENV-02) ───────────────────

/// Reads the environment: a discrete function writes `GMIN` and `SOURCE_SCALE` to `fg` and
/// `fs`; a reaction on `START` commits them to `rg` and `rs`.
/// pins: fg=0, fs=1, rg=2, rs=3 (PORT F64); start=4 (GLOBAL START)
/// fn 0 "env": reads [GMIN, SOURCE_SCALE] writes [fg, fs]
/// reaction 0 "env": on [start] reads [GMIN, SOURCE_SCALE] writes [S0 {rg}, S1 {rs}]
struct Environment;
static ENV_SHAPE: std::sync::OnceLock<piperine_abi::shape::ShapeRef> = std::sync::OnceLock::new();
static ENV_MODULE: std::sync::OnceLock<piperine_abi::raw::Module> = std::sync::OnceLock::new();

impl piperine_abi::device::Device for Environment {
    fn prototype() -> &'static piperine_abi::raw::Shape<'static> {
        ENV_SHAPE.get_or_init(FunctionCases::environment_shape).shape()
    }
    fn create(
        _: &piperine_abi::device::Values<'_>,
    ) -> Result<(Self, piperine_abi::shape::ShapeRef), piperine_abi::device::Fault> {
        Ok((Environment, std::sync::Arc::clone(ENV_SHAPE.get_or_init(FunctionCases::environment_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut piperine_abi::device::Eval) -> Result<(), piperine_abi::device::Fault> {
        let (g, s) = (cx.f64(0), cx.f64(1));
        cx.set(0, g);
        cx.set(1, s);
        Ok(())
    }
    fn react(
        &self,
        _k: u32,
        cx: &mut piperine_abi::device::React,
    ) -> Result<piperine_abi::device::Flow, piperine_abi::device::Fault> {
        let (g, s) = (cx.f64(0), cx.f64(1));
        cx.commit(0, &g.to_le_bytes(), 0.0)?;
        cx.commit(1, &s.to_le_bytes(), 0.0)?;
        Ok(piperine_abi::device::Flow::Continue)
    }
}

#[test]
fn reactions_and_functions_read_the_continuation_values() {
    // ENV-02 (abi.md §4.2): a reaction and a discrete function read GMIN and SOURCE_SCALE as the
    // owner set them; 0 and 1 until it does
    use piperine_digital_solver::engine::{Digital, Horizon, Mode};
    use piperine_digital_solver::landing::TimePointRule;
    use piperine_digital_solver::limits::DigitalLimits;
    let read = |set: Option<(f64, f64)>| {
        let handle = piperine_abi::caller::Handle::create(
            ENV_MODULE.get_or_init(|| piperine_abi::thunk::module::<Environment>("Environment")),
            &[],
        )
        .expect("creates");
        let plan = PlanBinding::bind_by_name(vec![("env", handle, vec![])], Resolution::default()).expect("binds");
        let mut d = Digital::new(plan, DigitalLimits::default(), TimePointRule::default());
        if let Some((g, s)) = set {
            d.set_continuation(g, s);
        }
        d.start().unwrap();
        d.run_instant(Tick(0), &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).expect("the instant runs");
        let f = |name: &str| f64::from_le_bytes(d.read(PlanBinding::net_named(d.plan(), name)).try_into().expect("an F64"));
        [f("env.fg"), f("env.fs"), f("env.rg"), f("env.rs")]
    };
    assert_eq!(read(Some((2e-12, 0.25))), [2e-12, 0.25, 2e-12, 0.25], "function then reaction, as set");
    assert_eq!(read(None), [0.0, 1.0, 0.0, 1.0], "0 and 1 until set");
}

/// The engines the function tests run.
pub struct FunctionCases;

impl FunctionCases {
    /// The plan helper `plan`.
    fn plan(devices: Vec<Placed<'_>>) -> DigitalPlan {
        PlanBinding::bind_by_name(devices, Resolution::default()).expect("the fixture plan binds")
    }

    /// The feedback loop `v = d | e`, `d = v` through a resolver and an AND, with `d` a `RELAX` net.
    fn keeper(e_value: &[u8]) -> Env {
        let mut env = Env::new(FunctionCases::plan(vec![
            ("res", DriveDisjunction::handle(), vec![("d0", "d"), ("d1", "e"), ("v", "v")]),
            ("keep", And2::handle(), vec![("a", "v"), ("b", "v"), ("y", "d")]),
        ]));
        env.write("e", e_value);
        env
    }

    /// The environment shape helper `environment_shape`.
    fn environment_shape() -> piperine_abi::shape::ShapeRef {
        use piperine_abi::layout::F64;
        use piperine_abi::raw::{Cell, Statement, ambient, cell, global, part, role};
        use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf};
        let mut s = ShapeBuf::new();
        let [fg, fs, rg, rs] = ["fg", "fs", "rg", "rs"].map(|n| s.pin(PinSpec::discrete(n, role::PORT, &F64)));
        let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
        let amb = |index| Cell { kind: cell::AMBIENT, index, part: part::VALUE };
        let pin = |index| Cell { kind: cell::PIN, index, part: part::VALUE };
        s.function(FunctionSpec {
            name: "env",
            origin: "functions.rs",
            reads: vec![amb(ambient::GMIN), amb(ambient::SOURCE_SCALE)],
            writes: vec![pin(fg), pin(fs)],
            ..FunctionSpec::default()
        });
        let st = |pin| Statement { pin, offset: 0, len: 8, flags: 0, min_after: 0.0 };
        s.reaction(ReactionSpec {
            name: "env",
            origin: "functions.rs",
            on: vec![start],
            reads: vec![amb(ambient::GMIN), amb(ambient::SOURCE_SCALE)],
            writes: vec![st(rg), st(rs)],
            ..ReactionSpec::default()
        });
        s.freeze()
    }
}
