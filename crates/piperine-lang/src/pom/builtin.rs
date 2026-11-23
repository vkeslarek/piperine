//! The closed table of names the language provides without a declaration:
//! primitive types, `Some`/`None`, bounds and
//! every row of the §14 table of core primitives. Plain rows and plain
//! functions; no macro.

use serde::{Deserialize, Serialize};

/// What a built-in is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuiltinKind {
    Type,
    Trait,
    Variant,
    Fn,
    Module,
    Method,
    Event,
    Statement,
    /// A reference node of the language: `gnd`.
    Node,
    /// A name every module instance carries (`m`, `dtemp`, §8.5): in scope
    /// only inside a module body and the blocks bound to a module.
    ImplicitParam,
    /// An associated item of a built-in trait (`Repr::WIDTH`), reached only
    /// through a generic bounded by that trait (`builtin_member`), never as a name.
    Member,
}

/// One row of [`BUILTINS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinInfo {
    /// The spelling; `uN`, `iN` and `fN` stand for every width of the family.
    pub name: &'static str,
    pub kind: BuiltinKind,
    /// The spec section that defines it.
    pub section: &'static str,
}

const fn row(name: &'static str, kind: BuiltinKind, section: &'static str) -> BuiltinInfo {
    BuiltinInfo { name, kind, section }
}

/// Index of the `uN`, `iN` and `fN` family rows in [`BUILTINS`].
const UINT_ROW: usize = 0;
const INT_ROW: usize = 1;
const FLOAT_ROW: usize = 2;

pub const BUILTINS: &[BuiltinInfo] = &[
    row("uN", BuiltinKind::Type, "§1"),
    row("iN", BuiltinKind::Type, "§1"),
    row("fN", BuiltinKind::Type, "§1"),
    row("bool", BuiltinKind::Type, "§1"),
    row("usize", BuiltinKind::Type, "§1"),
    row("isize", BuiltinKind::Type, "§1"),
    row("UInt", BuiltinKind::Type, "§1"),
    row("Int", BuiltinKind::Type, "§1"),
    row("Float", BuiltinKind::Type, "§1"),
    row("Option", BuiltinKind::Type, "§1"),
    row("Event", BuiltinKind::Type, "§13"),
    row("Analysis", BuiltinKind::Type, "§11.1"),
    row("Some", BuiltinKind::Variant, "§1"),
    row("None", BuiltinKind::Variant, "§1"),
    row("Dc", BuiltinKind::Variant, "§11.1.1"),
    row("Transient", BuiltinKind::Variant, "§11.1.1"),
    row("Ac", BuiltinKind::Variant, "§11.1.1"),
    row("Noise", BuiltinKind::Variant, "§11.1.1"),
    row("Static", BuiltinKind::Variant, "§11.1.1"),
    row("Repr", BuiltinKind::Trait, "§2"),
    row("Nature", BuiltinKind::Trait, "§4"),
    row("Fn", BuiltinKind::Trait, "§3"),
    row("PartialEq", BuiltinKind::Trait, "App. H"),
    row("Eq", BuiltinKind::Trait, "App. H"),
    row("PartialOrd", BuiltinKind::Trait, "App. H"),
    row("Ord", BuiltinKind::Trait, "App. H"),
    row("Default", BuiltinKind::Trait, "App. H"),
    row("Debug", BuiltinKind::Trait, "App. H"),
    row("Hash", BuiltinKind::Trait, "App. H"),
    row("str", BuiltinKind::Type, "§1"),
    row("gnd", BuiltinKind::Node, "§6"),
    row("m", BuiltinKind::ImplicitParam, "§8.5"),
    row("dtemp", BuiltinKind::ImplicitParam, "§8.5"),
    row("Repr::WIDTH", BuiltinKind::Member, "§2"),
    row("Repr::encode", BuiltinKind::Member, "§2"),
    row("Repr::decode", BuiltinKind::Member, "§2"),
    row("potential", BuiltinKind::Fn, "§14.1"),
    row("flow", BuiltinKind::Fn, "§14.1"),
    row("ddx", BuiltinKind::Fn, "§14.2"),
    row("ac_stim", BuiltinKind::Fn, "§14.3"),
    row("temperature", BuiltinKind::Fn, "§14.4"),
    row("simparam", BuiltinKind::Fn, "§14.4"),
    row("analysis", BuiltinKind::Fn, "§14.4"),
    row("now", BuiltinKind::Fn, "§14.4"),
    row("freq", BuiltinKind::Fn, "§14.4"),
    row("gmin", BuiltinKind::Fn, "§14.4"),
    row("source_scale", BuiltinKind::Fn, "§14.4"),
    row("ddt", BuiltinKind::Module, "§14.5"),
    row("delay", BuiltinKind::Module, "§14.6"),
    row("limit", BuiltinKind::Module, "§14.7"),
    row("noise", BuiltinKind::Module, "§14.8"),
    row("cross", BuiltinKind::Module, "§14.9"),
    row("at", BuiltinKind::Module, "§14.9"),
    row("settle", BuiltinKind::Event, "§14.9"),
    row("bound_step", BuiltinKind::Statement, "§14.10"),
    row("changed", BuiltinKind::Method, "§14.9"),
    row("prev", BuiltinKind::Method, "§14.12"),
    row("others", BuiltinKind::Method, "§14.12"),
    row("inflow", BuiltinKind::Method, "§14.1"),
];

