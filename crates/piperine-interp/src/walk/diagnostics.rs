//! Faults: how a form is named in a fault, and where it stood.

use piperine_abi::device::Fault;
use piperine_ir::{Env, Expr, ExprId, Origin};

use super::{Value, Walk};

impl<'a> Walk<'a> {
    /// The `file:line:col` text of an origin.
    pub(super) fn origin(origin: &Origin) -> String {
        format!("{}:{}:{}", origin.file, origin.line, origin.col)
    }

    /// A fault naming `what` and the origin of expression `id`.
    pub(super) fn fault(&self, id: ExprId, what: &str) -> Fault {
        Fault::new(Self::UNEXECUTED, format!("{what} (at {})", Self::origin(self.ir.expr_origin(id))))
    }

    /// The name of an expression form, for diagnostics.
    pub(super) fn form(expr: &Expr) -> String {
        match expr {
            // The forms msl-fixes added are named as the source spells them.
            Expr::Method { method, .. } => format!("method `{}`", method.name()),
            Expr::Env(Env::Freq) => "form `freq()`".to_string(),
            Expr::Env(Env::Analysis(kind)) => format!("form `analysis({kind:?})`"),
            Expr::Fired(_) => "form `fired`".to_string(),
            Expr::Unwrap { message: Some(_), .. } => "form `expect`".to_string(),
            Expr::Unwrap { message: None, .. } => "form `unwrap`".to_string(),
            Expr::Same { .. } => "form `same`".to_string(),
            Expr::LevelCast { .. } => "form `cast`".to_string(),
            other => format!("form `{}`", Self::form_name(other)),
        }
    }

    /// The variant name of an expression form.
    pub(super) fn form_name(expr: &Expr) -> &'static str {
        match expr {
            Expr::Real(_) => "Real",
            Expr::Bool(_) => "Bool",
            Expr::Unit => "Unit",
            Expr::Local(_) => "Local",
            Expr::Param(_) => "Param",
            Expr::Unary { .. } => "Unary",
            Expr::Binary { .. } => "Binary",
            Expr::If { .. } => "If",
            Expr::Block { .. } => "Block",
            Expr::Call { .. } => "Call",
            Expr::Math { .. } => "Math",
            Expr::Closure { .. } => "Closure",
            Expr::Probe { .. } => "Probe",
            Expr::Env(_) => "Env",
            Expr::Limit { .. } => "Limit",
            Expr::Stim { .. } => "Stim",
            Expr::Int(_) => "Int",
            Expr::FloatBits(_) => "FloatBits",
            Expr::Wide(_) => "Wide",
            Expr::Cast { .. } => "Cast",
            Expr::Method { .. } => "Method",
            Expr::Carrier(_) => "Carrier",
            Expr::Flow { .. } => "Flow",
            Expr::Derivative(_) => "Derivative",
            Expr::Delay(_) => "Delay",
            Expr::Noise(_) => "Noise",
            Expr::SessionParameter { .. } => "SessionParameter",
            Expr::ParamPrevious(_) => "ParamPrevious",
            Expr::Previous(_) => "Previous",
            Expr::CarrierElement { .. } => "CarrierElement",
            Expr::ElementPrevious { .. } => "ElementPrevious",
            Expr::ElementChanged { .. } => "ElementChanged",
            Expr::Others(_) => "Others",
            Expr::Resolve { .. } => "Resolve",
            Expr::Fired(_) => "Fired",
            Expr::Unwrap { .. } => "Unwrap",
            Expr::Same { .. } => "Same",
            Expr::LevelCast { .. } => "LevelCast",
            Expr::Loop { .. } => "Loop",
            Expr::While { .. } => "While",
            Expr::ForRange { .. } => "ForRange",
            Expr::Break(_) => "Break",
            Expr::Continue => "Continue",
            Expr::Return(_) => "Return",
            Expr::FunctionRef(_) => "FunctionRef",
            Expr::CallValue { .. } => "CallValue",
            Expr::Tuple(_) => "Tuple",
            Expr::Array(_) => "Array",
            Expr::ArrayRepeat { .. } => "ArrayRepeat",
            Expr::StructOf { .. } => "StructOf",
            Expr::VariantOf { .. } => "VariantOf",
            Expr::Field { .. } => "Field",
            Expr::Index { .. } => "Index",
            Expr::Length(_) => "Length",
            Expr::IsVariant { .. } => "IsVariant",
            Expr::VariantField { .. } => "VariantField",
            Expr::OptionPack { .. } => "OptionPack",
            Expr::OptionPart { .. } => "OptionPart",
            Expr::Default(_) => "Default",
            Expr::Table { .. } => "Table",
            Expr::TableSlope { .. } => "TableSlope",
        }
    }

    /// The fault of a read of a cell the running function does not declare.
    pub(super) fn undeclared(&self, id: ExprId) -> Fault {
        let origin = Self::origin(self.ir.expr_origin(id));
        Fault::new(
            Self::UNDECLARED,
            format!("a value is taken from a cell the function does not read (a wrong dependency set, at {origin})"),
        )
    }

    /// The name of the kind of a value, for a fault.
    pub(super) fn kind(value: &Value) -> &'static str {
        match value {
            Value::Real(_) => "a real",
            Value::Bool(_) => "a bool",
            Value::Unit => "a unit",
            Value::Unread => "a cell the function does not read",
            Value::Closure(_) => "a closure (only `limit` calls one)",
        }
    }
}
