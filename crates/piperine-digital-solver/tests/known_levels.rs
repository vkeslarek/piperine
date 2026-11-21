//! Per-level known flags in the digital engine (feature `unknown-semantics-per-level`, UL-05;
//! abi.md §9 D1, C7): a relaxation that does not settle clears exactly the levels still moving
//! in a `KNOWN_LEVELS` field and writes one record, a `MERGE` commit lands level by level, and a
//! device built against minor 1 (a plain known byte) binds and runs as before.

#[path = "support/devices.rs"]
#[allow(dead_code)] // each test target uses the devices it needs
mod devices;

use std::sync::Arc;

use devices::*;
use piperine_abi::caller::Handle;
use piperine_abi::device::{Eval, Factory, Fault, Flow, React, Values};
use piperine_abi::layout::{BYTE, is_canonical};
use piperine_abi::raw::{
    Cell, Field, Layout, List, Module, Shape, Statement, Str, cell, commit_flag, field_flag, leaf, part, pin_flag, role,
};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef};
use piperine_abi::thunk::module_with;
use piperine_digital_solver::engine::{Digital, Horizon, Mode};
use piperine_digital_solver::limits::DigitalLimits;
use piperine_digital_solver::net::NetId;
use piperine_digital_solver::observe::{Obs, Observation};
use piperine_digital_solver::plan::DigitalPlan;
use piperine_digital_solver::run::{Ambient, DirtySet, run_functions};
use piperine_digital_solver::store::Store;
use piperine_digital_solver::tick::{Resolution, Tick};

static LEVELS_BYTE_FIELDS: [Field; 1] = [Field {
    name: Str::from_static("v"),
    leaf: leaf::U8,
    offset: 0,
    count: 1,
    flags: field_flag::HAS_KNOWN | field_flag::KNOWN_LEVELS,
    known: 1,
}];
/// One `U8` and its mask: `{ v: U8 @0, known @1 }` with `KNOWN_LEVELS`, size 2.
static LEVELS_BYTE: Layout = Layout { size: 2, align: 1, fields: List::from_slice(&LEVELS_BYTE_FIELDS) };

/// What a [`Toggler`] writes to its output from its input byte.
#[derive(Clone, Copy, Debug)]
enum Behavior {
    /// Levels 0 and 1 true, level 2 false, level 3 flipped on every call, all four known:
    /// a `KNOWN_LEVELS` cycle that never settles through level 3.
    LevelsFlipLevelThree,
    /// A plain known byte that settles at 5.
    PlainSettles,
    /// A plain known byte whose value flips on every call: it never settles.
    PlainFlips,
}

/// A device with an input `a` and a `RELAX` output `y` of one layout, wired to one net: a relaxed
/// self-loop whose function is [`Behavior`].
/// pins: a=0 (PORT), y=1 (PORT, RELAX)   fn 0 "step": reads [a] writes [y]
struct Toggler {
    shape: ShapeRef,
    behavior: Behavior,
}

impl Toggler {
    /// An instance of the device over `layout` doing `behavior`.
    fn handle(layout: &'static Layout<'static>, behavior: Behavior) -> Handle {
        let mut shape = ShapeBuf::new();
        let a = shape.pin(PinSpec::discrete("a", role::PORT, layout));
        let y = shape.pin(PinSpec { flags: pin_flag::RELAX, ..PinSpec::discrete("y", role::PORT, layout) });
        let cell = |index| Cell { kind: cell::PIN, index, part: part::VALUE };
        shape.function(FunctionSpec { name: "step", reads: vec![cell(a)], writes: vec![cell(y)], ..FunctionSpec::default() });
        let toggler: &'static Toggler = Box::leak(Box::new(Toggler { shape: shape.freeze(), behavior }));
        let module: &'static Module = Box::leak(Box::new(module_with(
            match behavior {
                Behavior::LevelsFlipLevelThree => "levels",
                Behavior::PlainSettles => "plain-settles",
                Behavior::PlainFlips => "plain-flips",
            },
            toggler,
        )));
        Handle::create(module, &[]).expect("creates")
    }
}

