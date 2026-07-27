//! Declared-language-surface regression guard — T27 (DLS-25).
//!
//! After every P4 sub-phase (T16, T18, T19, T20, T22, T23, T25, T26)
//! shipped its `extern` declarations into stdlib headers, this fixture
//! is the permanent guard against the mechanism silently regressing
//! back into "magic": a future commit that adds an entry to `MATH_FNS`
//! or `TaskRegistry::with_builtins()` without authoring the matching
//! `extern fn`/`extern task` declaration will fail this test by name.
//!
//! Scope (per design.md's Error Handling Strategy, resolved Open Design
//! Item #3): native-table → extern-decl direction only. The reverse
//! direction (an `extern` decl with no native backing) is covered by
//! the DLS-05 distinct error path (T13, tested in
//! `extern_missing_native_binding.rs`), not this fixture.
//!
//! Sources of truth for the native surface (each gets its own test):
//!   - `math::MATH_FNS`            — `crates/piperine-lang/src/math.rs`
//!   - `eval::tasks::TaskRegistry` — `crates/piperine-lang/src/eval/tasks.rs`
//!   - Runtime operators           — no central Rust table (scattered
//!     string-match in codegen); spec.md
//!     P4-AC4 + headers/operators.phdl
//!     enumerate the contract
//!   - Primitive value types       — spec.md P4-AC1 (the 7 primitives)
//!   - `@device`/`@port` schemas   — `headers/device_port.phdl`
//!     (parsed by `PluginHost::seed_schemas`,
//!     not part of every project's prelude)
//!
//! Out of scope here (per spec Out of Scope):
//!   - `@rfport` — stays hardcoded in `ElabContext::new()` (T23 note);
//!     not a relocation target, deliberately not asserted.
//!   - Capability impls for primitives (`Add`/`Sub`/`Eq`/...) — binary
//!     operators are pure grammar, never dispatched through capabilities
//!     (T26's documented "none found" finding). No textual anchor needed.

use piperine_lang::SourceMap;
use piperine_lang::elab::registry::ElabContext;
use piperine_lang::eval::tasks::TaskRegistry;
use piperine_lang::math::MATH_FNS;
use piperine_lang::parse::ast::{ExternDecl, Item};
use piperine_lang::parse_str;

/// Elaborate a minimal PHDL source against the stdlib prelude, returning
/// the populated `ElabContext`. The prelude (`Resolver::prelude_items`)
/// embeds `headers/types.phdl`/`math.phdl`/`tasks.phdl`/`operators.phdl`
/// via `include_str!`, so this works regardless of the caller's cwd
/// (per T16's deviation note explaining why these four are embedded
/// rather than file-resolved like `headers/spice/`).
fn stdlib_ctx() -> ElabContext {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }
               mod Top ( inout p : Electrical ) { }";
    let (_design, ctx) = parse_str(src)
        .expect("parse minimal harness")
        .elaborate_with_context(&SourceMap::dummy())
        .expect("elaborate minimal harness against stdlib prelude");
    ctx
}

// ── MATH_FNS ─────────────────────────────────────────────────────────────────

/// Every entry in `math::MATH_FNS` (the libm intrinsics table backing
/// `extern fn` declarations in `headers/math.phdl`) must resolve through
/// `CallableRegistry` — i.e. the textual `extern fn` exists. Adding a
/// new libm wrapper to `math.rs` without authoring the matching
/// `extern fn {name}(...)` declaration in `headers/math.phdl` fails
/// this test by name. (T19, DLS-18.)
#[test]
fn every_math_fn_has_a_matching_extern_fn_declaration() {
    let ctx = stdlib_ctx();
    for f in MATH_FNS {
        let Some(_) = ctx.callables.lookup(f.name) else {
            panic!(
                "MATH_FNS entry `{name}` (arity {arity}) has no matching `extern fn {name}` \
                 declaration in headers/math.phdl — add one (mirroring the existing entries' \
                 shape) or remove the libm wrapper from math.rs",
                name = f.name,
                arity = f.arity,
            );
        };
    }
}

