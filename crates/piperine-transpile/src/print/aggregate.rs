//! The aggregate forms of the IR as Rust (composite-values CV-13, CV-20): construction of tuples,
//! arrays, structs and enum variants; field and element reads; `len`; variant tests and
//! fields; `Option` packing; `default`; the comparison of aggregates; and assignment through field
//! and index projections. Each is plain Rust over the items of `print/types.rs`. An index outside
//! its length is a panic naming the index and where it was written (`rt::Index`, §1.5.25); an index
//! that may be unknown reads and writes by merging the elements it may name (§3.3.13), which needs
//! elements that can be merged.

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
                let types = match self.ir.expr_type(e) {
                    ValueType::Tuple(types) => types.clone(),
                    _ => return Err(self.unsupported(e, "a tuple expression that is not typed as a tuple")),
                };
                let parts = self.parts(parts, &types)?;
                atom(if parts.len() == 1 { format!("({},)", parts[0]) } else { format!("({})", parts.join(", ")) })
            }
            Expr::Array(parts) => {
                let ValueType::Array(element, _) = self.ir.expr_type(e) else { return Err(self.unsupported(e, "an array expression that is not typed as an array")) };
                if parts.is_empty() {
                    // no element names the element type, so the empty array says it (H.5 rule 1)
                    return atom(format!("<[{}; 0]>::default()", self.rust_type(element, &format!("{} e{}", self.path, e.index()))?));
                }
                let parts = self.parts(parts, &vec![(**element).clone(); parts.len()])?;
                atom(format!("[{}]", parts.join(", ")))
            }
            Expr::ArrayRepeat { element, count } => {
                let ValueType::Array(want, _) = self.ir.expr_type(e) else { return Err(self.unsupported(e, "an array expression that is not typed as an array")) };
                atom(format!("[{}; {count}]", self.expr_as(*element, want)?))
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
                atom(format!("match {} {{ {name} {{ {label}: field_, .. }} => field_, _ => rt::Panic::raise({:?}.to_string()) }}", self.piece(*expr)?.text, format!("{}: the value is not the variant {name}", self.site(e))))
            }
            Expr::OptionPack { is, value } => {
                let ValueType::Option(inner) = self.ir.expr_type(e) else { return Err(self.unsupported(e, "an option that is not typed as an Option")) };
                Ok(Piece::new(format!("if {} {{ Some({}) }} else {{ None }}", self.known_condition(*is)?, self.expr_as(*value, inner)?), Prec::Low))
            }
            Expr::OptionPart { option, part: LevelPart::Is } => atom(format!("{}.is_some()", self.operand(*option)?)),
            Expr::OptionPart { option, part: LevelPart::Value } => atom(format!("{}.unwrap_or_default()", self.operand(*option)?)),
            Expr::Default(ty) => atom(format!("<{}>::default()", self.rust_type(ty, &format!("{} e{}", self.path, e.index()))?)),
            _ => Err(self.unsupported(e, "a form that is not an aggregate")),
        }
    }

    /// `parts` each as a value of the matching type of `types`.
    fn parts(&self, parts: &[ExprId], types: &[ValueType]) -> Result<Vec<String>, Error> {
        parts.iter().zip(types).map(|(part, ty)| self.expr_as(*part, ty)).collect()
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
        let types: Vec<ValueType> = declared.iter().map(|(_, ty)| ty.clone()).collect();
        let values = self.parts(fields, &types)?;
        let name = TypeItems::new(self.ir).name(ty).to_string();
        Ok(Piece::new(self.constructed(&name, *form, declared, &values), Prec::Atom))
    }

    /// `E::V { n: x }`, `E::V(x)` or `E::V` for variant `variant` of the enum `ty`.
    fn variant_expression(&self, e: ExprId, ty: TypeId, variant: u32, fields: &[ExprId]) -> Result<Piece, Error> {
        let TypeKind::Enum { variants, .. } = &self.ir.type_data(ty).kind else { return Err(self.unsupported(e, "a variant expression of a type that is not an enum")) };
        let chosen = variants.get(variant as usize).ok_or_else(|| self.unsupported(e, "a variant out of range"))?;
        let types: Vec<ValueType> = chosen.fields.iter().map(|(_, ty)| ty.clone()).collect();
        let values = self.parts(fields, &types)?;
        let name = format!("{}::{}", TypeItems::new(self.ir).name(ty), Text::ident(&chosen.name));
        Ok(Piece::new(self.constructed(&name, chosen.form, &chosen.fields, &values), Prec::Atom))
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
    fn index_value(&self, index: ExprId) -> Result<String, Error> {
        if Self::is_flagged(self.ir.expr_type(index)) {
            return Err(self.unsupported(index, "an index that may be unknown"));
        }
        Ok(format!("{}.get() as i128", self.operand(index)?))
    }

    /// An index that may be unknown as an `Option<i128>`: `None` when it is.
    fn optional_index(&self, index: ExprId) -> Result<String, Error> {
        if !Self::is_flagged(self.ir.expr_type(index)) {
            return Ok(format!("Some({})", self.index_value(index)?));
        }
        Ok(format!("{{ let index_ = {}; if index_.is_known() {{ Some(index_.known_or(Default::default()).get() as i128) }} else {{ None }} }}", self.piece(index)?.text))
    }

    /// `base[index]` with the index checked; for an index that may be unknown the merge of the
    /// elements it may name, which the elements must support.
    fn element_read(&self, e: ExprId, base: ExprId, index: ExprId) -> Result<String, Error> {
        let (base_text, site) = (self.operand(base)?, self.site(e));
        if Self::is_flagged(self.ir.expr_type(index)) {
            self.mergeable(e, self.ir.expr_type(e))?;
            return Ok(format!("rt::Index::read(&{base_text}, {}, {site:?})", self.optional_index(index)?));
        }
        Ok(format!("rt::Index::at(&{base_text}, {}, {site:?})", self.index_value(index)?))
    }

    /// An element read or written through an index that may be unknown has to be a value that
    /// merges: one that may itself be unknown.
    fn mergeable(&self, at: ExprId, element: &ValueType) -> Result<(), Error> {
        if Self::is_flagged(element) {
            Ok(())
        } else {
            Err(self.unsupported(at, "an index that may be unknown into elements that cannot be unknown (the elements must be `Flagged`)"))
        }
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
    /// expression, each index checked; an index that may be unknown (the last step) writes by merge.
    pub(super) fn assign_through(&self, place: &Place, op: Option<BinaryOp>, value: ExprId) -> Result<String, Error> {
        let (Some(name), Some(local_ty)) = (self.local(place.local), self.local_types.get(place.local as usize)) else {
            return Err(self.unsupported(value, "an assignment to a local out of range"));
        };
        let (mut target, mut ty) = (name.to_string(), local_ty.clone());
        let last = place.path.len().saturating_sub(1);
        for (position, step) in place.path.iter().enumerate() {
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
                    if Self::is_flagged(self.ir.expr_type(*index)) {
                        if position != last || op.is_some() {
                            return Err(self.unsupported(*index, "an index that may be unknown, other than as the last step of a plain assignment"));
                        }
                        self.mergeable(value, element)?;
                        let written = self.expr_as(value, element)?;
                        return Ok(format!("rt::Index::write(&mut {target}, {}, {written}, {site:?});", self.optional_index(*index)?));
                    }
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
