//! Scope: chapter 19 of the Book, "The solver" (feature `root-e2e`, RE-02 and RE-03): the statements the chapter
//! makes about what the solver does and reports, asserted through `piperine::` on the smallest design
//! that shows each (an instant that never goes quiet, an operating point that never settles, a step
//! that lands on a digital instant, a loop of undelayed definitions) and on the manifests of the two
//! sides of the solver; the chapter's circuit, `filter.msl`, run under every analysis a run table
//! names; and the chapter's reference of the options of a run: each default the tables state is held
//! to the solver's own value, and each option is shown to act (a bound that moves, a step that is
//! capped, a range that is refused). The sentences that describe the algorithm (the homotopy steps,
//! the rollback of a rejected step, the purpose of the chapter) are `prose` rows: no value of a run
//! states them. Each test opens with the inventory ids it holds, from `tests/book/inventory.tsv`.

use crate::support;

use piperine::request::Options;
use piperine::{Cell, Overrides, RunSpec};
use piperine_solver::prelude::{Context, PssAnalysisOptions, Tolerances, TransientAnalysisOptions};
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::manifests::Manifests;
use support::model::Model;

/// A top whose analog node follows a digital bit through a `transition`: the smallest crossing of the
/// two sides.
struct Crossing;

impl Crossing {
    /// The analog side: node `n` takes 1 V when `d` is high, 0 V otherwise.
    const ANALOG: &'static str = "module Top() { signal n: Electrical; signal d: Bit = false; }\nanalog for Top { v(n) +> transition(if d { 1.0 } else { 0.0 }); }\n";
}

#[test]
fn the_two_sides_of_the_solver_never_depend_on_each_other() {
    // solver~83cb975b "The analog and digital solvers never call each other: everything that crosses between
    // them is an event the owner carries.": neither crate lists the other, nor the owner above them
    for (side, other) in [("piperine-analog-solver", "piperine-digital-solver"), ("piperine-digital-solver", "piperine-analog-solver")] {
        assert!(!Manifests::depends_on(side, other), "{side} lists {other}");
        assert!(!Manifests::depends_on(side, "piperine-solver"), "{side} lists the owner of time");
    }
    assert!(Manifests::depends_on("piperine-solver", "piperine-analog-solver") && Manifests::depends_on("piperine-solver", "piperine-digital-solver"), "the owner carries both");
}

#[test]
fn an_operating_point_that_never_settles_stops_after_20_rounds_with_e0506_naming_its_nets() {
    // simulation~5c44ef1d "E0506"; solver~53123473 "After 20 rounds that still change something the solver stops with E0506 and names
    // the nets that kept moving, rather than return a wrong answer.": a comparator that inverts the
    // level it reads never settles
    let source = format!("{}digital for Top {{ on above(v(n) - 0.5) {{ d <- false; }} on above(0.5 - v(n)) {{ d <- true; }} }}\n", Crossing::ANALOG);
    let failure = Model::written("m.msl", &source).run(Faulty::operating_point_of("Top")).err().expect("the operating point does not settle");
    let said = format!("{failure:#}");
    assert!(said.contains("E0506: the operating point did not settle in 20 rounds; nets still changing: "), "{said}");
    assert!(said.contains("Top.transition$0.x"), "the nets that kept moving are named: {said}");
}

#[test]
fn an_instant_that_never_goes_quiet_stops_after_1000_microsteps_with_e0501_naming_its_nets() {
    // solver~dbaf149d "After 1000 microsteps the solver stops with E0501 and names the nets that keep
    // changing.": a bit that inverts itself every time it changes
    let source = "module Top() { signal x: Bit = false; }\ndigital for Top { on x.changed() { x <- !x; } on at(1u) { x <- true; } }\n";
    let failure = Model::written("m.msl", source).run(Faulty::transient_of_top(3e-6, None)).err().expect("the instant does not go quiet");
    let said = format!("{failure:#}");
    assert!(said.contains("E0501: the instant at tick 0 did not go quiet; still changing: Top.x (the bound is 1000 microsteps)"), "{said}");
}

