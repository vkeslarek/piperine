//! `AnalogPrinter`: the functions of a body's shape that run its `analog` block (design "From an
//! IR function to a Rust function").
//!
//! The `terms` function is two Rust functions. `terms_block<R: Real>` is the backward slice of the
//! block over its contributions: the top-level `let`s the contributions reach, in source order,
//! each contribution added to its branch's sums, and the terms as an array, in the plan's write
//! order. It takes the function's reads that the slice uses, in the plan's read order. `terms`
//! reads those cells, runs `terms_block` at the iterate, at every continuous pin read as 0, or
//! both, as the plan's `affine` flags ask (the interpreter's `Interpreted::terms`), and writes:
//! a term that is not affine as its value at the iterate; an affine term as
//! `c + (0.0 + g0 * x0 + g1 * x1 + …)`, `c` and the pin partials `g` from the evaluation at 0,
//! folded from `0.0` over its pin partials in declared order, the fold the assembly subtracts
//! (INT-12). A partial of an affine term against a pin comes from the evaluation at 0; every
//! other partial from the evaluation at the iterate.

use std::collections::BTreeSet;

use piperine_ir::{Body, BranchKind, Env, Expr, ExprId, Ir, Operator, Origin, QuantityRole, Stmt, StmtId, Terminal, ValueType};
use piperine_plan::{BodyPlan, FunctionPlan, Mode, Source, Target, Term};

use crate::Error;
use crate::print::value::Reads;
use crate::print::{Text, ValuePrinter};

/// Prints the analog functions of one body from its plan.
#[derive(Debug)]
pub struct AnalogPrinter<'a> {
    pub(crate) ir: &'a Ir,
    pub(crate) body: Body<'a>,
    pub(crate) plan: &'a BodyPlan,
    pub(crate) values: ValuePrinter<'a>,
}

/// What a slice of the `analog` block holds: its top-level statements in source order, and the
/// reads its expressions use.
#[derive(Debug, Default)]
pub struct Slice {
    pub stmts: Vec<StmtId>,
    pub sources: BTreeSet<Source>,
}

