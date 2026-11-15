//! `AnalogPrinter::{stim, check}`: the two functions of a body's shape that compute plain values.
//!
//! `stim:<k>` is the slice its magnitude and phase reach, as a block generic over `Real` run over
//! `f64` (the interpreter takes their values, `Walk::stim_site`): `RE = mag·cos(phase)`,
//! `IM = mag·sin(phase)`, and exactly `RE = mag`, `IM = 0` with no phase. `check:<p>` is the
//! param's `where` over the params it reads, as a generic block run over `f64` returning `bool`,
//! written as the known `BIT` `HIGH` or `LOW`.

use piperine_ir::{Expr, ExprId, Stmt};
use piperine_plan::{Mode, Source, Target};

use crate::Error;
use crate::print::{AnalogPrinter, Text, ValuePrinter};

impl<'a> AnalogPrinter<'a> {
    /// The site of `ac_stim` `k`: its top-level statement, its magnitude and its phase.
    fn stim_site(&self, k: u32) -> Option<(usize, ExprId, Option<ExprId>)> {
        let block = self.body.analog().unwrap_or(&[]);
        for (stmt, id) in block.iter().enumerate() {
            let mut exprs = match self.ir.stmt(*id) {
                Stmt::Let { init, .. } => vec![*init],
                Stmt::Expr(e) => vec![*e],
                Stmt::Contribute { addends, .. } => addends.iter().map(|a| a.value).collect(),
            };
            while let Some(e) = exprs.pop() {
                match self.ir.expr(e) {
                    Expr::Stim { site, mag, phase } if *site == k => return Some((stmt, *mag, *phase)),
                    other => ValuePrinter::children(self.ir, other, &mut exprs),
                }
            }
        }
        None
    }

    /// `let x = cx.f64(i);` for each read, `cx.bool` for a `bool` param.
    fn bind_reads(&self, reads: &[(usize, Source)]) -> Vec<String> {
        reads
            .iter()
            .map(|(i, s)| {
                let get = if self.is_bool(*s) { "bool" } else { "f64" };
                format!("let {} = cx.{get}({i});", self.read_name(*s))
            })
            .collect()
    }

    fn block_params(&self, reads: &[(usize, Source)]) -> String {
        let params: Vec<String> = reads
            .iter()
            .map(|(_, s)| format!("{}: {}", self.read_name(*s), if self.is_bool(*s) { "bool" } else { "R" }))
            .collect();
        params.join(", ")
    }

    fn block_args(&self, reads: &[(usize, Source)]) -> String {
        reads.iter().map(|(_, s)| self.read_name(*s)).collect::<Vec<_>>().join(", ")
    }

    /// `stim_<site>_block` and `stim_<site>`, for function `k` of the plan, which runs
    /// `Mode::Stim`.
    pub fn stim(&self, k: usize) -> Result<String, Error> {
        let Some((f, site_index)) = self.plan.functions().get(k).and_then(|f| match f.mode {
            Mode::Stim(s) => Some((f, s)),
            _ => None,
        }) else {
            return Err(self.unsupported(&format!("function {k} as a stimulus function")));
        };
        let Some((stmt, mag, phase)) = self.stim_site(site_index) else {
            return Err(self.unsupported(&format!("ac_stim site {site_index}, found in no statement")));
        };
        let site = self.body.stims().get(site_index as usize);
        let name = site.map_or_else(|| site_index.to_string(), |s| s.name.clone());
        let origin = site.map_or_else(|| self.body.origin(), |s| &s.origin);
        let exprs: Vec<ExprId> = [mag].into_iter().chain(phase).collect();
        let slice = self.slice_of(Mode::Stim(site_index), &exprs, stmt);
        let reads: Vec<(usize, Source)> =
            f.reads.iter().copied().enumerate().filter(|(_, s)| slice.sources.contains(s)).collect();

        let mut t = Text::new();
        let returns = if phase.is_some() { "(R, R)" } else { "R" };
        t.comment(&format!("{}.analog, `ac_stim` site `{name}`: its magnitude and phase", self.body.name()), origin);
        t.line(&format!("fn stim_{name}_block<R: Real>({}) -> {returns} {{", self.block_params(&reads)));
        let mut lines = Vec::new();
        for id in &slice.stmts {
            lines.push(self.values.stmt(*id)?);
        }
        lines.push(match phase {
            Some(p) => format!("({}, {})", self.values.expr(mag)?, self.values.expr(p)?),
            None => self.values.expr(mag)?,
        });
        t.indented(|t| {
            for l in &lines {
                t.line(l);
            }
        });
        t.line("}");
        t.line("");

        let names: Vec<String> = f.reads.iter().map(|s| self.read_name(*s)).collect();
        let writes: Vec<String> = f.writes.iter().map(|w| self.write_name(*w)).collect();
        t.line(&format!(
            "// function {k} `stim:{site_index}`: reads {}; writes {} @ {}",
            names.join(", "),
            writes.join(", "),
            self.function_origin(k)
        ));
        t.line(&format!("fn stim_{name}(cx: &mut Eval<'_>) -> Result<(), Fault> {{"));
        let mut body = self.bind_reads(&reads);
        let call = format!("Self::stim_{name}_block::<f64>({})", self.block_args(&reads));
        let (re, im) = if phase.is_some() {
            body.push(format!("let (mag, phase) = {call};"));
            body.push("// RE = mag·cos(phase), IM = mag·sin(phase)".to_string());
            ("mag * phase.cos()", "mag * phase.sin()")
        } else {
            body.push(format!("let mag = {call};"));
            body.push("// no phase: RE is the magnitude, IM is 0".to_string());
            ("mag", "0.0")
        };
        for (j, w) in f.writes.iter().enumerate() {
            match w {
                Target::StimRe(_) => body.push(format!("cx.set({j}, {re});")),
                Target::StimIm(_) => body.push(format!("cx.set({j}, {im});")),
                other => return Err(self.unsupported(&format!("the write {other:?} of stimulus {name}"))),
            }
        }
        body.push("Ok(())".to_string());
        t.indented(|t| {
            for l in &body {
                t.line(l);
            }
        });
        t.line("}");
        let mut text = t.finish();
        text.pop();
        Ok(text)
    }

