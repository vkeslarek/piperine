//! `TypeItems`: the type arena of the IR as Rust items. Each source struct or enum prints as one
//! Rust type, so the generated text reads like the source (CV-20 AC 4, design R3): a `struct` with
//! named fields, a tuple struct, a unit struct, an `enum` with its variants and their fields. The
//! derives are the ones the source states (Appendix H.8, `Repr` aside). A type that holds an `f64`,
//! directly or through another type, is generic over the code's real `R` (D11), so `analog` code
//! differentiates through its fields; a type with none is a plain item.

use std::collections::BTreeSet;

use piperine_ir::{Derives, FieldsForm, Ir, TypeData, TypeId, TypeKind, ValueType};

use crate::Error;
use crate::print::{Text, ValuePrinter};

/// Prints the type arena of one design.
#[derive(Debug)]
pub struct TypeItems<'a> {
    ir: &'a Ir,
    /// The Rust name of each type, by arena index: its source name, made distinct when two share one.
    names: Vec<String>,
}

impl<'a> TypeItems<'a> {
    /// The items of `ir`'s type arena, each type under its Rust name.
    pub fn new(ir: &'a Ir) -> TypeItems<'a> {
        let mut names: Vec<String> = Vec::with_capacity(ir.types().len());
        for (index, data) in ir.types().iter().enumerate() {
            let plain = Text::ident(&data.name);
            let name = if names.contains(&plain) { Text::generated(&format!("{}_{index}", data.name)) } else { plain };
            names.push(name);
        }
        TypeItems { ir, names }
    }

    /// The Rust name of type `id`.
    pub fn name(&self, id: TypeId) -> &str {
        &self.names[id.index()]
    }

    /// Whether a value of `ty` holds an `f64` that is the code's real: directly, in a tuple, an
    /// array or an option, or through a struct or an enum. A flagged real is a plain `f64`.
    pub fn holds_real(&self, ty: &ValueType) -> bool {
        self.holds_real_visiting(ty, &mut BTreeSet::new())
    }

    fn holds_real_visiting(&self, ty: &ValueType, visited: &mut BTreeSet<usize>) -> bool {
        match ty {
            ValueType::F64 => true,
            ValueType::Tuple(parts) => parts.iter().any(|part| self.holds_real_visiting(part, visited)),
            ValueType::Array(element, _) | ValueType::Slice(element) | ValueType::Option(element) => self.holds_real_visiting(element, visited),
            ValueType::Struct(id) | ValueType::Enum(id) => {
                if !visited.insert(id.index()) {
                    return false;
                }
                match &self.ir.type_data(*id).kind {
                    TypeKind::Struct { fields, .. } => fields.iter().any(|(_, field)| self.holds_real_visiting(field, visited)),
                    TypeKind::Enum { variants, .. } => variants.iter().any(|v| v.fields.iter().any(|(_, field)| self.holds_real_visiting(field, visited))),
                }
            }
            ValueType::Bool | ValueType::Unit | ValueType::UInt(_) | ValueType::Int(_) | ValueType::Fn(..) | ValueType::Flagged(_) => false,
        }
    }

    /// One Rust item per entry of the arena, in arena order.
    pub fn items(&self) -> Result<Vec<String>, Error> {
        (0..self.ir.types().len()).map(|index| self.item(TypeId(index as u32))).collect()
    }

    /// The item of type `id`: its doc comment, derives and definition.
    fn item(&self, id: TypeId) -> Result<String, Error> {
        let data = self.ir.type_data(id);
        let generic = if self.holds_real(&Self::as_value(self.ir, id)) { "<R>" } else { "" };
        let fields = ValuePrinter::of_type(self.ir, data);
        let mut t = Text::new();
        let kind = if matches!(data.kind, TypeKind::Struct { .. }) { "a struct" } else { "an enum" };
        t.line(&format!("/// MSL `{}`, {kind}, written at {}:{}:{}.", data.path, data.origin.file, data.origin.line, data.origin.col));
        t.line("#[allow(dead_code)]");
        if let Some(derive) = Self::derive_line(&data.derives) {
            t.line(&derive);
        }
        let name = self.name(id);
        match &data.kind {
            TypeKind::Struct { form, fields: list } => {
                let parts = self.fields(&fields, data, list)?;
                match form {
                    FieldsForm::Named => {
                        t.line(&format!("pub struct {name}{generic} {{"));
                        t.indented(|t| parts.iter().for_each(|(field, ty)| t.line(&format!("pub {field}: {ty},"))));
                        t.line("}");
                    }
                    FieldsForm::Tuple => {
                        let types: Vec<String> = parts.iter().map(|(_, ty)| format!("pub {ty}")).collect();
                        t.line(&format!("pub struct {name}{generic}({});", types.join(", ")));
                    }
                    FieldsForm::Unit => t.line(&format!("pub struct {name};")),
                }
            }
            TypeKind::Enum { variants, .. } => {
                // Rust allows a written discriminant only where no variant holds a field.
                let explicit = variants.iter().all(|v| v.fields.is_empty()) && variants.iter().all(|v| isize::try_from(v.discriminant).is_ok());
                t.line(&format!("pub enum {name}{generic} {{"));
                let mut lines = Vec::new();
                for variant in variants {
                    let parts = self.fields(&fields, data, &variant.fields)?;
                    let label = Text::ident(&variant.name);
                    let tail = match variant.form {
                        FieldsForm::Unit if explicit => format!(" = {}", variant.discriminant),
                        FieldsForm::Unit => String::new(),
                        FieldsForm::Tuple => format!("({})", parts.iter().map(|(_, ty)| ty.clone()).collect::<Vec<_>>().join(", ")),
                        FieldsForm::Named => format!(" {{ {} }}", parts.iter().map(|(field, ty)| format!("{field}: {ty}")).collect::<Vec<_>>().join(", ")),
                    };
                    lines.push(format!("{label}{tail},"));
                }
                t.indented(|t| lines.iter().for_each(|line| t.line(line)));
                t.line("}");
            }
        }
        Ok(t.finish())
    }

    /// The printed name and Rust type of each field.
    fn fields(&self, printer: &ValuePrinter<'_>, data: &TypeData, list: &[(String, ValueType)]) -> Result<Vec<(String, String)>, Error> {
        list.iter()
            .map(|(name, ty)| {
                let printed = if name.chars().all(|c| c.is_ascii_digit()) { name.clone() } else { Text::ident(name) };
                Ok((printed, printer.rust_type(ty, &format!("type {} field {name}", data.path))?))
            })
            .collect()
    }

    /// `#[derive(..)]` for the traits the source states and Rust can derive, `None` for none.
    fn derive_line(derives: &Derives) -> Option<String> {
        let all = [
            ("Clone", derives.clone),
            ("Copy", derives.copy),
            ("PartialEq", derives.partial_eq),
            ("Eq", derives.eq),
            ("PartialOrd", derives.partial_ord),
            ("Ord", derives.ord),
            ("Default", derives.default),
            ("Debug", derives.debug),
            ("Hash", derives.hash),
        ];
        let names: Vec<&str> = all.iter().filter(|(_, on)| *on).map(|(name, _)| *name).collect();
        (!names.is_empty()).then(|| format!("#[derive({})]", names.join(", ")))
    }

    /// The value type of arena entry `id`.
    fn as_value(ir: &Ir, id: TypeId) -> ValueType {
        match ir.type_data(id).kind {
            TypeKind::Struct { .. } => ValueType::Struct(id),
            TypeKind::Enum { .. } => ValueType::Enum(id),
        }
    }
}