#[test]
fn a_step_lands_exactly_on_a_digital_instant_and_what_it_changed_reaches_the_analog_side() {
    // solver~ce518374 "It stops at the first instant that touches the analog side, and the step is shortened
    // to land exactly there."; solver~c2aa698e "What the instant changed reaches the analog side.": a bit
    // that rises at 1.234567 us moves the node from 0 V to 1 V, and the table has the row of that instant
    let source = format!("{}digital for Top {{ on at(1.234567u) {{ d <- true; }} }}\n", Crossing::ANALOG);
    let ran = Model::written("m.msl", &source).run(Faulty::transient_of_top(3e-6, None)).unwrap_or_else(|failure| panic!("{failure}"));
    let landed = (0..ran.table.rows.len()).find(|row| (ran.real(*row, "time") - 1.234567e-6).abs() < 1e-12).expect("a row at the instant of the edge");
    assert_eq!(ran.cell(landed - 1, "Top.d"), &Cell::Bool(false), "the bit was low on the row before the instant");
    assert_eq!(ran.cell(landed, "Top.d"), &Cell::Bool(true), "and is high on the row that lands exactly on it");
    assert_eq!(ran.cell(landed, "Top.n"), &Cell::Real(0.0), "the analog side was solved on the values the digital side had just before the instant");
    assert_eq!(ran.cell(landed + 1, "Top.n"), &Cell::Real(1.0), "and sees what the instant changed on the next step");
}

#[test]
fn a_loop_of_undelayed_definitions_is_refused_when_the_design_is_built_naming_its_functions() {
    // solver~4642dd47 "A loop of functions is accepted only if it passes through a net marked to relax, which
    // is iterated until it stops changing. Any other loop is refused when the design is built, naming its
    // functions.": no source form marks a net to relax, so the refusal of the other loop is what a source
    // shows
    let faulty = Faulty::elaborating("solver~4642dd47", "module Top() { signal a: Bit; signal b: Bit; a = !b; b = !a; }\n", "Top");
    assert!(faulty.first("E0339").message.contains("undelayed definitions of `a`, `b` depend on each other"), "{}", faulty.first("E0339").message);
}

/// The chapter's circuit, `filter.msl`: 5 V into 1 kOhm and 1 uF.
struct Filter;

impl Filter {
    /// The file the Book marks `filter.msl`.
    fn model() -> Model {
        Chapter::read().marked("filter.msl", "solver#1")
    }

    /// The overrides of `piperine run filter.msl --analysis <analysis>`, with `extra` changes applied.
    fn asked(analysis: &str, extra: impl FnOnce(&mut Overrides)) -> Overrides {
        let mut overrides = Overrides { analysis: Some(analysis.to_string()), ..Overrides::default() };
        extra(&mut overrides);
        overrides
    }

    /// What the run of `overrides` on the filter gives. Panics with the failure when it fails.
    fn ran(overrides: Overrides) -> support::model::Ran {
        Self::model().run(overrides).unwrap_or_else(|failure| panic!("filter.msl fails: {failure:#}"))
    }

    /// What the refused run of `overrides` on the filter says.
    fn refused(overrides: Overrides) -> String {
        format!("{:#}", Self::model().run(overrides).err().expect("the run is refused"))
    }

    /// The filter with a monitor on its output, on the top `Top`: `reactions` is the body of the monitor's
    /// `digital` block.
    fn watched(reactions: &str) -> Model {
        let parts = Chapter::read().text("solver#1");
        let parts = parts.split("/// 5 V into").next().expect("the parts come before the top").to_string();
        let top = "module Top() { signal vin: Electrical; signal vout: Electrical = 0.0; let v1 = Source(vin, gnd) { dc: 5.0 }; let r1 = Resistor(vin, vout) { r: 1k }; let c1 = Capacitor(vout, gnd) { c: 1u }; let w = Watch(vout); }";
        Model::written("watched.msl", &format!("{parts}module Watch(a: Electrical) {{}}\ndigital for Watch {{ {reactions} }}\n{top}\n"))
    }