impl<'a> AnalogPrinter<'a> {
    pub fn new(body: Body<'a>, plan: &'a BodyPlan) -> AnalogPrinter<'a> {
        let reads = Reads {
            nodes: body.nodes().iter().map(|n| Text::ident(&n.name)).collect(),
            limits: body.limits().iter().map(|s| Text::generated(&format!("{}_limit", s.name))).collect(),
            stims: body.stims().iter().map(|s| Text::generated(&format!("{}_stim", s.name))).collect(),
            carriers: body.carriers().iter().map(|c| Text::ident(&c.name)).collect(),
            flows: (0..body.branches().len() as u32).map(|b| Self::flow_name(b)).collect(),
            previous: (0..body.carriers().len() as u32).map(|c| Self::carrier_part(body, c, "previous")).collect(),
        };
        let values = ValuePrinter::of_body(body.ir(), body).with_reads(reads);
        AnalogPrinter { ir: body.ir(), body, plan, values }
    }

    /// The value printer of the body, its analog reads named.
    pub fn values(&self) -> &ValuePrinter<'a> {
        &self.values
    }

    /// The variable a read of a function is bound to: a node's or a param's MSL name
    /// (`Text::ident`); for every other read a generated name, the site's MSL name first
    /// (`vd_limit_`, `vd_last_`, `vd_age_`, `ac_stim_`, `b0_flow_`, `is_static_`).
    pub fn read_name(&self, s: Source) -> String {
        let site = |list: &[piperine_ir::Site], k: u32, role: &str| {
            let name = list.get(k as usize).map_or_else(|| format!("site{k}"), |x| x.name.clone());
            Text::generated(&format!("{name}_{role}"))
        };
        match s {
            Source::Node(i) => {
                self.body.nodes().get(i as usize).map_or_else(|| Text::generated(&format!("node{i}")), |n| Text::ident(&n.name))
            }
            Source::BranchFlow(b) => Self::flow_name(b),
            Source::LimitValue(k) => site(self.body.limits(), k, "limit"),
            Source::StimValue(k) => site(self.body.stims(), k, "stim"),
            Source::Param(p) => {
                self.body.params().get(p as usize).map_or_else(|| Text::generated(&format!("param{p}")), |x| Text::ident(&x.name))
            }
            Source::Carrier(c) => {
                self.body.carriers().get(c as usize).map_or_else(|| Text::generated(&format!("carrier{c}")), |x| Text::ident(&x.name))
            }
            Source::CarrierChanged(c) => Self::carrier_part(self.body, c, "changed"),
            Source::CarrierPrevious(c) => Self::carrier_part(self.body, c, "previous"),
            Source::Delayed(k) | Source::DelayedChanged(k) => {
                let carrier = self.body.drives().get(k as usize).map_or(u32::MAX, |d| d.carrier);
                let name = self.body.carriers().get(carrier as usize).map_or_else(|| format!("drive{k}"), |x| x.name.clone());
                let part = if matches!(s, Source::Delayed(_)) { "now" } else { "now_changed" };
                Text::generated(&format!("{name}_{part}"))
            }
            Source::Crossed(k) => Text::generated(&format!("cross{k}_changed")),
            Source::Timer(k) => Text::generated(&format!("at{k}_changed")),
            Source::Temperature => ValuePrinter::env_name(Env::Temperature).to_string(),
            Source::Gmin => ValuePrinter::env_name(Env::Gmin).to_string(),
            Source::SourceScale => ValuePrinter::env_name(Env::SourceScale).to_string(),
            Source::LimitLast(k) => site(self.body.limits(), k, "last"),
            Source::LimitAge(k) => site(self.body.limits(), k, "age"),
            Source::Static => Text::generated("is_static"),
            Source::Time => Text::generated("time"),
        }
    }

    /// The variable of a part of carrier `c`'s pin: `clk_changed_`, `clk_previous_`.
    pub(crate) fn carrier_part(body: Body<'_>, c: u32, part: &str) -> String {
        let name = body.carriers().get(c as usize).map_or_else(|| format!("carrier{c}"), |x| x.name.clone());
        Text::generated(&format!("{name}_{part}"))
    }

    /// The variable of branch `b`'s flow pin: `b0_flow_`.
    fn flow_name(b: u32) -> String {
        Text::generated(&format!("b{b}_flow"))
    }

    /// `file:line:col` of function `k` of the shape.
    pub(crate) fn function_origin(&self, k: usize) -> String {
        self.plan.shape().shape().functions.get(k).map_or_else(String::new, |f| f.origin.as_str().to_string())
    }

    /// The name of term `t` in the shape: `b0.flow`, `b0.eq`, `b0.charge`.
    pub fn term_name(t: Term) -> String {
        match t {
            Term::Flow(b) | Term::Current(b) => format!("b{b}.flow"),
            Term::Eq(b) => format!("b{b}.eq"),
            Term::Charge(b) => format!("b{b}.charge"),
        }
    }

    /// The name of a write of a function, as the shape names its cell.
    pub fn write_name(&self, t: Target) -> String {
        let site = |list: &[piperine_ir::Site], k: u32| list.get(k as usize).map_or_else(|| k.to_string(), |x| x.name.clone());
        match t {
            Target::Term(term) => Self::term_name(term),
            Target::Proposed(k) => format!("{}.PROPOSED", site(self.body.limits(), k)),
            Target::Limited(k) => format!("{}.LIMITED", site(self.body.limits(), k)),
            Target::StimRe(k) => format!("{}.RE", site(self.body.stims(), k)),
            Target::StimIm(k) => format!("{}.IM", site(self.body.stims(), k)),
            Target::Check(p) => {
                format!("check:{}", self.body.params().get(p as usize).map_or_else(|| p.to_string(), |x| x.name.clone()))
            }
            Target::Carrier(c) => self.body.carriers().get(c as usize).map_or_else(|| c.to_string(), |x| x.name.clone()),
            Target::EventPin(k) => format!("event:{k}"),
            Target::Delayed(k) | Target::DelayedEventPin(k) => {
                let carrier = self.body.drives().get(k as usize).map_or(u32::MAX, |d| d.carrier);
                let name = self.body.carriers().get(carrier as usize).map_or_else(|| k.to_string(), |x| x.name.clone());
                let part = if matches!(t, Target::Delayed(_)) { "now" } else { "changed" };
                format!("drive:{name}.{part}")
            }
            Target::Watch(k) => format!("cross:{k}"),
            Target::Timer(k) => format!("at:{k}"),
            Target::Rearm { event, carrier } => {
                format!("at:{event}.{}", self.body.carriers().get(carrier as usize).map_or_else(|| carrier.to_string(), |x| x.name.clone()))
            }
            Target::Record(r) => self.body.streams().get(r as usize).map_or_else(|| format!("stream {r}"), |x| x.name.clone()),
        }
    }

    /// The backward slice of the `analog` block from the top-level statements `roots` keeps:
    /// those statements and every top-level `let` they reach, in source order. In `mode`, the
    /// arguments of the mode's own site are evaluated (`Limit(k)`, `Stim(k)`); every other site
    /// reads its held value and evaluates nothing.
    pub fn slice(&self, mode: Mode, roots: impl Fn(&Stmt) -> bool) -> Slice {
        let block = self.body.analog().unwrap_or(&[]);
        self.slice_before(mode, &[], block.len(), roots)
    }

    /// The top-level `let`s before statement `before` of the block that `exprs` reach, in
    /// source order, and the reads they and `exprs` use: the slice of a site's own arguments.
    pub fn slice_of(&self, mode: Mode, exprs: &[ExprId], before: usize) -> Slice {
        self.slice_before(mode, exprs, before, |_| false)
    }

    fn slice_before(&self, mode: Mode, exprs: &[ExprId], before: usize, roots: impl Fn(&Stmt) -> bool) -> Slice {
        let block = self.body.analog().unwrap_or(&[]);
        let mut needed = BTreeSet::new();
        let mut kept = Vec::new();
        let mut sources = BTreeSet::new();
        for e in exprs {
            self.reads(*e, mode, &mut needed, &mut sources);
        }
        for id in block[..before.min(block.len())].iter().rev() {
            let stmt = self.ir.stmt(*id);
            let keep = match stmt {
                Stmt::Let { local, .. } => needed.contains(local),
                _ => roots(stmt),
            };
            if !keep {
                continue;
            }
            for e in stmt.operands() {
                self.reads(e, mode, &mut needed, &mut sources);
            }
            kept.push(*id);
        }
        kept.reverse();
        Slice { stmts: kept, sources }
    }

    /// The locals and the reads `e` uses in `mode`.
    fn reads(&self, e: ExprId, mode: Mode, locals: &mut BTreeSet<u32>, sources: &mut BTreeSet<Source>) {
        let mut stack = vec![e];
        while let Some(e) = stack.pop() {
            match self.ir.expr(e) {
                Expr::Local(i) => {
                    locals.insert(*i);
                }
                Expr::Param(p) => {
                    sources.insert(Source::Param(*p));
                }
                Expr::Env(env) => sources.extend(Self::env_source(*env)),
                Expr::Probe { plus, minus } => {
                    for t in [plus, minus] {
                        if let Terminal::Node(i) = t {
                            sources.insert(Source::Node(*i));
                        }
                    }
                }
                Expr::Limit { site, x, f, seed } => {
                    if mode == Mode::Limit(*site) {
                        stack.extend([*x, *f].into_iter().chain(*seed));
                    } else {
                        sources.insert(Source::LimitValue(*site));
                    }
                }
                Expr::Stim { site, mag, phase } => {
                    if mode == Mode::Stim(*site) {
                        stack.extend([*mag].into_iter().chain(*phase));
                    } else {
                        sources.insert(Source::StimValue(*site));
                    }
                }
                Expr::Call { function, args } => {
                    sources.extend(ValuePrinter::env_reads(self.ir, *function).into_iter().filter_map(Self::env_source));
                    stack.extend(args);
                }
                Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::IsKnown(expr) | Expr::High(expr) => stack.push(*expr),
                Expr::KnownOr { value, default } => stack.extend([*value, *default]),
                Expr::Method { receiver, args, .. } => stack.extend([*receiver].into_iter().chain(args.iter().copied())),
                Expr::Binary { lhs, rhs, .. } => stack.extend([*lhs, *rhs]),
                Expr::If { cond, then, otherwise } => stack.extend([*cond, *then, *otherwise]),
                Expr::Block { stmts, value } => {
                    for s in stmts {
                        stack.extend(self.ir.stmt(*s).operands());
                    }
                    stack.extend(value);
                }
                Expr::Math { args, .. } => stack.extend(args),
                Expr::Closure { body, .. } => stack.push(*body),
                Expr::Carrier(c) => {
                    sources.insert(Source::Carrier(*c));
                }
                Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Int(_) | Expr::Unknown => {}
                Expr::Flow { branch } => {
                    sources.insert(Source::BranchFlow(*branch));
                }
                Expr::Previous(c) => {
                    sources.insert(Source::CarrierPrevious(*c));
                }
            }
        }
    }

    /// The plan read of an environment read.
    fn env_source(env: Env) -> Option<Source> {
        Some(match env {
            Env::Temperature => Source::Temperature,
            Env::Gmin => Source::Gmin,
            Env::SourceScale => Source::SourceScale,
            Env::Time => Source::Time,
            Env::Static => Source::Static,
        })
    }

    /// The Rust type a read is bound to in generic code: a `bool` param stays `bool`.
    pub(crate) fn is_bool(&self, s: Source) -> bool {
        matches!(s, Source::Param(p) if self.body.params().get(p as usize).is_some_and(|x| x.ty == ValueType::Bool))
    }

    /// Whether a generic block takes read `s` as it is rather than as a `Real` (so it is no
    /// `Dual`): a bool or integer param, a carrier, `analysis(Static)`.
    pub(crate) fn is_plain(&self, s: Source) -> bool {
        match s {
            Source::Param(p) => self.body.params().get(p as usize).is_some_and(|x| x.ty != ValueType::F64),
            Source::Carrier(_) | Source::CarrierPrevious(_) | Source::Static => true,
            _ => false,
        }
    }

    /// The type of read `s` in a generic block: `R`, `bool` (a `bool` param, `analysis(Static)`),
    /// an integer, or for a carrier the `rt::Flagged<T>` its pin decodes to.
    pub(crate) fn block_type(&self, s: Source) -> Result<String, Error> {
        match s {
            Source::Param(p) => match self.body.params().get(p as usize).map(|x| &x.ty) {
                Some(ty) if *ty != ValueType::F64 => self.values.type_of(ty),
                _ => Ok("R".to_string()),
            },
            Source::Carrier(c) | Source::CarrierPrevious(c) => match self.body.carriers().get(c as usize) {
                Some(carrier) => Ok(format!("rt::Flagged<{}>", self.values.pin_type(&carrier.ty)?)),
                None => Err(self.unsupported(&format!("a read of carrier {c}"))),
            },
            Source::Static => Ok("bool".to_string()),
            _ => Ok("R".to_string()),
        }
    }

    /// The `let` that binds read `i` of a function from its lens `cx`: a real as an `f64`, a
    /// `bool` param as a `bool`, an integer param decoded (an unwritten one is 0), a carrier's
    /// pin decoded into a value that may be unknown.
    pub(crate) fn decode(&self, cx: &str, i: usize, s: Source) -> Result<String, Error> {
        let name = self.read_name(s);
        Ok(match s {
            Source::Param(p) => match self.body.params().get(p as usize).map(|x| &x.ty) {
                Some(ValueType::Bool) => format!("let {name} = {cx}.bool({i});"),
                Some(ty @ (ValueType::UInt(_) | ValueType::Int(_))) => format!(
                    "let {name} = rt::Flagged::<{}>::decode({cx}.bytes({i})).known_or({});",
                    self.values.pin_type(ty)?,
                    ValuePrinter::zero(ty)
                ),
                _ => format!("let {name} = {cx}.f64({i});"),
            },
            Source::Carrier(c) | Source::CarrierPrevious(c) => match self.body.carriers().get(c as usize) {
                Some(carrier) => format!("let {name} = rt::Flagged::<{}>::decode({cx}.bytes({i}));", self.values.pin_type(&carrier.ty)?),
                None => return Err(self.unsupported(&format!("a read of carrier {c}"))),
            },
            // abi.md §4.2: `STATIC` is one `BOOL` byte, `TIME` an `F64`
            Source::Static => format!("let {name} = {cx}.bool({i});"),
            _ => format!("let {name} = {cx}.f64({i});"),
        })
    }

    /// `i(a, k)` or `v(p, n)`: what a contribution to `branch` writes, with its nature's access.
    fn branch_text(&self, branch: u32, quantity: QuantityRole) -> String {
        let Some(b) = self.body.branches().get(branch as usize) else { return format!("branch {branch}") };
        let node = |t: Terminal| match t {
            Terminal::Node(i) => self.body.nodes().get(i as usize),
            Terminal::Ground => None,
        };
        let nature = node(b.plus).or_else(|| node(b.minus)).map(|n| self.ir.nature(n.nature));
        let access = nature.map_or_else(
            || if quantity == QuantityRole::Flow { "i".to_string() } else { "v".to_string() },
            |n| match quantity {
                QuantityRole::Flow => n.flow().map_or_else(|| "flow".to_string(), |q| q.access.clone()),
                QuantityRole::Potential => n.potential().access.clone(),
            },
        );
        let name = |t: Terminal| node(t).map_or_else(|| "gnd".to_string(), |n| n.name.clone());
        match b.minus {
            Terminal::Ground => format!("{access}({})", name(b.plus)),
            _ => format!("{access}({}, {})", name(b.plus), name(b.minus)),
        }
    }

    /// The variable of a branch's direct sum: `b0_flow_` on a flow branch, `b0_potential_` on a
    /// potential branch (whose `b0_flow_` is its flow pin).
    fn direct_sum(&self, branch: u32) -> String {
        match self.body.branches().get(branch as usize).map(|b| b.kind) {
            Some(BranchKind::Potential) => Text::generated(&format!("b{branch}_potential")),
            _ => Text::generated(&format!("b{branch}_flow")),
        }
    }

    /// The variable of a branch's charge sum: `b0_charge_`.
    fn charge_sum(branch: u32) -> String {
        Text::generated(&format!("b{branch}_charge"))
    }

    /// `terms_block` and `terms`, for function `k` of the plan, which runs `Mode::Terms`.
    pub fn terms(&self, k: usize) -> Result<String, Error> {
        let Some(f) = self.plan.functions().get(k).filter(|f| f.mode == Mode::Terms) else {
            return Err(self.unsupported(&format!("function {k} as the terms function")));
        };
        let slice = self.slice(Mode::Terms, |s| matches!(s, Stmt::Contribute { .. }));
        let mut used = slice.sources.clone();
        for w in &f.writes {
            match w {
                Target::Term(Term::Current(b)) => {
                    used.insert(Source::BranchFlow(*b));
                }
                Target::Term(Term::Eq(b)) => {
                    if let Some(branch) = self.body.branches().get(*b as usize) {
                        for t in [branch.plus, branch.minus] {
                            if let Terminal::Node(i) = t {
                                used.insert(Source::Node(i));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        // The reads the slice uses, in the plan's order, each with its index in the plan.
        let reads: Vec<(usize, Source)> = f.reads.iter().copied().enumerate().filter(|(_, s)| used.contains(s)).collect();
        let mut t = Text::new();
        self.terms_block(&mut t, f, &slice, &reads)?;
        t.line("");
        self.terms_function(&mut t, k, f, &reads)?;
        let mut text = t.finish();
        text.pop();
        Ok(text)
    }

    fn terms_block(&self, t: &mut Text, f: &FunctionPlan, slice: &Slice, reads: &[(usize, Source)]) -> Result<(), Error> {
        let mut params: Vec<String> = Vec::with_capacity(reads.len());
        for (_, s) in reads {
            params.push(format!("{}: {}", self.read_name(*s), self.block_type(*s)?));
        }
        t.comment(&format!("{}.analog", self.body.name()), self.body.origin());
        t.line(&format!("fn terms_block<R: Real>({}) -> [R; {}] {{", params.join(", "), f.writes.len()));
        let mut lines = Vec::new();
        let mut defined: BTreeSet<String> = BTreeSet::new();
        for id in &slice.stmts {
            match self.ir.stmt(*id) {
                Stmt::Contribute { branch, quantity, addends } => {
                    if let Some(first) = addends.first() {
                        let at = self.ir.expr_origin(first.value);
                        lines.push(format!(
                            "// contribution to {}, branch b{branch} @ {}:{}:{}",
                            self.branch_text(*branch, *quantity),
                            at.file,
                            at.line,
                            at.col
                        ));
                    }
                    for a in addends {
                        let sum = match a.operator {
                            Operator::Direct => self.direct_sum(*branch),
                            Operator::Ddt => Self::charge_sum(*branch),
                        };
                        if defined.insert(sum.clone()) {
                            let value = if a.negated { self.values.negated(a.value)? } else { self.values.expr(a.value)? };
                            lines.push(format!("let {sum} = {value};"));
                        } else {
                            let value = if a.negated { self.values.negated(a.value)? } else { self.values.right_of_add(a.value)? };
                            lines.push(format!("let {sum} = {sum} + {value};"));
                        }
                    }
                }
                _ => lines.push(self.values.stmt(*id)?),
            }
        }
        let mut outputs = Vec::with_capacity(f.writes.len());
        for w in &f.writes {
            let Target::Term(term) = *w else {
                return Err(self.unsupported(&format!("the write {w:?} of the terms function")));
            };
            outputs.push(match term {
                Term::Flow(b) => self.direct_sum(b),
                Term::Charge(b) => {
                    let potential = self.body.branches().get(b as usize).is_some_and(|x| x.kind == BranchKind::Potential);
                    if potential { format!("-{}", Self::charge_sum(b)) } else { Self::charge_sum(b) }
                }
                Term::Current(b) => self.read_name(Source::BranchFlow(b)),
                Term::Eq(b) => {
                    let Some(branch) = self.body.branches().get(b as usize) else {
                        return Err(self.unsupported(&format!("the equation term of branch {b}")));
                    };
                    let Some(probe) = self.values.probe(branch.plus, branch.minus) else {
                        return Err(self.unsupported(&format!("the probe of branch {b}")));
                    };
                    if defined.contains(&self.direct_sum(b)) { format!("{probe} - {}", self.direct_sum(b)) } else { probe }
                }
            });
        }
        lines.push(format!("[{}]", outputs.join(", ")));
        t.indented(|t| {
            for l in &lines {
                t.line(l);
            }
        });
        t.line("}");
        Ok(())
    }

    fn terms_function(&self, t: &mut Text, k: usize, f: &FunctionPlan, reads: &[(usize, Source)]) -> Result<(), Error> {
        let pin = |slot: usize| f.reads.get(slot).is_some_and(|r| r.is_pin());
        let affine = |w: usize| f.affine.get(w).copied().unwrap_or(false);
        let at_iterate = (0..f.writes.len()).any(|w| !affine(w)) || f.partials.iter().any(|(w, slot)| affine(*w) && !pin(*slot));
        let at_zero = f.affine.iter().any(|a| *a);
        let names: Vec<String> = f.reads.iter().map(|s| self.read_name(*s)).collect();
        let writes: Vec<String> = f.writes.iter().map(|w| self.write_name(*w)).collect();
        t.line(&format!(
            "// function {k} `terms`: reads {}; writes {} @ {}",
            names.join(", "),
            writes.join(", "),
            self.function_origin(k)
        ));
        t.line("fn terms(cx_: &mut Eval<'_>) -> Result<(), Fault> {");
        let mut binds = Vec::with_capacity(reads.len());
        for (i, s) in reads {
            binds.push(self.decode("cx_", *i, *s)?);
        }
        t.indented(|t| {
            for line in &binds {
                t.line(line);
            }
            let call = |zero: bool| {
                let args: Vec<String> = reads
                    .iter()
                    .map(|(i, s)| {
                        let name = &names[*i];
                        if *i < f.slots {
                            let v = if zero && s.is_pin() { "0.0" } else { name.as_str() };
                            format!("Dual::<{}>::seeded({v}, {i})", f.slots)
                        } else if self.is_plain(*s) {
                            name.clone()
                        } else {
                            format!("Dual::<{}>::constant({name})", f.slots)
                        }
                    })
                    .collect();
                format!("Self::terms_block({})", args.join(", "))
            };
            if at_iterate {
                t.line("// the evaluation at the iterate");
                t.line(&format!("let at_iterate_ = {};", call(false)));
            }
            if at_zero {
                t.line("// the evaluation with every continuous pin read as 0 (the affine rule)");
                t.line(&format!("let at_zero_ = {};", call(true)));
            }
            for (w, name) in writes.iter().enumerate() {
                if affine(w) {
                    let fold: Vec<String> = f
                        .partials
                        .iter()
                        .filter(|(pw, slot)| *pw == w && pin(*slot))
                        .map(|(_, slot)| format!(" + at_zero_[{w}].d[{slot}] * {}", names[*slot]))
                        .collect();
                    t.line(&format!("// term {w} `{name}`: affine, c + Σ g·x from the evaluation at 0, folded from 0.0"));
                    if fold.is_empty() {
                        t.line(&format!("cx_.set({w}, at_zero_[{w}].v + 0.0);"));
                    } else {
                        t.line(&format!("cx_.set({w}, at_zero_[{w}].v + (0.0{}));", fold.concat()));
                    }
                } else {
                    t.line(&format!("// term {w} `{name}`: its value at the iterate"));
                    t.line(&format!("cx_.set({w}, at_iterate_[{w}].v);"));
                }
            }
            for (p, (w, slot)) in f.partials.iter().enumerate() {
                let from = if affine(*w) && pin(*slot) { "at_zero_" } else { "at_iterate_" };
                t.line(&format!("// partial {p}: d {} / d {}", writes[*w], names[*slot]));
                t.line(&format!("if cx_.wanted({p}) {{"));
                t.indented(|t| t.line(&format!("cx_.jac()[{p}] = {from}[{w}].d[{slot}];")));
                t.line("}");
            }
            t.line("Ok(())");
        });
        t.line("}");
        Ok(())
    }

    /// Refuses the function of a `limit` or `ac_stim` site (`what`, named `name`, at `origin`)
    /// that reads a carrier: no body of the buck slice has one and no lowering decodes a carrier
    /// there (verification F-B6).
    pub(crate) fn refuse_carrier_reads(&self, what: &str, name: &str, origin: &Origin, reads: &[Source]) -> Result<(), Error> {
        let carrier = reads.iter().find_map(|s| match *s {
            Source::Carrier(c) | Source::CarrierChanged(c) => Some(c),
            Source::Delayed(k) | Source::DelayedChanged(k) => self.body.drives().get(k as usize).map(|d| d.carrier),
            _ => None,
        });
        match carrier {
            Some(c) => Err(Error::Unsupported {
                form: format!(
                    "a `{what}` function reading the carrier `{}`",
                    self.body.carriers().get(c as usize).map_or_else(|| format!("carrier{c}"), |x| x.name.clone())
                ),
                path: format!("body {} {what} {name}", self.body.name()),
                origin: origin.clone(),
            }),
            None => Ok(()),
        }
    }

    pub(crate) fn unsupported(&self, form: &str) -> Error {
        Error::Unsupported { form: form.to_string(), path: format!("body {}", self.body.name()), origin: self.body.origin().clone() }
    }
}

#[cfg(test)]
mod tests {
    use piperine_ir::{BodyId, Ir, IrBuilder};
    use piperine_plan::{BodyPlan, Mode};

    use super::AnalogPrinter;
    use crate::devices::Devices;

    /// The five M1 bodies in one `Ir`.
    fn library() -> (Ir, [BodyId; 5]) {
        let mut b = IrBuilder::new();
        let r = Devices::resistor(&mut b);
        let c = Devices::capacitor(&mut b);
        let v = Devices::vsource(&mut b);
        let i = Devices::isource(&mut b);
        let d = Devices::diode(&mut b);
        let top = Devices::alone(&mut b, r, "R", 1, vec![1000.0]);
        (b.finish(top), [r, c, v, i, d])
    }

    /// The text of the `terms` function of `body`.
    fn terms(ir: &Ir, body: BodyId) -> String {
        let plan = BodyPlan::new(ir.body(body));
        let k = plan.functions().iter().position(|f| f.mode == Mode::Terms).expect("a terms function");
        AnalogPrinter::new(ir.body(body), &plan).terms(k).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn an_affine_term_is_printed_as_constant_plus_fold() {
        let (ir, [r, c, ..]) = library();
        // Functions run limit sites, stimuli, the terms, then the checks: a body with neither site
        // has `terms` at index 0.
        // `i(p, n) +> v(p, n) / r` (devices.msl:8): reads p, n (the pins, slots 0 and 1), r; the
        // term is affine in the pins, so it is evaluated once, with the pins read as 0, and
        // written as c + (0.0 + g0 * p + g1 * n)
        let resistor = "\
// Resistor.analog @ devices.msl:5:1
fn terms_block<R: Real>(p: R, n: R, r: R) -> [R; 1] {
    // contribution to i(p, n), branch b0 @ devices.msl:8:32
    let b0_flow_ = (p - n) / r;
    [b0_flow_]
}

// function 0 `terms`: reads p, n, r; writes b0.flow @ devices.msl:5:1
fn terms(cx_: &mut Eval<'_>) -> Result<(), Fault> {
    let p = cx_.f64(0);
    let n = cx_.f64(1);
    let r = cx_.f64(2);
    // the evaluation with every continuous pin read as 0 (the affine rule)
    let at_zero_ = Self::terms_block(Dual::<2>::seeded(0.0, 0), Dual::<2>::seeded(0.0, 1), Dual::<2>::constant(r));
    // term 0 `b0.flow`: affine, c + Σ g·x from the evaluation at 0, folded from 0.0
    cx_.set(0, at_zero_[0].v + (0.0 + at_zero_[0].d[0] * p + at_zero_[0].d[1] * n));
    // partial 0: d b0.flow / d p
    if cx_.wanted(0) {
        cx_.jac()[0] = at_zero_[0].d[0];
    }
    // partial 1: d b0.flow / d n
    if cx_.wanted(1) {
        cx_.jac()[1] = at_zero_[0].d[1];
    }
    Ok(())
}";
        assert_eq!(terms(&ir, r), resistor);

        // `i(p, n) +> ddt(c * v(p, n))` (devices.msl:14): a charge is never affine, so it is
        // written as its value at the iterate
        let capacitor = "\
// Capacitor.analog @ devices.msl:11:1
fn terms_block<R: Real>(p: R, n: R, c: R) -> [R; 1] {
    // contribution to i(p, n), branch b0 @ devices.msl:14:40
    let b0_charge_ = c * (p - n);
    [b0_charge_]
}

// function 0 `terms`: reads p, n, c; writes b0.charge @ devices.msl:11:1
fn terms(cx_: &mut Eval<'_>) -> Result<(), Fault> {
    let p = cx_.f64(0);
    let n = cx_.f64(1);
    let c = cx_.f64(2);
    // the evaluation at the iterate
    let at_iterate_ = Self::terms_block(Dual::<2>::seeded(p, 0), Dual::<2>::seeded(n, 1), Dual::<2>::constant(c));
    // term 0 `b0.charge`: its value at the iterate
    cx_.set(0, at_iterate_[0].v);
    // partial 0: d b0.charge / d p
    if cx_.wanted(0) {
        cx_.jac()[0] = at_iterate_[0].d[0];
    }
    // partial 1: d b0.charge / d n
    if cx_.wanted(1) {
        cx_.jac()[1] = at_iterate_[0].d[1];
    }
    Ok(())
}";
        assert_eq!(terms(&ir, c), capacitor);
    }

    #[test]
    fn a_slice_holds_only_what_its_outputs_reach() {
        let (ir, [.., d]) = library();
        // devices.msl:36-42. The terms read the limited `vd` (the site's held value, slot 0), not
        // `v(a, k)`; `vcrit` reaches only the `limit` site's seed and closure, so the terms' slice
        // has no `let vcrit`, and `limit(…)` is the held value `vd_limit_`. Reads in the plan's
        // order: the limit value, the params is, n, cj0, then temperature_ (through `vt()`) and
        // gmin_. Neither term is affine (`exp` of the limited value; a charge).
        let diode = "\
// Diode.analog @ devices.msl:31:1
fn terms_block<R: Real>(vd_limit_: R, is: R, n: R, cj0: R, temperature_: R, gmin_: R) -> [R; 2] {
    let nvt = n * vt(temperature_);
    let vd = vd_limit_;
    let id = is * ((vd / nvt).exp() - R::constant(1.0));
    // contribution to i(a, k), branch b0 @ devices.msl:41:17
    let b0_flow_ = id + gmin_ * vd;
    let b0_charge_ = cj0 * vd;
    [b0_flow_, b0_charge_]
}

// function 1 `terms`: reads vd_limit_, is, n, cj0, temperature_, gmin_; writes b0.flow, b0.charge @ devices.msl:31:1
fn terms(cx_: &mut Eval<'_>) -> Result<(), Fault> {
    let vd_limit_ = cx_.f64(0);
    let is = cx_.f64(1);
    let n = cx_.f64(2);
    let cj0 = cx_.f64(3);
    let temperature_ = cx_.f64(4);
    let gmin_ = cx_.f64(5);
    // the evaluation at the iterate
    let at_iterate_ = Self::terms_block(Dual::<1>::seeded(vd_limit_, 0), Dual::<1>::constant(is), Dual::<1>::constant(n), Dual::<1>::constant(cj0), Dual::<1>::constant(temperature_), Dual::<1>::constant(gmin_));
    // term 0 `b0.flow`: its value at the iterate
    cx_.set(0, at_iterate_[0].v);
    // term 1 `b0.charge`: its value at the iterate
    cx_.set(1, at_iterate_[1].v);
    // partial 0: d b0.flow / d vd_limit_
    if cx_.wanted(0) {
        cx_.jac()[0] = at_iterate_[0].d[0];
    }
    // partial 1: d b0.charge / d vd_limit_
    if cx_.wanted(1) {
        cx_.jac()[1] = at_iterate_[1].d[0];
    }
    Ok(())
}";
        assert_eq!(terms(&ir, d), diode);
        let text = terms(&ir, d);
        assert!(!text.contains("vcrit") && !text.contains("pnjlim") && !text.contains("v(a, k)"), "{text}");
    }

    #[test]
    fn a_potential_branch_prints_its_flow_and_equation_terms() {
        let (ir, [_, _, v, ..]) = library();
        // `v(p, n) +> dc * source_scale_() + ac_stim(ac)` (devices.msl:21), abi.md A1: term 0
        // `b0.flow` is the branch's flow pin, term 1 `b0.eq` is v(p, n) minus the contributed
        // potential. Reads: p, n, the flow pin, the stimulus (slots 0 to 3), dc, ac, source_scale_;
        // `ac` is read for the stimulus's site, not by the terms, so it is not bound. Both terms
        // are affine; the partial of `b0.eq` against the stimulus comes from the iterate.
        let vsource = "\
// Vsource.analog @ devices.msl:17:1
fn terms_block<R: Real>(p: R, n: R, b0_flow_: R, ac_stim_: R, dc: R, source_scale_: R) -> [R; 2] {
    // contribution to v(p, n), branch b0 @ devices.msl:21:30
    let b0_potential_ = dc * source_scale_ + ac_stim_;
    [b0_flow_, p - n - b0_potential_]
}

// function 1 `terms`: reads p, n, b0_flow_, ac_stim_, dc, ac, source_scale_; writes b0.flow, b0.eq @ devices.msl:17:1
fn terms(cx_: &mut Eval<'_>) -> Result<(), Fault> {
    let p = cx_.f64(0);
    let n = cx_.f64(1);
    let b0_flow_ = cx_.f64(2);
    let ac_stim_ = cx_.f64(3);
    let dc = cx_.f64(4);
    let source_scale_ = cx_.f64(6);
    // the evaluation at the iterate
    let at_iterate_ = Self::terms_block(Dual::<4>::seeded(p, 0), Dual::<4>::seeded(n, 1), Dual::<4>::seeded(b0_flow_, 2), Dual::<4>::seeded(ac_stim_, 3), Dual::<4>::constant(dc), Dual::<4>::constant(source_scale_));
    // the evaluation with every continuous pin read as 0 (the affine rule)
    let at_zero_ = Self::terms_block(Dual::<4>::seeded(0.0, 0), Dual::<4>::seeded(0.0, 1), Dual::<4>::seeded(0.0, 2), Dual::<4>::seeded(ac_stim_, 3), Dual::<4>::constant(dc), Dual::<4>::constant(source_scale_));
    // term 0 `b0.flow`: affine, c + Σ g·x from the evaluation at 0, folded from 0.0
    cx_.set(0, at_zero_[0].v + (0.0 + at_zero_[0].d[2] * b0_flow_));
    // term 1 `b0.eq`: affine, c + Σ g·x from the evaluation at 0, folded from 0.0
    cx_.set(1, at_zero_[1].v + (0.0 + at_zero_[1].d[0] * p + at_zero_[1].d[1] * n));
    // partial 0: d b0.flow / d b0_flow_
    if cx_.wanted(0) {
        cx_.jac()[0] = at_zero_[0].d[2];
    }
    // partial 1: d b0.eq / d p
    if cx_.wanted(1) {
        cx_.jac()[1] = at_zero_[1].d[0];
    }
    // partial 2: d b0.eq / d n
    if cx_.wanted(2) {
        cx_.jac()[2] = at_zero_[1].d[1];
    }
    // partial 3: d b0.eq / d ac_stim_
    if cx_.wanted(3) {
        cx_.jac()[3] = at_iterate_[1].d[3];
    }
    Ok(())
}";
        assert_eq!(terms(&ir, v), vsource);
    }
}
