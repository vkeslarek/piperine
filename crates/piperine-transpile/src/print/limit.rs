//! `AnalogPrinter::limit`: the function of a `limit` site (§18, abi.md A2), ported rule by rule
//! from the interpreter's `Walk::limit_site`.
//!
//! Two Rust functions. `<site>_limit_block<R: Real>` is the slice of the block the site's input
//! `x`, its function `f` and its seed reach, returning `x`, `f` as a `move` closure over `R` and
//! the seed. `<site>_limit` runs it twice: over `Dual<N>`, for `PROPOSED` (`x`) and its partials;
//! over plain `f64`, for `f` and the seed. Then the three rules: `old` is `LAST`; it is the seed
//! (or `x` with no seed) when nothing is held (`AGE` 0) or, in an equilibrium solve, when what is
//! held comes from the last accepted solution (`STATIC` and `AGE` 1); and a `LIMITED` that starts
//! from the seed and falls below it is raised to the seed. `LIMITED = f(x, old)`, with `x` the
//! value of `PROPOSED`. The values of `Dual` arithmetic are the `f64` operations themselves, so
//! the two runs agree on every value they share.
//!
//! A `limit` under a condition has the site's `enabled` (§14.7.4, analog-structure AS-06): a third
//! block, `<site>_limit_enabled_block`, evaluates it over `f64`, and when it is false the limiter
//! does not run: `PROPOSED = LIMITED = LAST` and every partial is 0, so the solver's veto sees no
//! gap and the held value stays (abi.md A3, a `limit` not evaluated).

use piperine_ir::{Expr, ExprId};
use piperine_plan::{Mode, Source, Target};

use crate::Error;
use crate::print::site::Guard;
use crate::print::{AnalogPrinter, Text, ExpressionPrinter};

/// Where a `limit` site sits: its top-level statement and its three arguments.
struct Site {
    stmt: usize,
    x: ExprId,
    f: ExprId,
    seed: Option<ExprId>,
}

impl<'a> AnalogPrinter<'a> {
    /// The lines of a `limit` that does not run: both writes take `last`, every partial is 0, and
    /// the function returns.
    fn disabled_limit(&self, f: &piperine_plan::FunctionPlan, last: &str, names: &[String]) -> Result<Vec<String>, Error> {
        let mut lines = Vec::new();
        for (j, w) in f.writes.iter().enumerate() {
            match w {
                Target::Proposed(_) | Target::Limited(_) => lines.push(format!("cx_.set({j}, {last});")),
                other => return Err(self.unsupported(&format!("the write {other:?} of a disabled limit"))),
            }
        }
        for (p, (w, slot)) in f.partials.iter().enumerate() {
            lines.push(format!("// partial {p}: d {} / d {}, 0 for a limit that did not run", self.write_name(f.writes[*w]), names[*slot]));
            lines.push(format!("if cx_.wanted({p}) {{\n    cx_.jac()[{p}] = 0.0;\n}}"));
        }
        lines.push("return Ok(());".to_string());
        Ok(lines)
    }

    /// The site of `limit` `k`: the first top-level statement that holds it.
    fn limit_site(&self, k: u32) -> Option<Site> {
        let block = self.body.analog().unwrap_or(&[]);
        for (stmt, id) in block.iter().enumerate() {
            let mut exprs = self.ir.stmt(*id).operands();
            while let Some(e) = exprs.pop() {
                match self.ir.expr(e) {
                    Expr::Limit { site, x, f, seed } if *site == k => return Some(Site { stmt, x: *x, f: *f, seed: *seed }),
                    other => ExpressionPrinter::children(self.ir, other, &mut exprs),
                }
            }
        }
        None
    }

    /// `<site>_limit_block` and `<site>_limit`, for function `k` of the plan, which runs
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
        let name = self.body.limits().get(site_index as usize).map_or_else(|| format!("site{site_index}"), |s| s.name.clone());
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
        // <site>_limit_block
        let params = self.block_params(&reads)?;
        let site_origin = self.body.limits().get(site_index as usize).map_or_else(|| self.body.origin(), |s| &s.origin);
        let returns = if site.seed.is_some() { "(R, impl Fn(R, R) -> R, R)" } else { "(R, impl Fn(R, R) -> R)" };
        t.comment(&format!("{}.analog, `limit` site `{name}`: its input, its function and its seed", self.body.name()), site_origin);
        t.line(&format!("fn {name}_limit_block<R: Real>({}) -> {returns} {{", params));
        let mut lines = Vec::new();
        for id in &slice.stmts {
            lines.push(self.values.stmt(*id)?);
        }
        let mut tuple = vec![self.values.expr(site.x)?, self.values.closure(site.f)?];
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

