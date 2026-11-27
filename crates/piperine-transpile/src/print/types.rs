//! `TypeItems`: the type arena of the IR as Rust items. Each source struct or enum prints as one
//! Rust type, so the generated text reads like the source (CV-20 AC 4, design R3): a `struct` with
//! named fields, a tuple struct, a unit struct, an `enum` with its variants and their fields. The
//! derives are the ones the source states (Appendix H.8, `Repr` aside). A type that holds an `f64`,
//! directly or through another type, is generic over the code's real `R` (D11), so `analog` code
//! differentiates through its fields; a type with none is a plain item.

use std::collections::BTreeSet;

use piperine_ir::{Derives, FieldsForm, FunctionId, Ir, ReprForm, TypeData, TypeId, TypeKind, ValueType, VariantData};

use crate::Error;
use crate::print::{RealLift, Text, ExpressionPrinter};

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
    /// The arena entries a composite param holds, with or without `Repr`: a param is never carried (§8.1.15) but its pin holds the
    /// value's bytes, so each is printed with `Clone, Copy, PartialEq` and its `Carried` form (a value is copied, §1.4.6).
    pinned: BTreeSet<usize>,
}

impl<'a> TypeItems<'a> {
    /// The items of `ir`'s type arena, each type under its Rust name.
    pub fn new(ir: &'a Ir) -> TypeItems<'a> {
        let mut names: Vec<String> = Vec::with_capacity(ir.types().len());
        for (index, data) in ir.types().iter().enumerate() {
            let plain = if data.instance.is_some() { Text::instance_type(&data.name) } else { Text::ident(&data.name) };
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
            for param in body.params().iter().filter(|param| ir.is_composite_value(&param.ty)) {
                Self::collect_aggregates(ir, &param.ty, &mut lifted);
            }
        }
        // every type a param pin holds, a unit enum included, has its `Carried` form (§8.1.15)
        let mut pinned = lifted.clone();
        for body in ir.bodies() {
            for param in body.params() {
                Self::collect_aggregates(ir, &param.ty, &mut pinned);
            }
        }
        let mut items = TypeItems { ir, names, optional, lifted, pinned };
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
    /// array or an option, or through a struct or an enum.
    pub fn holds_real(&self, ty: &ValueType) -> bool {
        self.holds_real_visiting(ty, &mut BTreeSet::new())
    }

    /// Whether `ty` holds an `f64`; `visited` holds the arena entries being walked, so a type that
    /// contains itself holds one only through its other fields.
    fn holds_real_visiting(&self, ty: &ValueType, visited: &mut BTreeSet<usize>) -> bool {
        match ty {
            ValueType::F64 => true,
            ValueType::Tuple(parts) => parts.iter().any(|part| self.holds_real_visiting(part, visited)),
            ValueType::Array(element, _) | ValueType::Option(element) => self.holds_real_visiting(element, visited),
            ValueType::Fn(params, ret) => params.iter().chain(std::iter::once(&**ret)).any(|part| self.holds_real_visiting(part, visited)),
            ValueType::Struct(id) | ValueType::Enum(id) => {
                if !visited.insert(id.index()) {
                    return false;
                }
                match &self.ir.type_data(*id).kind {
                    TypeKind::Struct { fields, .. } => fields.iter().any(|(_, field)| self.holds_real_visiting(field, visited)),
                    TypeKind::Enum { variants, .. } => variants.iter().any(|v| v.fields.iter().any(|(_, field)| self.holds_real_visiting(field, visited))),
                }
            }
            ValueType::Bool | ValueType::Unit | ValueType::UInt(_) | ValueType::Int(_) | ValueType::F32 | ValueType::F16 | ValueType::F128 => false,
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
        let fields = ExpressionPrinter::of_type(self.ir, data, false);
        let mut t = Text::new();
        let kind = if matches!(data.kind, TypeKind::Struct { .. }) { "a struct" } else { "an enum" };
        t.line(&format!("/// MSL `{}`, {kind}, written at {}:{}:{}.", data.path, data.origin.file, data.origin.line, data.origin.col));
        t.line("#[allow(dead_code)]");
        if let Some(derive) = Self::derive_line(&self.derives_of(id)) {
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
        if data.derives.repr || !data.repr.is_derived() || self.pinned.contains(&id.index()) {
            text.push('\n');
            text.push_str(&self.carried_impls(id)?);
        } else if self.optional.contains(&id.index()) && !(data.derives.default && matches!(data.kind, TypeKind::Struct { .. })) {
            // `None` unpacks to the default of the payload (`Option::unwrap_or_default`)
            text.push('\n');
            text.push_str(&self.default_impl(id, &ExpressionPrinter::of_type(self.ir, data, true))?);
        }
        Ok(text)
    }

    /// The derives type `id` is printed with: its own, and `Clone, Copy, PartialEq` when a param pin holds it (a type needs no `Repr`
    /// to be a param, §8.1.15, so the derives its pin needs do not come from the source).
    fn derives_of(&self, id: TypeId) -> Derives {
        let mut derives = self.ir.type_data(id).derives;
        if self.pinned.contains(&id.index()) {
            derives.clone = true;
            derives.copy = true;
            derives.partial_eq = true;
        }
        derives
    }

    /// The printed name and Rust type of each field.
    fn fields(&self, printer: &ExpressionPrinter<'_>, data: &TypeData, list: &[(String, ValueType)]) -> Result<Vec<(String, String)>, Error> {
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

    /// `impl rt::pin::Carried` for type `id` (a struct concatenates its fields, first lowest; an
    /// enum lays out its tag, then the widest payload, padded with false levels, §2), and a
    /// `Default` unless the source derives one (a pin whose tag names no variant decodes as the default; `try_from_levels` is `None`).
    fn carried_impls(&self, id: TypeId) -> Result<String, Error> {
        let data = self.ir.type_data(id);
        let derives = &self.derives_of(id);
        if !(derives.clone && derives.copy && derives.partial_eq) {
            return Err(Error::Unsupported { form: format!("the type {}, which derives Repr without Clone, Copy and PartialEq", data.path), path: format!("type {}", data.path), origin: data.origin.clone() });
        }
        let printer = ExpressionPrinter::of_type(self.ir, data, true);
        let head = self.head(id);
        let mut t = Text::new();
        t.line(&format!("/// MSL `{}` as a carried value: its bytes and levels follow section 2 of the language.", data.path));
        t.line("#[allow(dead_code)]");
        t.line(&format!("impl rt::pin::Carried for {head} {{"));
        let mut written = Ok(());
        t.indented(|t| {
            written = match (&data.repr, &data.kind) {
                (ReprForm::HandWritten { width, encode, decode }, _) => self.hand_written_carried(t, id, *width, *encode, *decode),
                (ReprForm::Derived, TypeKind::Struct { form, fields }) => self.struct_carried(t, &printer, id, *form, fields),
                (ReprForm::Derived, TypeKind::Enum { tag_levels, variants }) => self.enum_carried(t, &printer, id, *tag_levels, variants),
            };
        });
        written?;
        t.line("}");
        let derived_default = derives.default && matches!(data.kind, TypeKind::Struct { .. });
        if !derived_default {
            t.line("");
            t.line(&self.default_impl(id, &printer)?);
        }
        Ok(t.finish())
    }

    /// The constructor of fields at `f64`: pieces of `Name { a: x, b: y }`, `Name(x, y)` or `Name`.
    fn shape(&self, printer: &ExpressionPrinter<'_>, data: &TypeData, name: String, form: FieldsForm, fields: &[(String, ValueType)]) -> Result<Shape, Error> {
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

    /// `<ty as rt::pin::Carried>::<what>` summed over `types`, `0` for none.
    fn sum(types: &[String], what: &str) -> String {
        if types.is_empty() { "0".to_string() } else { types.iter().map(|ty| format!("<{ty} as rt::pin::Carried>::{what}")).collect::<Vec<_>>().join(" + ") }
    }

    /// The associated items of the `Carried` of a type whose `Repr` the author wrote (§2, GN-05):
    /// the value is `width` levels, written by the IR function `encode` (the value to `[bool; width]`)
    /// and read by `decode` (`[bool; width]` to `Option`). The pin holds the levels as an unsigned
    /// integer, level `k` being bit `k`. A `None` from `decode` is returned to the caller: it is
    /// `try_from_levels`'s result, and `decode` and `from_levels`, which cannot fail, give the
    /// type's default as an enum does for a tag that names no variant. No flag moves with the levels.
    fn hand_written_carried(&self, t: &mut Text, id: TypeId, width: u32, encode: FunctionId, decode: FunctionId) -> Result<(), Error> {
        let data = self.ir.type_data(id);
        let unsupported = |form: String| Error::Unsupported { form, path: format!("type {}", data.path), origin: data.origin.clone() };
        if !(1..=64).contains(&width) {
            return Err(unsupported(format!("the type {}, whose hand-written Repr has {width} levels outside 1..=64", data.path)));
        }
        let head = self.head(id);
        let bytes = Self::tag_bytes(width);
        let call = |function: FunctionId, argument: &str| -> Result<String, Error> {
            let callee = self.ir.function(function);
            if !ExpressionPrinter::env_reads(self.ir, function).is_empty() {
                return Err(unsupported(format!("the function {}, which reads the environment, in the hand-written Repr of {}", callee.name(), data.path)));
            }
            let generic = if ExpressionPrinter::is_generic(self.ir, function) { "::<f64>" } else { "" };
            Ok(format!("{}{generic}({argument})", Text::ident(callee.name())))
        };
        let (encoded, decoded) = (call(encode, "self")?, call(decode, "levels")?);
        t.line(&format!("const WIDTH: usize = {bytes};"));
        t.line(&format!("const LEVELS: u32 = {width};"));
        t.line("fn encode(self, out: &mut [u8]) {");
        t.indented(|t| {
            t.line("let word = rt::pin::Carried::to_levels(self).low();");
            t.line(&format!("out[..{bytes}].copy_from_slice(&word.to_le_bytes()[..{bytes}]);"));
        });
        t.line("}");
        t.line(&format!("fn decode(bytes: &[u8]) -> {head} {{"));
        t.indented(|t| {
            t.line("let mut raw = [0u8; 8];");
            t.line(&format!("raw[..{bytes}].copy_from_slice(&bytes[..{bytes}]);"));
            t.line(&format!("rt::pin::Carried::from_levels(&rt::pin::LevelWords::of(u64::from_le_bytes(raw), {width}))"));
        });
        t.line("}");
        t.line("fn to_levels(self) -> rt::pin::LevelWords {");
        t.indented(|t| {
            t.line(&format!("let levels = {encoded};"));
            t.line("let mut word = 0u64;");
            t.line("for (k, level) in levels.iter().enumerate() {");
            t.indented(|t| t.line("word |= u64::from(*level) << k;"));
            t.line("}");
            t.line(&format!("rt::pin::LevelWords::of(word, {width})"));
        });
        t.line("}");
        t.line(&format!("fn from_levels(words: &rt::pin::LevelWords) -> {head} {{"));
        t.indented(|t| t.line("rt::pin::Carried::try_from_levels(words).unwrap_or_default()"));
        t.line("}");
        t.line(&format!("fn try_from_levels(words: &rt::pin::LevelWords) -> Option<{head}> {{"));
        t.indented(|t| {
            t.line(&format!("let mut levels = [false; {width}];"));
            t.line("for (k, level) in levels.iter_mut().enumerate() {");
            t.indented(|t| t.line("*level = words.get(k as u32);"));
            t.line("}");
            t.line(&decoded);
        });
        t.line("}");
        Ok(())
    }

    /// The associated items of a struct's `Carried`: the fields concatenated, the first lowest.
    fn struct_carried(&self, t: &mut Text, printer: &ExpressionPrinter<'_>, id: TypeId, form: FieldsForm, fields: &[(String, ValueType)]) -> Result<(), Error> {
        let data = self.ir.type_data(id);
        let head = self.head(id);
        let shape = self.shape(printer, data, self.name(id).to_string(), form, fields)?;
        let accessors: Vec<String> = fields.iter().map(|(field, _)| format!("self.{}", Self::label(field))).collect();
        t.line(&format!("const WIDTH: usize = {};", Self::sum(&shape.types, "WIDTH")));
        t.line(&format!("const LEVELS: u32 = {};", Self::sum(&shape.types, "LEVELS")));
        t.line("fn encode(self, out: &mut [u8]) {");
        t.indented(|t| {
            t.line("let mut at = 0;");
            accessors.iter().for_each(|field| t.line(&format!("rt::pin::Field::write({field}, out, &mut at);")));
        });
        t.line("}");
        let reads: Vec<String> = fields.iter().map(|_| "rt::pin::Field::read(bytes, &mut at)".to_string()).collect();
        t.line(&format!("fn decode(bytes: &[u8]) -> {head} {{"));
        t.indented(|t| {
            t.line("let mut at = 0;");
            t.line(&(shape.build)(&reads));
        });
        t.line("}");
        t.line("fn to_levels(self) -> rt::pin::LevelWords {");
        t.indented(|t| t.line(&format!("rt::pin::LevelWords::concat::<{}>([{}])", fields.len(), accessors.iter().map(|field| format!("rt::pin::Carried::to_levels({field})")).collect::<Vec<_>>().join(", "))));
        t.line("}");
        let levels: Vec<String> = fields.iter().map(|_| "rt::pin::Field::levels(words, &mut at)".to_string()).collect();
        t.line(&format!("fn from_levels(words: &rt::pin::LevelWords) -> {head} {{"));
        t.indented(|t| {
            t.line("let mut at = 0;");
            t.line(&(shape.build)(&levels));
        });
        t.line("}");
        let tried: Vec<String> = fields.iter().map(|_| "rt::pin::Field::try_levels(words, &mut at)?".to_string()).collect();
        t.line(&format!("fn try_from_levels(words: &rt::pin::LevelWords) -> Option<{head}> {{"));
        t.indented(|t| {
            t.line("let mut at = 0;");
            t.line(&format!("Some({})", (shape.build)(&tried)));
        });
        t.line("}");
        Ok(())
    }

    /// The associated items of an enum's `Carried`: the tag, then the widest payload.
    fn enum_carried(&self, t: &mut Text, printer: &ExpressionPrinter<'_>, id: TypeId, tag_levels: u32, variants: &[VariantData]) -> Result<(), Error> {
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
        t.line(&format!("const WIDTH: usize = {tag_bytes} + rt::pin::Tag::widest(&[{}]);", widths.join(", ")));
        t.line(&format!("const LEVELS: u32 = {tag_levels} + rt::pin::Tag::widest(&[{}]) as u32;", levels.join(", ")));
        t.line("fn encode(self, out: &mut [u8]) {");
        t.indented(|t| {
            t.line("out[..Self::WIDTH].fill(0);");
            t.line("match self {");
            t.indented(|t| {
                for (variant, shape) in &shapes {
                    let writes: Vec<String> = (0..variant.fields.len()).map(|k| format!("rt::pin::Field::write({}, out, &mut at);", Text::generated(&format!("field{k}")))).collect();
                    let at = if writes.is_empty() { String::new() } else { format!(" let mut at = {tag_bytes}; {}", writes.join(" ")) };
                    t.line(&format!("{} => {{ rt::pin::Tag::put({}, out, {tag_bytes});{at} }}", shape.pattern, variant.discriminant));
                }
            });
            t.line("}");
        });
        t.line("}");
        t.line(&format!("fn decode(bytes: &[u8]) -> {head} {{"));
        t.indented(|t| {
            t.line(&format!("match rt::pin::Tag::get(bytes, {tag_bytes}) {{"));
            t.indented(|t| {
                for (variant, shape) in &shapes {
                    let reads: Vec<String> = variant.fields.iter().map(|_| "rt::pin::Field::read(bytes, &mut at)".to_string()).collect();
                    let built = (shape.build)(&reads);
                    let at = if reads.is_empty() { String::new() } else { format!("let mut at = {tag_bytes}; ") };
                    t.line(&format!("{} => {{ {at}{built} }}", variant.discriminant));
                }
                t.line(&format!("_ => <{head} as Default>::default(),"));
            });
            t.line("}");
        });
        t.line("}");
        t.line("fn to_levels(self) -> rt::pin::LevelWords {");
        t.indented(|t| {
            t.line("let (tag, payload) = match self {");
            t.indented(|t| {
                for (variant, shape) in &shapes {
                    let parts: Vec<String> = (0..variant.fields.len()).map(|k| format!("rt::pin::Carried::to_levels({})", Text::generated(&format!("field{k}")))).collect();
                    t.line(&format!("{} => ({}u64, rt::pin::LevelWords::concat::<{}>([{}])),", shape.pattern, variant.discriminant, parts.len(), parts.join(", ")));
                }
            });
            t.line("};");
            t.line(&format!("rt::pin::LevelWords::concat([rt::pin::LevelWords::of(tag, {tag_levels}), payload.padded(Self::LEVELS - {tag_levels})])"));
        });
        t.line("}");
        t.line(&format!("fn from_levels(words: &rt::pin::LevelWords) -> {head} {{"));
        t.indented(|t| {
            // an enum whose variants have no fields has no payload to read
            if shapes.iter().any(|(variant, _)| !variant.fields.is_empty()) {
                t.line(&format!("let payload = words.slice({tag_levels}, Self::LEVELS - {tag_levels});"));
            }
            t.line(&format!("match words.slice(0, {tag_levels}).low() {{"));
            t.indented(|t| {
                for (variant, shape) in &shapes {
                    let reads: Vec<String> = variant.fields.iter().map(|_| "rt::pin::Field::levels(&payload, &mut at)".to_string()).collect();
                    let built = (shape.build)(&reads);
                    let at = if reads.is_empty() { String::new() } else { "let mut at = 0; ".to_string() };
                    t.line(&format!("{} => {{ {at}{built} }}", variant.discriminant));
                }
                t.line(&format!("_ => <{head} as Default>::default(),"));
            });
            t.line("}");
        });
        t.line("}");
        t.line(&format!("fn try_from_levels(words: &rt::pin::LevelWords) -> Option<{head}> {{"));
        t.indented(|t| {
            if shapes.iter().any(|(variant, _)| !variant.fields.is_empty()) {
                t.line(&format!("let payload = words.slice({tag_levels}, Self::LEVELS - {tag_levels});"));
            }
            t.line(&format!("match words.slice(0, {tag_levels}).low() {{"));
            t.indented(|t| {
                for (variant, shape) in &shapes {
                    let reads: Vec<String> = variant.fields.iter().map(|_| "rt::pin::Field::try_levels(&payload, &mut at)?".to_string()).collect();
                    let built = (shape.build)(&reads);
                    let at = if reads.is_empty() { String::new() } else { "let mut at = 0; ".to_string() };
                    t.line(&format!("{} => {{ {at}Some({built}) }}", variant.discriminant));
                }
                t.line("_ => None,");
            });
            t.line("}");
        });
        t.line("}");
        Ok(())
    }

    /// How many bytes the tag of `levels` levels takes in the byte form: none for no levels, else the
    /// narrowest of 1, 2, 4 and 8 (the width of an integer of that many bits, `rt::pin`).
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
    fn default_impl(&self, id: TypeId, printer: &ExpressionPrinter<'_>) -> Result<String, Error> {
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
