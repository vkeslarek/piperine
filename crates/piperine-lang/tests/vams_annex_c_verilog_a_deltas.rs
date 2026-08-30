//! LRM-COVERS: C.1, C.2, C.3, C.4, C.5, C.6, C.8, C.9, C.10, C.11, C.12, C.13, C.14, C.16, C.17
//!
//! lrm-conformance-wave3 (Phase 6, T41): Annex C "Verilog-A Language
//! Reference" is a **delta list** over clauses 1-12 for a hypothetical
//! restricted "Verilog-A" compilation mode — this spec's own Out of
//! Scope table settles how Piperine treats it: "Piperine implements the
//! Verilog-AMS superset; a `.va` file is compiled with the same front
//! end. Annex C's per-clause deltas are covered by the corresponding
//! chapter tests, not by a second, restricted parser." Each Annex C
//! section below is therefore Covered by naming the chapter test that
//! already proves the underlying clause (never a bare claim), the
//! project's own established cross-declaration precedent (`vams_headers.
//! rs`'s Annex D cross-check, `tests/vpi_routines.rs`'s ch. 11/12
//! coverage). `dot_va_source_compiles_through_the_same_front_end` below
//! is the one new, tiny proof underlying all fifteen: a `.va`-suffixed,
//! Verilog-A-flavored source (a single discipline, one analog block, no
//! digital constructs, matching C.2's own feature list) elaborates
//! through the identical `parse_and_elaborate` entry point every VAMS
//! test in this suite uses — there is no separate restricted parser to
//! diverge from the superset.
//!
//! - **C.1** (Verilog-A overview) / **C.2** (language features: named
//!   branches, ranged parameters, operators, waveform filters, events,
//!   `$bound_step`, SPICE-primitive access): every shipped model in
//!   `headers/spice/*.va` IS a Verilog-A-flavored module using exactly
//!   this feature set —
//!   `spice_headers.rs::include_spice_header_resolves_via_the_builtin_
//!   namespace` proves the corpus parses and elaborates.
//! - **C.3** (lexical: `` `X`/`Z` `` limited to the mixed-signal context
//!   in an analog block, §7.3.2): `tests/signal_flow_disciplines.rs`
//!   (this wave's own T39 work) and the mixed-signal fixtures in
//!   `tests/vams_connect_modules_e2e.rs` exercise exactly the mixed
//!   analog/digital boundary this restriction concerns.
//! - **C.4** (data types: `discrete` domain binding, `wreal`,
//!   `` `default_discipline `` — all still Piperine features, simply
//!   *not disallowed* the way a real Verilog-A mode would):
//!   `vams_digital_net_types.rs`'s `wreal` classification tests and
//!   `vams_annex_f_domain.rs`'s domain-binding tests prove the
//!   underlying chapter-3 features these exceptions list.
//! - **C.5** (expressions: `===`/`!==` case-equality operators):
//!   `expressions.rs` parses and evaluates `===`/`!==` (`BinOp::CaseEq`/
//!   `CaseNe`) as ordinary Clause-4 operators.
//! - **C.6** (analog signals, §5.4 access functions): the whole
//!   `crates/piperine-codegen/tests/` branch-access-function suite
//!   (e.g. `resolve_lowering.rs`) proves §5.4 directly.
//! - **C.8** (hierarchical structures, real-value ports §6.5.3):
//!   `tests/vams_continuous_assign_e2e.rs`'s `wreal` port-connection
//!   proof (this wave's own §7.4.2 cross-declaration, T36).
//! - **C.9** (mixed signal, all of Clause 7): `tests/
//!   vams_connect_modules_e2e.rs`'s discipline-resolution and connect-
//!   module fixtures are Clause 7 end to end.
//! - **C.10** (scheduling semantics, the analog simulation cycle):
//!   `crates/piperine-solver/tests/checkpoint_restore.rs` (this wave's
//!   own §8.4.7 cross-declaration, T39) proves the accept/reject
//!   analog-cycle mechanics directly.
//! - **C.11** (ch. 9 system tasks/functions applicable in the analog
//!   context): `vams_system_tasks.rs`'s `$bound_step`/`$abstime`/
//!   `$temperature`/conversion-function tests all execute in an analog-
//!   context-equivalent environment.
//! - **C.12** (ch. 10 compiler directives, applicable to both):
//!   `vams_directives.rs` proves `` `timescale``/`` `celldefine``/etc.
//! - **C.13**/**C.14** (ch. 11/12 VPI routines and their definitions,
//!   applicable to both): `crates/piperine-plugin/tests/vpi_routines.rs`
//!   is this project's whole ch. 11/12 coverage gate.
//! - **C.16** (the keyword list: `connect`/`connectmodule`/
//!   `connectrules`/`driver_update`/`endconnectrules`/`merged`/
//!   `net_resolution`/`resolvedto`/`split`/`wreal` are VAMS-only,
//!   excluded from a real Verilog-A's reserved-word set):
//!   `annex_b_keywords.rs` re-derives and proves the full VAMS keyword
//!   set these ten are drawn from.
//! - **C.17** (Annex D standard definitions, except `discrete`-domain
//!   disciplines): `vams_headers.rs`'s own Annex D cross-checks
//!   (`disciplines.vams`/`constants.vams`) prove the definitions
//!   directly.

use piperine_lang::SourceMap;

/// The one proof underlying every cross-declaration above: a Verilog-A-
/// flavored source (single discipline, one analog behavior, none of the
/// digital-only constructs C.7/C.16 exclude from Verilog-A) elaborates
/// through the exact same `parse_and_elaborate` entry point as any other
/// VAMS source — there is no second, restricted "Verilog-A mode" parser
/// for Annex C's deltas to diverge from.
#[test]
fn dot_va_source_compiles_through_the_same_front_end() {
    let src = "
        nature Voltage;
            units = \"V\";
            access = V;
        endnature
        nature Current;
            units = \"A\";
            access = I;
        endnature
        discipline electrical;
            potential Voltage;
            flow Current;
        enddiscipline
        module resistor (inout electrical p, inout electrical n);
            parameter real r = 1.0e3 from (0:inf);
            branch (p, n) res;
            analog I(res) <+ V(res) / r;
        endmodule
    ";
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy())
        .expect("a Verilog-A-flavored source elaborates through the ordinary VAMS front end");
    assert!(design.module("resistor").is_some());
}
