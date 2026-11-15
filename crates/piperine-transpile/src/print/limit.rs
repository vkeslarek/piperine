//! `AnalogPrinter::limit`: the function of a `limit` site (§18, abi.md A2), ported rule by rule
//! from the interpreter's `Walk::limit_site`.
//!
//! Two Rust functions. `limit_<site>_block<R: Real>` is the slice of the block the site's input
//! `x`, its function `f` and its seed reach, returning `x`, `f` as a `move` closure over `R` and
//! the seed. `limit_<site>` runs it twice: over `Dual<N>`, for `PROPOSED` (`x`) and its partials;
//! over plain `f64`, for `f` and the seed. Then the three rules: `old` is `LAST`; it is the seed
//! (or `x` with no seed) when nothing is held (`AGE` 0) or, in an equilibrium solve, when what is
//! held comes from the last accepted solution (`STATIC` and `AGE` 1); and a `LIMITED` that starts
//! from the seed and falls below it is raised to the seed. `LIMITED = f(x, old)`, with `x` the
//! value of `PROPOSED`. The values of `Dual` arithmetic are the `f64` operations themselves, so
//! the two runs agree on every value they share.

use piperine_ir::{Expr, ExprId, Stmt};
use piperine_plan::{Mode, Source, Target};

use crate::Error;
use crate::print::{AnalogPrinter, Text, ValuePrinter};

/// Where a `limit` site sits: its top-level statement and its three arguments.
struct Site {
    stmt: usize,
    x: ExprId,
    f: ExprId,
    seed: Option<ExprId>,
}

impl<'a> AnalogPrinter<'a> {
    /// The site of `limit` `k`: the first top-level statement that holds it.
    fn limit_site(&self, k: u32) -> Option<Site> {
        let block = self.body.analog().unwrap_or(&[]);
        for (stmt, id) in block.iter().enumerate() {
            let mut exprs = match self.ir.stmt(*id) {
                Stmt::Let { init, .. } => vec![*init],
                Stmt::Expr(e) => vec![*e],
                Stmt::Contribute { addends, .. } => addends.iter().map(|a| a.value).collect(),
            };
            while let Some(e) = exprs.pop() {
                match self.ir.expr(e) {
                    Expr::Limit { site, x, f, seed } if *site == k => return Some(Site { stmt, x: *x, f: *f, seed: *seed }),
                    other => ValuePrinter::children(self.ir, other, &mut exprs),
                }
            }
        }
        None
    }

