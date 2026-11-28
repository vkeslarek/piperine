//! The shape of each `analog-operators` form (AO-02, AO-03, AO-07, AO-09): delay sites, noise
//! inputs, session parameter pins and trace watches. Each test builds the body `Op(a)` by hand
//! (`piperine-ir`'s `tests/support/operators.rs`) and reads its shape entry by entry; every
//! expected entry is written from the design (mechanisms 2 to 6) and abi.md, and every shape
//! passes `piperine_abi::validate`.

use piperine_abi::raw::{Cell, DEFAULT, GROUND, Module, Shape, cell, global, input, kind, op, part, pin_flag, role};
use piperine_ir::{BodyId, Env, Expr, Ir, NoiseSource, ValueType};
use piperine_plan::{BodyPlan, FunctionPlan, Mode, Modules, Netlist, Source, Target, Term};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/operators.rs"]
mod fixtures;

use fixtures::{Operators, Written};

/// Entries of a shape, found by name.
struct Find;

impl Find {
    /// The plan of the only body of `ir`.
    fn plan(ir: &Ir) -> BodyPlan {
        BodyPlan::build(ir.bodies().next().expect("a body")).expect("the body plans")
    }

    /// The index of the pin named `name`.
    fn pin(shape: &Shape<'_>, name: &str) -> u32 {
        shape.pins.iter().position(|p| p.name.as_str() == name).unwrap_or_else(|| panic!("no pin `{name}`")) as u32
    }

    /// The names of the pins of a shape, in order.
    fn pin_names<'s>(shape: &'s Shape<'_>) -> Vec<&'s str> {
        shape.pins.iter().map(|p| p.name.as_str()).collect()
    }

    /// The names of the terms of a shape, in order.
    fn term_names<'s>(shape: &'s Shape<'_>) -> Vec<&'s str> {
        shape.terms.iter().map(|t| t.name.as_str()).collect()
    }

    /// The names of the functions of a shape, in order.
    fn function_names<'s>(shape: &'s Shape<'_>) -> Vec<&'s str> {
        shape.functions.iter().map(|f| f.name.as_str()).collect()
    }

    /// The cell of a pin's value.
    fn value(pin: u32) -> Cell {
        Cell { kind: cell::PIN, index: pin, part: part::VALUE }
    }

    /// The plan of the `terms` function.
    fn terms_plan(plan: &BodyPlan) -> &FunctionPlan {
        plan.functions().iter().find(|f| f.mode == Mode::Terms).expect("a terms function")
    }
}


/// The bodies the tests shape.
struct Bodies;

impl Bodies {
    /// `i(a, gnd) +> 0.5 * delay(v(a), td, 2e-9)`.
    fn delayed() -> Ir {
        Operators::ir(|b| {
            let site = Operators::delay_site(b, Some(2e-9));
            Written { analog: vec![Operators::contribute_half_the_delay(b, 0)], delays: vec![site], ..Written::default() }
        })
    }

    /// `i(a, gnd) +> noise(<density>, "thermal")`, the density being `freq()` when `on_freq`, else `4e-21`.
    fn noisy(on_freq: bool) -> Ir {
        Operators::ir(|b| {
            let source = if on_freq {
                let density = b.expr(Expr::Env(Env::Freq), ValueType::F64, Operators::at());
                NoiseSource { name: "n".into(), group: Some("thermal".into()), density, origin: Operators::at() }
            } else {
                Operators::noise_site(b, Some("thermal"))
            };
            Written { analog: vec![Operators::contribute_the_noise(b, 0)], noise_sources: vec![source], ..Written::default() }
        })
    }

    /// `i(a, gnd) +> simparam("gmin").unwrap_or_default()`.
    fn parameterised() -> Ir {
        Operators::ir(|b| Written { analog: vec![Operators::contribute_the_session_parameter(b, 0)], session_parameters: vec![Operators::gmin()], ..Written::default() })
    }
}