// ── TaskRegistry::with_builtins() ────────────────────────────────────────────

/// Every entry in `TaskRegistry::with_builtins()` (the pure-context
/// system-task registry in `eval/tasks.rs`, backing `extern task`
/// declarations in `headers/tasks.phdl`) must resolve through
/// `CallableRegistry` under its `$`-prefixed name. Adding a new `Task`
/// impl to `with_builtins()` without authoring the matching
/// `extern task ${name}(...)` declaration in `headers/tasks.phdl`
/// fails this test. (T20, DLS-19.)
#[test]
fn every_task_registry_entry_has_a_matching_extern_task_declaration() {
    let ctx = stdlib_ctx();
    let tasks = TaskRegistry::with_builtins();
    for name in tasks.names() {
        let extern_name = format!("${name}");
        let Some(_) = ctx.callables.lookup(&extern_name) else {
            panic!(
                "TaskRegistry entry `{name}` has no matching `extern task ${name}` declaration \
                 in headers/tasks.phdl — add one mirroring the existing entries' shape",
            );
        };
    }
}

// ── Runtime operators ────────────────────────────────────────────────────────

/// Every runtime operator declared in `spec.md` P4-AC4 must have a
/// matching `extern operator` (or `extern task` for the `$limit` case)
/// declaration visible in `OperatorRegistry`/`CallableRegistry`. There
/// is no central Rust-side table to iterate here (operators are
/// string-matched inside codegen's `flatten/analog.rs` /
/// `resolve/pom/analog_ops.rs`); the contract lives in the spec and is
/// mirrored in `headers/operators.phdl`. This test asserts that
/// contract is upheld. (T22, DLS-20.)
#[test]
fn every_runtime_operator_has_a_matching_extern_operator_declaration() {
    let ctx = stdlib_ctx();

    // `Expr::Call`-shaped — recognized by `elab/resolve.rs`'s
    // `resolve_operator_call` (T22) before codegen's own string-match.
    let call_shaped = [
        "ddt", "idt", "ddx", "delay", "transition", "slew", "white_noise",
        "flicker_noise",
    ];
    for op in call_shaped {
        assert!(
            ctx.operators.lookup(op).is_some(),
            "Runtime operator `{op}` has no matching `extern operator {op}` declaration \
             in headers/operators.phdl — spec P4-AC4 requires it",
        );
    }

    // `EventSpec::Named`-shaped — `cross`/`above`/`timer` appear inside
    // event blocks (`@ above(x) { ... }`), a distinct grammar construct
    // from `Expr::Call`. Declared for textual/LSP presence only; not
    // enforced by `resolve.rs` (documented in headers/operators.phdl).
    // The regression guard still verifies their textual existence —
    // ctrl+click on the name must resolve.
    let event_shaped = ["cross", "above", "timer"];
    for op in event_shaped {
        assert!(
            ctx.operators.lookup(op).is_some(),
            "EventSpec::Named operator `{op}` has no matching `extern operator {op}` \
             declaration in headers/operators.phdl",
        );
    }

    // `$limit` lexes as `Expr::SysCall` (the `$`-prefixed form), so the
    // `extern operator` grammar can't spell it (parser uses
    // `parse_ident()`, not `parse_syscall_name()`). Declared as
    // `extern task $limit` for textual presence (T22 note in
    // headers/operators.phdl); enforced by no resolution pass today
    // (mirrors T20's identical finding for the value-returning
    // `$temperature()`-shaped calls).
    assert!(
        ctx.callables.lookup("$limit").is_some(),
        "Runtime operator `$limit` has no matching `extern task $limit` declaration \
         in headers/operators.phdl — declared as `extern task` because the \
         `extern operator` grammar cannot spell a `$`-prefixed name",
    );
}

// ── Primitive value types ────────────────────────────────────────────────────

