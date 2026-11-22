//! Modules with a result (DIG-06, part III §8.3): `module name(inputs) -> T { items result }`, its
//! result of type `T` (`E0301`), its placement in an expression, the value inputs it takes, and
//! what the `analog` blocks read of a carrier (`E0150`, part I §3.3.5). A module-body `let` that
//! reads itself is `E0332`, or `E0353` when the cycle passes through a result that depends
//! directly on its input (§8.3.5).

use std::collections::{BTreeMap, BTreeSet};

use crate::msl::{Diagnostic, Span};
use crate::pom::{CallClass, DeclId, DeclKind, ExprId, ExprKind, FileId, ModuleData, Resolution, StmtId};

use super::check::{Checker, Context};
use super::nature_rules::NatureRules;
use super::{Builtin, Callee, Ty};

/// The values the value inputs of a placement stand for while a result is read: each input is the
/// expression the caller wrote, read in the caller's own frame.
#[derive(Debug, Default)]
struct Frame {
    inputs: BTreeMap<DeclId, (ExprId, usize)>,
}

impl Checker<'_> {
    /// The result of a module: `-> T` and the trailing expression have the type `T` (`E0301`).
    pub(super) fn module_result(&mut self, decl: DeclId, module: &ModuleData) {
        let data = self.design.decl_data(decl);
        let (file, name_span) = (data.file, data.name.as_ref().map_or(data.span, |n| n.span));
        match (module.ret, module.result) {
            (Some(ret), Some(result)) => match self.lower_type(ret) {
                Some(ty) => self.check_as(result, &ty, "E0301"),
                None => {
                    self.infer_event_position(result);
                }
            },
            (Some(_), None) => {
                let message = "a module declared with `-> T` ends its body with no result expression (§8 rule 3)".to_string();
                self.coded(file, "E0301", name_span, message, "no result");
            }
            (None, Some(result)) => {
                let span = self.design.expr_node(result).span;
                self.unsupported(file, span, "module result");
                self.infer(result);
            }
            (None, None) => {}
        }
    }

    /// The type a placement of `module` has: its result type, or the module itself.
    pub(super) fn placement_type(&mut self, module: DeclId) -> Option<Ty> {
        let DeclKind::Module(m) = &self.design.decl_data(module).kind else { return None };
        match m.ret {
            Some(ret) => self.lower_type(ret),
            None => Some(Ty::Module(module)),
        }
    }

    /// The name `instance` of a placement of `module`, read as a value: an instance of a module
    /// without a result has no value, `E0333` naming it (§8.2.14). One with a result is that value.
    pub(super) fn instance_as_value(&mut self, file: FileId, span: Span, instance: DeclId, module: DeclId) -> Option<Ty> {
        let has_result = matches!(&self.design.decl_data(module).kind, DeclKind::Module(m) if m.ret.is_some());
        if has_result {
            return Some(Ty::Module(module));
        }
        let name = self.design.decl_data(instance).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let message = format!("the instance `{name}` has no result: a module without a result is not a value (§8.2.14)");
        self.coded(file, "E0333", span, message, "an instance without a result used as a value");
        None
    }

    /// `E0354` (§8.3.10): the name `binding` of a continuous result (a `let`, or a placement's
    /// result) read in a const context (a param default, a `where`, a `const` initializer), where no
    /// probe may be written. Reports and returns true when it is one.
    pub(super) fn continuous_result_in_const(&mut self, file: FileId, span: Span, binding: DeclId) -> bool {
        if self.context != Context::Const {
            return false;
        }
        let Some(init) = self.binding_init(binding) else { return false };
        if !self.continuous(init) {
            return false;
        }
        let name = self.design.decl_data(binding).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let message = format!("the continuous result `{name}` is read in a const context, where no probe may be written (§8.3.10)");
        self.coded(file, "E0354", span, message, "a continuous result where probes are not allowed");
        self.poisoned.insert(binding);
        true
    }

    /// `E0354` (§8.3.10): the placement `expr` has an `Event` result and stands outside an event
    /// position. Returns false for any other expression.
    pub(super) fn event_result_out_of_place(&mut self, expr: ExprId) -> bool {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Call { class: CallClass::Instance { module: Some(module), .. }, .. } = &node.kind else { return false };
        let name = self.design.decl_data(*module).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let file = self.file_of_expr(expr);
        let message = format!("the `Event` result of `{name}` is read outside an event position, where an event could not be written (§8.3.10)");
        self.coded(file, "E0354", node.span, message, "an `Event` result outside an event position");
        true
    }

    /// Whether `decl` is a value input: a port whose type is a value, not a nature.
    pub(super) fn is_value_input(&self, decl: DeclId) -> bool {
        matches!(&self.design.decl_data(decl).kind, DeclKind::Port(p) if self.design.nature_of(p.ty).is_none())
    }

    /// Whether `decl` is a port or a signal of a discrete nature.
    fn is_discrete_carrier(&self, decl: DeclId) -> bool {
        let ty = match &self.design.decl_data(decl).kind {
            DeclKind::Port(p) => p.ty,
            DeclKind::Signal(s) => s.ty,
            _ => return false,
        };
        self.design.nature_of(ty).is_some_and(|n| NatureRules::of(self.design, n).carries_values)
    }

    /// The value inputs of `module`, in port order.
    fn value_inputs(&self, module: DeclId) -> Vec<DeclId> {
        self.design.decl_data(module).children.iter().copied().filter(|c| self.is_value_input(*c)).collect()
    }

    // ---- an `analog` block reads no unknown (§3.3.5) -----------------------------------------

    /// The statements of an `analog` block read a discrete carrier only as the receiver of
    /// `known`, `known_or`, `is_known` or `high`: any other read, direct, through a `let` or
    /// through a value input, is `E0150` naming the carrier.
    pub(super) fn analog_reads(&mut self, stmts: &[StmtId]) {
        let mut frames = vec![Frame::default()];
        // one carrier read is one `E0150`, however many reads of the block's `let`s reach it
        let mut reported = BTreeSet::new();
        for stmt in stmts {
            for expr in crate::pom::stmt_children(self.design, *stmt) {
                self.carrier_reads_in(expr, 0, &mut frames, None, &mut Vec::new(), &mut reported);
            }
        }
    }

    /// The carriers `expr` reads bare, in the frame `frame`; `anchor` is where the chain of `let`s
    /// and results that led here was entered in the `analog` block, where the error is reported.
    /// `reported` holds the carrier reads already reported: a read reached again (a `let` of the
    /// block read after its own statement was checked) is not reported twice.
    fn carrier_reads_in(&mut self, expr: ExprId, frame: usize, frames: &mut Vec<Frame>, anchor: Option<(Span, FileId)>, followed: &mut Vec<ExprId>, reported: &mut BTreeSet<ExprId>) {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let here = anchor.unwrap_or((node.span, file));
        match &node.kind {
            ExprKind::MethodCall { recv, args, method, res, .. } if self.is_flag_read(res, &method.text) => {
                // The flag reads: the receiver is read through `level`; its arguments are read.
                if !self.is_carrier_path(*recv) {
                    self.carrier_reads_in(*recv, frame, frames, anchor, followed, reported);
                }
                for arg in args {
                    self.carrier_reads_in(arg.value, frame, frames, anchor, followed, reported);
                }
            }
            // The edges and `changed` make an `Event`, which an `analog` block cannot hold (`E0441`).
            ExprKind::MethodCall { res: Resolution::Builtin(id), .. } if id.info().name == "changed" => {}
            ExprKind::MethodCall { res: Resolution::Decl(f), .. } if matches!(self.stdlib_builtin(*f), Some(Builtin::Rise | Builtin::Fall)) => {}
            // A call of a function reads a carrier argument through flags only when the parameter it
            // binds is flag-total (§3.3.6); every other argument is read as written.
            ExprKind::Call { callee, args, class: CallClass::Function, .. } if matches!(self.callee_resolution(*callee), Some(Resolution::Decl(_))) => {
                let Some(Resolution::Decl(function)) = self.callee_resolution(*callee) else { return };
                for (index, arg) in args.iter().enumerate() {
                    if !(arg.name.is_none() && self.is_carrier_path(arg.value) && self.is_flag_total(function, index)) {
                        self.carrier_reads_in(arg.value, frame, frames, anchor, followed, reported);
                    }
                }
            }
            ExprKind::Field { res: Resolution::Decl(decl), .. } if self.is_discrete_carrier(*decl) => {
                if !reported.insert(expr) {
                    return;
                }
                let name = self.design.decl_data(*decl).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                let message = format!("an `analog` block reads the discrete carrier `{name}` of a child other than through `known`, `known_or`, `is_known` or `high`: it has no unknowns (§3.3.5)");
                self.coded(here.1, "E0150", here.0, message, "a discrete carrier read in `analog`");
            }
            ExprKind::Path(path) => {
                let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res) else { return };
                if self.is_discrete_carrier(decl) {
                    if !reported.insert(expr) {
                        return;
                    }
                    let name = self.design.decl_data(decl).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                    let message = format!("an `analog` block reads the discrete carrier `{name}` other than through `known`, `known_or`, `is_known` or `high`: it has no unknowns (§3.3.5)");
                    let help = "read it through `known()`, `known_or(d)`, or `high()` for a `Bit`";
                    self.report(here.1, Diagnostic::new("E0150", message, here.0, "a discrete carrier read in `analog`").with_help(help));
                } else if let Some((arg, parent)) = frames[frame].inputs.get(&decl).copied() {
                    self.carrier_reads_in(arg, parent, frames, Some(here), followed, reported);
                } else if let Some(init) = self.binding_init(decl)
                    && !followed.contains(&init)
                {
                    followed.push(init);
                    self.carrier_reads_in(init, frame, frames, Some(here), followed, reported);
                    followed.pop();
                }
            }
            ExprKind::Call { class: CallClass::Instance { module: Some(module), .. }, args, .. } => {
                // A placement read here is its result, with each value input replaced by the
                // caller's expression (§8.3.12).
                let DeclKind::Module(m) = &self.design.decl_data(*module).kind else { return };
                let Some(result) = m.result else { return };
                let inputs = self.value_inputs(*module);
                let ports: Vec<DeclId> = self.design.decl_data(*module).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Port(_))).collect();
                let mut callee = Frame::default();
                for input in inputs {
                    if let Some(i) = ports.iter().position(|p| *p == input)
                        && let Some(arg) = args.get(i)
                    {
                        callee.inputs.insert(input, (arg.value, frame));
                    }
                }
                frames.push(callee);
                let inner = frames.len() - 1;
                if !followed.contains(&result) {
                    followed.push(result);
                    self.carrier_reads_in(result, inner, frames, Some(here), followed, reported);
                    followed.pop();
                }
            }
            _ => {
                for child in crate::pom::expr_children(&node.kind, self.design) {
                    self.carrier_reads_in(child, frame, frames, anchor, followed, reported);
                }
            }
        }
    }

    /// Whether `expr` is a plain read of a discrete carrier.
    fn is_carrier_path(&self, expr: ExprId) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.is_carrier_path(*inner),
            ExprKind::Path(path) => matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Decl(d)) if self.is_discrete_carrier(d)),
            ExprKind::Field { res: Resolution::Decl(d), .. } => self.is_discrete_carrier(*d),
            _ => false,
        }
    }

    /// An argument for a value input that the `analog` blocks of `module` read: it is read in
    /// `analog`, so it may not read a carrier bare (`E0150`, §3.3.5 "through a value input").
    pub(super) fn analog_value_input(&mut self, module: DeclId, input: DeclId, arg: ExprId) {
        let DeclKind::Module(m) = &self.design.decl_data(module).kind else { return };
        let mut read = false;
        for behavior in m.behaviors.clone() {
            let DeclKind::Behavior(b) = &self.design.decl_data(behavior).kind else { continue };
            if b.kind != crate::pom::BehaviorKind::Analog {
                continue;
            }
            for stmt in b.body.clone() {
                for e in crate::pom::stmt_children(self.design, stmt) {
                    read |= self.reads_decl(e, input, &mut Vec::new());
                }
            }
        }
        if read {
            self.carrier_reads_in(arg, 0, &mut vec![Frame::default()], None, &mut Vec::new(), &mut BTreeSet::new());
        }
    }

    /// Whether `expr` reads the declaration `target`, directly or through a `let`.
    pub(super) fn reads_decl(&self, expr: ExprId, target: DeclId, followed: &mut Vec<ExprId>) -> bool {
        let node = self.design.expr_node(expr);
        if let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(d)) = self.design.path_node(*path).segments.last().map(|s| s.res)
        {
            if d == target {
                return true;
            }
            if let Some(init) = self.binding_init(d)
                && !followed.contains(&init)
            {
                followed.push(init);
                let found = self.reads_decl(init, target, followed);
                followed.pop();
                return found;
            }
        }
        crate::pom::expr_children(&node.kind, self.design).into_iter().any(|c| self.reads_decl(c, target, followed))
    }

    // ---- a `let` that reads itself (§8.2.16, §8.3.5) -----------------------------------------

    /// Whether the result of `module` reads one of its value inputs (rather than only its own
    /// carriers): a direct dependence (§8.3.5).
    fn result_depends_directly(&self, module: DeclId) -> bool {
        let DeclKind::Module(m) = &self.design.decl_data(module).kind else { return false };
        let Some(result) = m.result else { return false };
        self.value_inputs(module).into_iter().any(|input| self.reads_decl(result, input, &mut Vec::new()))
    }

    /// A read of the module-body `let` `binding` while its own value is typed (§8.2.16, §8.3.5):
    /// `E0332` outside a placement; inside one, `E0353` when every result on the cycle has a direct
    /// dependence (a core module's result always has one, §8.3.6), else the type of a feedback
    /// through an unknown: the declared result type of the placement the `let` is bound to.
    pub(super) fn self_read(&mut self, file: FileId, span: Span, binding: DeclId) -> Option<Ty> {
        let name = self.design.decl_data(binding).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        if self.placing.is_empty() {
            self.coded(file, "E0332", span, format!("the `let` `{name}` reads its own name (§8.2.16)"), "a `let` that reads itself");
        } else if self.placing.iter().all(|p| p.is_none_or(|m| self.result_depends_directly(m))) {
            let cycle = self.cycle_names(&name);
            let message = format!("the `let` `{name}` reads itself through results that all depend directly on their input: the cycle {cycle} has no unknown in it (§8.3.5)");
            self.coded(file, "E0353", span, message, "a cycle with no unknown");
        } else if let Some(ty) = self.feedback_type(binding) {
            return Some(ty);
        } else {
            self.unsupported(file, span, &format!("the feedback of `{name}` through an instance whose value is not a placement of a module with a result"));
        }
        self.poisoned.insert(binding);
        None
    }

    /// The names of the cycle `let` -> the placed modules -> `let`, as `y -> twice -> y`.
    fn cycle_names(&self, binding: &str) -> String {
        let mut names = vec![binding.to_string()];
        names.extend(self.placing.iter().map(|p| p.map_or("a core module".to_string(), |m| self.design.decl_data(m).name.as_ref().map_or(String::new(), |n| n.text.clone()))));
        names.push(binding.to_string());
        names.join(" -> ")
    }

    /// The type of the `let` `binding` that feeds back through an unknown: the declared result type of
    /// the placement that is the whole of its value (parentheses aside). `None` for a value that only
    /// contains the placement, or a core module, whose result type waits for its arguments.
    fn feedback_type(&mut self, binding: DeclId) -> Option<Ty> {
        let mut value = self.lets_open.get(&binding).copied()?;
        while let ExprKind::Paren(inner) = &self.design.expr_node(value).kind {
            value = *inner;
        }
        let module = match self.callees.get(&value) {
            Some(Callee::Module(module)) => *module,
            _ => return None,
        };
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return None };
        self.lower_type(data.ret?)
    }
}
