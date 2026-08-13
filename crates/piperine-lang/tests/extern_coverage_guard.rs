//! Declared-language-surface regression guard — **rewritten for the VAMS
//! surface, T36 (design R9: "rewritten, never deleted")**.
//!
//! **What this file used to guard (PHDL era, T16-T27, DLS-17..25):** eight
//! categories of "native Rust table → textual `extern` declaration"
//! coverage — math functions (`MATH_FNS`), system tasks
//! (`TaskRegistry::with_builtins()`), runtime operators, the seven
//! primitive value types, `@device`/`@port` attribute schemas, statistical
//! distributions, dv-core constraint helpers, and cast `from` overloads.
//! Every one of those was backed by MD-24's `extern type`/`fn`/`task`/
//! `operator`/`attribute`/`impl` mechanism: a PHDL-syntax-only construct
//! with no Verilog-AMS Annex A production, gutted in full at T21 (MD-45)
//! along with `piperine-lang/src/parse/` — `ElabContext` lost every
//! sub-registry (`callables`/`types`/`operators`/`constraint_helpers`/
//! `impl_methods`/`schemas`-the-old-kind) this file's tests read.
//!
//! **What is real today (asserted below):** only `@device`/`@port` has a
//! VAMS-native replacement — [`ElabContext::schemas`] (T35, VAMS-12/13),
//! seeded by [`piperine_lang::seed_device_port_schemas`]/
//! `PluginHost::seed_schemas` and consulted by `elab::lower::vams`'s
//! attribute-instance lowering, which fails loud
//! (`ElabErrorKind::UnknownAttrSchema`, E2022) on any attribute name that
//! is not registered — MD-24's "no silent fallback" rule holds for this
//! one surface, verbatim. The lowering behavior itself (the actual
//! `(* device = …, type = … *)` → POM `Attribute` shape, the E2023
//! missing-field case, the `port`/`kind` mirror) is exercised in depth by
//! `tests/vams_device_port_attrs.rs`, T35's own test target — this file
//! does not duplicate those assertions. What belongs here, matching the
//! file's original *regression-trap* spirit ("a name added to the native
//! side without its declared counterpart fails this test by name"), is
//! the inverse-direction guarantee: the registered schema *set* is
//! exactly `{"device", "port"}`, so silently registering (or silently
//! dropping) a schema name is caught here even if `vams_device_port_
//! attrs.rs`'s behavioral tests happen not to exercise it directly.
//!
//! **What is honestly gone, not replaced (documented, not asserted —
//! writing a test with no mechanism to check would assert nothing
//! meaningful, per this task's own instruction):**
//!
//! | Former guard | Native table | Textual decl | Status |
//! |---|---|---|---|
//! | Math functions | `math::MATH_FNS` | `extern fn` in `headers/math.phdl` | **Both gone.** `resolve/diff.rs` and friends now string-match function names directly in codegen (e.g. `"sin" => …`, `crates/piperine-codegen/src/resolve/diff.rs:409`) — there is no elaboration-time "is this a known math function" registry at all today. Unscheduled: no task currently re-establishes a declared-surface check for these; VAMS-07 (P1, ch.4 math functions) shipped the *parsing/evaluation* half only |
//! | System tasks | `eval::tasks::TaskRegistry` | `extern task` in `headers/tasks.phdl` | **Both gone**, pending T47 (Phase 9, VAMS-15, LRM ch.9 general system tasks) — the first task scheduled to give these a VAMS-native home, textual or otherwise |
//! | Runtime operators (`ddt`/`idt`/`cross`/…) | none (string-matched in codegen) | `extern operator` in `headers/operators.phdl` | **Textual half gone**, native string-matching in codegen unchanged (`CLAUDE.md`'s "known gaps" table already documents several of these by name) |
//! | Primitive value types | hardcoded `prims` list | `extern type` in `headers/types.phdl` | **Both gone.** VAMS has its own primitive type set (`Real`/`Integer`/… per LRM ch.3) parsed directly by `piperine-vams`; no elaboration-level registry mediates it |
//! | Statistical distributions (`gauss`/`uniform`) | n/a | `extern fn` in `headers/statistics.phdl` | **Gone with dv-core's constraint-block surface** (MD-45) — `tol`/`require` clauses that named these no longer exist in the POM at all |
//! | dv-core constraint helpers (`ac_gain`/…) | n/a | `extern fn` + `@quantity(...)` in `headers/constraints.phdl` | **Gone with `ConstraintBlock`/`QuantityClass`** (MD-45) |
//! | Cast `from` overloads | n/a | `extern impl` in `headers/types.phdl` | **Gone.** VAMS has no user-facing cast-call syntax needing this replacement surface; not reintroduced |
//!
//! None of the "gone" rows above have a scheduled landing task except
//! system tasks (T47). If a future phase reintroduces a declared-surface
//! mechanism for any of them, this file is where its regression guard
//! belongs (per design R9), and this table is the record of what needs
//! writing, not a promise already kept.