#[test]
fn a_delay_site_has_its_pin_terms_and_maximum() {
    let ir = Bodies::delayed();
    let plan = Find::plan(&ir);
    let shape = plan.shape().shape();
    // pins: the param `td`, the node `a`, the output `delay0.value` after the derivative pins,
    // and the `max` pin `delay0.max` after the derivative `abstol` pins
    assert_eq!(Find::pin_names(shape), ["td", "a", "delay0.value", "delay0.max"]);
    let (output, max) = (Find::pin(shape, "delay0.value"), Find::pin(shape, "delay0.max"));
    let (pin, bound) = (shape.pins.get(output as usize).expect("output"), shape.pins.get(max as usize).expect("max"));
    assert_eq!((pin.kind, pin.role), (kind::CONTINUOUS, role::PRIVATE));
    assert_eq!((bound.kind, bound.role), (kind::DISCRETE, role::PRIVATE));
    // terms: the branch's flow, then FLOW {u, gnd} and DELAY {u, gnd} naming the `max` pin
    assert_eq!(Find::term_names(shape), ["b0.flow", "delay0.value", "delay0.history"]);
    let terms: Vec<_> = shape.terms.iter().map(|t| (t.op, t.plus, t.minus, t.max)).collect();
    let a = Find::pin(shape, "a");
    assert_eq!(terms, [(op::FLOW, a, GROUND, DEFAULT), (op::FLOW, output, GROUND, DEFAULT), (op::DELAY, output, GROUND, max)]);
    // `terms` reads the node the input reads, the output pin (a tangent slot) and the param `td`;
    // it writes the three terms and then the `TIME` of the history term
    let function = Find::terms_plan(&plan);
    assert_eq!(function.reads, [Source::Node(0), Source::Delay(0), Source::Param(0)]);
    assert_eq!(function.slots, 2);
    assert_eq!(function.writes, [Target::Term(Term::Flow(0)), Target::Term(Term::DelayOutput(0)), Target::Term(Term::DelayHistory(0)), Target::DelayTime(0)]);
    // partials as (write, slot): the flow reads the output, the output term is the output, the
    // history term `-x` reads the node `x` reads
    assert_eq!(function.partials, [(0, 1), (1, 1), (2, 0)]);
    let f = shape.functions.iter().find(|f| f.name.as_str() == "terms").expect("terms");
    let writes: Vec<Cell> = f.writes.iter().copied().collect();
    assert_eq!(writes[3], Cell { kind: cell::TERM, index: 2, part: part::TIME });
    // `delay0.max`: a literal bound reads nothing and writes the pin
    assert_eq!(Find::function_names(shape), ["terms", "delay0.max"]);
    let bound_function = plan.functions().iter().find(|f| f.mode == Mode::DelayMaximum(0)).expect("the max function");
    assert!(bound_function.reads.is_empty());
    assert_eq!(bound_function.writes, [Target::DelayMaximum(0)]);
    let g = shape.functions.iter().find(|f| f.name.as_str() == "delay0.max").expect("max");
    assert_eq!(g.writes.iter().copied().collect::<Vec<_>>(), [Find::value(max)]);
    assert_eq!(piperine_abi::validate::Validator::validate(shape), Ok(()));
}

#[test]
fn a_delay_without_a_maximum_bounds_the_history_by_td() {
    // D4: the `max` function of a site with no `max` reads `td`'s cone and writes the pin.
    let ir = Operators::ir(|b| {
        let site = Operators::delay_site(b, None);
        Written { analog: vec![Operators::contribute_half_the_delay(b, 0)], delays: vec![site], ..Written::default() }
    });
    let plan = Find::plan(&ir);
    let bound_function = plan.functions().iter().find(|f| f.mode == Mode::DelayMaximum(0)).expect("the max function");
    assert_eq!(bound_function.reads, [Source::Param(0)]);
    assert_eq!(piperine_abi::validate::Validator::validate(plan.shape().shape()), Ok(()));
}

#[test]
fn a_body_without_a_new_form_keeps_its_shape() {
    // The pins, terms and functions of a body that uses none of the new forms are what they were.
    let ir = Operators::ir(|b| {
        let value = Operators::ex(b).real(1.0);
        Written { analog: vec![Operators::contribute(b, value)], ..Written::default() }
    });
    let plan = Find::plan(&ir);
    let shape = plan.shape().shape();
    assert_eq!(Find::pin_names(shape), ["td", "a"]);
    assert_eq!(Find::term_names(shape), ["b0.flow"]);
    assert_eq!(Find::function_names(shape), ["terms"]);
    assert_eq!(piperine_abi::validate::Validator::validate(shape), Ok(()));
}


