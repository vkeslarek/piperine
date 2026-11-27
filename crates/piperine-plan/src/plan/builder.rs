//! `BodyPlanBuilder`: plans one body, one method per table of its shape. The pins come first
//! (params, nodes, branch flows, rates, delays, the globals the body reads, the check pins, then
//! the digital pins), then the terms, the inputs and the functions, in the order the shape lists them.

use piperine_abi::layout::{BIT, F64};
use piperine_abi::raw::{DEFAULT, GROUND, global, input, op, pin_flag, role};
use piperine_abi::shape::{InputSpec, PinSpec, ShapeBuf, TermSpec};
use piperine_ir::{AnalysisKind, Body, BranchKind, Constant, Target as Query, ValueType};

use super::{BodyPlan, FunctionPlan, Mode, Pins, ReadSet, Received, Source, Target, Term};
use crate::digital::{CarrierLayout, Digital};
use crate::error::PlanError;
use crate::pin::PinLayout;

/// The global pins a body reads: its temperature, whether the run set it, and the analysis kinds.
struct GlobalPins {
    temperature: Option<u32>,
    temperature_given: Option<u32>,
    analysis: Vec<(AnalysisKind, u32)>,
}

/// The plan of one body under construction: the shape being filled, and what each branch receives.
pub(super) struct BodyPlanBuilder<'a> {
    body: Body<'a>,
    shape: ShapeBuf,
    received: Vec<Received>,
}

impl<'a> BodyPlanBuilder<'a> {
    /// A builder of `body`'s plan, with an empty shape.
    pub(super) fn new(body: Body<'a>) -> Self {
        BodyPlanBuilder { body, shape: ShapeBuf::new(), received: BodyPlan::received(body) }
    }

    /// Plans the body, or names what it holds that has no layout.
    pub(super) fn build(mut self) -> Result<BodyPlan, PlanError> {
        let pins = self.pins()?;
        let terms = self.terms(&pins)?;
        self.inputs();
        let functions = self.functions(&pins, &terms);
        let reactions = Digital::reactions(&mut self.shape, &pins, self.body);
        Ok(BodyPlan { shape: self.shape.freeze(), functions, reactions })
    }

    /// The `file:line:col` text of the origin of branch `b`: where it was first written, else the body.
    fn branch_origin(&self, b: usize) -> String {
        BodyPlan::origin_text(self.received[b].origin.as_ref().unwrap_or(self.body.origin()))
    }

    // ---- pins ---------------------------------------------------------------------------------

    /// Every pin of the shape, in the order it lists them.
    fn pins(&mut self) -> Result<Pins, PlanError> {
        let body = self.body;
        let check_pins = self.param_pins();
        let nodes = body.params().len() as u32;
        self.node_pins();
        let branches = self.branch_pins();
        let derivatives = self.derivative_pins();
        let delays = BodyPlan::delay_pins(&mut self.shape, body);
        let globals = self.global_pins();
        let checks = self.check_pins(&check_pins);
        let s = &mut self.shape;
        let carriers = Digital::carrier_pins(s, body)?;
        let events = Digital::event_pins(s, body);
        let crossed = Digital::crossed_pins(s, body);
        let delayed = Digital::delayed_pins(s, body);
        let timers = Digital::timer_pins(s, body);
        let start = Digital::start_pin(s, body);
        let rearms = Digital::rearm_pins(s, body);
        let records = Digital::record_pins(s, body)?;
        let tolerances = Digital::tolerance_pins(s, body);
        let derivative_abstols = Digital::derivative_abstol_pins(s, body);
        let delay_maxima = Digital::delay_maximum_pins(s, body);
        let settle = Digital::settle_pin(s, body);
        let element_drives = Digital::element_drive_pins(s, body);
        let resolved = Digital::resolved_pins(s, body)?;
        let session_parameters = BodyPlan::session_parameter_pins(s, body);
        let watches = Digital::watches(s, body, &events, &crossed, &tolerances);
        let caps = Digital::cap_watches(s, body);
        let traces = Digital::trace_watches(s, body)?;
        Ok(Pins {
            nodes,
            branches,
            derivatives,
            delays,
            temperature: globals.temperature,
            temperature_given: globals.temperature_given,
            analysis: globals.analysis,
            checks,
            carriers,
            events,
            crossed,
            watches,
            delayed,
            timers,
            start,
            rearms,
            records,
            tolerances,
            caps,
            derivative_abstols,
            delay_maxima,
            settle,
            element_drives,
            resolved,
            traces,
            session_parameters,
        })
    }