use piperine_lang::elab::registry::ElabContext;
use piperine_lang::{parse_and_elaborate, parse_and_elaborate_seeded, seed_device_port_schemas, SourceMap};

/// A minimal `.vams` module — no `@device`/`@port` attribute — used to
/// exercise a freshly-seeded [`ElabContext`] without depending on any
/// specific attribute-instance fixture.
const MINIMAL_SRC: &str = "module Top (inout electrical p); endmodule";

/// `piperine_lang::seed_device_port_schemas` registers **exactly**
/// `{"device", "port"}` — no more, no fewer. This is the direct
/// descendant of the original file's "native table has a matching
/// textual declaration" trap, inverted for the VAMS-native mechanism:
/// silently adding a third schema (or dropping one of these two) without
/// updating this list fails here by name, even though `vams_device_
/// port_attrs.rs`'s behavioral tests would not necessarily notice a
/// silently *added* schema.
#[test]
fn seed_device_port_schemas_registers_exactly_device_and_port() {
    let mut ctx = ElabContext::new();
    assert!(ctx.schemas.is_empty(), "a fresh ElabContext registers nothing until seeded");
    seed_device_port_schemas(&mut ctx);
    let mut names: Vec<&str> = ctx.schemas.iter().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec!["device", "port"],
        "seed_device_port_schemas must register exactly {{\"device\", \"port\"}} — the plugin \
         system's own two attribute-carried schemas (MD-21/D2, VAMS-12/13); add or remove a \
         schema here in lockstep with any change to seed_device_port_schemas",
    );
}

/// Without seeding, `ElabContext::schemas` is empty — an unseeded
/// elaboration has no registered attribute schema at all, so any
/// `(* ... *)` attribute instance on a module/port fails loud rather than
/// silently accepting one that only happens to be meaningful once a
/// plugin host is wired (MD-24's "no silent fallback", ported verbatim
/// to VAMS-13). `parse_and_elaborate` (the unseeded entry point) is
/// exactly this case — proven end-to-end (not just at the `ElabContext`
/// level) using the same minimal source both device-attribute tests in
/// `vams_device_port_attrs.rs` build from, so a regression in *either*
/// the seeding path or the unseeded default is caught by one of the two
/// files.
#[test]
fn an_unseeded_elaboration_context_registers_no_schema_at_all() {
    // `MINIMAL_SRC` elaborates cleanly on its own (it uses no attribute
    // instance) — proving the *absence* of registration doesn't itself
    // break ordinary elaboration.
    parse_and_elaborate(MINIMAL_SRC, &SourceMap::dummy())
        .expect("a module with no attribute instance elaborates with no schemas seeded");
    // Seeding is opt-in and additive: an elaboration explicitly seeded
    // with an empty closure behaves identically to the fully-unseeded
    // `parse_and_elaborate` above — the seeding hook itself never
    // registers anything on its own.
    parse_and_elaborate_seeded(MINIMAL_SRC, &SourceMap::dummy(), |ctx| {
        assert!(ctx.schemas.is_empty(), "the seed hook runs before any registration");
    })
    .expect("seeding with an empty closure changes nothing");
}