        // <site>_limit_enabled_block, when the site is read under a condition
        let enabled = self.body.limits().get(site_index as usize).and_then(|s| s.enabled);
        let guard = enabled.map(|condition| Guard { function: f, mode, stmt: site.stmt, condition });
        let enabled_reads = match &guard {
            Some(guard) => self.guard_block(&mut t, &format!("{name}_limit_enabled"), site_origin, guard)?,
            None => Vec::new(),
        };

        // <site>_limit
        let writes: Vec<String> = f.writes.iter().map(|w| self.write_name(*w)).collect();
        t.line(&format!(
            "// function {k} `limit:{name}`: reads {}; writes {} @ {}",
            names.join(", "),
            writes.join(", "),
            self.function_origin(k)
        ));
        t.line(&format!("fn {name}_limit(cx_: &mut Eval<'_>) -> Result<(), Fault> {{"));
        let mut body = Vec::new();
        // every read either block uses, in the plan's order
        let mut bound: Vec<(usize, Source)> = reads.iter().chain(&enabled_reads).copied().collect();
        bound.sort_by_key(|(i, _)| *i);
        bound.dedup_by_key(|(i, _)| *i);
        body.extend(self.bind_reads(&bound)?);
        let (last, age, is_static) = (&names[last], &names[age], &names[is_static]);
        body.push(format!("let {last} = cx_.f64({});", at(Source::LimitLast(site_index)).unwrap_or_default()));
        body.push(format!("let {age} = cx_.u8({});", at(Source::LimitAge(site_index)).unwrap_or_default()));
        body.push(format!("let {is_static} = cx_.bool({});", at(Source::Static).unwrap_or_default()));
        if enabled.is_some() {
            let args: Vec<String> = enabled_reads.iter().map(|(i, _)| names[*i].clone()).collect();
            body.push("// §14.7.4: a `limit` whose condition does not hold does not run: PROPOSED = LIMITED = LAST".to_string());
            body.push(format!("if !Self::{name}_limit_enabled_block::<f64>({}) {{", args.join(", ")));
            body.extend(self.disabled_limit(f, last, &names)?.into_iter().map(|line| format!("    {line}")));
            body.push("}".to_string());
        }
        let dual: Vec<String> = reads
            .iter()
            .map(|(i, s)| {
                if *i < f.slots {
                    format!("Dual::<{}>::seeded({}, {i})", f.slots, names[*i])
                } else if self.is_plain(*s) {
                    names[*i].clone()
                } else {
                    format!("Dual::<{}>::constant({})", f.slots, names[*i])
                }
            })
            .collect();
        let plain: Vec<String> = reads.iter().map(|(i, _)| names[*i].clone()).collect();
        body.push("// PROPOSED: the input x and its partials, at the iterate".to_string());
        let ignored = if site.seed.is_some() { ", _, _" } else { ", _" };
        // A block that reads no real has no argument to infer `R` from: it is named (text of the
        // twins unchanged, which always have one).
        let (dual_type, plain_type) = if reads.iter().all(|(_, s)| self.is_plain(*s)) { (format!("::<Dual<{}>>", f.slots), "::<f64>".to_string()) } else { (String::new(), String::new()) };
        body.push(format!("let (proposed_{ignored}) = Self::{name}_limit_block{dual_type}({});", dual.join(", ")));
        body.push("// the function and the seed, over plain f64".to_string());
        if site.seed.is_some() {
            body.push(format!("let (_, f_, seed_) = Self::{name}_limit_block{plain_type}({});", plain.join(", ")));
        } else {
            body.push(format!("let (_, f_) = Self::{name}_limit_block{plain_type}({});", plain.join(", ")));
        }
        body.push(
            "// rule `the seed start`: nothing held (AGE 0), or in an equilibrium solve a value held from the last accepted solution (STATIC, AGE 1)"
                .to_string(),
        );
        body.push(format!("let from_seed_ = {age} == 0 || ({is_static} && {age} == 1);"));
        let start = if site.seed.is_some() { "seed_" } else { "proposed_.v" };
        body.push(format!("// rule `old`: LAST, or where the seed starts the {}", if site.seed.is_some() { "seed" } else { "input x (no seed)" }));
        body.push(format!("let old_ = if from_seed_ {{ {start} }} else {{ {last} }};"));
        body.push("let limited_ = f_(proposed_.v, old_);".to_string());
        if site.seed.is_some() {
            body.push("// rule `the seed kept`: starting from the seed, a limited value below it is raised to it".to_string());
            body.push("let limited_ = if from_seed_ && limited_ < seed_ { seed_ } else { limited_ };".to_string());
        } else {
            body.push("// rule `the seed kept`: there is no seed, so nothing is raised".to_string());
        }
        for (j, w) in f.writes.iter().enumerate() {
            match w {
                Target::Proposed(_) => body.push(format!("cx_.set({j}, proposed_.v);")),
                Target::Limited(_) => body.push(format!("cx_.set({j}, limited_);")),
                other => return Err(self.unsupported(&format!("the write {other:?} of limit {name}"))),
            }
        }
        for (p, (w, slot)) in f.partials.iter().enumerate() {
            body.push(format!("// partial {p}: d {} / d {}", writes[*w], names[*slot]));
            body.push(format!("if cx_.wanted({p}) {{\n    cx_.jac()[{p}] = proposed_.d[{slot}];\n}}"));
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

    use piperine_ir_fixtures::devices::{Devices, Ex};
    use crate::print::AnalogPrinter;

    /// Associated test helpers of `limit`, grouped on one fixture type.
    struct LimitFixture;

    impl LimitFixture {
        fn printed(ir: &Ir, body: BodyId, mode: fn(Mode) -> bool) -> String {
            let plan = BodyPlan::build(ir.body(body)).expect("the body plans");
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
            let local = |name: &str| Local { name: name.into(), ty: ValueType::F64, mutable: false };
            let body = b.body(BodySpec {
                name: "Hold".into(),
                origin: at(1),
                params: vec![],
                nodes: vec![port("p"), port("n")],
                branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
                limits: vec![Site { name: "y".into(), origin: at(3), enabled: None }],
                stims: vec![],
                locals: vec![local("new"), local("old")],
                analog: Some(vec![contribute]),
                children: vec![],
            });
            let top = Devices::alone(&mut b, body, "H", 1, vec![]);
            (b.finish(top), body)
        }
    }

    #[test]
    fn a_limit_function_reads_last_age_and_static() {
        let mut b = IrBuilder::new();
        let d = Devices::diode(&mut b);
        let top = Devices::alone(&mut b, d, "D", 1, vec![1e-14, 1.0, 0.0]);
        let ir = b.finish(top);
        // devices.msl:37-39: `let vd = limit(v(a, k), |new, old| pnjlim(new, old, nvt, vcrit), vcrit);`
        // The site reads its dependency set (a, k: slots 0 and 1; is, n, temperature_ through
        // nvt and vcrit), then LAST, AGE and STATIC (abi.md A2). `PROPOSED` is v(a, k) with its
        // partials; `LIMITED` is pnjlim(x, old) over plain f64, old by the rules of §18.
        let diode = "\
// Diode.analog, `limit` site `vd`: its input, its function and its seed @ devices.msl:39:17
fn vd_limit_block<R: Real>(a: R, k: R, is: R, n: R, temperature_: R, temperature_given_: bool) -> (R, impl Fn(R, R) -> R, R) {
    let nvt = n * vt(if temperature_given_ {
        temperature_
    } else {
        R::constant(300.15)
    });
    let vcrit = nvt * (nvt / (R::constant(2.0).sqrt() * is)).ln();
    (a - k, move |new, old| pnjlim(new, old, nvt, vcrit), vcrit)
}

// function 0 `limit:vd`: reads a, k, is, n, temperature_, temperature_given_, vd_last_, vd_age_, is_static_; writes vd.PROPOSED, vd.LIMITED @ devices.msl:39:17
fn vd_limit(cx_: &mut Eval<'_>) -> Result<(), Fault> {
    let a = cx_.f64(0);
    let k = cx_.f64(1);
    let is = cx_.f64(2);
    let n = cx_.f64(3);
    let temperature_ = cx_.f64(4);
    let temperature_given_ = cx_.bool(5);
    let vd_last_ = cx_.f64(6);
    let vd_age_ = cx_.u8(7);
    let is_static_ = cx_.bool(8);
    // PROPOSED: the input x and its partials, at the iterate
    let (proposed_, _, _) = Self::vd_limit_block(Dual::<2>::seeded(a, 0), Dual::<2>::seeded(k, 1), Dual::<2>::constant(is), Dual::<2>::constant(n), Dual::<2>::constant(temperature_), temperature_given_);
    // the function and the seed, over plain f64
    let (_, f_, seed_) = Self::vd_limit_block(a, k, is, n, temperature_, temperature_given_);
    // rule `the seed start`: nothing held (AGE 0), or in an equilibrium solve a value held from the last accepted solution (STATIC, AGE 1)
    let from_seed_ = vd_age_ == 0 || (is_static_ && vd_age_ == 1);
    // rule `old`: LAST, or where the seed starts the seed
    let old_ = if from_seed_ { seed_ } else { vd_last_ };
    let limited_ = f_(proposed_.v, old_);
    // rule `the seed kept`: starting from the seed, a limited value below it is raised to it
    let limited_ = if from_seed_ && limited_ < seed_ { seed_ } else { limited_ };
    cx_.set(0, proposed_.v);
    cx_.set(1, limited_);
    // partial 0: d vd.PROPOSED / d a
    if cx_.wanted(0) {
        cx_.jac()[0] = proposed_.d[0];
    }
    // partial 1: d vd.PROPOSED / d k
    if cx_.wanted(1) {
        cx_.jac()[1] = proposed_.d[1];
    }
    Ok(())
}";
        assert_eq!(LimitFixture::printed(&ir, d, |m| matches!(m, Mode::Limit(_))), diode);

        // With no seed, `old` starts from the input x and nothing is raised; the closure ignores
        // `new`, which prints as `_` (an MSL name could spell `_new`).
        let (ir, body) = LimitFixture::hold();
        let text = LimitFixture::printed(&ir, body, |m| matches!(m, Mode::Limit(_)));
        assert!(text.contains("fn y_limit_block<R: Real>(p: R, n: R) -> (R, impl Fn(R, R) -> R) {"), "{text}");
        assert!(text.contains("    (p - n, move |_, old| old)\n"), "{text}");
        assert!(text.contains("let old_ = if from_seed_ { proposed_.v } else { y_last_ };"), "{text}");
        assert!(text.contains("let y_last_ = cx_.f64(2);\n    let y_age_ = cx_.u8(3);\n    let is_static_ = cx_.bool(4);"), "{text}");
        assert!(!text.contains("limited_ < seed_"), "{text}");
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
        // §18 rule 3, abi.md A2: the terms read the site's held value `vd_limit_`, never its input
        // v(a, k), its function or its seed; those are the limit function's
        let terms = LimitFixture::printed(&ir, d, |m| m == Mode::Terms);
        assert!(terms.contains("let vd = vd_limit_;"), "{terms}");
        assert!(terms.contains("let vd_limit_ = cx_.f64(0);"), "{terms}");
        for absent in ["a - k", "cx_.f64(7)", "pnjlim", "vcrit", "LAST", "_last", "_age", "is_static_"] {
            assert!(!terms.contains(absent), "`{absent}` in the terms function:\n{terms}");
        }
        let limit = LimitFixture::printed(&ir, d, |m| matches!(m, Mode::Limit(_)));
        assert!(limit.contains("pnjlim(new, old, nvt, vcrit)") && limit.contains("(a - k, "), "{limit}");

        // the seedless `Hold`: its terms are the held value alone
        let (ir, body) = LimitFixture::hold();
        let terms = LimitFixture::printed(&ir, body, |m| m == Mode::Terms);
        assert!(terms.contains("fn terms_block<R: Real>(y_limit_: R) -> [R; 1] {"), "{terms}");
        assert!(terms.contains("let b0_flow_ = y_limit_;"), "{terms}");
    }
}