/// Every primitive value type listed in `spec.md` P4-AC1 (the seven
/// names that used to live in `ElabContext::new()`'s hardcoded `prims`
/// vec) must resolve through `TypeRegistry` via a parsed
/// `extern type` declaration in `headers/types.phdl`. Re-introducing a
/// hardcoded primitive list (or removing one of the `extern type`
/// declarations) fails this test. (T16, DLS-17.)
#[test]
fn every_primitive_value_type_has_a_matching_extern_type_declaration() {
    let ctx = stdlib_ctx();
    let prims = ["Real", "Natural", "Integer", "Complex", "Boolean", "Quad", "String"];
    for ty in prims {
        assert!(
            ctx.types.lookup(ty).is_some(),
            "Primitive value type `{ty}` has no matching `extern type {ty}` declaration \
             in headers/types.phdl — spec P4-AC1 requires it",
        );
    }
}

// ── `@device`/`@port` attribute schemas ──────────────────────────────────────

/// The plugin system's own `@device`/`@port` attribute schemas
/// (formerly hardcoded in `piperine-plugin/src/host.rs`'s
/// `register_declared("device"/"port", …)` calls, replaced by T23)
/// must be textually declared in `headers/device_port.phdl`. This file
/// is NOT part of every project's prelude (it's parsed by
/// `PluginHost::seed_schemas` only when a plugin is loaded, per
/// T23's design), so the assertion works on the parsed text directly
/// rather than on a registry populated through elaborate. Removing
/// either declaration (or the whole file) fails this test. (T23,
/// DLS-21.)
#[test]
fn device_port_attribute_schemas_have_textual_extern_declarations() {
    let source = parse_str(include_str!("../headers/device_port.phdl"))
        .expect("headers/device_port.phdl must parse");
    let mut found: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for item in &source.items {
        if let Item::ExternDecl(ExternDecl::Attribute { name, .. }) = item {
            found.insert(name.as_str());
        }
    }
    for schema in &["device", "port"] {
        assert!(
            found.contains(schema),
            "Attribute schema `{schema}` has no matching `extern attribute {schema}` declaration \
             in headers/device_port.phdl — T23 replaced host.rs's hardcoded \
             `register_declared(\"{schema}\", …)` with this textual declaration",
        );
    }
}

// ── Statistical distributions ────────────────────────────────────────────────

/// dv-core T2 (DVC-02): every distribution a `tol` clause may name must be
/// declared as an `extern fn` in `headers/statistics.phdl` with the argument
/// spelling the spec uses — `gauss(sigma)`, `gauss(sigma_rel)`,
/// `uniform(half)`, `uniform(rel)` — and must resolve through
/// `CallableRegistry`. Adding a distribution name to a `tol` code path
/// without authoring the matching declaration fails this test by name; so
/// does renaming a declared argument (the keyed-argument spelling at the
/// use site is part of the contract).
#[test]
fn every_declared_distribution_resolves_through_the_callable_registry() {
    // The declared (name, argument-spelling) pairs the spec fixes.
    let declared = [
        ("gauss", "sigma"),
        ("gauss", "sigma_rel"),
        ("uniform", "half"),
        ("uniform", "rel"),
    ];

    // Textual half: the header itself carries each declaration with the
    // exact argument spelling (the registry's CallableDef does not retain
    // parameter names, so the spelling is asserted on the parsed text).
    let source = parse_str(include_str!("../headers/statistics.phdl"))
        .expect("headers/statistics.phdl must parse");
    let mut spellings: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    for item in &source.items {
        if let Item::ExternDecl(ExternDecl::Fn(sig)) = item {
            for param in &sig.params {
                if let piperine_lang::parse::ast::FnParam::Typed { name, .. } = param {
                    spellings.insert((sig.name.clone(), name.clone()));
                }
            }
        }
    }
    for (dist, arg) in &declared {
        assert!(
            spellings.contains(&((*dist).to_string(), (*arg).to_string())),
            "distribution `{dist}({arg})` has no matching `extern fn {dist}({arg}: Real) -> Real;` \
             declaration in headers/statistics.phdl — DVC-02 requires it",
        );
    }

    // Registry half: each declared name resolves through CallableRegistry
    // (both `gauss` overloads present), and an undeclared distribution name
    // does not resolve — it cannot pass.
    let ctx = stdlib_ctx();
    for dist in ["gauss", "uniform"] {
        assert_eq!(
            ctx.callables.candidates(dist).len(),
            2,
            "distribution `{dist}` must have its two declared spellings (absolute and relative) \
             registered from headers/statistics.phdl",
        );
    }
    assert!(
        ctx.callables.lookup("normal").is_none(),
        "`normal` is not a declared distribution and must not resolve",
    );
}

