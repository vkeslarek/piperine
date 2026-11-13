# Writer hand-off notes (for the integration review)

## Part V / VI / VIII writer
- Files: part_v_execution.md (§15–18, E0501–E0510), part_vi_io.md (§19–21, E0601–E0614), part_viii_host.md (§23, E0801–E0809). 32 `Resolution` notes.
- Notable resolutions: as-if ordering for speculative digital execution (§15 I3/I8); crossing found across a jump fires at (t, n+1); tick rounding of t+d, ties later; `finish` completes its time point, ignored when maybe/unknown; `emit!` stream name must be a string literal; anonymous-instance path names; host `force` on a node = potential source; §23.3 rule 4 expanded 4a–4h.
- Letter handles: §15 T1–T6 ticks, I1–I8 invariants, S1–S7 settle commit; §16 O1–O5, H1–H7.
- Possibly double-defined terms: solve, equilibrium solve, transient step, jump solve, small-signal sweep, time point, state, nodeset (§18), host/session (§23 vs index), operating-point variable (§21 vs §11 rule 6), record/stream/check.
- Assumed defined elsewhere: commit, reaction (§12.1); drive (§5.2); definition, relaxation (§8.2); event/at/cross/settle (§13); switch branch, small-signal rule (§11/§11.1); structural param (§8.1).
- Cross-part deps: B.3 has no emit!/include_str! production; E0509 (bound_step arg) and E0510 (cross ttol/etol) touch Part IV primitives; `at` at tick 0 fires at (0,1) — check vs §13/§14.
- Open items for App. G: Dc at a transient's last instant; settle commits there; continuing after finish; mixing time resolutions; which module is top; G.32 delayed-logic note; G.6 crossing landing.

## index / Part I / Part II / App. A writer
- Files: index.md, part_i_values.md, part_ii_carriers.md, appendix_a_lexical.md. 38 `Resolution` notes. Errors E0101–E0161, E0201–E0264, E0901–E0908.
- Rule-id convention (index §0.2): subsection rules `§8.2.3`; top-level-section rules `§11 rule 3` (avoid collision with subsections). Added rules continue v5 numbering. (Part V writer used letter handles T/I/S/O/H inside §15/§16 — reconcile.)
- v5 inconsistencies resolved: (a) `out` is a keyword → `signal out:` is a parse error (v5 §6, zi_nd in 22.2) → rename `vout`; (b) integer literals don't coerce to float → `rout: f64 = 100` must be `100.0` (22.4, F.1, D.6); (c) `impl Logic { pub const Z }` vs fn-only impl_block → associated consts allowed on value types only.
- Big decisions: panics (run error under known control; merge → unknown under unknown control, §1 rules 11–13); §3.3.5 "flag-total functions" with known/known_or/is_known/high mandatory; usize = 64 bit, no char; #[derivative] one fn per f64 param; compatible continuous natures need equal flow names; resolve order = lexicographic instance path, host first; carrier methods are errors on arrays/bundles; inflow() includes child-instance branches; unknown attributes are errors; `Unknown` reserved literal; SI literal lexing (A.2).
- Defines: value, level, Repr, function class, closure, handle, known/unknown/merge, panic, prelude, namespace module (I); carrier, signal, nature, Nature, quantity, drive, one-driver/resolved carrier, change, node, branch, probe, gnd, signal-flow node, bundle, view (II); host, parse/elaboration/run phases (index).
- G candidates resolved locally: panic under unknown control; loop-unrolling bound (§3.3.16); extern "C" fn symbol lookup.

## Galleries D/E/F writer
- 9 Resolution notes. Model fixes: `where` on const generics (Pwl, Delay N>=1; Divider N>=2 && even); int→f64 defaults (RSwitch.ron, Dac8.rsw; stdlib D2A/Bidir `rout: f64 = 100` in §22.4 too); BehavioralCell store guarded by `c.wl.prev().is_known()` (§13.6); ShareCharge a/b as internal `pub signal`s with initial values.
- Assumed: `Capacitor` declared `module Capacitor(p, n) { c: f64; }`.
- §17.2.2 reading: a guarded scheduled instant that cannot fire is not a time point ("falling edges cost nothing", D.5/F.1/F.2) — Part V must confirm.
- Findings (models unchanged): D.5/F.2/F.5 Dc/Ac/Noise singular (no DC path), incl. at transient's last instant (inferred from §16.4.2); D.3 negative freq sticks clk low; D.4 unknown code → 0.0 in equilibrium; D.6 ideal-Switch variant needs `= value` on internal nodes; F.3 Dram no write driver/restore; F.1 `start` level-sensitive reconverts.

## Part III / IV writer
- Files: part_iii_structure.md (§8–9, E0301–E0375), part_iv_behavior.md (§10–14, E0401–E0457). 44 Resolution notes. Added §14.1–§14.13 (one per core primitive, per-solve table; §16.2 governs on conflict). v5 8.2 "other rules 1–2" → §8.2.7/§8.2.8.
- Cites without codes: Part II drive-conflict errors, prev() of param, one-potential-driver; Part I §3.3.5 analog-unknown + §3.2 arg errors; Part VI opvar/anon path naming, record order; Part VIII host set refusal.
- B.3 fix needed: contribution/commit allowed at any statement depth (if/match/for), not just block top level (§10 rule 8).
- Review: m product along path, m/dtemp read effective values, potential noise / m (§8.5.3); let bound to module-with-result reads the result → its pub items unreachable (§8.2.14, G candidate); template calls take no param block, Fn(P) satisfied by modules with result (§8.4); core-module results count as direct dependence → `let y = ddt(f(y))` error (§8.3.6); flow-contribution selectors on potential-capable branch probe-independent (§11 rule 3); events read as bool in guards (§13 rule 9); at() arg discrete (§13 rule 11); param changed() not at (0,0) (§13 rule 16); undelayed definition + initial value error (§8.2.21).
- Open → G: analysis(Static) true in Dc at a transient's last instant; "constant during an analysis" row for periodic analyses.

## Part VII / App. B / C / G writer
- Files: part_vii_stdlib.md (§22.1–22.6, E0701–E0707), appendix_b_grammar.md, appendix_c_implementation.md (informative), appendix_g_scope.md. 31 Resolution notes.
- B.3 is the full EBNF; B.3.5 lists 10 amendments vs v5. Other parts quoting v5 productions must re-sync: module_decl, nature_decl, fn_decl, impl_block, instance, gen<X>.
- G.1–G.32 with status; G.33–G.39 open; G.40–G.44 closed in v5; G.45–G.48 flagged (panic under unknown control, loop-unrolling bound, extern "C" lookup, pub items behind result let).
- Inferred signatures (Resolutions): same, matches_bits, logic, noise_table, laplace_*, zi_np/zd/zp, table_model, db10/db20, deltalim, normal, parse_table/parse_hex.
- Did not adopt Part IV §14.1–§14.13 numbering (refs say §14).
