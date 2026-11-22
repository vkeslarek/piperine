//! `TypeItems`: the type arena of the IR as Rust items. Each source struct or enum prints as one
//! Rust type, so the generated text reads like the source (CV-20 AC 4, design R3): a `struct` with
//! named fields, a tuple struct, a unit struct, an `enum` with its variants and their fields. The
//! derives are the ones the source states (Appendix H.8, `Repr` aside). A type that holds an `f64`,
//! directly or through another type, is generic over the code's real `R` (D11), so `analog` code
//! differentiates through its fields; a type with none is a plain item.

use std::collections::BTreeSet;

use piperine_ir::{Derives, FieldsForm, Ir, TypeData, TypeId, TypeKind, ValueType, VariantData};

use crate::Error;
use crate::print::{RealLift, Text, ValuePrinter};

/// Prints the type arena of one design.
#[derive(Debug)]
pub struct TypeItems<'a> {
    ir: &'a Ir,
    /// The Rust name of each type, by arena index: its source name, made distinct when two share one.
    names: Vec<String>,
    /// The arena entries a value of `Option<T>` holds, directly or through a field: an unpacked
    /// `None` reads as the type's default, so each has one.
    optional: BTreeSet<usize>,
    /// The arena entries a composite param holds that hold a real: each has a `lift` (`lift.rs`).
    lifted: BTreeSet<usize>,
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
        let mut optional = BTreeSet::new();
        for ty in ir.expression_types() {
            if let ValueType::Option(inner) = ty {
                Self::collect_aggregates(ir, inner, &mut optional);
            }
        }
        let mut lifted = BTreeSet::new();
        for body in ir.bodies() {
            for param in body.params().iter().filter(|param| param.ty.is_composite()) {
                Self::collect_aggregates(ir, &param.ty, &mut lifted);
            }
        }
        let mut items = TypeItems { ir, names, optional, lifted };
        let held = std::mem::take(&mut items.lifted);
        items.lifted = held.into_iter().filter(|index| items.holds_real(&Self::as_value(ir, TypeId(*index as u32)))).collect();
        items
    }

    /// Adds to `found` every arena entry `ty` holds: itself, and what its fields hold.
    fn collect_aggregates(ir: &Ir, ty: &ValueType, found: &mut BTreeSet<usize>) {
        match ty {
            ValueType::Tuple(parts) => parts.iter().for_each(|part| Self::collect_aggregates(ir, part, found)),
            ValueType::Array(element, _) | ValueType::Option(element) => Self::collect_aggregates(ir, element, found),
            ValueType::Struct(id) | ValueType::Enum(id) => {
                if found.insert(id.index()) {
                    match &ir.type_data(*id).kind {
                        TypeKind::Struct { fields, .. } => fields.iter().for_each(|(_, field)| Self::collect_aggregates(ir, field, found)),
                        TypeKind::Enum { variants, .. } => variants.iter().flat_map(|v| &v.fields).for_each(|(_, field)| Self::collect_aggregates(ir, field, found)),
                    }
                }
            }
            _ => {}
        }
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
            ValueType::Array(element, _) | ValueType::Option(element) => self.holds_real_visiting(element, visited),
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
        let fields = ValuePrinter::of_type(self.ir, data, false);
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
        let mut text = t.finish();
        if self.lifted.contains(&id.index()) {
            text.push('\n');
            text.push_str(&RealLift::new(self.ir, self).item(id)?);
        }
        if data.derives.repr {
            text.push('\n');
            text.push_str(&self.carried_impls(id)?);
        } else if self.optional.contains(&id.index()) && !(data.derives.default && matches!(data.kind, TypeKind::Struct { .. })) {
            // `None` unpacks to the default of the payload (`Option::unwrap_or_default`)
            text.push('\n');
            text.push_str(&self.default_impl(id, &ValuePrinter::of_type(self.ir, data, true))?);
        }
        Ok(text)
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

// ---- the carried form of a type that derives `Repr` (§2) ----------------------------------------------------------

/// What one variant (or the one struct) contributes to the carried impl: the printed constructor
/// pieces over its fields.
struct Shape {
    /// The Rust type of each field, at `f64`.
    types: Vec<String>,
    /// The pattern binding each field to `field<k>_`, `None` for a form without fields.
    pattern: String,
    /// The expression building the value from the fields read as `read`.
    build: Box<dyn Fn(&[String]) -> String>,
}

impl TypeItems<'_> {
    /// The head of an impl for type `id`: the name, at `f64` where the type holds a real.
    fn head(&self, id: TypeId) -> String {
        if self.holds_real(&Self::as_value(self.ir, id)) { format!("{}<f64>", self.name(id)) } else { self.name(id).to_string() }
    }

    /// `impl rt::flag::Carried` and `Levelwise` for type `id` (a struct concatenates its fields, first
    /// lowest; an enum lays out its tag, then the widest payload, padded with false levels, §2), and
    /// a `Default` unless the source derives one (`Carried` holds an unknown value as the default).
    fn carried_impls(&self, id: TypeId) -> Result<String, Error> {
        let data = self.ir.type_data(id);
        let derives = &data.derives;
        if !(derives.clone && derives.copy && derives.partial_eq) {
            return Err(Error::Unsupported { form: format!("the type {}, which derives Repr without Clone, Copy and PartialEq", data.path), path: format!("type {}", data.path), origin: data.origin.clone() });
        }
        let printer = ValuePrinter::of_type(self.ir, data, true);
        let head = self.head(id);
        let mut t = Text::new();
        t.line(&format!("/// MSL `{}` as a carried value: its bytes and levels follow section 2 of the language.", data.path));
        t.line("#[allow(dead_code)]");
        t.line(&format!("impl rt::flag::Carried for {head} {{"));
        let mut written = Ok(());
        t.indented(|t| {
            written = match &data.kind {
                TypeKind::Struct { form, fields } => self.struct_carried(t, &printer, id, *form, fields),
                TypeKind::Enum { tag_levels, variants } => self.enum_carried(t, &printer, id, *tag_levels, variants),
            };
        });
        written?;
        t.line("}");
        t.line("");
        t.line(&format!("impl rt::flag::Levelwise for {head} {{"));
        t.indented(|t| {
            t.line(&format!("fn same_levels(self, other: {head}) -> bool {{"));
            t.indented(|t| t.line("rt::flag::Carried::to_levels(self) == rt::flag::Carried::to_levels(other)"));
            t.line("}");
        });
        t.line("}");
        let derived_default = derives.default && matches!(data.kind, TypeKind::Struct { .. });
        if !derived_default {
            t.line("");
            t.line(&self.default_impl(id, &printer)?);
        }
        Ok(t.finish())
    }

    /// The constructor of fields at `f64`: pieces of `Name { a: x, b: y }`, `Name(x, y)` or `Name`.
    fn shape(&self, printer: &ValuePrinter<'_>, data: &TypeData, name: String, form: FieldsForm, fields: &[(String, ValueType)]) -> Result<Shape, Error> {
        let types = fields.iter().map(|(field, ty)| printer.rust_type(ty, &format!("type {} field {field}", data.path))).collect::<Result<Vec<_>, _>>()?;
        let labels: Vec<String> = fields.iter().map(|(field, _)| Self::label(field)).collect();
        let bound: Vec<String> = (0..fields.len()).map(|k| Text::generated(&format!("field{k}"))).collect();
        let pattern = match form {
            FieldsForm::Unit => name.clone(),
            FieldsForm::Tuple => format!("{name}({})", bound.join(", ")),
            FieldsForm::Named => format!("{name} {{ {} }}", labels.iter().zip(&bound).map(|(label, b)| format!("{label}: {b}")).collect::<Vec<_>>().join(", ")),
        };
        let build = move |values: &[String]| match form {
            FieldsForm::Unit => name.clone(),
            FieldsForm::Tuple => format!("{name}({})", values.join(", ")),
            FieldsForm::Named => format!("{name} {{ {} }}", labels.iter().zip(values).map(|(label, value)| format!("{label}: {value}")).collect::<Vec<_>>().join(", ")),
        };
        Ok(Shape { types, pattern, build: Box::new(build) })
    }

    /// A field label as it prints: a number as it is, a name as `Text::ident`.
    fn label(name: &str) -> String {
        if name.chars().all(|c| c.is_ascii_digit()) { name.to_string() } else { Text::ident(name) }
    }

    /// `<ty as rt::flag::Carried>::<what>` summed over `types`, `0` for none.
    fn sum(types: &[String], what: &str) -> String {
        if types.is_empty() { "0".to_string() } else { types.iter().map(|ty| format!("<{ty} as rt::flag::Carried>::{what}")).collect::<Vec<_>>().join(" + ") }
    }

    /// The associated items of a struct's `Carried`: the fields concatenated, the first lowest.
    fn struct_carried(&self, t: &mut Text, printer: &ValuePrinter<'_>, id: TypeId, form: FieldsForm, fields: &[(String, ValueType)]) -> Result<(), Error> {
        let data = self.ir.type_data(id);
        let head = self.head(id);
        let shape = self.shape(printer, data, self.name(id).to_string(), form, fields)?;
        let accessors: Vec<String> = fields.iter().map(|(field, _)| format!("self.{}", Self::label(field))).collect();
        t.line(&format!("const WIDTH: usize = {};", Self::sum(&shape.types, "WIDTH")));
        t.line("const SIZE: usize = Self::WIDTH + 1;");
        t.line(&format!("const LEVELS: u32 = {};", Self::sum(&shape.types, "LEVELS")));
        t.line("fn encode(self, out: &mut [u8]) {");
        t.indented(|t| {
            t.line("let mut at = 0;");
            accessors.iter().for_each(|field| t.line(&format!("rt::flag::Field::write({field}, out, &mut at);")));
        });
        t.line("}");
        let reads: Vec<String> = fields.iter().map(|_| "rt::flag::Field::read(bytes, &mut at)".to_string()).collect();
        t.line(&format!("fn decode(bytes: &[u8]) -> {head} {{"));
        t.indented(|t| {
            t.line("let mut at = 0;");
            t.line(&(shape.build)(&reads));
        });
        t.line("}");
        t.line("fn to_levels(self) -> rt::flag::LevelWords {");
        t.indented(|t| t.line(&format!("rt::flag::LevelWords::concat::<{}>([{}])", fields.len(), accessors.iter().map(|field| format!("rt::flag::Carried::to_levels({field})")).collect::<Vec<_>>().join(", "))));
        t.line("}");
        let levels: Vec<String> = fields.iter().map(|_| "rt::flag::Field::levels(words, &mut at)".to_string()).collect();
        t.line(&format!("fn from_levels(words: &rt::flag::LevelWords) -> {head} {{"));
        t.indented(|t| {
            t.line("let mut at = 0;");
            t.line(&(shape.build)(&levels));
        });
        t.line("}");
        Ok(())
    }

    /// The associated items of an enum's `Carried`: the tag, then the widest payload.
    fn enum_carried(&self, t: &mut Text, printer: &ValuePrinter<'_>, id: TypeId, tag_levels: u32, variants: &[VariantData]) -> Result<(), Error> {
        let data = self.ir.type_data(id);
        let head = self.head(id);
        let tag_bytes = Self::tag_bytes(tag_levels);
        let mut shapes = Vec::new();
        for variant in variants {
            let name = format!("{}::{}", self.name(id), Text::ident(&variant.name));
            shapes.push((variant, self.shape(printer, data, name, variant.form, &variant.fields)?));
        }
        let widths: Vec<String> = shapes.iter().map(|(_, shape)| Self::sum(&shape.types, "WIDTH")).collect();
        let levels: Vec<String> = shapes.iter().map(|(_, shape)| format!("({}) as usize", Self::sum(&shape.types, "LEVELS"))).collect();
        t.line(&format!("const WIDTH: usize = {tag_bytes} + rt::flag::Tag::widest(&[{}]);", widths.join(", ")));
        t.line("const SIZE: usize = Self::WIDTH + 1;");
        t.line(&format!("const LEVELS: u32 = {tag_levels} + rt::flag::Tag::widest(&[{}]) as u32;", levels.join(", ")));
        t.line("fn encode(self, out: &mut [u8]) {");
        t.indented(|t| {
            t.line("out[..Self::WIDTH].fill(0);");
            t.line("match self {");
            t.indented(|t| {
                for (variant, shape) in &shapes {
                    let writes: Vec<String> = (0..variant.fields.len()).map(|k| format!("rt::flag::Field::write({}, out, &mut at);", Text::generated(&format!("field{k}")))).collect();
                    let at = if writes.is_empty() { String::new() } else { format!(" let mut at = {tag_bytes}; {}", writes.join(" ")) };
                    t.line(&format!("{} => {{ rt::flag::Tag::put({}, out, {tag_bytes});{at} }}", shape.pattern, variant.discriminant));
                }
            });
            t.line("}");
        });
        t.line("}");
        t.line(&format!("fn decode(bytes: &[u8]) -> {head} {{"));
        t.indented(|t| {
            t.line(&format!("match rt::flag::Tag::get(bytes, {tag_bytes}) {{"));
            t.indented(|t| {
                for (variant, shape) in &shapes {
                    let reads: Vec<String> = variant.fields.iter().map(|_| "rt::flag::Field::read(bytes, &mut at)".to_string()).collect();
                    let built = (shape.build)(&reads);
                    let at = if reads.is_empty() { String::new() } else { format!("let mut at = {tag_bytes}; ") };
                    t.line(&format!("{} => {{ {at}{built} }}", variant.discriminant));
                }
                t.line(&format!("_ => <{head} as Default>::default(),"));
            });
            t.line("}");
        });
        t.line("}");
        let tags: Vec<String> = variants.iter().map(|v| v.discriminant.to_string()).collect();
        t.line("fn is_valid(bytes: &[u8]) -> bool {");
        t.indented(|t| t.line(&format!("matches!(rt::flag::Tag::get(bytes, {tag_bytes}), {})", tags.join(" | "))));
        t.line("}");
        t.line("fn to_levels(self) -> rt::flag::LevelWords {");
        t.indented(|t| {
            t.line("let (tag, payload) = match self {");
            t.indented(|t| {
                for (variant, shape) in &shapes {
                    let parts: Vec<String> = (0..variant.fields.len()).map(|k| format!("rt::flag::Carried::to_levels({})", Text::generated(&format!("field{k}")))).collect();
                    t.line(&format!("{} => ({}u64, rt::flag::LevelWords::concat::<{}>([{}])),", shape.pattern, variant.discriminant, parts.len(), parts.join(", ")));
                }
            });
            t.line("};");
            t.line(&format!("rt::flag::LevelWords::concat([rt::flag::LevelWords::of(tag, {tag_levels}), payload.padded(Self::LEVELS - {tag_levels})])"));
        });
        t.line("}");
        t.line(&format!("fn from_levels(words: &rt::flag::LevelWords) -> {head} {{"));
        t.indented(|t| {
            // an enum whose variants have no fields has no payload to read
            if shapes.iter().any(|(variant, _)| !variant.fields.is_empty()) {
                t.line(&format!("let payload = words.slice({tag_levels}, Self::LEVELS - {tag_levels});"));
            }
            t.line(&format!("match words.slice(0, {tag_levels}).low() {{"));
            t.indented(|t| {
                for (variant, shape) in &shapes {
                    let reads: Vec<String> = variant.fields.iter().map(|_| "rt::flag::Field::levels(&payload, &mut at)".to_string()).collect();
                    let built = (shape.build)(&reads);
                    let at = if reads.is_empty() { String::new() } else { "let mut at = 0; ".to_string() };
                    t.line(&format!("{} => {{ {at}{built} }}", variant.discriminant));
                }
                t.line(&format!("_ => <{head} as Default>::default(),"));
            });
            t.line("}");
        });
        t.line("}");
        Ok(())
    }

    /// How many bytes the tag of `levels` levels takes in the byte form: none for no levels, else the
    /// narrowest of 1, 2, 4 and 8 (the width of an integer of that many bits, `rt::flag`).
    fn tag_bytes(levels: u32) -> usize {
        match levels {
            0 => 0,
            1..=8 => 1,
            9..=16 => 2,
            17..=32 => 4,
            _ => 8,
        }
    }

    /// `impl Default` for a type that does not derive one: every field its default; an enum is its
    /// first variant (an unknown value is the default, and an enum has none of its own, H.8 rule 4).
    fn default_impl(&self, id: TypeId, printer: &ValuePrinter<'_>) -> Result<String, Error> {
        let data = self.ir.type_data(id);
        let generic = self.holds_real(&Self::as_value(self.ir, id));
        let (impl_head, ty) = if generic { ("impl<R: Default> Default".to_string(), format!("{}<R>", self.name(id))) } else { ("impl Default".to_string(), self.name(id).to_string()) };
        let defaults = |count: usize| vec!["Default::default()".to_string(); count];
        let built = match &data.kind {
            TypeKind::Struct { form, fields } => (self.shape(printer, data, self.name(id).to_string(), *form, fields)?.build)(&defaults(fields.len())),
            TypeKind::Enum { variants, .. } => {
                let first = variants.first().ok_or_else(|| Error::Unsupported { form: format!("the enum {} without a variant", data.path), path: format!("type {}", data.path), origin: data.origin.clone() })?;
                let name = format!("{}::{}", self.name(id), Text::ident(&first.name));
                (self.shape(printer, data, name, first.form, &first.fields)?.build)(&defaults(first.fields.len()))
            }
        };
        let mut t = Text::new();
        t.line("#[allow(dead_code)]");
        t.line(&format!("{impl_head} for {ty} {{"));
        t.indented(|t| {
            t.line(&format!("fn default() -> {ty} {{"));
            t.indented(|t| t.line(&built));
            t.line("}");
        });
        t.line("}");
        Ok(t.finish())
    }
}
