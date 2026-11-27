//! Finishing an expression: combining the values its operands left on the stack.

use super::{Task, Walk};
use super::super::{Class, Sources, Value};
use crate::ids::ExprId;
use crate::model::{BinaryOp, Expr, UnaryOp};

impl<'a> Walk<'a> {
    /// Combines the operand values of `id`, left on the value stack by its operands' tasks.
    pub(super) fn finish(&mut self, id: ExprId, next: &mut Vec<Task>) {
        let ir = self.ir;
        let value = match &ir.exprs[id.index()].kind {
            Expr::Unary { op, .. } => Self::unary(*op, self.pop()),
            Expr::Binary { op, .. } => {
                let r = self.pop();
                let l = self.pop();
                Walk::binary(*op, l, r)
            }
            Expr::If { .. } => self.join_arms(),
            Expr::Block { value, .. } => match value {
                Some(_) => self.pop(),
                None => Value::constant(),
            },
            Expr::Call { function, args } => {
                let values = self.pop_n(args.len());
                self.enter(id, *function, values, next);
                return;
            }
            Expr::Math { args, .. } => {
                let values = self.pop_n(args.len());
                Value::nonlinear(&values)
            }
            Expr::CallValue { args, .. } => {
                let values = self.pop_n(1 + args.len());
                Value::nonlinear(&values)
            }
            Expr::Cast { .. } | Expr::LevelCast { .. } => {
                let value = self.pop();
                Value::nonlinear([&value])
            }
            Expr::Unwrap { .. } | Expr::Same { .. } => {
                let values = self.pop_n(2);
                Value::nonlinear(&values)
            }
            Expr::Method { args, .. } => {
                let values = self.pop_n(1 + args.len());
                Value::nonlinear(&values)
            }
            // An element read is a read of its carrier (CV-24) and of what its index reads.
            Expr::CarrierElement { carrier, index } | Expr::ElementPrevious { carrier, index } | Expr::ElementChanged { carrier, index } => self.element_read(id, *carrier, *index),
            // An aggregate and each of its projections read everything its parts read.
            Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) | Expr::Table { .. } | Expr::TableSlope { .. } => {
                let values = self.pop_n(ir.exprs[id.index()].kind.operands().len());
                Value::nonlinear(&values)
            }
            Expr::Closure { .. } => {
                let value = self.pop();
                Value::of(value.sources, Class::Nonlinear)
            }
            Expr::Limit { site, seed, .. } => self.limit_read(id, *site, seed.is_some()),
            Expr::Stim { site, phase, .. } => self.stimulus_read(id, *site, phase.is_some()),
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Int(_) | Expr::FloatBits(_) | Expr::Wide(_) | Expr::Carrier(_) | Expr::Local(_) | Expr::Param(_) | Expr::Env(_) | Expr::Probe { .. } | Expr::Flow { .. } | Expr::Previous(_) | Expr::Fired(_) | Expr::Derivative(_) | Expr::Delay(_) | Expr::Noise(_) | Expr::SessionParameter { .. } | Expr::ParamPrevious(_) | Expr::Others(_) | Expr::Resolve { .. } | Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } | Expr::Break(_) | Expr::Continue | Expr::Return(_) | Expr::FunctionRef(_) => {
                self.refuse(id, "a leaf has no operands to finish")
            }
        };
        self.done(id, value);
    }

    /// The value of `l op r`: the class follows the operator, the sources join.
    pub(super) fn binary(op: BinaryOp, l: Value, r: Value) -> Value {
        let class = match op {
            BinaryOp::Add | BinaryOp::Sub => l.class.max(r.class),
            BinaryOp::Mul if l.class == Class::Constant => r.class,
            BinaryOp::Mul | BinaryOp::Div if r.class == Class::Constant => l.class,
            _ => Value::nonlinear([&l, &r]).class,
        };
        let mut sources = l.sources;
        sources.join(&r.sources);
        Value::of(sources, class)
    }

    /// An element read of an array carrier: it reads the carrier (the element when the index is a constant) and what its index reads.
    fn element_read(&mut self, id: ExprId, carrier: u32, index: ExprId) -> Value {
        let ir = self.ir;
        let values = self.pop_n(1);
        let mut value = Value::nonlinear(&values);
        let constant = match ir.exprs[index.index()].kind {
            Expr::Int(element) => u64::try_from(element).ok(),
            _ => None,
        };
        match (&ir.exprs[id.index()].kind, constant) {
            (Expr::ElementPrevious { .. }, Some(element)) => value.sources.element_previous.insert((carrier, element)),
            (Expr::ElementPrevious { .. }, None) => value.sources.previous.insert(carrier),
            (Expr::ElementChanged { .. }, Some(element)) => value.sources.element_changed.insert((carrier, element)),
            (Expr::ElementChanged { .. }, None) => value.sources.whole_changed.insert(carrier),
            (_, Some(element)) => value.sources.elements.insert((carrier, element)),
            (_, None) => value.sources.carriers.insert(carrier),
        };
        value
    }

    /// A `limit` read: the site reads its input, function and seed, and the read itself is the site alone, affine.
    fn limit_read(&mut self, id: ExprId, site: u32, seeded: bool) -> Value {
        let mut own = Sources::default();
        for value in self.pop_n(2 + usize::from(seeded)) {
            own.join(&value.sources);
        }
        match self.limits.get_mut(site as usize) {
            Some(slot) => slot.join(&own),
            None => self.refuse(id, &format!("limit site {site} out of range")),
        }
        let mut sources = Sources::default();
        sources.limits.insert(site);
        Value::of(sources, Class::Affine)
    }

    /// An `ac_stim` read: the site reads the magnitude and the phase, and the read is nonlinear in them plus the site.
    fn stimulus_read(&mut self, id: ExprId, site: u32, phased: bool) -> Value {
        let values = self.pop_n(1 + usize::from(phased));
        let mut value = Value::nonlinear(&values);
        match self.stims.get_mut(site as usize) {
            Some(slot) => slot.join(&value.sources),
            None => self.refuse(id, &format!("ac_stim site {site} out of range")),
        }
        value.sources.stims.insert(site);
        value
    }

    /// `-x` keeps the class of `x`; `!x` is nonlinear in what it reads.
    fn unary(op: UnaryOp, value: Value) -> Value {
        match op {
            UnaryOp::Neg => value,
            UnaryOp::Not => Value::nonlinear([&value]),
        }
    }

    /// An `if` whose condition, `then` and `otherwise` values are on the stack: it reads all three,
    /// and joins the arms only when the condition is `Constant`.
    fn join_arms(&mut self) -> Value {
        let o = self.pop();
        let t = self.pop();
        let c = self.pop();
        let class = if c.class == Class::Constant { t.class.max(o.class) } else { Class::Nonlinear };
        let mut sources = c.sources;
        sources.join(&t.sources);
        sources.join(&o.sources);
        Value::of(sources, class)
    }
}