    /// The default of each param as the bytes its pin holds as `init`.
    fn param_defaults(&self) -> Vec<Vec<u8>> {
        let ir = self.body.ir();
        self.body
            .params()
            .iter()
            .map(|p| match p.default {
                Some(Constant::Real(r)) => BodyPlan::real_bytes(r.get()),
                Some(Constant::Bool(b)) => vec![u8::from(b)],
                // An integer param has a carrier's layout and bytes (`digital.rs`).
                Some(c @ (Constant::Int(_) | Constant::FloatBits(_))) => CarrierLayout::bytes(&p.ty, c).unwrap_or_default(),
                // A constant of an integer wider than 64 bits is a `U64` array, laid out as a composite's single leaf.
                Some(c @ Constant::Wide(_)) => PinLayout::bytes_in(ir, &p.ty, c).unwrap_or_default(),
                // A composite param has the layout and bytes of a composite carrier (`composite.rs`).
                Some(c @ Constant::Aggregate(_)) => PinLayout::bytes_in(ir, &p.ty, c).unwrap_or_default(),
                None => Vec::new(),
            })
            .collect()
    }

    /// Params, in declaration order (P5), each with its default as `init`. A param with a `where`
    /// names its check pin, the last pins of the shape (P6): the pins before them keep their indices.
    /// Returns the check pin of each param.
    fn param_pins(&mut self) -> Vec<Option<u32>> {
        let body = self.body;
        let ir = body.ir();
        let defaults = self.param_defaults();
        let pin_count = body.params().len()
            + body.nodes().len()
            + body.branches().iter().filter(|b| b.kind != BranchKind::Flow).count()
            + body.derivatives().len()
            + body.delays().len()
            + usize::from(BodyPlan::reads_temperature(body))
            + usize::from(BodyPlan::reads_temperature_given(body))
            + BodyPlan::reads_analysis(body).len();
        let mut next_check = pin_count as u32;
        let mut check_pins = Vec::with_capacity(body.params().len());
        for (p, init) in body.params().iter().zip(&defaults) {
            let composite = match PinLayout::in_ir(ir, &p.ty) {
                Some(layout @ PinLayout::Composite(_)) => Some(layout),
                _ => None,
            };
            let composite_fields = composite.as_ref().map(PinLayout::fields).unwrap_or_default();
            let composite_layout = composite.as_ref().map(|c| c.layout(&composite_fields));
            let scalar = match &p.ty {
                ValueType::Bool => &BIT,
                ty if ty.is_integer() || ty.is_other_float() => CarrierLayout::of(ty).unwrap_or(&F64),
                // a unit enum is its tag (PI-02)
                ty @ ValueType::Enum(_) => CarrierLayout::in_ir(ir, ty).unwrap_or(&F64),
                _ => &F64,
            };
            let layout = composite_layout.as_ref().unwrap_or(scalar);
            let mut flags = if init.is_empty() { 0 } else { pin_flag::HAS_INIT };
            if p.structural {
                flags |= pin_flag::STRUCTURAL;
            }
            let check = p.check.map(|_| {
                flags |= pin_flag::HAS_CHECK;
                next_check += 1;
                next_check - 1
            });
            check_pins.push(check);
            let at = BodyPlan::origin_text(&p.origin);
            self.shape.pin(PinSpec { origin: &at, flags, init, check: check.unwrap_or(0), ..PinSpec::discrete(&p.name, role::PARAM, layout) });
        }
        check_pins
    }