    /// The times of the rows of a transient table.
    fn times(ran: &support::model::Ran) -> Vec<f64> {
        (0..ran.table.rows.len()).map(|row| ran.real(row, "time")).collect()
    }
}

#[test]
fn the_noise_model_runs_and_its_density_is_the_resistors_shaped_by_the_filter() {
    // solver#16 (run, `hiss.msl`) with its listings solver#17 and solver#18; solver~611c58a3 "Only the resistor of
    // hiss.msl makes noise, so the density is its 4 k T / r shaped by the filter, and the input-referred density is
    // flat.": at 300.15 K the resistor's density is `sqrt(4 k T r)` = 4.071372e-9 V/sqrt(Hz), the 1 kOhm and 1 uF
    // pole is at 159.15 Hz, so `onoise = sqrt(4 k T r) / sqrt(1 + (f / f_c)^2)` and `inoise` is the same density
    // divided by that gain: the resistor's own, at every frequency
    let ran = Chapter::read().marked("hiss.msl", "solver#16").run(Overrides {
        analysis: Some("noise".to_string()),
        input: Some("v1".to_string()),
        output: Some("vout".to_string()),
        from: Some(10.0),
        to: Some(1e4),
        per_decade: Some(1),
        ..Overrides::default()
    });
    let ran = ran.unwrap_or_else(|failure| panic!("hiss.msl fails: {failure:#}"));
    let resistor = (4.0 * 1.380649e-23 * 300.15 * 1000.0_f64).sqrt();
    let corner = 1.0 / (2.0 * std::f64::consts::PI * 1000.0 * 1e-6);
    assert_eq!(ran.table.rows.len(), 4, "one row per decade from 10 Hz to 10 kHz");
    for (row, frequency) in [10.0, 100.0, 1e3, 1e4].into_iter().enumerate() {
        let wanted = resistor / (1.0 + (frequency / corner) * (frequency / corner)).sqrt();
        assert!((ran.real(row, "onoise") / wanted - 1.0).abs() < 1e-3, "onoise at {frequency} Hz: {} against {wanted}", ran.real(row, "onoise"));
        assert!((ran.real(row, "inoise") / resistor - 1.0).abs() < 1e-3, "inoise at {frequency} Hz is the resistor's own: {}", ran.real(row, "inoise"));
    }
}