/// dv-core T2 (DVC-02, AC2): resolving a distribution name that
/// `headers/statistics.phdl` does not declare fails loud, naming it — the
/// same `CallableRegistry::resolve` path the `tol` clause's elaboration
/// consults (T4). The end-to-end `tol normal(...)` fixture lives in T4.
#[test]
fn undeclared_distribution_resolution_fails_loud_naming_it() {
    let ctx = stdlib_ctx();
    let err = match ctx.callables.resolve("normal", &[piperine_lang::pom::ValueType::Real]) {
        Ok(_) => panic!("`normal` is not declared in headers/statistics.phdl and must fail loud"),
        Err(err) => err,
    };
    let msg = err.to_string();
    assert!(msg.contains("normal"), "error should name the undeclared distribution: {msg}");
}

// ── Constraint helpers ───────────────────────────────────────────────────────

/// dv-core T7 (DVC-09): every constraint helper must be declared as an
/// `extern fn` in `headers/constraints.phdl` *carrying its class in the
/// text* — a `@quantity(class = "…", analysis = "…")` attribute, not a
/// comment — and that class must reach the elaborator's
/// `ConstraintHelperRegistry` unchanged. A helper added without its class
/// marker (or with a class the registry does not reproduce) fails this
/// test by name.
#[test]
fn every_declared_constraint_helper_carries_its_class_to_the_registry() {
    // The declared (name, class, analysis) triples the spec fixes (design
    // §C2): `ac_gain`/`ac_phase` are pointwise, `ac_unity_gain_freq`/
    // `ac_phase_margin`/`ac_gain_margin` are reduced.
    let declared = [
        ("ac_gain", "pointwise", "ac"),
        ("ac_phase", "pointwise", "ac"),
        ("ac_unity_gain_freq", "reduced", "ac"),
        ("ac_phase_margin", "reduced", "ac"),
        ("ac_gain_margin", "reduced", "ac"),
    ];

    // Textual half: the header itself carries each declaration with a
    // `@quantity(class, analysis)` marker — the class must be readable
    // from the parsed attribute, never from a comment.
    let source = parse_str(include_str!("../headers/constraints.phdl"))
        .expect("headers/constraints.phdl must parse");
    let mut markers: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    for item in &source.items {
        if let Item::ExternDecl(ExternDecl::Fn(sig)) = item {
            let quantity = sig.attrs.iter().find(|a| a.name == "quantity");
            let arg = |attr: &piperine_lang::parse::ast::Attribute, key: &str| {
                attr.args.iter().find(|a| a.name == key).and_then(|a| match &a.expr {
                    piperine_lang::parse::ast::Expr::Literal(
                        piperine_lang::parse::ast::Literal::String(s),
                    ) => Some(s.clone()),
                    _ => None,
                })
            };
            let (class, analysis) = match quantity {
                Some(attr) => (arg(attr, "class"), arg(attr, "analysis")),
                None => (None, None),
            };
            markers.insert(
                sig.name.clone(),
                (
                    class.unwrap_or_else(|| panic!("helper `{}` has no `@quantity(class = …)` marker — the class must be carried by an attribute, not a comment", sig.name)),
                    analysis.unwrap_or_else(|| panic!("helper `{}` has no `@quantity(analysis = …)` marker", sig.name)),
                ),
            );
        }
    }
    for (name, class, analysis) in &declared {
        let found = markers.get(*name).unwrap_or_else(|| {
            panic!("constraint helper `{name}` has no `extern fn` declaration in headers/constraints.phdl — DVC-09 requires it")
        });
        assert_eq!(
            found,
            &((*class).to_string(), (*analysis).to_string()),
            "helper `{name}` must declare `@quantity(class = \"{class}\", analysis = \"{analysis}\")`",
        );
    }

    // Registry half: each helper resolves through `CallableRegistry` (the
    // signature) and through `ConstraintHelperRegistry` with the *same*
    // class the text declares.
    let ctx = stdlib_ctx();
    for (name, class, analysis) in &declared {
        assert!(
            ctx.callables.lookup(name).is_some(),
            "constraint helper `{name}` does not resolve through CallableRegistry",
        );
        let helper = ctx.constraint_helpers.lookup(name).unwrap_or_else(|| {
            panic!("constraint helper `{name}` is missing from ConstraintHelperRegistry")
        });
        let expected_class = match *class {
            "pointwise" => piperine_lang::pom::QuantityClass::Pointwise,
            "reduced" => piperine_lang::pom::QuantityClass::Reduced,
            other => panic!("test bug: unknown class `{other}`"),
        };
        assert_eq!(helper.class, expected_class, "helper `{name}`'s class changed between text and registry");
        assert_eq!(helper.analysis, *analysis, "helper `{name}`'s analysis changed between text and registry");
    }
}