impl Factory for Toggler {
    type Instance = Behavior;
    fn prototype(&self) -> &Shape<'_> {
        self.shape.shape()
    }
    fn create(&self, _: &Values<'_>) -> Result<(Behavior, ShapeRef), Fault> {
        Ok((self.behavior, Arc::clone(&self.shape)))
    }
    fn eval(behavior: &Behavior, _: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        let a = cx.bytes(0)[0];
        let out = match behavior {
            Behavior::LevelsFlipLevelThree => [0x03 | ((a & 0x08) ^ 0x08), 0x0F],
            Behavior::PlainSettles => [5, 1],
            Behavior::PlainFlips => [a ^ 1, 1],
        };
        cx.out_bytes(0).copy_from_slice(&out);
        Ok(())
    }
    fn react(_: &Behavior, _: u32, _: &mut React<'_>) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// One run of the discrete functions of a plan at tick 5, microstep 0.
struct Relaxing {
    plan: DigitalPlan,
    store: Store,
    obs: Obs,
}

impl Relaxing {
    /// Binds `handle` with `a` and `y` on one net `x`, runs every function once, and observes.
    fn of(handle: Handle) -> Relaxing {
        let plan = bind_by_name(vec![("t", handle, vec![("a", "x"), ("y", "x")])], Resolution::default())
            .expect("the fixture plan binds");
        let mut store = Store::new(&plan.nets);
        let mut dirty = DirtySet::all(&plan);
        let mut obs = Obs::new();
        let amb = Ambient::at(Tick(5), 1e-15);
        run_functions(&plan, &mut store, &mut dirty, (Tick(5), 0), &amb, &mut obs).expect("the cycle runs");
        obs.flush();
        Relaxing { plan, store, obs }
    }

    /// The bytes of net `x`.
    fn x(&self) -> Vec<u8> {
        self.store.value(net_named(&self.plan, "x")).to_vec()
    }

    /// How many records the run wrote about net `x` not settling.
    fn unsettled_records(&self) -> usize {
        self.obs.list().iter().filter(|o| matches!(o, Observation::Unsettled { net, .. } if net == "x")).count()
    }
}

/// abi.md §9 D1 and UL-05 AC 4: two passes of a `RELAX` loop whose level 3 keeps moving clear
/// level 3 (flag and value bit) and keep levels 0 to 2 known; one record names the net.
#[test]
fn a_relaxation_clears_only_the_levels_still_moving() {
    let run = Relaxing::of(Toggler::handle(&LEVELS_BYTE, Behavior::LevelsFlipLevelThree));
    // levels 0 and 1 true, level 2 false: value 0b0011; levels 0..=2 known: mask 0b0111
    assert_eq!(run.x(), [0x03, 0x07], "level 3 cleared, levels 0 to 2 keep value and flag");
    assert!(is_canonical(&LEVELS_BYTE, &run.x()), "the settled value is canonical (P3)");
    assert_eq!(run.unsettled_records(), 1, "one record: {:?}", run.obs.list());
}

/// UL-05 AC 2: a device built against minor 1, one known byte per element, binds and runs
/// unchanged: a loop that settles keeps its bytes, one that moves is zeroed whole (D1).
#[test]
fn a_minor_one_byte_device_binds_and_runs_unchanged() {
    let settled = Relaxing::of(Toggler::handle(&BYTE, Behavior::PlainSettles));
    assert_eq!(settled.x(), [5, 1], "a known byte, as written");
    assert_eq!(settled.unsettled_records(), 0, "it settled");

    let moving = Relaxing::of(Toggler::handle(&BYTE, Behavior::PlainFlips));
    assert_eq!(moving.x(), [0, 0], "a plain known byte that still moves is zeroed whole");
    assert_eq!(moving.unsettled_records(), 1);
}

/// A reaction that commits `commits` (bytes, flags) to a levels pin `out` when `go` fires.
/// pins: go=0 (PORT event); out=1 (PORT, `LEVELS_BYTE`)   reaction 0: on [go] writes [S0 {out}, ..]
struct Committer {
    commits: Vec<(Vec<u8>, u32)>,
    shape: ShapeRef,
}

impl Committer {
    /// A committer of `commits` to a `LEVELS_BYTE` pin, leaked so its module can borrow it.
    fn handle(commits: Vec<(Vec<u8>, u32)>) -> Handle {
        let mut shape = ShapeBuf::new();
        let go = shape.pin(PinSpec::event("go", role::PORT));
        let out = shape.pin(PinSpec::discrete("out", role::PORT, &LEVELS_BYTE));
        shape.reaction(ReactionSpec {
            name: "drive",
            on: vec![go],
            writes: commits
                .iter()
                .map(|(bytes, _)| Statement { pin: out, offset: 0, len: bytes.len() as u32, flags: 0, min_after: 0.0 })
                .collect(),
            ..ReactionSpec::default()
        });
        let committer: &'static Committer = Box::leak(Box::new(Committer { commits, shape: shape.freeze() }));
        let module: &'static Module = Box::leak(Box::new(module_with("Committer", committer)));
        Handle::create(module, &[]).expect("creates")
    }
}

impl Factory for Committer {
    type Instance = Vec<(Vec<u8>, u32)>;
    fn prototype(&self) -> &Shape<'_> {
        self.shape.shape()
    }
    fn create(&self, _: &Values<'_>) -> Result<(Self::Instance, ShapeRef), Fault> {
        Ok((self.commits.clone(), Arc::clone(&self.shape)))
    }
    fn eval(_: &Self::Instance, _: u32, _: &mut Eval<'_>) -> Result<(), Fault> {
        Ok(())
    }
    fn react(commits: &Self::Instance, _: u32, cx: &mut React<'_>) -> Result<Flow, Fault> {
        for (statement, (bytes, flags)) in commits.iter().enumerate() {
            cx.commit_with(statement as u32, bytes, 0.0, *flags)?;
        }
        Ok(Flow::Continue)
    }
}

