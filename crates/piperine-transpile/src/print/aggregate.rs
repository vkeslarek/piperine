//! The aggregate forms of the IR as Rust (composite-values CV-13, CV-20): construction of tuples,
//! arrays, structs and enum variants; field and element reads; `len`; variant tests and
//! fields; `Option` packing; `default`; the comparison of aggregates; and assignment through field
//! and index projections. Each is plain Rust over the items of `print/types.rs`. An index outside
//! its length is a panic naming the index and where it was written (`rt::Index`, §1.5.25).

use piperine_ir::{BinaryOp, Expr, ExprId, LevelPart, Place, Projection, TypeId, TypeKind, ValueType};

use crate::Error;
use crate::print::value::{Piece, Prec};
use crate::print::{Text, TypeItems, ValuePrinter};

impl ValuePrinter<'_> {
    /// The Rust of the aggregate form `e`.
    pub(super) fn aggregate(&self, e: ExprId) -> Result<Piece, Error> {
        let atom = |text: String| Ok(Piece::new(text, Prec::Atom));
        match self.ir.expr(e) {
            Expr::Tuple(parts) => {
                if !matches!(self.ir.expr_type(e), ValueType::Tuple(_)) {
                    return Err(self.unsupported(e, "a tuple expression that is not typed as a tuple"));
                }
                let parts = self.parts(parts)?;
                atom(if parts.len() == 1 { format!("({},)", parts[0]) } else { format!("({})", parts.join(", ")) })
            }
            Expr::Array(parts) => {
                let ValueType::Array(element, _) = self.ir.expr_type(e) else { return Err(self.unsupported(e, "an array expression that is not typed as an array")) };
                if parts.is_empty() {
                    // no element names the element type, so the empty array says it (H.5 rule 1)
                    return atom(format!("<[{}; 0]>::default()", self.rust_type(element, &format!("{} e{}", self.path, e.index()))?));
                }
                let parts = self.parts(parts)?;
                atom(format!("[{}]", parts.join(", ")))
            }
            Expr::ArrayRepeat { element, count } => {
                if !matches!(self.ir.expr_type(e), ValueType::Array(..)) {
                    return Err(self.unsupported(e, "an array expression that is not typed as an array"));
                }
                atom(format!("[{}; {count}]", self.expr(*element)?))
            }
            Expr::StructOf { ty, fields } => self.struct_expression(e, *ty, fields),
            Expr::VariantOf { ty, variant, fields } => self.variant_expression(e, *ty, *variant, fields),
            Expr::Field { expr, index } => {
                let name = self.field_name(self.ir.expr_type(*expr), *index).ok_or_else(|| self.unsupported(e, "a field read of a value with no such field"))?;
                atom(format!("{}.{name}", self.operand(*expr)?))
            }
            Expr::Index { expr, index } => atom(self.element_read(e, *expr, *index)?),
            Expr::Length(expr) => atom(format!("rt::UInt::<64>::new({}.len() as u64)", self.operand(*expr)?)),
            Expr::IsVariant { expr, variant } => {
                let (name, _) = self.variant_label(self.ir.expr_type(*expr), *variant).ok_or_else(|| self.unsupported(e, "a variant test of a value that is not an enum"))?;
                atom(format!("matches!({}, {name} {{ .. }})", self.piece(*expr)?.text))
            }
            Expr::VariantField { expr, variant, field } => {
                let (name, fields) = self.variant_label(self.ir.expr_type(*expr), *variant).ok_or_else(|| self.unsupported(e, "a variant field of a value that is not an enum"))?;
                let label = fields.get(*field as usize).map(|(label, _)| label.clone()).ok_or_else(|| self.unsupported(e, "a variant field out of range"))?;
                atom(format!("match {} {{ {name} {{ {label}: field_, .. }} => field_, _ => rt::Panic::raise({:?}.to_string()) }}", self.piece(*expr)?.at(Prec::Atom), format!("{}: the value is not the variant {name}", self.site(e))))
            }
            Expr::OptionPack { is, value } => {
                if !matches!(self.ir.expr_type(e), ValueType::Option(_)) {
                    return Err(self.unsupported(e, "an option that is not typed as an Option"));
                }
                Ok(Piece::new(format!("if {} {{ Some({}) }} else {{ None }}", self.condition(*is)?, self.expr(*value)?), Prec::Low))
            }
            Expr::OptionPart { option, part: LevelPart::Is } => atom(format!("{}.is_some()", self.operand(*option)?)),
            // `None` unpacks to the zero of the payload; an array has no `Default` past 32 elements (T54)
            Expr::OptionPart { option, part: LevelPart::Value } if matches!(self.ir.expr_type(e), ValueType::Array(..)) => atom(format!("{}.unwrap_or_else(rt::Carried::zeroed)", self.operand(*option)?)),
            Expr::OptionPart { option, part: LevelPart::Value } => atom(format!("{}.unwrap_or_default()", self.operand(*option)?)),
            Expr::Default(ty) => atom(format!("<{}>::default()", self.rust_type(ty, &format!("{} e{}", self.path, e.index()))?)),
            _ => Err(self.unsupported(e, "a form that is not an aggregate")),
        }
    }

    /// `parts` each as a Rust expression.
    fn parts(&self, parts: &[ExprId]) -> Result<Vec<String>, Error> {
        parts.iter().map(|part| self.expr(*part)).collect()
    }

    /// `where` an access was written: `file:line:col`, what a panic names.
    fn site(&self, e: ExprId) -> String {
        self.ir.expr_origin(e).to_string()
    }

    /// The printed label of field `index` of a struct or a tuple: its name, or its number.
    fn field_name(&self, ty: &ValueType, index: u32) -> Option<String> {
        match ty {
            ValueType::Tuple(parts) => ((index as usize) < parts.len()).then(|| index.to_string()),
            ValueType::Struct(id) => match &self.ir.type_data(*id).kind {
                TypeKind::Struct { fields, .. } => fields.get(index as usize).map(|(name, _)| Self::label(name)),
                TypeKind::Enum { .. } => None,
            },
            _ => None,
        }
    }

    /// A field name as it prints: a number as it is, a name as `Text::ident`.
    fn label(name: &str) -> String {
        if name.chars().all(|c| c.is_ascii_digit()) { name.to_string() } else { Text::ident(name) }
    }

    /// `Enum::Variant` and the labelled fields of variant `variant` of the enum `ty`.
    fn variant_label(&self, ty: &ValueType, variant: u32) -> Option<(String, Vec<(String, ValueType)>)> {
        let ValueType::Enum(id) = ty else { return None };
        let TypeKind::Enum { variants, .. } = &self.ir.type_data(*id).kind else { return None };
        let chosen = variants.get(variant as usize)?;
        let fields = chosen.fields.iter().map(|(name, ty)| (Self::label(name), ty.clone())).collect();
        Some((format!("{}::{}", TypeItems::new(self.ir).name(*id), Text::ident(&chosen.name)), fields))
    }

    /// `S { a: x, b: y }`, `S(x, y)` or `S` for the struct `ty`.
    fn struct_expression(&self, e: ExprId, ty: TypeId, fields: &[ExprId]) -> Result<Piece, Error> {
        let data = self.ir.type_data(ty);
        let TypeKind::Struct { form, fields: declared } = &data.kind else { return Err(self.unsupported(e, "a struct expression of a type that is not a struct")) };
        let values = self.parts(fields)?;
        let name = TypeItems::new(self.ir).name(ty).to_string();
        Ok(Piece::new(self.constructed(&name, *form, declared, &values), Self::literal_prec(*form)))
    }

    /// `E::V { n: x }`, `E::V(x)` or `E::V` for variant `variant` of the enum `ty`.
    fn variant_expression(&self, e: ExprId, ty: TypeId, variant: u32, fields: &[ExprId]) -> Result<Piece, Error> {
        let TypeKind::Enum { variants, .. } = &self.ir.type_data(ty).kind else { return Err(self.unsupported(e, "a variant expression of a type that is not an enum")) };
        let chosen = variants.get(variant as usize).ok_or_else(|| self.unsupported(e, "a variant out of range"))?;
        let values = self.parts(fields)?;
        let items = TypeItems::new(self.ir);
        // an enum that holds a real is generic over it, and a variant that holds none (`Dot`) names no real
        // for Rust to infer: it is written on the instance, `Shape::<f64>::Dot`
        let real = if self.plain { "f64" } else { "R" };
        let owner = if items.holds_real(&ValueType::Enum(ty)) { format!("{}::<{real}>", items.name(ty)) } else { items.name(ty).to_string() };
        let name = format!("{owner}::{}", Text::ident(&chosen.name));
        Ok(Piece::new(self.constructed(&name, chosen.form, &chosen.fields, &values), Self::literal_prec(chosen.form)))
    }

    /// How strongly a literal of `form` binds: a braced one (`S { a: x }`) is no atom, because Rust
    /// refuses it bare in the head of an `if`, a `while` or a `match`, so it is parenthesized wherever
    /// it is an operand or a scrutinee; a unit or a tuple one is an atom.
    fn literal_prec(form: piperine_ir::FieldsForm) -> Prec {
        if form == piperine_ir::FieldsForm::Named { Prec::Low } else { Prec::Atom }
    }

    /// `name` built from `values` the way `form` writes it.
    fn constructed(&self, name: &str, form: piperine_ir::FieldsForm, declared: &[(String, ValueType)], values: &[String]) -> String {
        match form {
            piperine_ir::FieldsForm::Unit => name.to_string(),
            piperine_ir::FieldsForm::Tuple => format!("{name}({})", values.join(", ")),
            piperine_ir::FieldsForm::Named => {
                let pairs: Vec<String> = declared.iter().zip(values).map(|((field, _), value)| format!("{}: {value}", Self::label(field))).collect();
                format!("{name} {{ {} }}", pairs.join(", "))
            }
        }
    }

    /// An index as a Rust `i128`: the integer's value, whatever its width and sign.
    pub(super) fn index_value(&self, index: ExprId) -> Result<String, Error> {
        Ok(format!("{}.get() as i128", self.operand(index)?))
    }

    /// `base[index]` with the index checked.
    fn element_read(&self, e: ExprId, base: ExprId, index: ExprId) -> Result<String, Error> {
        let (base_text, site) = (self.operand(base)?, self.site(e));
        Ok(format!("rt::Index::at(&{base_text}, {}, {site:?})", self.index_value(index)?))
    }

    /// Whether `e` has an aggregate type.
    pub(super) fn is_aggregate_operand(&self, e: ExprId) -> bool {
        matches!(self.ir.expr_type(e), ValueType::Tuple(_) | ValueType::Array(..) | ValueType::Struct(_) | ValueType::Enum(_) | ValueType::Option(_))
    }

    /// `a == b`, `a < b` over two aggregates: Rust's derived operators. The code's real `R` compares
    /// by value (`rt::Real` is `PartialEq + PartialOrd`), so an aggregate that holds one derives them.
    pub(super) fn aggregate_comparison(&self, e: ExprId, op: BinaryOp, lhs: ExprId, rhs: ExprId) -> Result<Piece, Error> {
        let symbol = match op {
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            _ => return Err(self.unsupported(e, "an operator that does not apply to aggregates")),
        };
        Ok(Piece::new(format!("{} {symbol} {}", self.piece(lhs)?.at(Prec::Unary), self.piece(rhs)?.at(Prec::Unary)), Prec::Compare))
    }

    /// `place = value;` or `place op= value;` where the place has projections: Rust's place
    /// expression, each index checked.
    pub(super) fn assign_through(&self, place: &Place, op: Option<BinaryOp>, value: ExprId) -> Result<String, Error> {
        let (Some(name), Some(local_ty)) = (self.local(place.local), self.local_types.get(place.local as usize)) else {
            return Err(self.unsupported(value, "an assignment to a local out of range"));
        };
        let (mut target, mut ty) = (name.to_string(), local_ty.clone());
        for step in &place.path {
            match step {
                Projection::Field(k) => {
                    let label = self.field_name(&ty, *k).ok_or_else(|| self.unsupported(value, "an assignment to a field that does not exist"))?;
                    let next = self.field_type(&ty, *k).ok_or_else(|| self.unsupported(value, "an assignment to a field that does not exist"))?;
                    target = format!("{target}.{label}");
                    ty = next;
                }
                Projection::Index(index) => {
                    let ValueType::Array(element, _) = &ty else { return Err(self.unsupported(value, "an assignment to an element of a value that is not an array")) };
                    let site = self.site(value);
                    target = format!("{target}[rt::Index::of({}, {target}.len(), {site:?})]", self.index_value(*index)?);
                    ty = (**element).clone();
                }
            }
        }
        self.assign_to(&target, &ty, op, value)
    }

    /// The type of field `index` of a struct or a tuple type.
    fn field_type(&self, ty: &ValueType, index: u32) -> Option<ValueType> {
        match ty {
            ValueType::Tuple(parts) => parts.get(index as usize).cloned(),
            ValueType::Struct(id) => match &self.ir.type_data(*id).kind {
                TypeKind::Struct { fields, .. } => fields.get(index as usize).map(|(_, ty)| ty.clone()),
                TypeKind::Enum { .. } => None,
            },
            _ => None,
        }
    }
}
