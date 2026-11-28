//! Relaxation below the language (feature `resolved-nets`, RN-05; spec §8.2.3 steps 1 and 4,
//! §5.2.12; abi.md §9 D1 and its drive presence field, design D3): a relaxing cycle starts with
//! every drive absent, stops within the drive count plus one passes, keeps no level of an earlier
//! microstep once released, leaves an absent drive out of what `resolve` receives and passes a
//! present one of any value, ends with a plain `driving` byte; a loop that never settles is `E0511`
//! naming its drives. The devices are stubs built on `ShapeBuf`: a driver writes the value and
//! `driving`, a resolver reads `driving` per drive and folds the present ones.

use piperine_device_fixtures::binding::{Placed, PlanBinding};
use std::cell::RefCell;
use std::sync::{Arc, OnceLock};

use piperine_abi::caller::Handle;
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, HIGH, LOW};
use piperine_abi::raw::{Cell, Field, Layout, List, Module, Shape, Str, cell, leaf, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module;
use piperine_digital_solver::error::DigitalError;
use piperine_digital_solver::net::NetId;
use piperine_digital_solver::observe::Obs;
use piperine_digital_solver::plan::DigitalPlan;
use piperine_digital_solver::run::{Ambient, DirtySet, FunctionRun};
use piperine_digital_solver::store::Store;
use piperine_digital_solver::tick::{Resolution, Tick};

/// The value field of a drive: a `BIT`'s value.
const VALUE: Str = Str::from_static("v");
/// The presence field of a drive (design D3): `driving: BOOL`.
const DRIVING: Str = Str::from_static("driving");

static DRIVE_FIELDS: [Field; 2] = [
    Field { name: VALUE, leaf: leaf::BOOL, offset: 0, count: 1 },
    Field { name: DRIVING, leaf: leaf::BOOL, offset: 1, count: 1 },
];
/// A drive net's layout (abi.md §9 D1): the value's layout followed by `driving`, size 2.
static DRIVE: Layout = Layout { size: 2, align: 1, fields: List::from_slice(&DRIVE_FIELDS) };

/// The bytes of a drive: its value, then `driving`.
struct Drive;

impl Drive {
    /// A present drive (zero bytes are absent): the value, then `driving` 1.
    fn present(value: u8) -> [u8; 2] {
        [value, 1]
    }

    /// Whether `bytes` is a present drive: `driving` is set. Zero bytes, the state a relaxing
    /// cycle starts from, are absent.
    fn is_present(bytes: &[u8]) -> bool {
        bytes[1] != 0
    }
}

thread_local! {
    /// The count of present drives each `resolve` call of this thread received, in call order.
    static RECEIVED: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

/// What the stub resolver saw, per thread (a test runs on its own thread, so counts never mix).
struct Received;

impl Received {
    /// Notes that one `resolve` call received `count` present drives.
    fn note(count: usize) {
        RECEIVED.with(|r| r.borrow_mut().push(count));
    }

    /// The counts noted so far, in call order.
    fn counts() -> Vec<usize> {
        RECEIVED.with(|r| r.borrow().clone())
    }
}

/// The index of a pin as a shape cell.
struct Pins;

impl Pins {
    /// The value part of pin `index`.
    fn cell(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }
}

/// Writes a drive from its enable and its value: present with the value when `en` is true.
///
/// pins: en=0, a=1 (PORT BIT); y=2 (PORT DRIVE, RELAX).  fn 0 "drive": reads [en, a] writes [y]
struct Driver;
static DRIVER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static DRIVER: OnceLock<Module> = OnceLock::new();

impl Driver {
    /// The shape every `Driver` shares.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let en = s.pin(PinSpec::discrete("en", role::PORT, &BIT));
        let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
        let y = s.pin(PinSpec { flags: pin_flag::RELAX, ..PinSpec::discrete("y", role::PORT, &DRIVE) });
        s.function(FunctionSpec {
            name: "drive",
            reads: vec![Pins::cell(en), Pins::cell(a)],
            writes: vec![Pins::cell(y)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// A live instance.
    fn handle() -> Handle {
        Handle::create(DRIVER.get_or_init(|| module::<Driver>("driver")), &[]).expect("a stub creates")
    }
}

impl Device for Driver {
    fn prototype() -> &'static Shape<'static> {
        DRIVER_SHAPE.get_or_init(Driver::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Driver, Arc::clone(DRIVER_SHAPE.get_or_init(Driver::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (en, a) = (cx.bytes(0), cx.bytes(1));
        cx.out_bytes(0).copy_from_slice(&if en[0] != 0 { Drive::present(a[0]) } else { [0; 2] });
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A keeper: drives the level it reads.
///
/// pins: a=0 (PORT BIT); y=1 (PORT DRIVE, RELAX).  fn 0 "keep": reads [a] writes [y]
struct Keeper;
static KEEPER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static KEEPER: OnceLock<Module> = OnceLock::new();

impl Keeper {
    /// The shape every `Keeper` shares.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
        let y = s.pin(PinSpec { flags: pin_flag::RELAX, ..PinSpec::discrete("y", role::PORT, &DRIVE) });
        s.function(FunctionSpec {
            name: "keep",
            reads: vec![Pins::cell(a)],
            writes: vec![Pins::cell(y)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// A live instance.
    fn handle() -> Handle {
        Handle::create(KEEPER.get_or_init(|| module::<Keeper>("keeper")), &[]).expect("a stub creates")
    }
}

impl Device for Keeper {
    fn prototype() -> &'static Shape<'static> {
        KEEPER_SHAPE.get_or_init(Keeper::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Keeper, Arc::clone(KEEPER_SHAPE.get_or_init(Keeper::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let level = cx.bytes(0)[0];
        cx.out_bytes(0).copy_from_slice(&Drive::present(level));
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A resolver over two drives: reads `driving` per drive, passes only the present ones to a
/// `wired`-like fold (no drive: low; two levels that differ: low; else the level), and notes how
/// many it passed.
///
/// pins: d0=0, d1=1 (PORT DRIVE, RELAX); v=2 (PORT BIT).  fn 0 "resolve": reads [d0, d1] writes [v]
struct Resolver;
static RESOLVER_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static RESOLVER: OnceLock<Module> = OnceLock::new();

impl Resolver {
    /// The shape every `Resolver` shares.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let d0 = s.pin(PinSpec { flags: pin_flag::RELAX, ..PinSpec::discrete("d0", role::PORT, &DRIVE) });
        let d1 = s.pin(PinSpec { flags: pin_flag::RELAX, ..PinSpec::discrete("d1", role::PORT, &DRIVE) });
        let v = s.pin(PinSpec::discrete("v", role::PORT, &BIT));
        s.function(FunctionSpec {
            name: "resolve",
            reads: vec![Pins::cell(d0), Pins::cell(d1)],
            writes: vec![Pins::cell(v)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// A live instance.
    fn handle() -> Handle {
        Handle::create(RESOLVER.get_or_init(|| module::<Resolver>("resolver")), &[]).expect("a stub creates")
    }

    /// The fold over the present drives' values, as a `BIT`.
    fn fold(present: &[u8]) -> [u8; 1] {
        let Some(first) = present.first() else { return LOW };
        if present.iter().all(|d| d == first) { [*first] } else { LOW }
    }
}

impl Device for Resolver {
    fn prototype() -> &'static Shape<'static> {
        RESOLVER_SHAPE.get_or_init(Resolver::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Resolver, Arc::clone(RESOLVER_SHAPE.get_or_init(Resolver::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let present: Vec<u8> = (0..2).map(|i| cx.bytes(i)).filter(|b| Drive::is_present(b)).map(|b| b[0]).collect();
        Received::note(present.len());
        cx.out_bytes(0).copy_from_slice(&Resolver::fold(&present));
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// An oscillator: drives the inverse of the level it reads.
/// Two of them on one resolver never settle.
///
/// pins: a=0 (PORT BIT); y=1 (PORT DRIVE, RELAX).  fn 0 "flip": reads [a] writes [y]
struct Oscillator;
static OSCILLATOR_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static OSCILLATOR: OnceLock<Module> = OnceLock::new();

impl Oscillator {
    /// The shape every `Oscillator` shares.
    fn shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
        let y = s.pin(PinSpec { flags: pin_flag::RELAX, ..PinSpec::discrete("y", role::PORT, &DRIVE) });
        s.function(FunctionSpec {
            name: "flip",
            reads: vec![Pins::cell(a)],
            writes: vec![Pins::cell(y)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// A live instance.
    fn handle() -> Handle {
        Handle::create(OSCILLATOR.get_or_init(|| module::<Oscillator>("oscillator")), &[]).expect("a stub creates")
    }
}

impl Device for Oscillator {
    fn prototype() -> &'static Shape<'static> {
        OSCILLATOR_SHAPE.get_or_init(Oscillator::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Oscillator, Arc::clone(OSCILLATOR_SHAPE.get_or_init(Oscillator::shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let level = cx.bytes(0)[0] == 0;
        cx.out_bytes(0).copy_from_slice(&Drive::present(level as u8));
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// The relaxing loop under test: `res` resolves the drives `d0` (from a keeper reading the
/// resolved level `v`, closing the loop) and `d1` (from an enabled driver outside the loop) into `v`.
struct Loop {
    /// The plan the stub devices are bound into, with its relaxing cycle.
    plan: DigitalPlan,
    /// The nets' values, as the digital solver holds them.
    store: Store,
    /// The functions to run in the current microstep.
    dirty: DirtySet,
    /// What the run observed, flushed after each `run`.
    obs: Obs,
    /// The microstep of the instant the loop is at.
    step: u32,
}

impl Loop {
    /// The loop bound by net name; `d1` is driven by a `Driver` reading nets `en` and `a`.
    fn new() -> Loop {
        Loop::of(vec![
            ("res", Resolver::handle(), vec![("d0", "d0"), ("d1", "d1"), ("v", "v")]),
            ("keep", Keeper::handle(), vec![("a", "v"), ("y", "d0")]),
            ("src", Driver::handle(), vec![("en", "en"), ("a", "a"), ("y", "d1")]),
        ])
    }

    /// Two oscillators on one resolver, their drive nets named `zeta` and `alpha` (bound in that
    /// order, so the name order of an observation differs from the bind order).
    fn oscillating() -> Loop {
        Loop::of(vec![
            ("res", Resolver::handle(), vec![("d0", "zeta"), ("d1", "alpha"), ("v", "v")]),
            ("osc0", Oscillator::handle(), vec![("a", "v"), ("y", "zeta")]),
            ("osc1", Oscillator::handle(), vec![("a", "v"), ("y", "alpha")]),
        ])
    }

    /// The loop of `devices`, bound by net name.
    fn of(devices: Vec<Placed<'_>>) -> Loop {
        let plan = PlanBinding::bind_by_name(devices, Resolution::default()).expect("the loop binds");
        let store = Store::new(&plan.nets);
        let dirty = DirtySet::none(&plan);
        Loop { plan, store, dirty, obs: Obs::new(), step: 0 }
    }

    /// The id of the net called `name`.
    fn net(&self, name: &str) -> NetId {
        PlanBinding::net_named(&self.plan, name)
    }

    /// The host writes `bytes` to the net called `name` in the current microstep.
    fn write(&mut self, name: &str, bytes: &[u8]) {
        let id = self.net(name);
        FunctionRun::new(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs).write_net(id, 0, bytes, (Tick(5), self.step));
    }

    /// Runs the dirty functions of the current microstep.
    fn run(&mut self) -> Result<(), DigitalError> {
        let ambient = Ambient::at(Tick(5), 1e-15);
        FunctionRun::new(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs).run((Tick(5), self.step), &ambient)?;
        self.obs.flush();
        Ok(())
    }

    /// Moves on to the next microstep of the instant.
    fn next_microstep(&mut self) {
        self.store.begin_microstep();
        self.step += 1;
    }

    /// The bytes of the net called `name`.
    fn value(&self, name: &str) -> Vec<u8> {
        self.store.value(self.net(name)).to_vec()
    }

    /// How many `RELAX` nets the one relaxing cycle of the plan passes through (its drive count).
    fn drive_count(&self) -> usize {
        use piperine_digital_solver::dag::Step;
        self.plan
            .dag()
            .order()
            .iter()
            .find_map(|step| match step {
                Step::Relax { nets, .. } => Some(nets.len()),
                _ => None,
            })
            .expect("the loop has one relaxing cycle")
    }
}

#[test]
fn a_relaxing_cycle_starts_absent_and_stops_within_drives_plus_one_passes() {
    // §8.2.3 step 1: every drive of the loop starts as not driving; RN-05 AC 2: repeat until no
    // drive changes, at most the loop's drive count plus one passes.
    let mut run = Loop::new();
    run.write("en", &HIGH);
    run.write("a", &HIGH);
    run.run().expect("the loop runs");
    let counts = Received::counts();
    assert_eq!(counts[0], 1, "pass 1: the keeper's drive is absent, only the source's is present: {counts:?}");
    assert!(counts.len() <= run.drive_count() + 1, "at most drives + 1 passes: {counts:?}");
    assert_eq!(counts.last(), Some(&2), "the last pass sees both drives present: {counts:?}");
    assert_eq!(run.value("v"), HIGH, "the loop settled on the source's level");
    // The bound is reached, not only respected: a loop that never settles (two oscillators) runs
    // exactly its drive count plus one passes, so a looser bound would fail here.
    let mut oscillating = Loop::oscillating();
    oscillating.dirty = DirtySet::all(&oscillating.plan);
    let before = Received::counts().len();
    oscillating.run().expect_err("a loop that does not settle is a fault");
    assert_eq!(Received::counts().len() - before, oscillating.drive_count() + 1, "exactly drives + 1 passes: {:?}", Received::counts());
}

#[test]
fn a_released_loop_keeps_no_earlier_level() {
    // RN-05 AC 6: a loop cannot latch. The keeper holds `v` while the source drives it; once the
    // source releases the net, the cycle restarts absent and `v` does not keep the earlier level.
    let mut run = Loop::new();
    run.write("en", &HIGH);
    run.write("a", &HIGH);
    run.run().expect("the loop runs");
    assert_eq!(run.value("v"), HIGH, "driven");
    run.next_microstep();
    run.write("en", &LOW);
    run.run().expect("the loop runs");
    assert_eq!(run.value("v"), LOW, "released: no drive is left, so no level is kept");
}

#[test]
fn an_absent_drive_is_left_out_and_a_present_one_is_passed_whatever_its_value() {
    // RN-05 AC 7 (§8.2.3 step 1, §5.2.12, design D3): the absent drive is not passed, so the
    // other decides alone; a present drive is passed whatever its value, a zero value included:
    // absence is not a value
    let mut run = Loop::new();
    run.write("en", &HIGH);
    run.write("a", &HIGH);
    run.run().expect("the loop runs");
    assert_eq!(Received::counts()[0], 1, "the absent keeper drive is left out of the first pass");
    assert_eq!(run.value("v"), HIGH, "the one present drive decides alone");

    let mut zero = Loop::new();
    zero.write("en", &HIGH);
    zero.write("a", &LOW);
    let before = Received::counts().len();
    zero.run().expect("the loop runs");
    let counts = Received::counts();
    assert_eq!(counts[before], 1, "a present drive of the zero value is passed: {counts:?}");
    assert_eq!(zero.value("v"), LOW, "and it decides the level");
}

#[test]
fn an_unsettled_loop_is_e0511_naming_its_drives() {
    // §8.2.3 step 4, abi.md §9 D1: a loop still moving after its last pass stops the run with
    // `E0511`, not a level made up: it names the loop's drives, in name order
    let mut run = Loop::oscillating();
    run.dirty = DirtySet::all(&run.plan);
    let error = run.run().expect_err("a loop that does not settle is E0511");
    assert!(matches!(&error, DigitalError::Unsettled { code: "E0511", drives, .. } if drives == &["alpha", "zeta"]), "{error:?}");
    assert!(error.to_string().contains("E0511") && error.to_string().contains("alpha, zeta"), "{error}");
}

#[test]
fn a_drive_pin_ends_with_a_plain_driving_byte() {
    // EU-16 AC 3, abi.md §9 D1: a drive pin is the bytes of its driven type, then `driving: BOOL`,
    // 0 for absent; nothing else is in it. A present drive of a zero value is `[0, 1]`, an absent
    // one is `[0, 0]`: the byte, not the value, says whether it drives
    let mut run = Loop::new();
    run.write("en", &HIGH);
    run.write("a", &HIGH);
    run.run().expect("the loop runs");
    assert_eq!(run.value("d1"), [1, 1], "the source's drive: the value, then `driving` 1");
    assert_eq!(run.value("d0"), [1, 1], "the keeper's drive, present once the level is read");
    let mut zero = Loop::new();
    zero.write("en", &HIGH);
    zero.write("a", &LOW);
    zero.run().expect("the loop runs");
    assert_eq!(zero.value("d1"), [0, 1], "a present drive of the zero value: `driving` is 1");
    let mut absent = Loop::new();
    absent.write("en", &LOW);
    absent.write("a", &HIGH);
    absent.dirty = DirtySet::all(&absent.plan);
    absent.run().expect("the loop runs");
    assert_eq!(absent.value("d1"), [0, 0], "an absent drive: `driving` 0 and no value");
}