/// dv-core T7 (DVC-09): an undeclared helper name cannot pass — the
/// registry's loud lookup fails naming it. (The end-to-end fixture, an
/// undeclared helper *used in a constraint*, lands with constraint
/// elaboration in T8.)
#[test]
fn undeclared_constraint_helper_fails_loud_naming_it() {
    let ctx = stdlib_ctx();
    assert!(
        ctx.constraint_helpers.lookup("unity_gain").is_none(),
        "`unity_gain` is not a declared constraint helper and must not resolve",
    );
    let err = ctx
        .constraint_helpers
        .require("unity_gain")
        .expect_err("an undeclared constraint helper must fail loud");
    let msg = err.to_string();
    assert!(msg.contains("unity_gain"), "error should name the undeclared helper: {msg}");
}

// ── Cast associated functions ────────────────────────────────────────────────

/// The four primitive target types that previously had bare-name cast
/// forms (`real(x)`/`int(x)`/`bit(x)`/`Boolean(x)`/`Quad(x)`) must
/// each have an `extern impl` block declaring `fn from(...)` overloads
/// in `headers/types.phdl`. This is the cast-replacement surface
/// (T17, DLS-23) — removing any of these blocks (or the `from` methods)
/// would leave the corresponding `Type::from(x)` call sites
/// (e.g. `Real::from(i)` in `tests/examples/sar_adc.phdl`) without a
/// declaration.
#[test]
fn cast_target_types_have_from_overloads_in_extern_impl_blocks() {
    let source = parse_str(include_str!("../headers/types.phdl"))
        .expect("headers/types.phdl must parse");
    let mut from_targets: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for item in &source.items {
        if let Item::ExternDecl(ExternDecl::Impl { target, methods, .. }) = item
            && methods.iter().any(|m| m.name == "from")
        {
            from_targets.insert(target.as_str());
        }
    }
    // T17 declared four target types (the five bare-cast names collapsed
    // `bit`/`Quad` into one Quad block).
    for target in &["Real", "Integer", "Quad", "Boolean"] {
        assert!(
            from_targets.contains(target),
            "Cast target type `{target}` has no `extern impl {target} {{ fn from(...) ... }}` \
             block in headers/types.phdl — T17 (DLS-23) requires it as the replacement for \
             the deleted bare-name `{target_lower}(x)` cast special case",
            target_lower = target.to_lowercase(),
        );
    }
}
