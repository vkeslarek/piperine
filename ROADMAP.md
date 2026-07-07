# ROADMAP.md — Open work items

Distilled from the closed-out `SPEC_BENCH_GAPS.md` / `IDE_GAPS.md` handoff drafts
(2026-07-04). Everything listed in those documents that got implemented is gone; this file
keeps only what is still open. Conventions: fail-loud until closed — an unimplemented bench
task is an elaboration error (`bench_task_implemented` allowlist in
`piperine-lang/src/eval/tasks.rs`), never a silent no-op. Closing an item updates the bench
spec §11 row (`crates/piperine-bench/docs/SPEC.md`) in the same change.

---

## Bench

### `$plot(waveform, title)` (was G1)

**Spec:** bench spec §8 table row, §11 — "emit artifacts".
**Today:** elaboration-rejected (not in `bench_task_implemented`). `$write` (CSV) is the
reference `SimTask` to copy.

Sketch:
1. Artifact format: hand-rolled SVG line chart (~100 lines, zero deps, viewable anywhere).
   Axis autoscale from `Waveform.points`, polyline, title text.
2. New `Plot` struct in `piperine-bench/src/tasks.rs` implementing `SimTask`; accepts
   `(Value::Object(Waveform | ComplexWaveform), Value::Str(title))`; downcast via
   `Object::as_any` exactly like `$noise` does for `NetRef`.
3. Output path: `<title>.svg` in the CWD (same convention as `$write`); sanitize the title
   into a filename.
4. Add `"plot"` to `bench_task_implemented`; flip the spec §11 row; e2e test in
   `piperine-bench/tests/bench.rs` asserting the file exists and starts with `<svg`.

### The uniform API (was G12) — milestone

Bench spec §8 in full: public `load()` + `Design::op/tran/ac/noise` Rust surface first
(`SimSession` renamed/made public with typed config structs), Python via `pyo3` only after
the Rust surface settles. The §8 identical-shape rule is the review gate for every signature.

### `extract` / `.attach` / `.meta` (was G13)

Blocked on writing the extensibility spec (plugin model). Do not implement ahead of it; the
only prep is keeping `Attribute` surfaces public on POM nodes (they are).

---

## Codegen / solver

- `transition`, `laplace_*`, `zi_*` analog operators — recognized in the IR, fail loud at
  codegen. Each is its own companion-model follow-up.
- **`ac_stim` in *potential* contributions — DONE (2026-07-04).** A `V(p,n) <+ … + ac_stim(mag,phase)`
  now attaches the AC drive to the force branch: `FlatForce.ac_stim`, compiled to
  `force_ac_mag`/`force_ac_phase` rows in `jit/analog.rs`, stamped onto the branch-equation
  RHS in `device/analog.rs::load_ac`. This is what makes a faithful independent **voltage
  source** (`vsrc`) drive AC analysis (previously only Norton current sources could).
  Multiple `ac_stim` per contribution is still fail-loud.
- **`$limit("pnjlim", …)` — DONE for the single-junction case (2026-07-05).** Lowered in the
  JIT with the full stateful machinery: a `vold` slot per unique `$limit` appended to the
  state bank (`jit/analog.rs` `collect_limits`/`limit_update`), the pnjlim formula in
  `jit/emit.rs::emit_pnjlim`, a `vcrit` seed at device creation (ngspice MODEINITJCT), the
  *limited* Norton linearization point (`device/analog.rs::limited_volts` — `cdeq = cd −
  gd·vlim`), and a convergence veto while limiting is active (`Device::limiting_active`, the
  ngspice `Check==1`/`noncon`). `diff.rs` treats the limiter as transparent (`d/dV =
  d(vnew)/dV`). A stiff diode (5 V → 1 kΩ → 1e-14 A) now converges to its physical operating
  point — see `spec_simulation::sim_dc_diode_pnjlim_converges`.
  **Still open:** multi-junction convergence. `bjt` (coupled B-E/B-C/substrate junctions,
  base resistance) hits NaN and `mos1` (mode/vdsat discontinuities; uses gmin not `$limit`)
  stalls. Needs per-junction limited-Norton handling for shared nodes and mode-switch
  damping. `fetlim` is stubbed to identity (no current device needs it).
- **`@initial` cannot force a branch.** `@ initial { V(p,n) <- ic; }` (the SPICE `.ic`/UIC
  seed used by `dio`/`cap`/`ind`) fails loud: "statement Force … in an analog event body".
  Event bodies only support variable assignments today; an initial-condition force needs the
  solver to accept a branch constraint for the first timepoint.
- **Large analog bodies exceeding Cranelift's function-size limit — DONE (2026-07-05).**
  The residual is one straight-line block (control flow folded to branchless `select`s), so
  the emitter now does exact common-subexpression elimination keyed by `(op-tag, child Value
  ids)` — `jit/emit.rs` `CseKey`/`cse_*`. Fully-inlined `var`/helper-`fn` bodies stop
  exploding; `dio` and `mos1` compile. Also a large speedup (shared subexpressions emit once).
- `idt` AC small-signal `1/jω` admittance not stamped (contributes 0 in AC).
- `Trace.i` over time on devices with runtime state/vars — fails loud (per-step var/state
  banks are not recorded in `TransientAnalysisResult`).

## Digital