    /// `check_<param>_block` and `check_<param>`, for function `k` of the plan, which runs
    /// `Mode::Check`.
    pub fn check(&self, k: usize) -> Result<String, Error> {
        let Some((f, p)) = self.plan.functions().get(k).and_then(|f| match f.mode {
            Mode::Check(p) => Some((f, p)),
            _ => None,
        }) else {
            return Err(self.unsupported(&format!("function {k} as a check function")));
        };
        let Some((param, condition)) = self.body.params().get(p as usize).and_then(|x| x.check.map(|c| (x, c))) else {
            return Err(self.unsupported(&format!("param {p}, which has no `where`")));
        };
        let name = &param.name;
        let reads: Vec<(usize, Source)> = f.reads.iter().copied().enumerate().collect();
        let mut t = Text::new();
        t.comment(&format!("{}.{name}: its `where`", self.body.name()), &param.origin);
        t.line(&format!("fn check_{name}_block<R: Real>({}) -> bool {{", self.block_params(&reads)));
        let condition = self.values.expr(condition)?;
        t.indented(|t| t.line(&condition));
        t.line("}");
        t.line("");
        let names: Vec<String> = f.reads.iter().map(|s| self.read_name(*s)).collect();
        let writes: Vec<String> = f.writes.iter().map(|w| self.write_name(*w)).collect();
        t.line(&format!(
            "// function {k} `check:{name}`: reads {}; writes {} @ {}",
            names.join(", "),
            writes.join(", "),
            self.function_origin(k)
        ));
        t.line(&format!("fn check_{name}(cx: &mut Eval<'_>) -> Result<(), Fault> {{"));
        let mut body = self.bind_reads(&reads);
        body.push(format!("let ok = Self::check_{name}_block::<f64>({});", self.block_args(&reads)));
        for (j, w) in f.writes.iter().enumerate() {
            match w {
                Target::Check(_) => body.push(format!("cx.set({j}, if ok {{ HIGH }} else {{ LOW }});")),
                other => return Err(self.unsupported(&format!("the write {other:?} of the check of {name}"))),
            }
        }
        body.push("Ok(())".to_string());
        t.indented(|t| {
            for l in &body {
                t.line(l);
            }
        });
        t.line("}");
        let mut text = t.finish();
        text.pop();
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use piperine_ir::{
        Addend, BodyId, BodyNode, BodySpec, Branch, BranchKind, Ir, IrBuilder, NodeRole, Operator, Origin, Param,
        QuantityRole, Site, Stmt, Terminal, ValueType,
    };
    use piperine_plan::{BodyPlan, Mode};

    use crate::devices::{Devices, Ex};
    use crate::print::AnalogPrinter;

    /// The text of the function of `body` whose mode `pick` selects.
    fn printed(ir: &Ir, body: BodyId, pick: impl Fn(Mode) -> bool) -> String {
        let plan = BodyPlan::new(ir.body(body));
        let k = plan.functions().iter().position(|f| pick(f.mode)).expect("the function");
        let printer = AnalogPrinter::new(ir.body(body), &plan);
        match plan.functions()[k].mode {
            Mode::Stim(_) => printer.stim(k),
            _ => printer.check(k),
        }
        .unwrap_or_else(|e| panic!("{e}"))
    }

    /// `Ac(p, n) { mag: f64 = 1.0; phase: f64 = 0.0; }` with `i(p, n) +> ac_stim(mag, phase)`.
    fn phased() -> (Ir, BodyId) {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let mut e = Ex::new(&mut b, "ac.msl", 4, 20);
        let mag = e.param(0);
        let phase = e.param(1);
        let stim = e.stim(0, mag, Some(phase));
        let contribute = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value: stim }],
        });
        let at = |line: u32| Origin { file: "ac.msl".into(), line, col: 1 };
        let real = |name: &str, v: f64, line| Param {
            name: name.into(),
            ty: ValueType::F64,
            default: Some(piperine_ir::Constant::Real(piperine_ir::Real::new(v))),
            check: None,
            structural: false,
            origin: at(line),
        };
        let port = |name: &str| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: at(1) };
        let body = b.body(BodySpec {
            name: "Ac".into(),
            origin: at(1),
            params: vec![real("mag", 1.0, 2), real("phase", 0.0, 3)],
            nodes: vec![port("p"), port("n")],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![Site { name: "s".into(), origin: at(4) }],
            locals: vec![],
            analog: Some(vec![contribute]),
            children: vec![],
        });
        let top = Devices::alone(&mut b, body, "A", 1, vec![1.0, 0.0]);
        (b.finish(top), body)
    }

    #[test]
    fn stim_and_check_print_plain_values() {
        let mut b = IrBuilder::new();
        let r = Devices::resistor(&mut b);
        let v = Devices::vsource(&mut b);
        let d = Devices::diode(&mut b);
        let top = Devices::alone(&mut b, r, "R", 1, vec![1000.0]);
        let ir = b.finish(top);

        // devices.msl:21: `ac_stim(ac)` reads the param `ac` (pin 1, the function's read 0) and
        // has no phase: RE is the magnitude, IM is 0
        let vsource = "\
// Vsource.analog, `ac_stim` site `ac`: its magnitude and phase @ devices.msl:21:62
fn stim_ac_block<R: Real>(ac: R) -> R {
    ac
}

// function 0 `stim:0`: reads ac; writes ac.RE, ac.IM @ devices.msl:21:62
fn stim_ac(cx: &mut Eval<'_>) -> Result<(), Fault> {
    let ac = cx.f64(0);
    let mag = Self::stim_ac_block::<f64>(ac);
    // no phase: RE is the magnitude, IM is 0
    cx.set(0, mag);
    cx.set(1, 0.0);
    Ok(())
}";
        assert_eq!(printed(&ir, v, |m| matches!(m, Mode::Stim(_))), vsource);

        // a stimulus with a phase: RE = mag·cos(phase), IM = mag·sin(phase)
        let (ac, body) = phased();
        let text = printed(&ac, body, |m| matches!(m, Mode::Stim(_)));
        assert!(text.contains("fn stim_s_block<R: Real>(mag: R, phase: R) -> (R, R) {\n    (mag, phase)\n}"), "{text}");
        assert!(text.contains("let (mag, phase) = Self::stim_s_block::<f64>(mag, phase);"), "{text}");
        assert!(text.contains("cx.set(0, mag * phase.cos());\n    cx.set(1, mag * phase.sin());"), "{text}");

        // devices.msl:6: `r: f64 where r > 0.0`, read as the function's read 0, written as a BIT
        let resistor = "\
// Resistor.r: its `where` @ devices.msl:6:5
fn check_r_block<R: Real>(r: R) -> bool {
    r.value() > 0.0
}

// function 1 `check:r`: reads r; writes check:r @ devices.msl:6:5
fn check_r(cx: &mut Eval<'_>) -> Result<(), Fault> {
    let r = cx.f64(0);
    let ok = Self::check_r_block::<f64>(r);
    cx.set(0, if ok { HIGH } else { LOW });
    Ok(())
}";
        assert_eq!(printed(&ir, r, |m| matches!(m, Mode::Check(_))), resistor);

        // devices.msl:34: `cj0: f64 = 0.0 where cj0 >= 0.0`, the diode's third check
        let cj0 = printed(&ir, d, |m| m == Mode::Check(2));
        assert!(cj0.contains("fn check_cj0_block<R: Real>(cj0: R) -> bool {\n    cj0.value() >= 0.0\n}"), "{cj0}");
        assert!(cj0.starts_with("// Diode.cj0: its `where` @ devices.msl:34:5\n"), "{cj0}");
        assert!(cj0.contains("// function 4 `check:cj0`: reads cj0; writes check:cj0 @ devices.msl:34:5"), "{cj0}");
    }
}