#[test]
fn the_filter_runs_under_every_analysis_a_run_table_names() {
    // solver#1 (run, `filter.msl`) with its listings solver#3 to solver#15; solver~f3c07a6c "One small circuit
    // serves every command of this chapter: 5 V into 1 kOhm and 1 uF."
    // solver~c62ffb0e "Every signal of the top is printed unless --print names some."
    let op = Filter::model().run(Filter::asked("op", |_| {})).unwrap_or_else(|failure| panic!("{failure:#}"));
    assert_eq!(op.table.rows.len(), 2, "both signals of the top: {:?}", op.table.rows);
    assert!((op.signal("Filter.vin") - 5.0).abs() < 1e-9 && (op.signal("Filter.vout") - 5.0).abs() < 1e-9, "{:?}", op.table.rows);
    let named = Filter::ran(Filter::asked("op", |o| o.print = vec!["vout".to_string()]));
    assert_eq!(named.table.rows.len(), 1, "only the named signal: {:?}", named.table.rows);

    let dc = Filter::ran(Filter::asked("dc", |o| {
        (o.sweep, o.from, o.to, o.step) = (Some("v1.dc".to_string()), Some(0.0), Some(5.0), Some(2.5));
        o.print = vec!["vout".to_string()];
    }));
    let points: Vec<(f64, f64)> = (0..dc.table.rows.len()).map(|row| (dc.real(row, "Filter.v1.dc"), dc.real(row, "Filter.vout"))).collect();
    assert_eq!(points.len(), 3, "from, from + step, and to: {points:?}");
    assert!(points.iter().zip([0.0, 2.5, 5.0]).all(|((at, out), value)| (at - value).abs() < 1e-12 && (out - value).abs() < 1e-9), "{points:?}");

    // solver~27f60967 "The grid is logarithmic with per_decade points per decade, or linear with points points in
    // all; a run gives exactly one of the two."; solver~ff13a07c "A run gives per_decade or points, never both."
    let grid = |extra: fn(&mut Overrides)| {
        Filter::asked("ac", |o| {
            (o.from, o.to) = (Some(10.0), Some(1e4));
            o.print = vec!["vout".to_string()];
            extra(o);
        })
    };
    let decades = Filter::ran(grid(|o| o.per_decade = Some(1)));
    let frequencies: Vec<f64> = (0..decades.table.rows.len()).map(|row| decades.real(row, "freq")).collect();
    assert!(frequencies.len() == 4 && frequencies.iter().zip([10.0, 100.0, 1e3, 1e4]).all(|(given, decade)| (given / decade - 1.0).abs() < 1e-12), "one point per decade: {frequencies:?}");
    assert!((decades.real(1, "mag(Filter.vout)") - 8.467330e-1).abs() < 1e-6, "the magnitude at 100 Hz, under the 159 Hz corner");
    let linear = Filter::ran(grid(|o| o.points = Some(5)));
    assert_eq!(linear.table.rows.len(), 5, "five points in all");
    assert!((linear.real(1, "freq") - linear.real(0, "freq") - 2497.5).abs() < 1e-6, "evenly spaced from 10 Hz to 10 kHz");
    assert!(Filter::refused(grid(|o| (o.per_decade, o.points) = (Some(1), Some(5)))).contains("P0010: run `filter`: an `ac` run gives exactly one of `per_decade` and `points`, not both"));
    assert!(Filter::refused(grid(|_| {})).contains("P0010: run `filter`: an `ac` run gives exactly one of `per_decade` and `points`"));

    // solver~871f37eb "The run starts at time zero from the initial values the design gives, here vout at 0 V.";
    // solver~b1a27ddc "An initial value is not an option of a run: it belongs to the design. signal vout: Electrical
    // = 0.0; starts a transient with that node at 0 V, as the transient above shows in its first row, and the
    // operating point pays it no attention, as the first listing shows." (the operating point above is 5 V)
    let transient = Filter::ran(Filter::asked("transient", |o| {
        (o.stop, o.print_step) = (Some(5e-3), Some(1e-3));
        o.print = vec!["vout".to_string()];
    }));
    assert_eq!(Filter::times(&transient).len(), 6, "a row per millisecond, both ends included");
    assert_eq!((transient.real(0, "time"), transient.real(0, "Filter.vout")), (0.0, 0.0), "the transient starts at time zero, at the initial value");
    assert!((transient.real(1, "Filter.vout") - 5.0 * (1.0 - (-1.0f64).exp())).abs() < 5e-3, "one time constant later: {}", transient.real(1, "Filter.vout"));

    // solver~a00b14aa "Transfer function and pole-zero. input names a voltage source by its instance and output a
    // node. tf gives three numbers at DC; pz lists the poles and zeros of the same transfer, in rad/s."
    let transfer = |analysis: &str| Filter::ran(Filter::asked(analysis, |o| (o.input, o.output) = (Some("v1".to_string()), Some("vout".to_string()))));
    let tf = transfer("tf");
    let quantities: Vec<&Cell> = tf.table.rows.iter().map(|row| &row[0]).collect();
    assert_eq!(quantities, [&Cell::Text("gain".to_string()), &Cell::Text("input_impedance".to_string()), &Cell::Text("output_impedance".to_string())]);
    assert!((tf.real(0, "value") - 1.0).abs() < 1e-9 && (tf.real(2, "value") - 1e3).abs() < 1e-6, "{:?}", tf.table.rows);
    let pz = transfer("pz");
    assert_eq!(pz.table.rows.len(), 1, "one pole, no zero: {:?}", pz.table.rows);
    assert_eq!(pz.cell(0, "kind"), &Cell::Text("pole".to_string()));
    assert!((pz.real(0, "real") + 1e3).abs() < 1e-6 && pz.real(0, "imag") == 0.0, "the pole at -1/RC: {:?}", pz.table.rows);

    // solver~6826722d "Sensitivity. wrt names params, each by its path; the table has the derivative of every
    // printed signal by each."
    let sens = Filter::ran(Filter::asked("sens", |o| {
        o.wrt = vec!["v1.dc".to_string(), "r1.r".to_string()];
        o.print = vec!["vout".to_string()];
    }));
    assert_eq!(sens.table.columns, ["param", "d(Filter.vout)"]);
    assert!((sens.real(0, "d(Filter.vout)") - 1.0).abs() < 1e-6 && sens.real(1, "d(Filter.vout)").abs() < 1e-9, "{:?}", sens.table.rows);
}