    /// Body nodes, ports and signals alike, with their nature's tolerances.
    fn node_pins(&mut self) {
        let body = self.body;
        let ir = body.ir();
        let inits: Vec<Vec<u8>> = body.nodes().iter().map(|n| n.init.map_or_else(Vec::new, |r| BodyPlan::real_bytes(r.get()))).collect();
        for (n, init) in body.nodes().iter().zip(&inits) {
            let nature = ir.nature(n.nature);
            let abstol = [nature.potential().abstol.get(), nature.flow().map_or(0.0, |q| q.abstol.get())];
            let flags = if init.is_empty() { 0 } else { pin_flag::HAS_INIT };
            let at = BodyPlan::origin_text(&n.origin);
            self.shape.pin(PinSpec { origin: &at, flags, init, abstol, ..PinSpec::continuous(&n.name, role::PORT) });
        }
    }

    /// One private flow pin per potential branch.
    fn branch_pins(&mut self) -> Vec<Option<u32>> {
        let body = self.body;
        let mut branches = Vec::with_capacity(body.branches().len());
        for (b, branch) in body.branches().iter().enumerate() {
            branches.push(match branch.kind {
                BranchKind::Flow => None,
                // A switch branch has a potential branch's private flow unknown (§11 rule 2).
                BranchKind::Potential | BranchKind::Switch => {
                    let at = self.branch_origin(b);
                    let abstol = BodyPlan::branch_abstol(body, branch);
                    Some(self.shape.pin(PinSpec { origin: &at, abstol, ..PinSpec::continuous(&format!("b{b}.flow"), role::PRIVATE) }))
                }
            });
        }
        branches
    }

    /// One private rate pin per derivative site, after the branch flow pins (abi.md A1).
    fn derivative_pins(&mut self) -> Vec<u32> {
        let shape = &mut self.shape;
        self.body
            .derivatives()
            .iter()
            .enumerate()
            .map(|(k, site)| {
                let at = BodyPlan::origin_text(&site.origin);
                shape.pin(PinSpec { origin: &at, ..PinSpec::continuous(&format!("d{k}.rate"), role::PRIVATE) })
            })
            .collect()
    }

    /// The `GLOBAL` pins the body reads: `temperature()`, whether the run set it, and one `BIT`
    /// per analysis kind it reads (`analysis(kind)`).
    fn global_pins(&mut self) -> GlobalPins {
        let body = self.body;
        let at = BodyPlan::origin_text(body.origin());
        let shape = &mut self.shape;
        let temperature = BodyPlan::reads_temperature(body).then(|| {
            shape.pin(PinSpec { origin: &at, global: global::TEMPERATURE, ..PinSpec::discrete("temperature", role::GLOBAL, &F64) })
        });
        // The `GLOBAL` `BIT` pin of whether the run set its temperature (`temperature()`'s given half).
        let temperature_given = BodyPlan::reads_temperature_given(body).then(|| {
            shape.pin(PinSpec { origin: &at, global: global::TEMPERATURE_GIVEN, ..PinSpec::discrete("temperature_given", role::GLOBAL, &BIT) })
        });
        let analysis: Vec<(AnalysisKind, u32)> = BodyPlan::reads_analysis(body)
            .into_iter()
            .map(|kind| {
                let (global, name) = BodyPlan::analysis_pin(kind);
                (kind, shape.pin(PinSpec { origin: &at, global, ..PinSpec::discrete(name, role::GLOBAL, &BIT) }))
            })
            .collect();
        GlobalPins { temperature, temperature_given, analysis }
    }

    /// The check pins: a private `BIT` per `where`, written by `check:<p>`.
    fn check_pins(&mut self, expected: &[Option<u32>]) -> Vec<Option<u32>> {
        let mut checks = Vec::with_capacity(expected.len());
        for (p, check) in self.body.params().iter().zip(expected) {
            checks.push(check.map(|expected| {
                let at = BodyPlan::origin_text(&p.origin);
                let pin = self.shape.pin(PinSpec { origin: &at, ..PinSpec::discrete(&format!("check:{}", p.name), role::PRIVATE, &BIT) });
                debug_assert_eq!(pin, expected);
                pin
            }));
        }
        checks
    }

    // ---- terms and inputs ---------------------------------------------------------------------

