//! POM [`Nature`] — a stand-alone, derivable continuous-quantity declaration,
//! LRM §3.6.1.
//!
//! PHDL folded the nature inline inside a discipline; Verilog-AMS promotes
//! it to a top-level, independently declared and referenced construct. The
//! POM mirrors that structure (spec VAMS-01, design.md §5.1): a `Nature` is
//! a POM node in its own right, looked up by name from a discipline's
//! `potential`/`flow` binding and from `idt_nature`/`ddt_nature` references.
//!
//! The five standard attributes (§3.6.1.2) are **named fields**, not map
//! entries — `abstol` drives solver tolerance, `ddt_nature`/`idt_nature`
//! drive operator typing, and a stringly-keyed typo in a map would fail
//! silently, which is precisely the fail-loud violation `CLAUDE.md`
//! forbids. User-defined attributes (§3.6.1.3) live in their own map,
//! preserving the type-level separation between LRM-standard and
//! author-defined surface.

use std::collections::HashMap;

use crate::pom::node::Kind;
use crate::pom::traits::{Kinded, Named};
use crate::value::Value;

/// A Verilog-AMS `nature` declaration, LRM §3.6.1 — a named continuous
/// quantity (Voltage, Current, Charge, …) optionally derived from a parent
/// nature, carrying the five LRM-standard attributes plus any number of
/// user-defined ones.
///
/// Post-elaboration form: the AST carries `abstol`/`units` as `Expr`
/// (constant expressions in source); the POM stores the evaluated `f64`
/// and `String` values. Evaluation happens in T23 (`lower natures,
/// disciplines and branches`); the POM type is landed first so T18/T19/T20
/// can reference it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Nature {
    #[serde(skip)]
    pub span: Option<miette::SourceSpan>,
    /// Attached `///` doc comment, if any (MD-25 — additive, LSP-07).
    #[serde(default)]
    pub doc: Option<String>,
    pub name: String,
    /// `nature Voltage : Potential;` — a derived nature, §3.6.1.1. `None`
    /// for a base nature. The elaborator resolves the parent name to its
    /// `Nature` and inherits un-overridden attributes per §3.6.1.2.
    #[serde(default)]
    pub parent: Option<String>,
    /// **`abstol`** — the maximum negligible value for signals of this
    /// nature, §3.6.1.2. Required for base natures; a derived nature
    /// inherits its parent's value if unset. Stored evaluated; reaches
    /// the solver's per-net tolerance.
    #[serde(default)]
    pub abstol: Option<f64>,
    /// **`access`** — the access-function name (e.g. `V` for Voltage,
    /// `I` for Current), §3.6.1.2. Required for base natures; illegal for
    /// a derived nature to change.
    #[serde(default)]
    pub access: Option<String>,
    /// **`units`** — the unit string annotation (e.g. `"V"`, `"A"`),
    /// §3.6.1.2. Required for base natures; illegal for a derived nature
    /// to change. Used in net-compatibility checks (§3.11.1).
    #[serde(default)]
    pub units: Option<String>,
    /// **`idt_nature`** — the nature representing this nature's time
    /// integral, §3.6.1.2. Optional; defaults to the nature itself when
    /// unset (the common case for `Charge`/`Voltage`).
    #[serde(default)]
    pub idt_nature: Option<String>,
    /// **`ddt_nature`** — the nature representing this nature's time
    /// derivative, §3.6.1.2. Optional; defaults to the nature itself.
    #[serde(default)]
    pub ddt_nature: Option<String>,
    /// User-defined attributes only (§3.6.1.3) — typical examples are
    /// `max`/`min`/`maxval` value-range bounds. Anything that is not one
    /// of the five standard fields above lands here, evaluated to a
    /// [`Value`].
    #[serde(default)]
    pub user_attrs: HashMap<String, Value>,
}

impl Nature {
    /// The nature's declared name (`nature Voltage` → `"Voltage"`).
    pub fn name(&self) -> &str {
        &self.name
    }
    /// The parent nature name, if this is a derived nature.
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }
    /// Whether this nature is derived from another (`parent.is_some()`).
    pub fn is_derived(&self) -> bool {
        self.parent.is_some()
    }
    /// The `abstol` value, if set on this nature (does not chase the
    /// parent chain — that is an elaboration-time resolution, T23).
    pub fn abstol(&self) -> Option<f64> {
        self.abstol
    }
    /// The access-function name, if set.
    pub fn access(&self) -> Option<&str> {
        self.access.as_deref()
    }
    /// The unit-string annotation, if set.
    pub fn units(&self) -> Option<&str> {
        self.units.as_deref()
    }
    /// The `idt_nature` name, if set.
    pub fn idt_nature(&self) -> Option<&str> {
        self.idt_nature.as_deref()
    }
    /// The `ddt_nature` name, if set.
    pub fn ddt_nature(&self) -> Option<&str> {
        self.ddt_nature.as_deref()
    }
    /// User-defined attributes (§3.6.1.3).
    pub fn user_attrs(&self) -> &HashMap<String, Value> {
        &self.user_attrs
    }
    /// Attached `///` doc comment, if any.
    pub fn doc(&self) -> Option<&str> {
        self.doc.as_deref()
    }
}

impl Named for Nature {
    fn name(&self) -> &str {
        self.name()
    }
}

impl Kinded for Nature {
    fn kind(&self) -> Kind {
        Kind::Nature
    }
}

/// §3.6.2.5: applies a discipline's own attribute overrides for one slot
/// (`potential`/`flow`) on top of `base` — the nature currently bound
/// there — producing the *effective* nature nets of that discipline see.
/// Only `abstol` gets typed handling (it is the LRM's own worked example
/// and the attribute that reaches the solver's per-node tolerance,
/// §6.5.8); any other override name lands in `user_attrs` evaluated as a
/// constant expression. `access`/`units` overrides never reach here —
/// §3.6.1.2's restriction is enforced at parse time
/// ([`crate::error::ParseErrorKind::RestrictedNatureAttributeOverride`]).
pub fn apply_discipline_overrides(
    base: &Nature,
    disc: &crate::ast::DisciplineDecl,
    slot: crate::ast::NatureSlot,
) -> Result<Nature, crate::elab::const_eval::ConstEvalError> {
    let mut n = base.clone();
    let env = crate::elab::const_eval::ConstEnv::new();
    for (s, attr, expr) in &disc.attr_overrides {
        if *s != slot {
            continue;
        }
        if attr == "abstol" {
            n.abstol = Some(env.eval_f64(expr)?);
        } else {
            let v = env.eval(expr)?;
            n.user_attrs.insert(attr.clone(), v);
        }
    }
    Ok(n)
}