#[test]
fn every_default_the_chapter_states_is_the_solver_s_own() {
    // solver~ebde9a95 "An option that is not given keeps the default of the tables below; a run that gives none is
    // a run with every default.": a request made of an analysis alone carries no option, and the session then
    // leaves each number of the solver where the solver has it
    let bare = RunSpec::from_flags("filter", &Overrides { analysis: Some("op".to_string()), ..Overrides::default() }).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(bare.options, Options::default(), "no option is set by a request that gives none");
    assert_eq!(Options::default().nodeset, Vec::new(), "`nodeset` is empty unless given");
    // the cells of the column `Default`: solver~5377a9ef "1e-3" (reltol), solver~c2d4b93e "1e-6 (1 µV)" (vntol),
    // solver~e1c0761b "1e-12 (1 pA)" (abstol), solver~af63aa4c "7" (trtol), solver~08030107 "27" (temp, degrees
    // Celsius: 300.15 K in the solver), and of the table of what is fixed, solver~8b782f93 "1e-12 S" (gmin)
    let tolerances = Tolerances::default();
    assert_eq!((tolerances.reltol, tolerances.vntol, tolerances.abstol, tolerances.trtol), (1e-3, 1e-6, 1e-12, 7.0));
    assert_eq!(tolerances.temperature, 27.0 + 273.15);
    assert_eq!(tolerances.gmin, 1e-12);
    // solver~08030407 "20" (maxrounds); solver~a10a0edf "2000000" (max_steps); solver~571dbf5f "1e-15 s (1 fs)" (the
    // shortest step); solver~39210a09 "1e-12 s (1 ps)" (ttol); "Without it a transient starts with a step of
    // stop/1000 and takes none longer than stop/100" is held on a run below and by `book_simulation`
    assert_eq!(Context::DEFAULT_MAX_ROUNDS, 20);
    assert_eq!(Context::default().max_rounds, 20);
    let transient = TransientAnalysisOptions::new(1.0, 1e-3);
    assert_eq!((transient.max_steps, transient.dt_min, transient.ttol, transient.dt_max), (2_000_000, 1e-15, 1e-12, 1.0 / 100.0));
    assert_eq!(PssAnalysisOptions::new(4e-6).tstab, 0.0, "`tstab` is 0 unless given");
    // solver~646ead0b "1e-15 (1 fs)" (resolution) and solver~0d8269f0 "1000" (maxmicro) are the digital engine's:
    // `the_digital_engine_ticks_in_femtoseconds_and_bounds_an_instant_at_1000_microsteps` holds them on a run
}