    /// Every term, in the order the shape lists them: the branch terms, the derivative sites'
    /// rate and state terms, then the delay sites'.
    fn terms(&mut self, pins: &Pins) -> Result<Vec<Term>, PlanError> {
        let mut terms = Vec::new();
        self.branch_terms(pins, &mut terms)?;
        self.derivative_terms(pins, &mut terms);
        BodyPlan::delay_terms(&mut self.shape, self.body, pins, &mut terms);
        Ok(terms)
    }

    /// The terms of each branch: a flow branch has a `FLOW` term for its direct addends and a `DDT`
    /// term for its `ddt` addends; a potential branch has the two `FLOW` terms of abi.md A1.
    fn branch_terms(&mut self, pins: &Pins, terms: &mut Vec<Term>) -> Result<(), PlanError> {
        let body = self.body;
        for (b, branch) in body.branches().iter().enumerate() {
            let at = self.branch_origin(b);
            let (plus, minus) = (pins.terminal(body, branch.plus)?, pins.terminal(body, branch.minus)?);
            let shape = &mut self.shape;
            let mut term = |name: String, operator: u32, plus: u32, minus: u32, what: Term| {
                shape.term(TermSpec { name: &name, origin: &at, op: operator, plus, minus, abstol: DEFAULT, max: DEFAULT });
                terms.push(what);
            };
            let received = &self.received[b];
            let b32 = b as u32;
            match (branch.kind, pins.branches[b]) {
                (BranchKind::Potential | BranchKind::Switch, Some(f)) => {
                    term(format!("b{b}.flow"), op::FLOW, plus, minus, Term::Current(b32));
                    term(format!("b{b}.eq"), op::FLOW, f, GROUND, Term::Eq(b32));
                    if received.ddt {
                        term(format!("b{b}.charge"), op::DDT, f, GROUND, Term::Charge(b32));
                    }
                }
                _ => {
                    if received.direct {
                        term(format!("b{b}.flow"), op::FLOW, plus, minus, Term::Flow(b32));
                    }
                    if received.ddt {
                        term(format!("b{b}.charge"), op::DDT, plus, minus, Term::Charge(b32));
                    }
                }
            }
        }
        Ok(())
    }

    /// Derivative sites: the rate term `u` and the state term `-x`, after the branch terms.
    fn derivative_terms(&mut self, pins: &Pins, terms: &mut Vec<Term>) {
        for (k, site) in self.body.derivatives().iter().enumerate() {
            let (k, at) = (k as u32, BodyPlan::origin_text(&site.origin));
            let rate = pins.derivative(k);
            let shape = &mut self.shape;
            let mut term = |name: &str, operator: u32, abstol: u32, what: Term| {
                shape.term(TermSpec { name, origin: &at, op: operator, plus: rate, minus: GROUND, abstol, max: DEFAULT });
                terms.push(what);
            };
            term(&format!("d{k}.rate"), op::FLOW, DEFAULT, Term::DerivativeRate(k));
            term(&format!("d{k}.state"), op::DDT, pins.derivative_abstol(k), Term::DerivativeState(k));
        }
    }

    /// Inputs: limits, then stimuli, each named after its site, then the noise sources.
    fn inputs(&mut self) {
        let body = self.body;
        for site in body.limits() {
            let at = BodyPlan::origin_text(&site.origin);
            self.shape.input(InputSpec { name: &site.name, origin: &at, kind: input::LIMIT });
        }
        for site in body.stims() {
            let at = BodyPlan::origin_text(&site.origin);
            self.shape.input(InputSpec { name: &site.name, origin: &at, kind: input::STIM });
        }
        BodyPlan::noise_inputs(&mut self.shape, body);
    }

    // ---- functions ----------------------------------------------------------------------------

    /// Functions: one per site, then the terms, then one per check, bound step and derivative
    /// tolerance, then the digital ones and the delay bounds.
    fn functions(&mut self, pins: &Pins, terms: &[Term]) -> Vec<FunctionPlan> {
        let body = self.body;
        let mut functions = Vec::new();
        self.limit_functions(pins, terms, &mut functions);
        self.stimulus_functions(pins, terms, &mut functions);
        BodyPlan::noise_functions(&mut self.shape, pins, terms, body, &mut functions);
        BodyPlan::terms_function(&mut self.shape, pins, terms, body, &mut functions);
        self.check_functions(pins, terms, &mut functions);
        self.cap_functions(pins, terms, &mut functions);
        functions.extend(Digital::functions(&mut self.shape, pins, body));
        self.derivative_abstol_functions(pins, terms, &mut functions);
        BodyPlan::delay_functions(&mut self.shape, pins, terms, body, &mut functions);
        functions
    }