- **Fused combinational network JIT — BUILT (2026-07-05), integration pending.** A pure-
  combinational cone compiles to one Cranelift function (`jit/digital/compile.rs::NetworkComb`)
  driven by `jit/digital/network.rs::DigitalNetwork` (one `DigitalEventModel`). Tested
  standalone (`spec_simulation::digital_network_fuses_combinational_chain`). TODO: wire into
  `circuit.rs::run_digital_at` (detect cones, build the network, fall back per-device on
  clocked/analog members); fuse clocked/register members too (comb-only today). See
  `piperine-codegen/docs/DIGITAL_JIT.md`.
- **Sequential logic cannot be clocked through `$op`.** `$op` is a *pure function* of
  (design + staged overrides): `session.rs::run_op` re-elaborates and builds a **fresh**
  circuit each call, so no digital state persists between `$op`s — a register/shift-register
  can't be stepped by staging a clock across calls (each rebuild also fires a spurious X→1
  posedge). The digital kernel and *cross-module* NBA sampling are correct — proven by
  `spec_simulation::digital_cross_module_flops_sample_simultaneously` (two flip-flops in
  separate modules; the downstream flop captures the upstream's pre-edge output). Verifying
  sequential multi-module logic through a bench needs `$tran` to record digital nets over time
  (see the `Trace` gap above). Combinational multi-module logic verifies fine through `$op` —
  see `examples/17_ripple_adder_4bit`, `18_mux4_tree`, `19_multiplier_2x2`,
  `20_comparator_4bit` (all exhaustive).

## Type system

- **Optional params `T?` + `none` — DONE (2026-07-05).** `param x : Real? = none;` is now a
  first-class optional. Syntax: a trailing `?` on any type (`Type.optional`), the `none`
  literal (`Literal::None` → `Value::Option(None)`). Read through `.is_present()` /
  `.get_or(default)` (aliases `.is_some()`/`.unwrap_or()`). In the interpreter/const layer
  these evaluate on `Value::Option`; in an analog body they lower onto the parameter-presence
  mechanism — `is_present` ≡ `$param_given(x)`, `get_or(d)` ≡ `param_given ? x : d` — so it
  works per-instance without specializing the module. Test:
  `spec_simulation::sim_dc_optional_param_get_or`.
  **Follow-up:** migrate the `piperine-spice` device models off their sentinel/`$param_given`
  encoding (`bv = 1e99`, `rbm = 0`) onto `T?` now that it exists. Optional *bundle fields*
  (`model.rbm.get_or(…)`) still need the field-receiver lowering path (today only a direct
  `param.method()` receiver folds).

## Language / interpreter gaps

**Closed 2026-07-04** (each with a gate test in `piperine-bench/tests/bench.rs`):
`impl` method dispatch everywhere (interpreter via `Host::resolve_method` on tagged
`Value::Record`s; analog/digital via `Bundle::method` IR fns with `self` flattened
per-field); bench fn → sibling bench fn calls (`Callable::BenchFn`, effectful); tuple
index `t.0`; bundle-typed fn params (flattened like module bundle params, call sites
expand the argument); the lowering's silent `Real(0.0)` fallbacks for method calls and
value-layer expressions are now loud `LowerError`s. Digital nets read directly off `$op`
results (`r.v(bit_net)` → 0/1, NaN for X/Z).

Still open:
- `for` patterns can't destructure tuples (`for (a, b) in …`); loop bodies index `case.0`.
- A bundle *literal* passed as a bundle-typed argument inside an analog body must name
  every field — the declared field defaults aren't expanded at that call site yet.
- Net/instance arrays are not addressable from a bench (`tap[2]`, `bank[0]`), and a
  bench-built circuit collapses a `wire x : T[N]` array into a single net.
- A bench top module must have at least one instance (leaf top = empty circuit);
  `.i(a, b)` needs a unique two-terminal match between the named nets.
- `Trace` (transient) does not record digital net values over time — digital readback is
  `$op`-only today.

## Language server

- True scope-aware name resolution: `symbol_index::resolve_at` is still a global first-match
  lookup; hovering `p` in module `B` can show module `A`'s port. Needs the elaborator's
  name→id maps exposed as a query.
- References/rename/highlight are word-occurrence scans gated by `resolve_at`, not
  resolver-driven use-site lists; comments/strings containing the word match.
- Project-unit elaboration: documents elaborate per-file with a project `SourceMap`;
  cross-file goto/rename and per-file diagnostic fan-out need
  `ServerState.projects: HashMap<Root, ProjectState>`.
- Protocol-level tests: drive the server over real JSON-RPC via
  `lsp_server::Connection::memory()` (init → didOpen → hover/completion round-trips);
  today's tests exercise helpers only.
- Error-accumulating elaboration (first `ElabError` stops analysis) — the editor shows one
  elaboration error at a time.

## Spec / implementation divergences (2026-07-07 spec audit)

Cases where the formal specification (`docs/new-spec/`) describes intended behavior the
compiler does not yet enforce. The spec is the contract; these are bugs/gaps to close.

- **`white_noise` / `flicker_noise` return `0.0` placeholder.** Spec (Part V §2):
  inject a noise spectral density into the contribution RHS. Code
  (`lower/pom/analog_ops.rs:204-209`) returns `0.0` — a silent stub that violates the
  no-silent-`0.0` rule (AGENTS.md). Either lower the noise stamp or make it fail-loud.
- **Keyword reservation is parser-level, not lexical.** The lexer
  (`parse/lexer.rs:14-17`) emits every keyword as `Tok::Ident(String)`; reservation is a
  parser concern. Documented as the current design in Part I §4.2; a future lexer
  refactor could tokenize keywords for robustness.

---

## Extension / packaging (user-owned, deliberately out of agent scope)

VS Code extension productization, marketplace packaging, grammar/registry sync tests,
release/versioning story — see `editors/vscode/`.