#[test]
fn the_digital_engine_ticks_in_femtoseconds_and_bounds_an_instant_at_1000_microsteps() {
    // solver~646ead0b "1e-15 (1 fs)": a reaction at 1 us runs at tick 1000000000, and at tick 1000000 with a
    // resolution of 1 ps; solver~0d8269f0 "1000" and solver~e8f12a9b "How many microsteps one instant may take
    // before the run fails with E0501.": the bound the error names is 1000, and 50 when the run says so
    let faulting = "module Top() {}\ndigital for Top { on at(1u) { let a = 1u8; let b = 0u8; emit!(\"q\", q = a / b); } }\n";
    let tick = |resolution: Option<f64>| format!("{:#}", Model::written("m.msl", faulting).run(Overrides { resolution, ..Faulty::transient_of_top(2e-6, None) }).err().expect("the division faults"));
    assert!(tick(None).contains("in `Top` at tick 1000000000:"), "{}", tick(None));
    assert!(tick(Some(1e-12)).contains("in `Top` at tick 1000000:"), "{}", tick(Some(1e-12)));
    let restless = "module Top() { signal x: Bit = false; }\ndigital for Top { on x.changed() { x <- !x; } }\n";
    let bound = |maxmicro: Option<u32>| format!("{:#}", Model::written("m.msl", restless).run(Overrides { maxmicro, ..Faulty::transient_of_top(2e-6, None) }).err().expect("the instant does not go quiet"));
    assert!(bound(None).contains("E0501") && bound(None).contains("(the bound is 1000 microsteps)"), "{}", bound(None));
    assert!(bound(Some(50)).contains("(the bound is 50 microsteps)"), "{}", bound(Some(50)));
}

#[test]
fn the_bound_on_the_rounds_of_an_operating_point_is_the_run_s_maxrounds() {
    // solver~bfc62286 "How many times an operating point of a mixed design may alternate between the analog solve
    // and the digital reactions before it fails with E0506.": the comparator that never settles stops after 5
    // rounds when the run says 5 (after 20 by default: solver~53123473)
    let source = format!("{}digital for Top {{ on above(v(n) - 0.5) {{ d <- false; }} on above(0.5 - v(n)) {{ d <- true; }} }}\n", Crossing::ANALOG);
    let failure = Model::written("m.msl", &source).run(Overrides { maxrounds: Some(5), ..Faulty::operating_point_of("Top") }).err().expect("the operating point does not settle");
    assert!(format!("{failure:#}").contains("E0506: the operating point did not settle in 5 rounds;"), "{failure:#}");
}

#[test]
fn the_step_options_of_a_transient_bound_its_steps_and_its_budget() {
    // The table "Time and steps": without `max_step` the first step is stop/1000 and none is longer than
    // stop/100; with it, it is the first step and the longest; `from` keeps the rows from it on; `print_step`
    // spaces the rows and changes no step; a lower `reltol` takes more steps
    let steps = |extra: fn(&mut Overrides)| {
        let ran = Filter::ran(Filter::asked("transient", |o| {
            o.stop = Some(5e-3);
            o.print = vec!["vout".to_string()];
            extra(o);
        }));
        Filter::times(&ran)
    };
    let lengths = |times: &[f64]| times.windows(2).map(|pair| pair[1] - pair[0]).collect::<Vec<f64>>();
    let plain = steps(|_| {});
    let free = lengths(&plain);
    assert!((free[0] - 5e-6).abs() < 1e-12, "the first step is stop/1000: {}", free[0]);
    assert!(free.iter().all(|step| *step <= 5e-5 * (1.0 + 1e-9)) && free.iter().any(|step| (*step - 5e-5).abs() < 1e-10), "no step is longer than stop/100, and some reach it");
    let capped = lengths(&steps(|o| o.max_step = Some(1e-5)));
    assert!((capped[0] - 1e-5).abs() < 1e-12 && capped.iter().all(|step| *step <= 1e-5 * (1.0 + 1e-6)), "`max_step` is the first step and the longest");
    assert!(lengths(&steps(|o| o.reltol = Some(1e-6))).len() > 5 * free.len(), "a tighter `reltol` takes more steps");
    let late = steps(|o| o.from = Some(3e-3));
    assert!(late[0] >= 3e-3 && *late.last().expect("a row") == 5e-3 && late.len() < plain.len(), "`from` keeps the rows from 3 ms on: {} of {}", late.len(), plain.len());
    assert_eq!(steps(|o| o.print_step = Some(1e-3)).len(), 6, "`print_step` gives a row per multiple of it");
    // solver~8cf56b9b "A run that uses it up stops with an error that says where it was." (the listing solver#18)
    let spent = Filter::refused(Filter::asked("transient", |o| (o.stop, o.max_steps) = (Some(5e-3), Some(20))));
    assert!(spent.contains("exceeded the 20 step budget at t = 5.528574e-4 s of 5.000000e-3 s") && spent.contains("accepted 20, rejected 0"), "{spent}");
}

