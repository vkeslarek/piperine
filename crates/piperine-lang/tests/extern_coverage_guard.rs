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
//!                                   string-match in codegen); spec.md
//!                                   P4-AC4 + headers/operators.phdl
//!                                   enumerate the contract
//!   - Primitive value types       — spec.md P4-AC1 (the 7 primitives)
//!   - `@device`/`@port` schemas   — `headers/device_port.phdl`
//!                                   (parsed by `PluginHost::seed_schemas`,
//!                                   not part of every project's prelude)
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
        if let Item::ExternDecl(ExternDecl::Impl { target, methods, .. }) = item {
            if methods.iter().any(|m| m.name == "from") {
                from_targets.insert(target.as_str());
            }
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