    /// `limit_<site>_block` and `limit_<site>`, for function `k` of the plan, which runs
    /// `Mode::Limit`.
    pub fn limit(&self, k: usize) -> Result<String, Error> {
        let Some((f, site_index)) = self.plan.functions().get(k).and_then(|f| match f.mode {
            Mode::Limit(s) => Some((f, s)),
            _ => None,
        }) else {
            return Err(self.unsupported(&format!("function {k} as a limit function")));
        };
        let Some(site) = self.limit_site(site_index) else {
            return Err(self.unsupported(&format!("limit site {site_index}, found in no statement")));
        };
        if !matches!(self.ir.expr(site.f), Expr::Closure { .. }) {
            return Err(Error::Unsupported {
                form: "a `limit` function that is not a closure".into(),
                path: format!("body {} e{}", self.body.name(), site.f.index()),
                origin: self.ir.expr_origin(site.f).clone(),
            });
        }
        let name = self.body.limits().get(site_index as usize).map_or_else(|| site_index.to_string(), |s| s.name.clone());
        let mode = Mode::Limit(site_index);
        let args: Vec<ExprId> = [site.x, site.f].into_iter().chain(site.seed).collect();
        let slice = self.slice_of(mode, &args, site.stmt);
        // The block's reads, in the plan's order: its dependency set without LAST, AGE, STATIC.
        let reads: Vec<(usize, Source)> =
            f.reads.iter().copied().enumerate().filter(|(_, s)| slice.sources.contains(s)).collect();
        let names: Vec<String> = f.reads.iter().map(|s| self.read_name(*s)).collect();
        let at = |s: Source| f.reads.iter().position(|r| *r == s);
        let (Some(last), Some(age), Some(is_static)) =
            (at(Source::LimitLast(site_index)), at(Source::LimitAge(site_index)), at(Source::Static))
        else {
            return Err(self.unsupported(&format!("limit {name}, whose function does not read LAST, AGE and STATIC")));
        };

        let mut t = Text::new();
        // limit_<site>_block
        let params: Vec<String> = reads
            .iter()
            .map(|(_, s)| format!("{}: {}", self.read_name(*s), if self.is_bool(*s) { "bool" } else { "R" }))
            .collect();
        let site_origin = self.body.limits().get(site_index as usize).map_or_else(|| self.body.origin(), |s| &s.origin);
        let returns = if site.seed.is_some() { "(R, impl Fn(R, R) -> R, R)" } else { "(R, impl Fn(R, R) -> R)" };
        t.comment(&format!("{}.analog, `limit` site `{name}`: its input, its function and its seed", self.body.name()), site_origin);
        t.line(&format!("fn limit_{name}_block<R: Real>({}) -> {returns} {{", params.join(", ")));
        let mut lines = Vec::new();
        for id in &slice.stmts {
            lines.push(self.values.stmt(*id)?);
        }
        let mut tuple = vec![self.values.expr(site.x)?, format!("move {}", self.values.expr(site.f)?)];
        if let Some(seed) = site.seed {
            tuple.push(self.values.expr(seed)?);
        }
        lines.push(format!("({})", tuple.join(", ")));
        t.indented(|t| {
            for l in &lines {
                t.line(l);
            }
        });
        t.line("}");
        t.line("");

        // limit_<site>
        let writes: Vec<String> = f.writes.iter().map(|w| self.write_name(*w)).collect();
        t.line(&format!(
            "// function {k} `limit:{name}`: reads {}; writes {} @ {}",
            names.join(", "),
            writes.join(", "),
            self.function_origin(k)
        ));
        t.line(&format!("fn limit_{name}(cx: &mut Eval<'_>) -> Result<(), Fault> {{"));
        let mut body = Vec::new();
        for (i, s) in &reads {
            let get = if self.is_bool(*s) { "bool" } else { "f64" };
            body.push(format!("let {} = cx.{get}({i});", names[*i]));
        }
        let (last, age, is_static) = (&names[last], &names[age], &names[is_static]);
        body.push(format!("let {last} = cx.f64({});", at(Source::LimitLast(site_index)).unwrap_or_default()));
        body.push(format!("let {age} = cx.u8({});", at(Source::LimitAge(site_index)).unwrap_or_default()));
        body.push(format!("let {is_static} = cx.bool({});", at(Source::Static).unwrap_or_default()));
        let dual: Vec<String> = reads
            .iter()
            .map(|(i, s)| {
                if *i < f.slots {
                    format!("Dual::<{}>::seeded({}, {i})", f.slots, names[*i])
                } else if self.is_bool(*s) {
                    names[*i].clone()
                } else {
                    format!("Dual::<{}>::constant({})", f.slots, names[*i])
                }
            })
            .collect();
        let plain: Vec<String> = reads.iter().map(|(i, _)| names[*i].clone()).collect();
        body.push("// PROPOSED: the input x and its partials, at the iterate".to_string());
        let ignored = if site.seed.is_some() { ", _, _" } else { ", _" };
        body.push(format!("let (proposed{ignored}) = Self::limit_{name}_block({});", dual.join(", ")));
        body.push("// the function and the seed, over plain f64".to_string());
        if site.seed.is_some() {
            body.push(format!("let (_, f, seed) = Self::limit_{name}_block({});", plain.join(", ")));
        } else {
            body.push(format!("let (_, f) = Self::limit_{name}_block({});", plain.join(", ")));
        }
        body.push(
            "// rule `the seed start`: nothing held (AGE 0), or in an equilibrium solve a value held from the last accepted solution (STATIC, AGE 1)"
                .to_string(),
        );
        body.push(format!("let from_seed = {age} == 0 || ({is_static} && {age} == 1);"));
        let start = if site.seed.is_some() { "seed" } else { "proposed.v" };
        body.push(format!("// rule `old`: LAST, or where the seed starts the {}", if site.seed.is_some() { "seed" } else { "input x (no seed)" }));
        body.push(format!("let old = if from_seed {{ {start} }} else {{ {last} }};"));
        body.push("let limited = f(proposed.v, old);".to_string());
        if site.seed.is_some() {
            body.push("// rule `the seed kept`: starting from the seed, a limited value below it is raised to it".to_string());
            body.push("let limited = if from_seed && limited < seed { seed } else { limited };".to_string());
        } else {
            body.push("// rule `the seed kept`: there is no seed, so nothing is raised".to_string());
        }
        for (j, w) in f.writes.iter().enumerate() {
            match w {
                Target::Proposed(_) => body.push(format!("cx.set({j}, proposed.v);")),
                Target::Limited(_) => body.push(format!("cx.set({j}, limited);")),
                other => return Err(self.unsupported(&format!("the write {other:?} of limit {name}"))),
            }
        }
        for (p, (w, slot)) in f.partials.iter().enumerate() {
            body.push(format!("// partial {p}: d {} / d {}", writes[*w], names[*slot]));
            body.push(format!("if cx.wanted({p}) {{\n    cx.jac()[{p}] = proposed.d[{slot}];\n}}"));
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
        Addend, BodyId, BodyNode, BodySpec, Branch, BranchKind, Ir, IrBuilder, Local, NodeRole, Operator, Origin,
        QuantityRole, Site, Stmt, Terminal, ValueType,
    };
    use piperine_plan::{BodyPlan, Mode};

    use crate::devices::{Devices, Ex};
    use crate::print::AnalogPrinter;

    fn printed(ir: &Ir, body: BodyId, mode: fn(Mode) -> bool) -> String {
        let plan = BodyPlan::new(ir.body(body));
        let k = plan.functions().iter().position(|f| mode(f.mode)).expect("the function");
        let printer = AnalogPrinter::new(ir.body(body), &plan);
        match plan.functions()[k].mode {
            Mode::Limit(_) => printer.limit(k),
            _ => printer.terms(k),
        }
        .unwrap_or_else(|e| panic!("{e}"))
    }

    /// `Hold(p, n)`: `i(p, n) +> limit(v(p, n), |new, old| old)`, with no seed (the interpreter's
    /// limit fixture, `hold.msl`).
    fn hold() -> (Ir, BodyId) {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let mut e = Ex::new(&mut b, "hold.msl", 3, 30);
        let x = e.probe(Terminal::Node(0), Terminal::Node(1));
        let old = e.local(1);
        let f = e.closure(vec![0, 1], old);
        let limited = e.limit(0, x, f, None);
        let contribute = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value: limited }],
        });
        let at = |line: u32| Origin { file: "hold.msl".into(), line, col: 1 };
        let port = |name: &str| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: at(1) };
        let local = |name: &str| Local { name: name.into(), ty: ValueType::F64 };
        let body = b.body(BodySpec {
            name: "Hold".into(),
            origin: at(1),
            params: vec![],
            nodes: vec![port("p"), port("n")],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![Site { name: "y".into(), origin: at(3) }],
            stims: vec![],
            locals: vec![local("new"), local("old")],
            analog: Some(vec![contribute]),
            children: vec![],
        });
        let top = Devices::alone(&mut b, body, "H", 1, vec![]);
        (b.finish(top), body)
    }

    #[test]
    fn a_limit_function_reads_last_age_and_static() {
        let mut b = IrBuilder::new();
        let d = Devices::diode(&mut b);
        let top = Devices::alone(&mut b, d, "D", 1, vec![1e-14, 1.0, 0.0]);
        let ir = b.finish(top);
        // devices.msl:37-39: `let vd = limit(v(a, k), |new, old| pnjlim(new, old, nvt, vcrit), vcrit);`
        // The site reads its dependency set (a, k: slots 0 and 1; is, n, temperature through
        // nvt and vcrit), then LAST, AGE and STATIC (abi.md A2). `PROPOSED` is v(a, k) with its
        // partials; `LIMITED` is pnjlim(x, old) over plain f64, old by the rules of §18.
        let diode = "\
// Diode.analog, `limit` site `vd`: its input, its function and its seed @ devices.msl:39:17
fn limit_vd_block<R: Real>(a: R, k: R, is: R, n: R, temperature: R) -> (R, impl Fn(R, R) -> R, R) {
    let nvt = n * vt(temperature);
    let vcrit = nvt * (nvt / (R::constant(2.0).sqrt() * is)).ln();
    (a - k, move |new, old| pnjlim(new, old, nvt, vcrit), vcrit)
}

// function 0 `limit:vd`: reads a, k, is, n, temperature, limit_vd_last, limit_vd_age, is_static; writes vd.PROPOSED, vd.LIMITED @ devices.msl:39:17
fn limit_vd(cx: &mut Eval<'_>) -> Result<(), Fault> {
    let a = cx.f64(0);
    let k = cx.f64(1);
    let is = cx.f64(2);
    let n = cx.f64(3);
    let temperature = cx.f64(4);
    let limit_vd_last = cx.f64(5);
    let limit_vd_age = cx.u8(6);
    let is_static = cx.bool(7);
    // PROPOSED: the input x and its partials, at the iterate
    let (proposed, _, _) = Self::limit_vd_block(Dual::<2>::seeded(a, 0), Dual::<2>::seeded(k, 1), Dual::<2>::constant(is), Dual::<2>::constant(n), Dual::<2>::constant(temperature));
    // the function and the seed, over plain f64
    let (_, f, seed) = Self::limit_vd_block(a, k, is, n, temperature);
    // rule `the seed start`: nothing held (AGE 0), or in an equilibrium solve a value held from the last accepted solution (STATIC, AGE 1)
    let from_seed = limit_vd_age == 0 || (is_static && limit_vd_age == 1);
    // rule `old`: LAST, or where the seed starts the seed
    let old = if from_seed { seed } else { limit_vd_last };
    let limited = f(proposed.v, old);
    // rule `the seed kept`: starting from the seed, a limited value below it is raised to it
    let limited = if from_seed && limited < seed { seed } else { limited };
    cx.set(0, proposed.v);
    cx.set(1, limited);
    // partial 0: d vd.PROPOSED / d a
    if cx.wanted(0) {
        cx.jac()[0] = proposed.d[0];
    }
    // partial 1: d vd.PROPOSED / d k
    if cx.wanted(1) {
        cx.jac()[1] = proposed.d[1];
    }
    Ok(())
}";
        assert_eq!(printed(&ir, d, |m| matches!(m, Mode::Limit(_))), diode);

        // With no seed, `old` starts from the input x and nothing is raised; the closure ignores
        // `new`, which prints as `_new`.
        let (ir, body) = hold();
        let text = printed(&ir, body, |m| matches!(m, Mode::Limit(_)));
        assert!(text.contains("fn limit_y_block<R: Real>(p: R, n: R) -> (R, impl Fn(R, R) -> R) {"), "{text}");
        assert!(text.contains("    (p - n, move |_new, old| old)\n"), "{text}");
        assert!(text.contains("let old = if from_seed { proposed.v } else { limit_y_last };"), "{text}");
        assert!(text.contains("let limit_y_last = cx.f64(2);\n    let limit_y_age = cx.u8(3);\n    let is_static = cx.bool(4);"), "{text}");
        assert!(!text.contains("limited < seed"), "{text}");
        for rule in ["rule `old`", "rule `the seed start`", "rule `the seed kept`"] {
            assert!(text.contains(rule), "{rule}: {text}");
        }
    }

    #[test]
    fn the_terms_function_reads_the_limit_value_only() {
        let mut b = IrBuilder::new();
        let d = Devices::diode(&mut b);
        let top = Devices::alone(&mut b, d, "D", 1, vec![1e-14, 1.0, 0.0]);
        let ir = b.finish(top);
        // §18 rule 3, abi.md A2: the terms read the site's held value `limit_vd`, never its input
        // v(a, k), its function or its seed; those are the limit function's
        let terms = printed(&ir, d, |m| m == Mode::Terms);
        assert!(terms.contains("let vd = limit_vd;"), "{terms}");
        assert!(terms.contains("let limit_vd = cx.f64(0);"), "{terms}");
        for absent in ["a - k", "cx.f64(6)", "pnjlim", "vcrit", "LAST", "_last", "_age", "is_static"] {
            assert!(!terms.contains(absent), "`{absent}` in the terms function:\n{terms}");
        }
        let limit = printed(&ir, d, |m| matches!(m, Mode::Limit(_)));
        assert!(limit.contains("pnjlim(new, old, nvt, vcrit)") && limit.contains("(a - k, "), "{limit}");

        // the seedless `Hold`: its terms are the held value alone
        let (ir, body) = hold();
        let terms = printed(&ir, body, |m| m == Mode::Terms);
        assert!(terms.contains("fn terms_block<R: Real>(limit_y: R) -> [R; 1] {"), "{terms}");
        assert!(terms.contains("let b0_flow = limit_y;"), "{terms}");
    }
}