#[test]
fn a_value_outside_its_range_and_a_key_outside_its_analysis_are_p0010() {
    // solver~79471dce "A value outside the range of its option is refused with P0010 before the design is
    // compiled."; solver~7c9343cf "A key that the chosen analysis does not read is refused with P0010 before anything
    // runs, so a setting is never silently ignored.": the ranges of the chapter's tables, one refusal each
    let op = |extra: fn(&mut Overrides)| Filter::refused(Filter::asked("op", extra));
    for (said, expected) in [
        (op(|o| o.reltol = Some(0.0)), "`reltol` must be greater than 0, found 0"),
        (op(|o| o.vntol = Some(-1e-6)), "`vntol` must be greater than 0, found -0.000001"),
        (op(|o| o.abstol = Some(0.0)), "`abstol` must be greater than 0, found 0"),
        (op(|o| o.trtol = Some(0.0)), "`trtol` must be greater than 0, found 0"),
        (op(|o| o.resolution = Some(0.0)), "`resolution` must be greater than 0, found 0"),
        (op(|o| o.temp = Some(-300.0)), "`temp` must be greater than -273.15 (degrees Celsius), found -300"),
        (op(|o| o.maxrounds = Some(0)), "`maxrounds` must be a whole number of at least 1, found 0"),
        (op(|o| o.maxmicro = Some(0)), "`maxmicro` must be a whole number of at least 1, found 0"),
        (op(|o| o.max_steps = Some(0)), "`max_steps` must be a whole number of at least 1, found 0"),
        (op(|o| o.stop = Some(1.0)), "`stop` is not read by a `op` run"),
        (op(|o| o.period = Some(1.0)), "`period` is not read by a `op` run"),
        (Filter::refused(Filter::asked("transient", |_| {})), "a `transient` run needs `stop`"),
        (Filter::refused(Filter::asked("transient", |o| o.stop = Some(0.0))), "`stop` must be greater than 0, found 0"),
        (Filter::refused(Filter::asked("transient", |o| (o.stop, o.from) = (Some(1e-3), Some(1e-3)))), "`from` must be at least 0 and less than `stop` (0.001), found 0.001"),
        (Filter::refused(Filter::asked("transient", |o| (o.stop, o.max_step) = (Some(1e-3), Some(0.0)))), "`max_step` must be greater than 0, found 0"),
        (Filter::refused(Filter::asked("dc", |o| (o.sweep, o.from, o.to, o.step) = (Some("v1.dc".to_string()), Some(0.0), Some(5.0), Some(0.0)))), "`step` must not be 0"),
        (Filter::refused(Filter::asked("dc", |o| (o.sweep, o.from, o.to, o.step) = (Some("v1.dc".to_string()), Some(0.0), Some(5.0), Some(-1.0)))), "`step` -1 points away from `to` (0 to 5)"),
        (Filter::refused(Filter::asked("ac", |o| (o.from, o.to, o.points) = (Some(0.0), Some(1e3), Some(5)))), "`from` must be greater than 0, found 0"),
        (Filter::refused(Filter::asked("ac", |o| (o.from, o.to, o.points) = (Some(1e3), Some(10.0), Some(5)))), "`to` (10) is less than `from` (1000)"),
        (Filter::refused(Filter::asked("pss", |o| (o.period, o.tstab) = (Some(1e-3), Some(-1.0)))), "`tstab` must be at least 0, found -1"),
        (Filter::refused(Filter::asked("sens", |_| {})), "a `sens` run needs `wrt`"),
        (Filter::refused(Filter::asked("tf", |o| o.input = Some("v1".to_string()))), "a `tf` run needs `output`"),
    ] {
        assert!(said.contains("P0010") && said.contains(expected), "expected `{expected}`: {said}");
    }
}

