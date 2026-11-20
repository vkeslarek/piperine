//! `DigitalPrinter`: the discrete functions of a body's shape, `drive:<carrier>` and `event:<k>`
//! (design "Phase B: generated digital code"; spec RS-12 AC 2, 3, 5; abi.md §11.4 `eval`).
//!
//! Each is one Rust `fn` over `Eval`, named from the MSL name first: `<carrier>_drive`,
//! `event<k>`, `<carrier>_drive_changed`. It binds the cells it reads (decoded: a carrier's pin
//! is an `rt::Flagged<T>` from the pin's bytes, a `CHANGED` byte a `bool`), only those its code
//! uses, and writes its cells: a drive writes the pin's bytes (`Flagged::write`; an unknown value
//! is all zero bytes, RS-11 AC 4); an event writes its `CHANGED` byte, nonzero when it fires
//! (abi.md §4.1). An event fires when its source fired and its guard holds; the guard is
//! evaluated only then, and a guard that may be unknown stops the event only when it is known
//! false.

use piperine_ir::{Body, EventSource, Ir, ValueType};
use piperine_plan::{BodyPlan, FunctionPlan, Mode, Source};

use crate::Error;
use crate::print::value::Reads;
use crate::print::{AnalogPrinter, Text, ValuePrinter};

/// Prints the discrete functions of one body from its plan.
#[derive(Debug)]
pub struct DigitalPrinter<'a> {
    pub(crate) ir: &'a Ir,
    pub(crate) body: Body<'a>,
    pub(crate) plan: &'a BodyPlan,
    /// The names of the reads and writes, shared with the analog printer.
    pub(crate) names: AnalogPrinter<'a>,
}