/// An entry of the built-in table. The low byte is the row of [`BUILTINS`];
/// the high byte is the width of a `uN`, `iN` or `fN` type (0 otherwise).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BuiltinId(pub u16);

impl BuiltinId {
    fn new(row: usize, width: u8) -> BuiltinId {
        BuiltinId(row as u16 | (u16::from(width) << 8))
    }

    /// The table row; for a sized type, the row of its family.
    pub fn info(self) -> &'static BuiltinInfo {
        &BUILTINS[usize::from(self.0 & 0xff).min(BUILTINS.len() - 1)]
    }

    /// The width of a `uN`, `iN` or `fN` entry.
    pub fn width(self) -> Option<u8> {
        match self.0 >> 8 {
            0 => None,
            w => Some(w as u8),
        }
    }

    /// The exact spelling: `u8` for the entry the family row `uN` gives width 8.
    pub fn name(self) -> String {
        let info = self.info();
        match self.width() {
            Some(w) => format!("{}{w}", &info.name[..1]),
            None => info.name.to_string(),
        }
    }
}

/// The entry named `name`, if any. Method rows are not names: they are
/// reached only as members (`builtin_method`).
pub fn builtin(name: &str) -> Option<BuiltinId> {
    if let Some(id) = sized(name) {
        return Some(id);
    }
    let row = BUILTINS.iter().position(|b| b.name == name && !matches!(b.kind, BuiltinKind::Method | BuiltinKind::ImplicitParam | BuiltinKind::Member))?;
    // The family rows `uN`, `iN`, `fN` are spellings of no name.
    if row <= FLOAT_ROW {
        return None;
    }
    Some(BuiltinId::new(row, 0))
}

/// The method row named `name` (`prev`, `others`, `level`, `inflow`, `changed`).
pub fn builtin_method(name: &str) -> Option<BuiltinId> {
    let row = BUILTINS.iter().position(|b| b.name == name && b.kind == BuiltinKind::Method)?;
    Some(BuiltinId::new(row, 0))
}

/// The member row `owner::name` (`Repr::WIDTH`).
pub fn builtin_member(owner: &str, name: &str) -> Option<BuiltinId> {
    let full = format!("{owner}::{name}");
    let row = BUILTINS.iter().position(|b| b.name == full && b.kind == BuiltinKind::Member)?;
    Some(BuiltinId::new(row, 0))
}

/// The implicit param row named `name` (`m`, `dtemp`): a name only inside a
/// module body and the blocks bound to a module, so `builtin` never gives it.
pub fn builtin_implicit(name: &str) -> Option<BuiltinId> {
    let row = BUILTINS.iter().position(|b| b.name == name && b.kind == BuiltinKind::ImplicitParam)?;
    Some(BuiltinId::new(row, 0))
}

/// `u1`..`u128`, `i1`..`i128`, `f16`/`f32`/`f64`/`f128`.
fn sized(name: &str) -> Option<BuiltinId> {
    let digits = name.get(1..)?;
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let width: u16 = digits.parse().ok()?;
    match name.as_bytes()[0] {
        b'u' if (1..=128).contains(&width) => Some(BuiltinId::new(UINT_ROW, width as u8)),
        b'i' if (1..=128).contains(&width) => Some(BuiltinId::new(INT_ROW, width as u8)),
        b'f' if [16, 32, 64, 128].contains(&width) => Some(BuiltinId::new(FLOAT_ROW, width as u8)),
        _ => None,
    }
}

/// Is `name` a core name a model must not redefine (§14 rule 1, `E0457`):
/// a core primitive of §14, `gnd`, `Nature` or `str`? `m` and `dtemp` are not
/// reported here: their rule is `E0375` (§8.5.2), left to elaboration.
pub(crate) fn is_core_primitive(name: &str) -> bool {
    matches!(name, "gnd" | "Nature" | "str")
        || BUILTINS.iter().any(|b| b.name == name && b.section.starts_with("§14") && b.kind != BuiltinKind::Method)
}