#[test]
fn a_noise_source_is_an_input_with_its_density_function() {
    // abi.md A2: one `NOISE` input `noise:<k>` after the `STIM` inputs; the density is a function
    // that writes its `PSD`; `terms` reads the input's value as a tangent slot.
    let ir = Bodies::noisy(false);
    let plan = Find::plan(&ir);
    let shape = plan.shape().shape();
    let inputs: Vec<(&str, u32)> = shape.inputs.iter().map(|i| (i.name.as_str(), i.kind)).collect();
    // the input is named by the source's group (`thermal`)
    assert_eq!(inputs, [("thermal", input::NOISE)]);
    let function = Find::terms_plan(&plan);
    assert_eq!(function.reads, [Source::NoiseValue(0)]);
    assert!(Source::NoiseValue(0).is_differentiable() && !Source::NoiseValue(0).is_pin());
    assert_eq!(function.slots, 1);
    assert_eq!(function.partials, [(0, 0)]);
    let noise = plan.functions().iter().find(|f| f.mode == Mode::Noise(0)).expect("the density function");
    // the density function always reads the sweep's frequency, so that a fault names it
    assert_eq!(noise.reads, [Source::Freq]);
    assert_eq!(noise.writes, [Target::NoiseDensity(0)]);
    let f = shape.functions.iter().find(|f| f.name.as_str() == "noise:0").expect("noise:0");
    assert_eq!(f.writes.iter().copied().collect::<Vec<_>>(), [Cell { kind: cell::INPUT, index: 0, part: part::PSD }]);
    assert_eq!(piperine_abi::validate::Validator::validate(shape), Ok(()));
    // a density that reads `freq()` reads the ambient `FREQ`, in the density function only
    let plan = Find::plan(&Bodies::noisy(true));
    let noise = plan.functions().iter().find(|f| f.mode == Mode::Noise(0)).expect("the density function");
    assert_eq!(noise.reads, [Source::Freq]);
    assert_eq!(Find::terms_plan(&plan).reads, [Source::NoiseValue(0)]);
    assert_eq!(piperine_abi::validate::Validator::validate(plan.shape().shape()), Ok(()));
}


#[test]
fn a_session_parameter_is_a_named_global_optional_pin() {
    // D12: a `GLOBAL NAMED` pin named the parameter, after every other pin, always `OPTIONAL`; the read is a discrete `Source::SessionParameter`.
    let ir = Bodies::parameterised();
    let plan = Find::plan(&ir);
    let shape = plan.shape().shape();
    assert_eq!(Find::pin_names(shape), ["td", "a", "gmin"]);
    let pin = shape.pins.get(2).expect("the session pin");
    assert_eq!((pin.kind, pin.role, pin.global), (kind::DISCRETE, role::GLOBAL, global::NAMED));
    assert_eq!(pin.flags & pin_flag::OPTIONAL, pin_flag::OPTIONAL);
    let function = Find::terms_plan(&plan);
    assert_eq!(function.reads, [Source::SessionParameter(0)]);
    assert_eq!(function.slots, 0);
    let reads: Vec<Cell> = shape.functions.iter().find(|f| f.name.as_str() == "terms").expect("terms").reads.iter().copied().collect();
    assert_eq!(reads, [Find::value(2)]);
    assert_eq!(piperine_abi::validate::Validator::validate(shape), Ok(()));
}

/// A supplier with no module: every body has nothing to run.
struct Nothing;

impl Modules for Nothing {
    fn module(&self, _: BodyId) -> Option<&'static Module> {
        None
    }
}

#[test]
fn the_netlist_hands_its_session_parameters_to_the_builder() {
    // The netlist keeps the parameters in the order given and the circuit it builds accepts them
    // (their effect on a bound pin is proved with the host, `piperine-api`'s `session_parameters`).
    let ir = Bodies::parameterised();
    let given = [("gmin".to_string(), 1e-9), ("tnom".to_string(), 27.0)];
    let netlist = Netlist::new(&ir).with_session_parameters(&given);
    assert_eq!(netlist.session_parameters(), given);
    assert!(netlist.circuit(&Nothing).is_ok());
    assert!(Netlist::new(&ir).session_parameters().is_empty());
}

#[test]
fn a_trace_site_is_a_trace_watch_written_by_terms() {
    // abi.md A5: one `TRACE` watch `trace:<k>` per site, its layout a `BOOL` presence byte then the
    // stream's record layout (`x: f64`), written by `terms` where the statement executes.
    let ir = Operators::tracing();
    let plan = Find::plan(&ir);
    let shape = plan.shape().shape();
    assert_eq!(shape.watches.len(), 1);
    let watch = shape.watches.get(0).expect("the watch");
    assert_eq!((watch.name.as_str(), watch.kind), ("trace:0", piperine_abi::raw::watch::TRACE));
    let fields: Vec<(&str, u32)> = watch.layout.fields.iter().map(|f| (f.name.as_str(), f.offset)).collect();
    assert_eq!(fields, [("present", 0), ("x", 8)]);
    assert_eq!((watch.layout.size, watch.layout.align), (16, 8));
    // `terms` has no term, reads the node the field reads, and writes the watch
    let function = Find::terms_plan(&plan);
    assert_eq!(function.reads, [Source::Node(0)]);
    assert_eq!(function.writes, [Target::Trace(0)]);
    let f = shape.functions.iter().find(|f| f.name.as_str() == "terms").expect("terms");
    assert_eq!(f.writes.iter().copied().collect::<Vec<_>>(), [Cell { kind: cell::WATCH, index: 0, part: part::VALUE }]);
    assert_eq!(piperine_abi::validate::Validator::validate(shape), Ok(()));
}