#[test]
fn a_nodeset_seeds_the_operating_point_and_the_temperature_reaches_the_models() {
    // The table "The operating point and the environment"; solver~9e407280 "A path that names no node is E0801.": a
    // circuit with one equilibrium ends at it whatever the guess; `temp` is degrees Celsius in the run and kelvin
    // in `temperature()` (the ambient fallback when none is set), and `vt(t)` follows it
    let seeded = Filter::ran(Filter::asked("op", |o| o.nodeset = vec![("vout".to_string(), 2.5)]));
    assert!((seeded.signal("Filter.vout") - 5.0).abs() < 1e-9, "the one equilibrium, whatever the guess");
    assert!(Filter::refused(Filter::asked("op", |o| o.nodeset = vec![("nope".to_string(), 1.0)])).contains("E0801: the nodeset entry `nope` names no node of the design"));
    let environment = |temp: Option<f64>| {
        let ran = Filter::watched("on at(1m) { emit!(\"env\", kelvin = temperature().unwrap_or(AMBIENT_TEMPERATURE), vt = vt(temperature().unwrap_or(AMBIENT_TEMPERATURE))); }").run(Overrides { temp, ..Faulty::transient_of_top(2e-3, None) }).unwrap_or_else(|failure| panic!("{failure:#}"));
        let record = ran.stream("env")[0].clone();
        (support::records::Fields::real(&record, "kelvin"), support::records::Fields::real(&record, "vt"))
    };
    let (kelvin, vt) = environment(None);
    assert!((kelvin - 300.15).abs() < 1e-9 && (vt - 2.586493e-2).abs() < 1e-8, "27 degrees Celsius by default: {kelvin} K, vt {vt}");
    let (kelvin, vt) = environment(Some(85.0));
    assert!((kelvin - 358.15).abs() < 1e-9 && (vt - 3.086298e-2).abs() < 1e-8, "85 degrees Celsius: {kelvin} K, vt {vt}");
}

#[test]
fn a_crossing_takes_its_own_ttol_and_etol() {
    // solver~da323789 "Per crossing: cross(e, 1n)."; solver~a7fb5a0a "Per crossing: cross(e, etol: 1m).": both forms
    // run, and each locates the output's rise through 2.5 V at RC ln 2
    let ran = Filter::watched("on cross(v(a) - 2.5, 1n) { emit!(\"tight\", t = now()); } on cross(v(a) - 2.5, etol: 1m) { emit!(\"loose\", t = now()); }").run(Faulty::transient_of_top(2e-3, None)).unwrap_or_else(|failure| panic!("{failure:#}"));
    for stream in ["tight", "loose"] {
        let records = ran.stream(stream);
        assert_eq!(records.len(), 1, "one crossing: {:?}", ran.records);
        assert!((records[0].time - 1e-3 * 2.0f64.ln()).abs() < 1e-6, "{stream}: located at {}, RC ln 2 is {}", records[0].time, 1e-3 * 2.0f64.ln());
    }
}
