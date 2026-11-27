//! Starting an expression: a leaf is valued now, any other form schedules its operands.

use super::{Task, Walk};
use super::super::{Class, Sources, Value};
use crate::ids::ExprId;
use crate::model::{Expr, Terminal};

impl<'a> Walk<'a> {
    /// Starts `id`: a memoized or leaf value is pushed now, any other form schedules its operands
    /// and its `Finish`.
    pub(super) fn expr(&mut self, id: ExprId, next: &mut Vec<Task>) {
        let ir = self.ir;
        if let Some(value) = self.scope().memo.get(&id).cloned() {
            self.values.push(value);
            return;
        }
        let in_body = self.scope().in_body;
        match &ir.exprs[id.index()].kind {
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Int(_) | Expr::FloatBits(_) | Expr::Wide(_) | Expr::Local(_) | Expr::Param(_) | Expr::Env(_) | Expr::Fired(_) | Expr::Carrier(_) | Expr::Others(_) | Expr::Resolve { .. } | Expr::Previous(_) | Expr::Flow { .. } | Expr::ParamPrevious(_) | Expr::Derivative(_) | Expr::Delay(_) | Expr::SessionParameter { .. } | Expr::Noise(_) | Expr::Probe { .. } => {
                let value = self.leaf_value(id, in_body);
                return self.done(id, value);
            }
            Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } => return self.begin_loop(id, next),
            Expr::Break(_) | Expr::Continue | Expr::Return(_) | Expr::FunctionRef(_) => return self.leave_or_call(id, next),
            _ => self.schedule_operands(id, in_body, next),
        }
        // Any form with operands is combined once they are walked.
        next.push(Task::Finish(id));
    }

    /// Schedules the operands of a form that is combined once they are walked (`finish`).
    fn schedule_operands(&mut self, id: ExprId, in_body: bool, next: &mut Vec<Task>) {
        let ir = self.ir;
        match &ir.exprs[id.index()].kind {
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => next.push(Task::Expr(*expr)),
            Expr::Same { a, b } => next.extend([Task::Expr(*a), Task::Expr(*b)]),
            Expr::LevelCast { expr, .. } => next.push(Task::Expr(*expr)),
            Expr::Unwrap { is, value, .. } => next.extend([Task::Expr(*is), Task::Expr(*value)]),
            Expr::Method { receiver, args, .. } => {
                next.push(Task::Expr(*receiver));
                next.extend(args.iter().map(|&a| Task::Expr(a)));
            }
            Expr::Binary { lhs, rhs, .. } => next.extend([Task::Expr(*lhs), Task::Expr(*rhs)]),
            Expr::If { cond, then, otherwise } => {
                // Each arm is walked inside its condition, which a contribution there reads.
                next.extend([Task::Expr(*cond), Task::Hold(id)]);
                for arm in [*then, *otherwise] {
                    next.extend([Task::Enter(id), Task::Expr(arm), Task::Leave]);
                }
            }
            Expr::Block { stmts, value } => {
                next.extend(stmts.iter().map(|&s| Task::Stmt(s)));
                next.extend(value.iter().map(|&v| Task::Expr(v)));
                // The block's `let`s end with it.
                let outside = self.scope().locals.clone();
                next.push(Task::Restore(outside));
            }
            Expr::Call { args, .. } | Expr::Math { args, .. } => next.extend(args.iter().map(|&a| Task::Expr(a))),
            // An aggregate is one abstract value: what its parts read (`finish`).
            Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) | Expr::Table { .. } | Expr::TableSlope { .. } | Expr::CarrierElement { .. } | Expr::ElementPrevious { .. } | Expr::ElementChanged { .. } => {
                next.extend(ir.exprs[id.index()].kind.operands().into_iter().map(Task::Expr));
            }
            Expr::CallValue { callee, args } => next.extend([Task::Expr(*callee)].into_iter().chain(args.iter().map(|&a| Task::Expr(a)))),
            Expr::Closure { body, .. } => {
                // The parameters end with the closure.
                let outside = self.scope().locals.clone();
                next.extend([Task::BindParams(id), Task::Expr(*body), Task::Restore(outside)]);
            }
            Expr::Limit { x, f, seed, .. } => {
                if !in_body {
                    self.refuse(id, "limit outside a body");
                }
                next.extend([Task::Expr(*x), Task::Expr(*f)]);
                next.extend(seed.iter().map(|&s| Task::Expr(s)));
            }
            Expr::Stim { mag, phase, .. } => {
                if !in_body {
                    self.refuse(id, "ac_stim outside a body");
                }
                next.push(Task::Expr(*mag));
                next.extend(phase.iter().map(|&p| Task::Expr(p)));
            }
            _ => self.refuse(id, "a form with no operands to schedule"),
        }
    }

    /// A `break`, `continue`, `return` or function used as a value: it schedules its own tasks and is not finished.
    fn leave_or_call(&mut self, id: ExprId, next: &mut Vec<Task>) {
        let ir = self.ir;
        match &ir.exprs[id.index()].kind {
            Expr::Break(value) => {
                next.extend(value.iter().map(|&v| Task::Expr(v)));
                next.push(Task::Escape { at: id, valued: value.is_some() });
            }
            Expr::Continue => {
                next.push(Task::Escape { at: id, valued: false });
            }
            Expr::Return(value) => {
                next.extend(value.iter().map(|&v| Task::Expr(v)));
                next.push(Task::Yield { at: id, valued: value.is_some() });
            }
            // A function used as a value reads what its body reads, for arguments nothing is known of.
            Expr::FunctionRef(function) => {
                let unknown = Value::of(Sources::default(), Class::Nonlinear);
                let arguments = vec![unknown; ir.functions[function.index()].params.len()];
                self.enter(id, *function, arguments, next);
            }
            _ => self.refuse(id, "not an exit or a function value"),
        }
    }

    /// The value of a leaf form: a constant, or a read of one source. A read outside a body is refused.
    fn leaf_value(&mut self, id: ExprId, in_body: bool) -> Value {
        let ir = self.ir;
        match &ir.exprs[id.index()].kind {
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Int(_) | Expr::FloatBits(_) | Expr::Wide(_) => Value::constant(),
            Expr::Local(i) => match self.scope().locals.get(*i as usize) {
                Some(Some(value)) => value.clone(),
                Some(None) => self.refuse(id, &format!("local {i} read before its `let`")),
                None => self.refuse(id, &format!("local {i} out of range")),
            },
            Expr::Param(i) => self.body_read(id, in_body, "param read outside a body", Class::Constant, |sources| {
                sources.params.insert(*i);
            }),
            Expr::Env(env) => Self::read_of(Class::Constant, |sources| {
                sources.env.insert(*env);
            }),
            // The microstep's event byte: a read of its own, not in `Deps` (`fired_reads`).
            Expr::Fired(e) => Self::read_of(Class::Constant, |sources| {
                sources.fired.insert(*e);
            }),
            Expr::Carrier(i) => self.body_read(id, in_body, "carrier read outside a body", Class::Constant, |sources| {
                sources.carriers.insert(*i);
            }),
            Expr::Others(i) => self.body_read(id, in_body, "`others()` read outside a body", Class::Constant, |sources| {
                sources.others.insert(*i);
            }),
            // the combination of the drives' values: it reads each drive carrier and is no function of them a sum can follow
            Expr::Resolve { drives, .. } => self.body_read(id, in_body, "`resolve` outside a body", Class::Nonlinear, |sources| {
                sources.carriers.extend(drives);
                sources.resolve_drives.extend(drives);
            }),
            Expr::Previous(i) => self.body_read(id, in_body, "previous-value read outside a body", Class::Constant, |sources| {
                sources.previous.insert(*i);
            }),
            // The flow of a branch is one more unknown of the system, as a node potential is: it reaches a sum affinely.
            Expr::Flow { branch } => self.body_read(id, in_body, "flow read outside a body", Class::Affine, |sources| {
                sources.flows.insert(*branch);
            }),
            Expr::ParamPrevious(p) => self.body_read(id, in_body, "previous-value read of a param outside a body", Class::Constant, |sources| {
                sources.param_previous.insert(*p);
            }),
            // The rate of a site is one more unknown of the system, as a flow is: it reaches a sum affinely.
            Expr::Derivative(k) => self.body_read(id, in_body, "derivative read outside a body", Class::Affine, |sources| {
                sources.derivatives.insert(*k);
            }),
            // The output of a site is one more unknown of the system, as a flow is: it reaches a sum affinely.
            Expr::Delay(k) => self.body_read(id, in_body, "delay read outside a body", Class::Affine, |sources| {
                sources.delays.insert(*k);
            }),
            // constant during an analysis: the sum sees a constant
            Expr::SessionParameter { parameter } => self.body_read(id, in_body, "session parameter read outside a body", Class::Constant, |sources| {
                sources.session_parameters.insert(*parameter);
            }),
            // The value of a source is an input that no node potential moves: it is a constant of the sum.
            Expr::Noise(k) => self.body_read(id, in_body, "noise read outside a body", Class::Constant, |sources| {
                sources.noises.insert(*k);
            }),
            Expr::Probe { plus, minus } => self.body_read(id, in_body, "probe outside a body", Class::Affine, |sources| {
                for terminal in [plus, minus] {
                    if let Terminal::Node(node) = terminal {
                        sources.nodes.insert(*node);
                    }
                }
            }),
            _ => self.refuse(id, "not a leaf form"),
        }
    }

    /// A read of one source of class `class`: `fill` records it.
    fn read_of(class: Class, fill: impl FnOnce(&mut Sources)) -> Value {
        let mut sources = Sources::default();
        fill(&mut sources);
        Value::of(sources, class)
    }

    /// A read that only a body may make (`what` names it when refused): `fill` records the source.
    fn body_read(&self, id: ExprId, in_body: bool, what: &str, class: Class, fill: impl FnOnce(&mut Sources)) -> Value {
        if !in_body {
            self.refuse(id, what);
        }
        Self::read_of(class, fill)
    }
}