/// An engine with one [`Committer`] and the ticks it is driven at.
struct Merging {
    engine: Digital,
}

impl Merging {
    /// One tick, in ticks of 1 fs.
    const TICK: i64 = 1_000_000;

    /// The bytes of net `q` after one reaction made `commits` to it while `q` held `held`.
    fn after(held: [u8; 2], commits: Vec<(Vec<u8>, u32)>) -> Vec<u8> {
        let plan = bind_by_name(vec![("d", Committer::handle(commits), vec![("go", "go"), ("out", "q")])], Resolution::default())
            .expect("binds");
        let mut merging = Merging { engine: Digital::new(plan, DigitalLimits::default()) };
        merging.engine.start().expect("starts");
        merging.run(Tick(0));
        let q = merging.net("q");
        merging.engine.host_commit(q, 0, &held, Tick(Merging::TICK)).expect("the host writes q");
        merging.run(Tick(Merging::TICK));
        let at = Tick(2 * Merging::TICK);
        let go = merging.net("go");
        merging.engine.host_commit(go, 0, &[], at).expect("the host fires go");
        merging.run(at);
        merging.engine.read(q).to_vec()
    }

    fn net(&self, name: &str) -> NetId {
        net_named(self.engine.plan(), name)
    }

    fn run(&mut self, at: Tick) {
        self.engine.run_instant(at, &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).expect("the instant runs");
    }
}

/// abi.md C7 and UL-05 AC 7: a `MERGE` commit to a `KNOWN_LEVELS` field keeps each level both
/// values hold known and equal and clears the others, value bits included.
#[test]
fn a_merge_commit_lands_per_level() {
    // q holds levels 0..=3 known with values 0b1010 (levels 1 and 3 true); the commit is 0b0011,
    // all four known. They differ at levels 0 and 3 (0b1010 ^ 0b0011 = 0b1001), so levels 1 and 2
    // stay: mask 0b0110, value 0b1010 & 0b0110 = 0b0010.
    let flags = commit_flag::MAYBE | commit_flag::MERGE;
    assert_eq!(Merging::after([0x0A, 0x0F], vec![(vec![0x03, 0x0F], flags)]), [0x02, 0x06]);
    // the commit knows levels 0 and 1 only (values 0 and 1), both equal to q's: they stay, and
    // levels 2 and 3, unknown on the commit's side, are cleared
    assert_eq!(Merging::after([0x0A, 0x0F], vec![(vec![0x02, 0x03], flags)]), [0x02, 0x03]);
    // a plain commit still overwrites whole
    assert_eq!(Merging::after([0x0A, 0x0F], vec![(vec![0x03, 0x0F], 0)]), [0x03, 0x0F]);
}