impl<'a> DigitalPrinter<'a> {
    pub fn new(body: Body<'a>, plan: &'a BodyPlan) -> DigitalPrinter<'a> {
        DigitalPrinter { ir: body.ir(), body, plan, names: AnalogPrinter::new(body, plan) }
    }

    /// The reads of `ValuePrinter`s of this body: nodes and carriers by their variables.
    pub(crate) fn reads(&self) -> Reads {
        Reads {
            nodes: self.body.nodes().iter().map(|n| Text::ident(&n.name)).collect(),
            limits: Vec::new(),
            stims: Vec::new(),
            carriers: self.body.carriers().iter().map(|c| Text::ident(&c.name)).collect(),
            flows: Vec::new(),
            derivatives: (0..self.body.derivatives().len() as u32).map(AnalogPrinter::derivative_pin).collect(),
            param_previous: (0..self.body.params().len() as u32).map(|p| self.names.read_name(Source::ParamPrevious(p))).collect(),
            previous: (0..self.body.carriers().len() as u32).map(|c| AnalogPrinter::carrier_part(self.body, c, "previous")).collect(),
        }
    }

    /// The name of the Rust function that runs function `f` of the plan.
    pub fn function_name(&self, f: &FunctionPlan) -> String {
        let carrier = |k: u32| {
            let c = self.body.drives().get(k as usize).map_or(u32::MAX, |d| d.carrier);
            self.body.carriers().get(c as usize).map_or_else(|| format!("drive{k}"), |x| Text::ident(&x.name))
        };
        match f.mode {
            Mode::Drive(k) => format!("{}_drive", carrier(k)),
            Mode::DelayedEvent(k) => format!("{}_drive_changed", carrier(k)),
            Mode::Event(k) => format!("event{k}"),
            Mode::Cross(k) => format!("cross{k}"),
            Mode::Ttol(k) => format!("ttol{k}"),
            Mode::Etol(k) => format!("etol{k}"),
            Mode::Rearm { event, carrier } => {
                let name = self.body.carriers().get(carrier as usize).map_or_else(|| format!("carrier{carrier}"), |x| Text::ident(&x.name));
                format!("at{event}_{name}_changed")
            }
            Mode::Cap(site) => format!("cap{site}"),
            Mode::DerivativeAbstol(k) => format!("derivative{k}_abstol"),
            Mode::Limit(_) | Mode::Stim(_) | Mode::Terms | Mode::Check(_) => "terms".to_string(),
        }
    }

    /// The comment over a function: its index and name in the shape, its reads and writes.
    pub(crate) fn header(&self, t: &mut Text, k: usize, f: &FunctionPlan) {
        let name = self.plan.shape().shape().functions.get(k).map_or("", |x| x.name.as_str());
        let reads: Vec<String> = f.reads.iter().map(|s| self.names.read_name(*s)).collect();
        let writes: Vec<String> = f.writes.iter().map(|w| self.names.write_name(*w)).collect();
        t.line(&format!(
            "// function {k} `{name}`: reads {}; writes {} @ {}",
            reads.join(", "),
            writes.join(", "),
            self.names.function_origin(k)
        ));
    }

    /// The `let` that binds read `i` of a function (or reaction) from its lens `cx`: a carrier's
    /// pin decoded into a flagged value, a `CHANGED` byte as a `bool`, a real as an `f64`.
    /// `None` for a read no discrete code has.
    pub(crate) fn bind(&self, values: &ValuePrinter<'_>, cx: &str, i: usize, s: Source) -> Result<Option<String>, Error> {
        let name = self.names.read_name(s);
        let carrier_of_drive = |k: u32| self.body.drives().get(k as usize).map_or(u32::MAX, |d| d.carrier);
        let pin = |ty: &ValueType| -> Result<String, Error> {
            Ok(format!("let {name} = rt::Flagged::<{}>::decode({cx}.bytes({i}));", values.pin_type(ty)?))
        };
        Ok(Some(match s {
            // a previous value is in the carrier's own layout (abi.md §4.1)
            Source::Carrier(c) | Source::CarrierPrevious(c) => match self.body.carriers().get(c as usize) {
                Some(carrier) => pin(&carrier.ty)?,
                None => return Ok(None),
            },
            Source::Delayed(k) => match self.body.carriers().get(carrier_of_drive(k) as usize) {
                Some(carrier) => pin(&carrier.ty)?,
                None => return Ok(None),
            },
            Source::CarrierChanged(_)
            | Source::DelayedChanged(_)
            | Source::Crossed(_)
            | Source::Timer(_)
            | Source::Fired(_)
            | Source::Settle
            | Source::ParamChanged(_) => format!("let {name} = {cx}.bool({i});"),
            Source::Node(_) | Source::Derivative(_) | Source::Temperature | Source::Gmin | Source::SourceScale | Source::Time | Source::Freq | Source::Resolution => {
                format!("let {name} = {cx}.f64({i});")
            }
            // abi.md §4.2: `STATIC` is one `BOOL` byte
            Source::Static | Source::Analysis(_) => format!("let {name} = {cx}.bool({i});"),
            // a param's previous bytes are in the param's own layout (abi.md §4.1)
            Source::Param(p) | Source::ParamPrevious(p) => match self.body.params().get(p as usize).map(|x| &x.ty) {
                Some(ValueType::Bool) => format!("let {name} = {cx}.bool({i});"),
                Some(ty @ (ValueType::UInt(_) | ValueType::Int(_))) => {
                    let (rust, zero) = (values.type_of(ty)?, ValuePrinter::zero(ty));
                    format!("let {name} = rt::Flagged::<{rust}>::decode({cx}.bytes({i})).known_or({zero});")
                }
                _ => format!("let {name} = {cx}.f64({i});"),
            },
            Source::BranchFlow(_) | Source::LimitValue(_) | Source::StimValue(_) | Source::LimitLast(_) | Source::LimitAge(_) => {
                return Ok(None);
            }
        }))
    }

    /// The `let` lines binding the reads of `f` its code uses (`values` printed them) or in
    /// `forced`, in the plan's read order.
    pub(crate) fn binds(&self, values: &ValuePrinter<'_>, cx: &str, reads: &[Source], forced: &[Source]) -> Result<Vec<String>, Error> {
        let mut lines = Vec::new();
        for (i, s) in reads.iter().enumerate() {
            if !values.uses(&self.names.read_name(*s)) && !forced.contains(s) {
                continue;
            }
            if let Some(line) = self.bind(values, cx, i, *s)? {
                lines.push(line);
            }
        }
        Ok(lines)
    }

    /// The name of carrier `c` of the body.
    fn carrier_name(&self, c: u32) -> String {
        self.body.carriers().get(c as usize).map_or_else(|| format!("carrier{c}"), |x| x.name.clone())
    }

    fn unsupported(&self, form: &str) -> Error {
        Error::Unsupported { form: form.to_string(), path: format!("body {}", self.body.name()), origin: self.body.origin().clone() }
    }

    /// The function `k` of the plan, which runs `Mode::Drive`: the drive's value to the pin of
    /// its carrier (or, with an `after`, to the pin holding the undelayed value).
    pub fn drive(&self, k: usize) -> Result<String, Error> {
        let Some(f) = self.plan.functions().get(k) else {
            return Err(self.unsupported(&format!("function {k} as a drive")));
        };
        let Mode::Drive(d) = f.mode else {
            return Err(self.unsupported(&format!("function {k} as a drive")));
        };
        let Some(drive) = self.body.drives().get(d as usize) else {
            return Err(self.unsupported(&format!("drive {d}")));
        };
        let Some(carrier) = self.body.carriers().get(drive.carrier as usize) else {
            return Err(self.unsupported(&format!("a drive of carrier {}", drive.carrier)));
        };
        let values = ValuePrinter::of_digital(self.ir, self.body).with_reads(self.reads());
        let value = values.expr_as(drive.value, &ValueType::Flagged(Box::new(carrier.ty.clone())))?;
        let mut t = Text::new();
        self.header(&mut t, k, f);
        t.line(&format!("fn {}(cx_: &mut Eval<'_>) -> Result<(), Fault> {{", self.function_name(f)));
        let binds = self.binds(&values, "cx_", &f.reads, &[])?;
        t.indented(|t| {
            for line in &binds {
                t.line(line);
            }
            t.comment(&format!("drive of {}.{}: the value its pin takes", self.body.name(), carrier.name), &drive.origin);
            t.line(&format!("let value_ = {value};"));
            t.line("value_.write(cx_.out_bytes(0));");
            t.line("Ok(())");
        });
        t.line("}");
        let mut text = t.finish();
        text.pop();
        Ok(text)
    }

    /// The function `k` of the plan, which runs `Mode::Event` or `Mode::DelayedEvent`: the
    /// event pin's `CHANGED` byte, nonzero when the event's source fired and its guard holds.
    pub fn event(&self, k: usize) -> Result<String, Error> {
        let Some(f) = self.plan.functions().get(k) else {
            return Err(self.unsupported(&format!("function {k} as an event")));
        };
        let values = ValuePrinter::of_digital(self.ir, self.body).with_reads(self.reads());
        // what fired it, and its guard
        let (fired, guard, origin, what) = match f.mode {
            Mode::Event(e) => {
                let Some(event) = self.body.events().get(e as usize) else {
                    return Err(self.unsupported(&format!("event {e}")));
                };
                let (source, what) = match event.source {
                    EventSource::Changed(c) => {
                        (self.names.read_name(Source::CarrierChanged(c)), format!("carrier {} changed", self.carrier_name(c)))
                    }
                    EventSource::ParamChanged(p) => {
                        let name = self.body.params().get(p as usize).map_or_else(|| format!("param{p}"), |x| x.name.clone());
                        (self.names.read_name(Source::ParamChanged(p)), format!("param {name} changed"))
                    }
                    EventSource::Cross { .. } => (self.names.read_name(Source::Crossed(e)), format!("its crossing fired (event {e})")),
                    // only a guarded timer has an event function: it gates the timer's own pin
                    EventSource::At(_) => (self.names.read_name(Source::Timer(e)), format!("its timer fired (event {e})")),
                    // fail loud until the settle event function is printed (T3)
                    EventSource::Settle => return Err(self.unsupported("the event function of settle")),
                };
                (source, event.guard, event.origin.clone(), what)
            }
            Mode::DelayedEvent(d) => {
                let Some(drive) = self.body.drives().get(d as usize) else {
                    return Err(self.unsupported(&format!("drive {d}")));
                };
                let what = format!("the undelayed value of {} changed", self.carrier_name(drive.carrier));
                (self.names.read_name(Source::DelayedChanged(d)), None, drive.origin.clone(), what)
            }
            Mode::Rearm { event, carrier } => {
                let Some(at) = self.body.events().get(event as usize) else {
                    return Err(self.unsupported(&format!("event {event}")));
                };
                let what = format!("carrier {} changed, which the time of event {event} reads", self.carrier_name(carrier));
                (self.names.read_name(Source::CarrierChanged(carrier)), None, at.origin.clone(), what)
            }
            _ => return Err(self.unsupported(&format!("function {k} as an event"))),
        };
        // a guard that may be unknown does not stop the event: it fires unless the guard is known
        // false, and its reaction then runs as a maybe (§13 rule 1)
        let guard_flagged = guard.is_some_and(|g| matches!(self.ir.expr_type(g), ValueType::Flagged(_)));
        let condition = match guard {
            Some(g) if guard_flagged => Some(values.expr(g)?),
            Some(g) => Some(values.known_condition(g)?),
            None => None,
        };
        let forced: Vec<Source> = f.reads.iter().copied().filter(|s| self.names.read_name(*s) == fired).collect();
        let binds = self.binds(&values, "cx_", &f.reads, &forced)?;
        let mut t = Text::new();
        self.header(&mut t, k, f);
        t.line(&format!("fn {}(cx_: &mut Eval<'_>) -> Result<(), Fault> {{", self.function_name(f)));
        t.indented(|t| {
            for line in &binds {
                t.line(line);
            }
            match &condition {
                Some(guard) if guard_flagged => {
                    t.comment(&format!("event of {}: it fires when {what} and its guard is not known false", self.body.name()), &origin);
                    t.line(&format!("let guard_ = {guard};"));
                    t.line(&format!("let fired_ = {fired} && (!guard_.is_known() || guard_.high());"));
                }
                Some(guard) => {
                    t.comment(&format!("event of {}: it fires when {what} and its guard holds", self.body.name()), &origin);
                    t.line(&format!("let fired_ = {fired} && {guard};"));
                }
                None => {
                    t.comment(&format!("event of {}: it fires when {what}", self.body.name()), &origin);
                    t.line(&format!("let fired_ = {fired};"));
                }
            }
            t.line("cx_.set(0, u8::from(fired_));");
            t.line("Ok(())");
        });
        t.line("}");
        let mut text = t.finish();
        text.pop();
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use piperine_ir::{BodyId, IrBuilder};
    use piperine_plan::{BodyPlan, Mode};

    use super::DigitalPrinter;
    use crate::alone::Alone;
    use crate::digital::Buck;

    /// Every discrete function of the body `make` builds, by its name in the shape.
    fn printed(make: fn(&mut IrBuilder) -> BodyId, name: &str) -> String {
        let (ir, body) = Alone::design(make);
        let plan = BodyPlan::new(ir.body(body));
        let printer = DigitalPrinter::new(ir.body(body), &plan);
        let k = plan
            .shape()
            .shape()
            .functions
            .iter()
            .position(|f| f.name.as_str() == name)
            .unwrap_or_else(|| panic!("no function `{name}`"));
        match plan.functions()[k].mode {
            Mode::Drive(_) => printer.drive(k),
            _ => printer.event(k),
        }
        .unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn drives_and_events_print_as_discrete_functions() {
        // dpwm.msl:51 `sample = count == 0;`: `count` is read from its pin as a value that may
        // be unknown (§3.3.1), so the comparison is unknown until `count` is written; the drive
        // writes the carrier's pin bytes, all zero when unknown (RS-11 AC 4)
        let sample = "\
// function 0 `drive:sample`: reads count; writes sample @ dpwm.msl:51:5
fn sample_drive(cx_: &mut Eval<'_>) -> Result<(), Fault> {
    let count = rt::Flagged::<rt::UInt<9>>::decode(cx_.bytes(0));
    // drive of Dpwm.sample: the value its pin takes @ dpwm.msl:51:5
    let value_ = count.equal(rt::Flagged::known(rt::UInt::<9>::new(0)));
    value_.write(cx_.out_bytes(0));
    Ok(())
}";
        assert_eq!(printed(Buck::dpwm, "drive:sample"), sample);

        // abi.md §11.4: the flip-flop's `rise` is `clk` changed with `clk` high, written as the
        // pin's `CHANGED` byte, nonzero when it fires (the IR's `rise()` reads `[clk.VALUE,
        // clk.CHANGED]`: `PREVIOUS` has no IR form, T32)
        let rise = "\
// function 0 `event:0`: reads clk, clk_changed_; writes event:0 @ dff.msl:2:22
fn event0(cx_: &mut Eval<'_>) -> Result<(), Fault> {
    let clk = rt::Flagged::<bool>::decode(cx_.bytes(0));
    let clk_changed_ = cx_.bool(1);
    // event of Dff: it fires when carrier clk changed and its guard holds @ dff.msl:2:22
    let fired_ = clk_changed_ && clk.high();
    cx_.set(0, u8::from(fired_));
    Ok(())
}";
        assert_eq!(printed(Buck::dff, "event:0"), rise);

        // dpwm.msl:10 `on clk.changed()`: no guard, so the event is the `CHANGED` byte, and
        // the clock's value is not read at all
        let tick = "\
// function 1 `event:0`: reads clk, clk_changed_; writes event:0 @ dpwm.msl:10:22
fn event0(cx_: &mut Eval<'_>) -> Result<(), Fault> {
    let clk_changed_ = cx_.bool(1);
    // event of Osc: it fires when carrier clk changed @ dpwm.msl:10:22
    let fired_ = clk_changed_;
    cx_.set(0, u8::from(fired_));
    Ok(())
}";
        assert_eq!(printed(Buck::osc, "event:0"), tick);

        // power.msl:76 `hs_gate = (hs_cmd & !ls_cmd) after tpd`: an `after` moves the value to the
        // private pin `drive:hs_gate.now` (RS-12 AC 5), the drive's own function writes it, and the
        // `changed` event of that pin (its `CHANGED` byte) fires the reaction that commits it later
        let gate = "\
// function 1 `drive:hs_gate`: reads hs_cmd, ls_cmd; writes drive:hs_gate.now @ power.msl:76:5
fn hs_gate_drive(cx_: &mut Eval<'_>) -> Result<(), Fault> {
    let hs_cmd = rt::Flagged::<bool>::decode(cx_.bytes(0));
    let ls_cmd = rt::Flagged::<bool>::decode(cx_.bytes(1));
    // drive of GateDriver.hs_gate: the value its pin takes @ power.msl:76:5
    let value_ = hs_cmd & !ls_cmd;
    value_.write(cx_.out_bytes(0));
    Ok(())
}";
        assert_eq!(printed(Buck::gate_driver, "drive:hs_gate"), gate);
        let changed = "\
// function 3 `drive:hs_gate.changed`: reads hs_gate_now_, hs_gate_now_changed_; writes drive:hs_gate.changed @ power.msl:76:5
fn hs_gate_drive_changed(cx_: &mut Eval<'_>) -> Result<(), Fault> {
    let hs_gate_now_changed_ = cx_.bool(1);
    // event of GateDriver: it fires when the undelayed value of hs_gate changed @ power.msl:76:5
    let fired_ = hs_gate_now_changed_;
    cx_.set(0, u8::from(fired_));
    Ok(())
}";
        assert_eq!(printed(Buck::gate_driver, "drive:hs_gate.changed"), changed);

        // control.msl:24 `at_target = reference == target;` over two integers
        let at_target = "\
// function 0 `drive:at_target`: reads target, reference; writes at_target @ control.msl:24:5
fn at_target_drive(cx_: &mut Eval<'_>) -> Result<(), Fault> {
    let target = rt::Flagged::<rt::UInt<10>>::decode(cx_.bytes(0));
    let reference = rt::Flagged::<rt::UInt<10>>::decode(cx_.bytes(1));
    // drive of RefRamp.at_target: the value its pin takes @ control.msl:24:5
    let value_ = reference.equal(target);
    value_.write(cx_.out_bytes(0));
    Ok(())
}";
        assert_eq!(printed(Buck::ref_ramp, "drive:at_target"), at_target);
    }
}
