//! `RealLift`: a value of a composite type that holds a real, read from a pin at `f64`, as the
//! same value at the real of generic code (`R`, `Dual<N>`): each `f64` leaf becomes
//! `S::constant(leaf)` (`Dual::<N>::constant(leaf)` at a dual). An `analog` block is generic over its real, and a composite param is the
//! bytes of a pin, which decode at `f64` (CV-18, a read in every block).

use crate::Error;
use crate::print::TypeItems;
use piperine_ir::{FieldsForm, Ir, TypeId, TypeKind, ValueType};

/// Prints the lift of the types a design's composite params hold.
pub struct RealLift<'r, 'ir> {
    ir: &'ir Ir,
    items: &'r TypeItems<'ir>,
}

impl<'r, 'ir> RealLift<'r, 'ir> {
    /// The lift over the arena `items` prints.
    pub fn new(ir: &'ir Ir, items: &'r TypeItems<'ir>) -> RealLift<'r, 'ir> {
        RealLift { ir, items }
    }

    /// The Rust type of a value of `ty` at `f64`: what a pin decodes to.
    pub fn concrete_type(&self, ty: &ValueType) -> String {
        match ty {
            ValueType::F64 => "f64".to_string(),
            ValueType::Bool => "bool".to_string(),
            ValueType::Unit => "()".to_string(),
            ValueType::UInt(n) => format!("rt::UInt<{n}>"),
            ValueType::Int(n) => format!("rt::Int<{n}>"),
            ValueType::Fn(..) => "()".to_string(),
            ValueType::Tuple(parts) => {
                let parts: Vec<String> = parts.iter().map(|part| self.concrete_type(part)).collect();
                if parts.len() == 1 { format!("({},)", parts[0]) } else { format!("({})", parts.join(", ")) }
            }
            ValueType::Array(element, length) => format!("[{}; {length}]", self.concrete_type(element)),
            ValueType::Option(inner) => format!("Option<{}>", self.concrete_type(inner)),
            ValueType::Struct(id) | ValueType::Enum(id) => {
                let generic = if self.items.holds_real(ty) { "<f64>" } else { "" };
                format!("{}{generic}", self.items.name(*id))
            }
        }
    }

    /// The expression that is `source`, a value of `ty` at `f64`, at the real `real`. `depth`
    /// numbers the closure variables of nested arrays and options.
    pub fn expr(&self, ty: &ValueType, source: &str, real: &str, depth: usize) -> String {
        if !self.items.holds_real(ty) {
            return source.to_string();
        }
        match ty {
            ValueType::F64 => format!("{}::constant({source})", real.replacen('<', "::<", 1)),
            ValueType::Tuple(parts) => {
                let items: Vec<String> = parts.iter().enumerate().map(|(k, part)| self.expr(part, &format!("tuple{depth}_.{k}"), real, depth + 1)).collect();
                let tail = if items.len() == 1 { format!("({},)", items[0]) } else { format!("({})", items.join(", ")) };
                format!("{{ let tuple{depth}_ = {source}; {tail} }}")
            }
            ValueType::Array(element, _) | ValueType::Option(element) => {
                let inner = self.expr(element, &format!("element{depth}_"), real, depth + 1);
                format!("({source}).map(|element{depth}_| {inner})")
            }
            ValueType::Struct(_) | ValueType::Enum(_) => format!("({source}).lift::<{real}>()"),
            ValueType::Bool | ValueType::Unit | ValueType::UInt(_) | ValueType::Int(_) | ValueType::Fn(..) => source.to_string(),
        }
    }

    /// `impl Name<f64> { pub fn lift }` of the type `id`, which holds a real.
    pub fn item(&self, id: TypeId) -> Result<String, Error> {
        let name = self.items.name(id);
        let data = self.ir.type_data(id);
        let mut lines = vec![
            format!("/// MSL `{}` at the real `S`: each `f64` leaf is `S::constant(leaf)`.", data.path),
            "#[allow(dead_code)]".to_string(),
            format!("impl {name}<f64> {{"),
            format!("    pub fn lift<S: rt::Real>(self) -> {name}<S> {{"),
        ];
        match &data.kind {
            TypeKind::Struct { form, fields } => {
                let parts: Vec<String> = fields.iter().map(|(field, ty)| self.expr(ty, &format!("self.{}", Self::label(field)), "S", 0)).collect();
                lines.push(format!("        {}", Self::build(name, *form, fields, &parts)));
            }
            TypeKind::Enum { variants, .. } => {
                lines.push("        match self {".to_string());
                for variant in variants {
                    let bound: Vec<String> = (0..variant.fields.len()).map(|k| format!("field{k}_")).collect();
                    let path = format!("{name}::{}", crate::print::Text::ident(&variant.name));
                    let parts: Vec<String> = variant.fields.iter().zip(&bound).map(|((_, ty), b)| self.expr(ty, b, "S", 0)).collect();
                    let pattern = Self::build_pattern(&path, variant.form, &variant.fields, &bound);
                    lines.push(format!("            {pattern} => {},", Self::build(&path, variant.form, &variant.fields, &parts)));
                }
                lines.push("        }".to_string());
            }
        }
        lines.push("    }".to_string());
        lines.push("}".to_string());
        Ok(lines.join("\n"))
    }

    /// A field label as it prints: a number as it is, a name through `Text::ident`.
    fn label(name: &str) -> String {
        if name.chars().all(|c| c.is_ascii_digit()) { name.to_string() } else { crate::print::Text::ident(name) }
    }

    /// `Path { a: x, b: y }`, `Path(x, y)` or `Path` over `values`.
    fn build(path: &str, form: FieldsForm, fields: &[(String, ValueType)], values: &[String]) -> String {
        match form {
            FieldsForm::Unit => path.to_string(),
            FieldsForm::Tuple => format!("{path}({})", values.join(", ")),
            FieldsForm::Named => format!("{path} {{ {} }}", fields.iter().zip(values).map(|((field, _), value)| format!("{}: {value}", Self::label(field))).collect::<Vec<_>>().join(", ")),
        }
    }

    /// The pattern of a variant binding each field to `bound`.
    fn build_pattern(path: &str, form: FieldsForm, fields: &[(String, ValueType)], bound: &[String]) -> String {
        Self::build(path, form, fields, bound)
    }
}