    /// `limit:<site>` per `limit` site.
    fn limit_functions(&mut self, pins: &Pins, terms: &[Term], functions: &mut Vec<FunctionPlan>) {
        let body = self.body;
        for (k, site) in body.limits().iter().enumerate() {
            let k = k as u32;
            let set = ReadSet::target(body, Query::Limit(k));
            let mut plan = BodyPlan::function(Mode::Limit(k), &set, vec![(Target::Proposed(k), Some(&set)), (Target::Limited(k), None)]);
            plan.reads.extend([Source::LimitLast(k), Source::LimitAge(k)]);
            // a site that reads `static()` itself already has the ambient among its reads
            if !plan.reads.contains(&Source::Static) {
                plan.reads.push(Source::Static);
            }
            let name = format!("limit:{}", site.name);
            BodyPlan::declare(&mut self.shape, pins, terms, body, &name, &BodyPlan::origin_text(&site.origin), &plan);
            functions.push(plan);
        }
    }

    /// `stim:<k>` per `ac_stim` site.
    fn stimulus_functions(&mut self, pins: &Pins, terms: &[Term], functions: &mut Vec<FunctionPlan>) {
        let body = self.body;
        for (k, site) in body.stims().iter().enumerate() {
            let k = k as u32;
            let set = ReadSet::target(body, Query::Stim(k));
            let plan = BodyPlan::function(Mode::Stim(k), &set, vec![(Target::StimRe(k), None), (Target::StimIm(k), None)]);
            BodyPlan::declare(&mut self.shape, pins, terms, body, &format!("stim:{k}"), &BodyPlan::origin_text(&site.origin), &plan);
            functions.push(plan);
        }
    }

    /// `check:<p>` per param with a `where`: it reads what the condition reads and writes the check pin.
    fn check_functions(&mut self, pins: &Pins, terms: &[Term], functions: &mut Vec<FunctionPlan>) {
        let body = self.body;
        for (p, param) in body.params().iter().enumerate() {
            if param.check.is_none() {
                continue;
            }
            let p = p as u32;
            let set = ReadSet::target(body, Query::Check(p));
            let plan = BodyPlan::function(Mode::Check(p), &set, vec![(Target::Check(p), None)]);
            BodyPlan::declare(&mut self.shape, pins, terms, body, &format!("check:{}", param.name), &BodyPlan::origin_text(&param.origin), &plan);
            functions.push(plan);
        }
    }

    /// `cap:<site>` per `bound_step` site.
    fn cap_functions(&mut self, pins: &Pins, terms: &[Term], functions: &mut Vec<FunctionPlan>) {
        let body = self.body;
        for (site, _) in &pins.caps {
            let set = ReadSet::target(body, Query::BoundStep(*site));
            let plan = BodyPlan::function(Mode::Cap(*site), &set, vec![(Target::Cap(*site), None)]);
            let at = BodyPlan::origin_text(body.origin());
            BodyPlan::declare(&mut self.shape, pins, terms, body, &format!("cap:{site}"), &at, &plan);
            functions.push(plan);
        }
    }

    /// `d<k>.abstol` per derivative site that gives an absolute tolerance.
    fn derivative_abstol_functions(&mut self, pins: &Pins, terms: &[Term], functions: &mut Vec<FunctionPlan>) {
        let body = self.body;
        for (k, site) in body.derivatives().iter().enumerate().filter(|(_, site)| site.abstol.is_some()) {
            let k = k as u32;
            let set = ReadSet::target(body, Query::DerivativeAbstol(k));
            let plan = BodyPlan::function(Mode::DerivativeAbstol(k), &set, vec![(Target::DerivativeAbstol(k), None)]);
            BodyPlan::declare(&mut self.shape, pins, terms, body, &format!("d{k}.abstol"), &BodyPlan::origin_text(&site.origin), &plan);
            functions.push(plan);
        }
    }
}
