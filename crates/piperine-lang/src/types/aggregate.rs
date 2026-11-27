//! Struct, tuple and array types (§1.2.4 to §1.2.6, §1.3) and the expressions
//! that build and read them: struct expressions, field access, tuple and array expressions,
//! indexing.
//!
//! A `struct` is a nominal value type ([`Ty::Struct`]): its fields are typed once, on first use,
//! into [`FieldType`]s kept in `Checker::layouts`. A type that contains itself, directly or
//! through structs, enums, tuples, arrays and `Option`, is `E0186` (§1.3.8).

use crate::refusal::forms::ValueForm;
use crate::msl::Span;
use crate::pom::{Arg, DeclId, DeclKind, ExprId, ExprKind, FieldInit, FileId, Name, Resolution, TypeId};

use super::check::Checker;
use super::{ConstTerm, Length, Nominal, Ty};

/// One typed field of a struct or of an enum variant, in declaration order.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldType {
    /// The field's name; `None` for a positional field.
    pub name: Option<String>,
    /// The field's declaration.
    pub decl: DeclId,
    /// The field's type.
    pub ty: Ty,
}

impl Checker<'_> {
    // ---- types as written ------------------------------------------------------------------

    /// The type of the struct `decl` named at `span` of `file`: [`Ty::Struct`] once its fields are
    /// typed, else a diagnostic and `None` (a bundle is outside the subset, a duplicate field or a
    /// self-containing type is `E0186`). A generic struct is typed over its own parameters; its uses
    /// carry the arguments (`generic_nominal`).
    pub(super) fn struct_type(&mut self, decl: DeclId, file: FileId, span: Span) -> Option<Ty> {
        if self.broken.contains(&decl) {
            return None;
        }
        if self.layouts.contains_key(&decl) {
            return Some(Ty::structure(decl));
        }
        if self.declaring.contains(&decl) {
            return self.cyclic_type(decl, file, span);
        }
        self.declaring.insert(decl);
        let declared = self.declare_struct(decl, file, span);
        self.declaring.remove(&decl);
        if declared {
            Some(Ty::structure(decl))
        } else {
            self.broken.insert(decl);
            None
        }
    }

    /// A type met again while its own fields are being typed: `E0186` (§1.3.8).
    pub(super) fn cyclic_type(&mut self, decl: DeclId, file: FileId, span: Span) -> Option<Ty> {
        let kind = &self.design.decl_data(decl).kind;
        let ty = if matches!(kind, DeclKind::Enum(_)) { Ty::enumeration(decl) } else { Ty::structure(decl) };
        let name = ty.name(self.design);
        self.coded(file, "E0186", span, format!("{name} contains itself: a value has a fixed size and there is no indirection (§1.3.8)"), "contains itself");
        None
    }

    /// Types the fields of the struct `decl`: a struct with a carrier field is a bundle, not a
    /// value type (§1.3), and a duplicate field name is `E0186` (§1.3.2).
    fn declare_struct(&mut self, decl: DeclId, file: FileId, span: Span) -> bool {
        let data = self.design.decl_data(decl).clone();
        let name = data.name.as_ref().map_or("", |n| n.text.as_str());
        let carries = data.children.iter().any(|c| matches!(&self.design.decl_data(*c).kind, DeclKind::Field(f) if f.dir.is_some()));
        if matches!(&data.kind, DeclKind::Struct(s) if s.bundle == Some(true)) || carries {
            // the type of a handle: a bundle is named, never held (`bundle_handles.rs`)
            if self.naming_bundle {
                return true;
            }
            self.refuse_with(file, span, ValueForm::StructItemUsedAsTypeHasCarrier, [name.to_string()]);
            return false;
        }
        let Some(fields) = self.fields_of(decl) else { return false };
        self.judge_repr(decl, file, &fields) & self.judge_derives(decl, file, span, &fields)
    }

    /// The typed fields of `owner`, a struct or an enum variant, typed on the first call and kept.
    pub(super) fn fields_of(&mut self, owner: DeclId) -> Option<Vec<FieldType>> {
        if let Some(found) = self.layouts.get(&owner) {
            return Some(found.clone());
        }
        let mut fields: Vec<FieldType> = Vec::new();
        let mut ok = true;
        for child in self.design.decl_data(owner).children.clone() {
            let field = self.design.decl_data(child).clone();
            let DeclKind::Field(data) = &field.kind else { continue };
            let name = field.name.as_ref().map(|n| n.text.clone());
            if let Some(name) = &name
                && fields.iter().any(|f| f.name.as_deref() == Some(name))
            {
                self.coded(field.file, "E0186", field.span, format!("the field `{name}` is declared twice (§1.3.2)"), "declared twice");
                ok = false;
                continue;
            }
            match self.lower_type(data.ty) {
                Some(ty) if self.is_field_type(&ty) => fields.push(FieldType { name, decl: child, ty }),
                Some(ty) => {
                    let what = ty.name(self.design);
                    self.refuse_with(field.file, field.span, ValueForm::FieldOfTypeInValueType, [what.to_string()]);
                    ok = false;
                }
                None => ok = false,
            }
        }
        ok.then(|| {
            self.layouts.insert(owner, fields.clone());
            fields
        })
    }

    /// Whether a value type may be held by a field: what a value is (§1.2).
    fn is_field_type(&self, ty: &Ty) -> bool {
        ty.is_value()
    }

    /// `(A, B)` and `(A,)` (§1.2.4).
    pub(super) fn tuple_type(&mut self, items: &[TypeId]) -> Option<Ty> {
        let types: Option<Vec<Ty>> = items.iter().map(|t| self.lower_type(*t)).collect();
        let types = types?;
        for (ty, node) in types.iter().zip(items) {
            if !ty.is_value() {
                let (file, span) = self.type_place(*node);
                self.refuse_with(file, span, ValueForm::InsideTupleType, [ty.name(self.design).to_string()]);
                return None;
            }
        }
        Some(Ty::Tuple(types))
    }

    /// `[T; N]` (§1.2.5): `N` is a const expression the checker folds, else `E0188`; one that reads a module
    /// param stays an expression the elaborator folds per body variant (§8.1.2, QC2).
    pub(super) fn array_type(&mut self, file: FileId, span: Span, elem: TypeId, len: ExprId) -> Option<Ty> {
        let element = self.lower_type(elem);
        let length = self.array_length(file, span, len)?;
        let element = element?;
        // an array of carriers or of nodes (CV-23): `Ty::Array` over a `Carrier` or a `Node` element, which no value is
        if element.is_carrier_element() {
            return Some(Ty::Array(Box::new(element), length));
        }
        if !element.is_value() && !matches!(element, Ty::Module(_)) {
            self.refuse_with(file, span, ValueForm::ArrayOf, [element.name(self.design).to_string()]);
            return None;
        }
        Some(Ty::Array(Box::new(element), length))
    }

    /// The length `len` of an array type: a number the checker folds, a length over const parameters, or one that reads a
    /// module param; else `E0188` naming it (§1.2.5).
    pub(super) fn array_length(&mut self, file: FileId, span: Span, len: ExprId) -> Option<Length> {
        let Some(length) = self.fold_length(len).map(Length::Fixed).or_else(|| self.generic_length(len)).or_else(|| self.module_param_length(len)) else {
            let what = self.expression_name(len);
            self.coded(file, "E0188", span, format!("the length of an array type is a constant, and {what} is not one the checker folds (§1.2.5)"), "not a constant");
            return None;
        };
        Some(length)
    }

    /// The length that reads a module param (`[T; n]`, `[T; n + 1]`), left as the expression: the checker types it so the
    /// elaborator's evaluator reads it, and the elaborator folds it per body variant (§8.1.2, QC2).
    fn module_param_length(&mut self, len: ExprId) -> Option<Length> {
        if !self.reads_module_param(len) {
            return None;
        }
        self.infer(len)?;
        Some(Length::Generic(ConstTerm::Expr(len)))
    }

    /// The length `[T; N]` or `[T; N + 1]` writes when it names const parameters (§3.8.3, D14): a
    /// generic length, folded when the item is instantiated; `None` for any other expression.
    fn generic_length(&mut self, len: ExprId) -> Option<Length> {
        match self.generic_parameter_of(len) {
            Some(parameter) => Some(Length::Generic(ConstTerm::Param(parameter))),
            // `[bool; T::WIDTH]`: the level count of the type argument, known at the instantiation (§2)
            None if let Some(parameter) = self.width_parameter_of(len) => Some(Length::Generic(ConstTerm::Width(parameter))),
            None if self.mentions_generic_parameter(len) => {
                self.defer_constant(len);
                Some(Length::Generic(ConstTerm::Expr(len)))
            }
            None => None,
        }
    }

    /// The file and span of a type node.
    fn type_place(&self, ty: TypeId) -> (FileId, Span) {
        let node = self.design.type_node(ty);
        (self.design.decl_data(node.owner).file, node.span)
    }

    // ---- struct expressions ----------------------------------------------------------------

    /// `P { x: 1.0, y }`, `Tx::Data { n: 3 }` (§1.3.3): every field named once, in any order, each
    /// checked against the field's type. A field left out, named twice or not in the type is
    /// `E0186` naming it.
    pub(super) fn struct_expression(&mut self, expr: ExprId, path: crate::pom::PathId, fields: &[FieldInit]) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let Some(Resolution::Decl(target)) = self.design.path_node(path).segments.last().map(|s| s.res) else {
            self.refuse(file, node.span, ValueForm::StructExpressionOfNameThatIsNo);
            return None;
        };
        let (open, owner) = self.constructed_type(target, file, node.span)?;
        let declared = self.fields_of(owner)?;
        let mut seen: Vec<&str> = Vec::new();
        let mut values: Vec<(usize, ExprId)> = Vec::new();
        for init in fields {
            let name = init.name.text.as_str();
            let Some(position) = declared.iter().position(|f| f.name.as_deref() == Some(name)) else {
                let of = open.name(self.design);
                self.coded(file, "E0186", init.span, format!("{of} has no field `{name}` (§1.3.3)"), "no such field");
                self.infer(init.value);
                continue;
            };
            if seen.contains(&name) {
                self.coded(file, "E0186", init.span, format!("the field `{name}` is named twice in one struct expression (§1.3.3)"), "named twice");
                self.infer(init.value);
                continue;
            }
            seen.push(name);
            values.push((position, init.value));
        }
        let (ty, layout) = self.close_constructor(expr, open, declared, &values, file, node.span)?;
        for (position, value) in &values {
            self.check(*value, &layout[*position].ty);
        }
        let left_out: Vec<String> = layout.iter().filter_map(|f| f.name.as_deref()).filter(|n| !seen.contains(n)).map(|n| format!("`{n}`")).collect();
        if !left_out.is_empty() {
            let of = ty.name(self.design);
            self.coded(file, "E0186", node.span, format!("this struct expression for {of} leaves out the field {} (§1.3.3)", left_out.join(", ")), "field left out");
        }
        Some(ty)
    }

    /// The type a struct, a tuple struct or a variant constructs, with the declaration that owns
    /// its fields.
    pub(super) fn constructed_type(&mut self, target: DeclId, file: FileId, span: Span) -> Option<(Ty, DeclId)> {
        match &self.design.decl_data(target).kind {
            DeclKind::Struct(_) => self.struct_type(target, file, span).map(|_| (self.open_type(target), target)),
            DeclKind::Variant(_) => {
                let parent = self.design.decl_data(target).parent?;
                self.enum_type(parent, file, span).map(|_| (self.open_type(parent), target))
            }
            other => {
                self.refuse_with(file, span, ValueForm::AsStructExpression, [super::unsupported::Unsupported::item(other).to_string()]);
                None
            }
        }
    }

    /// `P(1.0, 2.0)`, `Tx::Data(3)` (§1.3.3, §1.3.6): a tuple struct or a variant with positional
    /// fields built by a call; each argument is checked against its field.
    pub(super) fn constructor_call(&mut self, expr: ExprId, target: DeclId, args: &[Arg]) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let (open, owner) = self.constructed_type(target, file, node.span)?;
        let declared = self.fields_of(owner)?;
        if declared.iter().any(|f| f.name.is_some()) {
            self.refuse(file, node.span, ValueForm::CallOfStructOrVariantWithNamed);
            return None;
        }
        let positional: Vec<(usize, ExprId)> = args.iter().enumerate().filter(|(_, arg)| arg.name.is_none()).map(|(position, arg)| (position, arg.value)).collect();
        let (ty, layout) = if declared.len() == args.len() { self.close_constructor(expr, open, declared, &positional, file, node.span)? } else { (open, declared) };
        if layout.len() != args.len() {
            let message = format!("{} takes {} argument{}, found {}", ty.name(self.design), layout.len(), if layout.len() == 1 { "" } else { "s" }, args.len());
            self.mismatch_text(file, node.span, &format!("{} fields", layout.len()), &format!("{} arguments ({message})", args.len()));
            for arg in args {
                self.infer(arg.value);
            }
            return Some(ty);
        }
        for (arg, field) in args.iter().zip(&layout) {
            if arg.name.is_some() {
                self.refuse(file, arg.span, ValueForm::NamedArgumentOfConstructor);
                continue;
            }
            self.check(arg.value, &field.ty);
        }
        Some(ty)
    }

    /// A struct or a variant named as a value: a unit one is its value, any other is a
    /// constructor that only a call or a struct expression uses.
    pub(super) fn constructor_value(&mut self, expr: ExprId, target: DeclId, file: FileId, span: Span) -> Option<Ty> {
        let (open, owner) = self.constructed_type(target, file, span)?;
        let declared = self.fields_of(owner)?;
        let (ty, layout) = self.close_constructor(expr, open, declared, &[], file, span)?;
        if !layout.is_empty() {
            let name = ty.name(self.design);
            self.refuse_with(file, span, ValueForm::NameOfWhichHasFieldsUsedAs, [name.to_string()]);
            return None;
        }
        Some(ty)
    }

    // ---- field access ----------------------------------------------------------------------

    /// `s.x` and `t.0` (§1.3.4): the type of the field, or `E0186`
    /// naming the field a type does not have.
    pub(super) fn field_of_value(&mut self, expr: ExprId, receiver: ExprId, name: &Name) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let found = self.infer(receiver)?;
        match &found {
            Ty::Struct(Nominal { decl, .. }) => {
                let layout = self.instance_layout(*decl, &found)?;
                let named = match name.text.parse::<usize>() {
                    Ok(position) => layout.get(position).filter(|f| f.name.is_none()),
                    Err(_) => layout.iter().find(|f| f.name.as_deref() == Some(name.text.as_str())),
                };
                match named {
                    Some(field) => Some(field.ty.clone()),
                    None => self.no_field(file, node.span, &found, name),
                }
            }
            Ty::Tuple(items) => match name.text.parse::<usize>().ok().and_then(|i| items.get(i)) {
                Some(ty) => Some(ty.clone()),
                None => self.no_field(file, node.span, &found, name),
            },
            Ty::Module(_) => None,
            other => self.no_field(file, node.span, other, name),
        }
    }

    /// `E0186`: a field access or tuple index the type does not have (§1.3.4).
    fn no_field(&mut self, file: FileId, span: Span, ty: &Ty, name: &Name) -> Option<Ty> {
        let of = ty.name(self.design);
        self.coded(file, "E0186", span, format!("{of} has no field `{}` (§1.3.4)", name.text), "no such field");
        None
    }

    // ---- tuples ----------------------------------------------------------------------------

    /// `(a, b)`, `(a,)` and `()` (§1.2.4): a tuple of the items' types, each item checked against
    /// the matching field of `expected` when the position gives a tuple.
    pub(super) fn tuple_expression(&mut self, items: &[ExprId], expected: Option<&Ty>) -> Option<Ty> {
        if items.is_empty() {
            return Some(Ty::Unit);
        }
        let wanted: Option<&Vec<Ty>> = match expected {
            Some(Ty::Tuple(types)) if types.len() == items.len() => Some(types),
            _ => None,
        };
        let mut types = Vec::new();
        let mut typed = true;
        for (i, item) in items.iter().enumerate() {
            let ty = match wanted.and_then(|w| w.get(i)) {
                Some(want) => {
                    self.check(*item, want);
                    Some(want.clone())
                }
                None => self.infer(*item),
            };
            match ty {
                Some(ty) if ty.is_value() => types.push(ty),
                Some(ty) => {
                    let file = self.file_of_expr(*item);
                    let span = self.design.expr_node(*item).span;
                    self.refuse_with(file, span, ValueForm::InsideTuple, [ty.name(self.design).to_string()]);
                    typed = false;
                }
                None => typed = false,
            }
        }
        typed.then(|| Ty::Tuple(types))
    }

    // ---- arrays ----------------------------------------------------------------------------

    /// `[a, b, c]` (§1.2.5): one element type, taken from `expected` or from the first item that
    /// does not wait for another's type. An empty array has no element type of its own (`E0181`).
    pub(super) fn array_expression(&mut self, expr: ExprId, items: &[ExprId], expected: Option<&Ty>) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        let element = match expected {
            Some(Ty::Array(element, _)) => Some((**element).clone()),
            _ => None,
        };
        let element = match (element, items) {
            (Some(element), _) => element,
            (None, []) => {
                self.coded(file, "E0181", span, "an empty array has no element type: write the type where it is used (§1.4.4)".to_string(), "element type unknown");
                return None;
            }
            (None, _) => {
                let first = items.iter().copied().find(|i| !self.integer_literal_waits(*i)).unwrap_or(items[0]);
                let ty = self.infer(first)?;
                // an array of templates is a value of the elaboration (§8.4.1, GAPS L88)
                if !ty.is_value() && !matches!(ty, Ty::Module(_)) {
                    let span = self.design.expr_node(first).span;
                    self.refuse_with(file, span, ValueForm::InsideArray, [ty.name(self.design).to_string()]);
                    return None;
                }
                for item in items.iter().filter(|i| **i != first) {
                    self.check(*item, &ty);
                }
                return Some(Ty::array(ty, items.len() as u64));
            }
        };
        for item in items {
            self.check(*item, &element);
        }
        Some(Ty::array(element, items.len() as u64))
    }

    /// `[x; N]` (§1.2.5): `x` is `Copy`; `N` a const expression the checker folds, else `E0180`
    /// naming it (the specification gives no code of its own, D6).
    pub(super) fn array_repeat(&mut self, expr: ExprId, element: ExprId, len: ExprId, expected: Option<&Ty>) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let ty = match expected {
            Some(Ty::Array(want, _)) => {
                self.check(element, want);
                Some((**want).clone())
            }
            _ => self.infer(element),
        };
        // `[x; N + 1]`: a length over const parameters is generic, folded at an instantiation (§3.8.3)
        let Some(length) = self.fold_length(len).map(Length::Fixed).or_else(|| self.generic_length(len)).or_else(|| self.module_param_length(len)) else {
            let span = self.design.expr_node(len).span;
            let what = self.expression_name(len);
            self.mismatch_text(file, span, "a constant array length", &what);
            return None;
        };
        let ty = ty?;
        Some(Ty::Array(Box::new(ty), length))
    }

    /// `e[i]` reads an element of an array (§1.5.9); an index is
    /// any integer type.
    pub(super) fn index_expression(&mut self, expr: ExprId, base: ExprId, index: ExprId) -> Option<Ty> {
        if matches!(self.design.expr_node(index).kind, ExprKind::Range { .. }) {
            return self.range_index(expr, base, index);
        }
        let file = self.file_of_expr(expr);
        let found = self.infer(base);
        self.integer_index(index);
        let found = found?;
        let element = match &found {
            Ty::Array(element, _) => (**element).clone(),
            other => {
                let span = self.design.expr_node(base).span;
                self.mismatch_text(file, span, "an array", &other.name(self.design));
                return None;
            }
        };
        Some(element)
    }

    /// An index or a bound of a range: any integer type (§1.5.9).
    pub(super) fn integer_index(&mut self, index: ExprId) {
        match self.infer(index) {
            Some(ty) if ty.is_integer() => {}
            Some(other) => {
                let file = self.file_of_expr(index);
                let span = self.design.expr_node(index).span;
                self.mismatch_text(file, span, "an integer type", &other.name(self.design));
            }
            None => {}
        }
    }

    /// The name of a const expression for a message: a path as written, else a plain word.
    pub(super) fn expression_name(&self, expr: ExprId) -> String {
        match &self.design.expr_node(expr).kind {
            ExprKind::Path(path) => {
                let segments: Vec<&str> = self.design.path_node(*path).segments.iter().map(|s| s.name.text.as_str()).collect();
                format!("`{}`", segments.join("::"))
            }
            ExprKind::Paren(inner) => self.expression_name(*inner),
            _ => "this expression".to_string(),
        }
    }
}
