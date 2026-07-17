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
reference `BenchTask` to copy.

Sketch:
1. Artifact format: hand-rolled SVG line chart (~100 lines, zero deps, viewable anywhere).
   Axis autoscale from `Waveform.points`, polyline, title text.
2. New `Plot` struct in `piperine-bench/src/tasks.rs` implementing `BenchTask`; accepts
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

The extensibility spec is now written — `docs/spec/part_vi_plugins.md` (Part VI). These
land as plugin-registered `BenchTask`s / staging calls once the plugin system below exists.
Do not implement ahead of it; the only prep is keeping `Attribute` surfaces public on POM
nodes (they are).

---

## Codegen / solver

### Live parameter mutation — DONE (solver-live-params, 2026-07-17)

Live `set` on a compiled circuit by PHDL name (POM `set_param` parity), MD-18
proof (zero recompiles across set+solve loops), scheduled mid-transient sets on
the unified breakpoint table (exact landing, last-write-wins, ≥OP re-solve at
`t`), and the Python `LiveSession` (`module.compile()` → `set`/`schedule_set` +
`op/tran/ac/noise` on one compilation; `examples/live_optimize.py`). Structural
sets auto re-elaborate at the host layer with net-name state carry; a
mid-transient structural set restarts the run from `t` with carried ICs and
stitches one continuous trace. En route: TR-stage backward-Euler restart
convention after discontinuities (`TrBdf2::stage_coeffs`) and
`TransientAnalysisOptions::with_start` (absolute start clock). Spec: `docs/spec/`
Part VII §10.5; feature spec `.specs/features/solver-live-params/`. Remaining
host surface (interactive `step()`/`run_until()`, GUI/streaming delivery) is the
future real-time feature.

### Epic: TR-BDF2 Transient Integration Engine with PI Timestep Controller

**Architecture:** TR-BDF2 (Trapezoidal Rule / Backward Differentiation Formula 2) with a
Proportional-Integral (PI) timestep controller.

**Goal:** Guarantee unbreakable convergence on stiff non-linear circuits and switched
circuits, without backtracking or complex integration-method switching heuristics.

**Why this architecture:**

- **Trapezoidal ringing immunity.** The BDF-2 stage at the end of every step acts as a
  native low-pass filter, guaranteeing $L$-stability. The simulator cannot produce false
  numerical ringing on fast digital nodes.
- **Goodbye "timestep too small".** PI controller damping avoids the reactive behaviour of
  classic SPICE, keeping $\Delta t$ fluid and maximizing step size.
- **Zero backtracking.** Problems are solved by moving forward. This drastically reduces
  Rust borrow-checker complexity since there is no need to restore past matrix state.

**Execution pipeline (main loop):**

1. **Phase 1 — Trapezoidal (TR).** The simulator advances time by $\gamma \cdot h$ (where
   $\gamma = 2 - \sqrt{2}$). The engine evaluates stamps (LHS and RHS), faer performs LU
   factorization, and Newton-Raphson solves for the intermediate point $x_{n+\gamma}$.
2. **Phase 2 — BDF-2 (Gear).** Advance the remaining time $(1 - \gamma) \cdot h$. The LHS
   is filtered/reused from the previous phase. The RHS history is updated using $x_n$ and
   $x_{n+\gamma}$. Newton-Raphson solves for the final timestep state $x_{n+1}$.
3. **LTE computation.** Milne's device is used. The Local Truncation Error is obtained by
   subtracting the final state $x_{n+1}$ from a simple predictor (extrapolation of $x_n$
   and $x_{n+\gamma}$). Computational cost is near-zero.
4. **PI controller (next step).** The error is normalized by tolerance (RELTOL). The
   algorithm applies integral and proportional gains based on the current error and the
   past error to compute the next timestep smoothly, without aggressive jumps.

**Deliverables:**
- `math/integration.rs`: TR-BDF2 companion-coefficient formula alongside the existing
  Trapezoidal and Gear/BDF variants.
- `solver/transient.rs`: two-phase step with intermediate point, Milne LTE estimate, and
  PI controller replacing the current LTE stepper.
- `codegen/device/analog.rs`: kernel evaluation for both TR and BDF-2 phases (LHS reuse
  across phases).
- Tests: stiff ODE validation case (e.g., van der Pol oscillator), switched-circuit case
  (PWM + RC filter), comparison of PI vs current LTE stepper on step-count.

**Status (2026-07-15):** the TR-BDF2 engine core is **DONE and active** — it is
the sole integration scheme (`IntegrationMethod` removal is the last cleanup
step). Landed: `TrBdf2` phase coefficients + Milne LTE (`math/integration.rs`),
two-phase driver (TR → `x_{n+γ}`, BDF2 → `x_{n+1}`), the trapezoidal companion
(re-derives the previous capacitor current from the prior BDF2 — the kernel was
pure-derivative before), a stateful **PI timestep controller** replacing the
reactive LTE stepper (`solver/convergence.rs::PiController`), and **always-on
adaptive** stepping (SPICE has been adaptive since v2; `.step` is the initial
dt). Backtracking on failure is ÷8. Bench waveform `mean`/`rms` are now
dt-weighted (trapezoidal) so statistics stay correct on the adaptive grid.
Spec: `.specs/features/solver-trbdf2-engine/`.

**Still open under this Epic:**
- **Breakpoints (T8-T10) — the efficiency gate for switched circuits.** Without
  them, the LTE-reject backtracking still resolves pulse edges (the TRB-20
  narrow-pulse probe is now monotonic and distinguishes 1 ns from 10 ns), but
  it thrashes ~40k steps at the edges. A source-declared breakpoint schedule
  (codegen-extracted from the periodic `floor(($abstime-td)/per)` phase trick +
  the `if (ph < pw)` threshold, or a `$periodic_breakpoints` declaration) lands
  the integrator on each edge in one step. Design decision pending.
- **Output interpolation onto the `.step` print grid.** The recorded waveform is
  currently the raw adaptive time grid (correct, but uneven). SPICE interpolates
  the internal adaptive steps onto the user's `.tran tstep` print interval;
  piperine should too (linear or quadratic). Until then, `Waveform::at(t)`
  already interpolates point queries, and dt-weighted stats are correct.
- **Inductor flux companion** uses the pure-derivative form for the TR stage
  (the dual — previous-voltage tracking — is a follow-up; no regression).
- Remove the vestigial `IntegrationMethod` enum + `suggest_transient_step`'s
  `method` param; migrate the last callers.



- **Newton convergence checks only the voltage step, not the current residual — DONE.**
  The current-residual half landed 2026-07-12 (`NIconvTest` — see SOLVER_GAPS §2).
  **Transistor ngspice parity achieved 2026-07-16 (spice-stdlib T7–T10):** the remaining
  MOS1 (~1.5× Id / NaN), JFET (~15 mV) and BJT (saturation / mirror non-convergence)
  discrepancies were all the **conditional-force penalty pattern** in the models
  (`V(x,xp) <- 0.0` under `if` lowered to a 1e12 penalty conductance) — fixed by exact
  `V = R·I` series-impedance forces (`FlatForce::current_terms`, stamped on the branch
  current column in DC/AC/tran). All 8 validation circuits plus Id–Vgs/Id–Vds sweep
  goldens are green with zero `#[ignore]` (`cargo test -p piperine-bench ngspice`).
- `transition`, `laplace_*`, `zi_*` analog operators — recognized in the IR, fail loud at
  codegen. Each is its own companion-model follow-up.
- **`table(x, xs, ys, mode)` operator (spec Part V §2) — not registered at all.** The
  resolved form has a `Table` symbol (`lower/symbols.rs`) but `lower/pom/analog_ops.rs`
  never registers a `"table"` operator, so a PHDL `table(...)` call doesn't even reach the
  fail-loud codegen path — it resolves as an unknown function. Register it (fail-loud until
  the interpolation companion model exists), then implement 1-D lookup + interpolation.
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
  **Multi-junction convergence — DONE (2026-07-11) via gmin stepping.** `bjt` (coupled
  B-E/B-C), `mos1`, and `jfet` all converge (spice `tests/junction.phdl` green). The fix was
  SPICE **gmin stepping** homotopy in the DC solver (`solver/dc.rs::solve_gmin_stepping`): on
  plain-Newton failure, ramp a node-to-ground conductance from 0.1 S → 0, warm-starting each
  step. Two bugs fixed en route: `emit_analog_binary` treats the `BitXor` pow-carrier as
  `pow` (BJT `qb` Jacobian), and a `var`/node name clash (`mos1` `var s` vs source node `s`)
  now resolves to the node. `fetlim`/`DEVlimvds` still identity (not needed for convergence;
  may matter for exact ngspice parity, which hasn't been diff'd against a built ngspice). See
  ROADMAP_REFINEMENT.md B5.
- **`@initial` cannot force a branch.** `@ initial { V(p,n) <- ic; }` (the SPICE `.ic`/UIC
  seed used by `dio`/`cap`/`ind`) fails loud: "statement Force … in an analog event body".
  Event bodies only support variable assignments today; an initial-condition force needs the
  solver to accept a branch constraint for the first timepoint.
- **Large analog bodies exceeding Cranelift's function-size limit — DONE (2026-07-05, emit CSE)
  + shared-temporary flattening (2026-07-11).** Two layers: (1) the emitter does exact
  common-subexpression elimination keyed by `(op-tag, child Value ids)` (`CseKey`/`cse_*`);
  (2) the flattener no longer inlines `var`s at all — each becomes a `__temp(id)` leaf on a
  value tape, differentiated once via a per-branch derivative tape (`__dtemp`), so the
  residual/Jacobian trees stay linear instead of exploding multiplicatively (a var reassigned
  under a guard and reused many times used to OOM). `dio` compiles and converges; `bjt`/`mos1`
  compile (convergence is the separate `$limit` item). See ROADMAP_REFINEMENT.md B0.
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
  **Follow-up:** migrate the spice-stdlib device models (`headers/spice/`) off their sentinel/`$param_given`
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
- **Attribute schema support.** `@schema_name(field = value, ...)` attributes are now
  validated and populated into the POM, but the LSP and VS Code extension don't yet:
  - Show `@schema_name` in completion (autocomplete registered schema names).
  - Validate attribute arguments in-editor (red squiggles on unknown fields, wrong
    types, missing required fields).
  - Hover on `@schema_name` → show the backing bundle's fields and types.
  - Goto-definition on `@schema_name` → jump to the `@attribute(schema = "...")`
    declaration on the bundle.
  - Show `@attribute(schema = "...")` bundles in the symbol outline.

## Spec / implementation divergences (2026-07-07 spec audit)

Cases where the formal specification (`docs/new-spec/`) describes intended behavior the
compiler does not yet enforce. The spec is the contract; these are bugs/gaps to close.

- **`white_noise` / `flicker_noise` — RESOLVED as correct-by-design (2026-07-11 audit).**
  The `0.0` in `lower/pom/analog_ops.rs` is the *residual* value — a noise source has
  zero mean current, so contributing 0 to DC/transient is the right semantics, not a
  silent stub. The PSD is extracted separately: `jit/flatten.rs` collects noise sources
  into dedicated rows (`FlatBody.noise`), `jit/analog.rs` compiles per-source PSD +
  flicker-exponent functions, and `device/analog.rs::noise_current_psd` evaluates them
  for `$noise`. No action needed; documented here so the audit doesn't re-flag it.
- **Keyword reservation is parser-level, not lexical.** The lexer
  (`parse/lexer.rs:14-17`) emits every keyword as `Tok::Ident(String)`; reservation is a
  parser concern. Documented as the current design in Part I §4.2; a future lexer
  refactor could tokenize keywords for robustness.
- **E2021 `PrivateItem` is defined but never raised.** The error variant exists
  (`pom/error.rs`), but privacy is enforced by *filtering* during `use` resolution, so an
  access to a private item surfaces as E2002/E2003 ("not in scope") — a worse diagnostic
  than "item exists but is private". Wire the resolver to remember filtered-out names and
  raise E2021 when one is referenced. (Part I §16 documents the current behavior.)
- **Selector axes `driver::`, `load::`, `parent::`, `ancestor::` — `AxisNotImplemented`.**
  Spec Part IV §10 defines all ten axes; `pom/selector/eval.rs` fails loud on these four
  (structural connectivity + parent chain). `driver::`/`load::` need per-net driver/load
  tracking on the POM `Net`; `parent::`/`ancestor::` need a child→parent instance link.
- **`piperine::` stdlib exemption from `pub` filtering.** Currently the resolver
  (`resolve.rs`) skips privacy filtering for `use piperine::...` — stdlib items are
  always exported regardless of `pub`. This is because the frozen header files
  (`headers/*.phdl`) don't declare their items `pub`. Fix: add `pub` to all header
  declarations and remove the exemption so the stdlib follows the same visibility rules
  as user packages.

---

## Type system explicitness — make implicit rules visible via capabilities

Several type-system rules are currently hardcoded in the compiler (typechecker widening
table, interpreter truthiness, intrinsic capability satisfaction). Expressing them as
explicit capabilities — the same mechanism already used for `Add`, `Eq`, `Ord`, etc. —
would make the rules visible, extensible, and self-documenting in the prelude.

### Conversion / widening capabilities (`From<T>`)

**Spec (Part I §6.1):** "`Boolean` widens to `Quad` implicitly; other casts are explicit
(`real(x)`, `int(x)`, `bit(x)`)."

**Today:** the widening table is hardcoded in `typecheck.rs:518-526` — a `matches!` block
listing six allowed pairs: `(Quad ← Boolean)`, `(Boolean ← Integer)`, `(Quad ← Integer)`,
`(Natural ← Integer)`, `(Boolean ← Natural)`, `(Quad ← Natural)`. Adding a new widening
requires editing the compiler; the rule is invisible to users reading the prelude.

**Goal:** express widening as a capability in the prelude so the relationship is explicit
and extensible:

```phdl
capability From<T> { fn from(v: T) -> Self; }
impl From<Boolean> for Quad { fn from(v: Boolean) -> Self { ... } }
impl From<Integer> for Natural { fn from(v: Integer) -> Self { ... } }
```

The typechecker would check a `From` bound instead of a hardcoded table. New conversions
(e.g. a future `SInt` → `Real`) would be a prelude `impl`, not a compiler change.

### Intrinsic capability satisfaction — make it explicit

**Spec (Part I §6.6):** "Primitives satisfy the relevant [operator] capabilities
intrinsically."

**Today:** `Real`, `Natural`, `Integer`, `Boolean`, `Quad` satisfy `Add`, `Sub`, `Mul`,
`Div`, `Eq`, `Ord`, `BitAnd`, `BitOr`, `BitXor`, `Not` — but there are no `impl` blocks
for this. It's hardcoded in the operator-desugar pass. A user reading the prelude sees
`capability Add { fn add(self, o: Self) -> Self; }` but never sees *who* satisfies it.

**Goal:** add explicit `impl Add for Real`, `impl Eq for Boolean`, etc. to the prelude
(or to a generated intrinsic-impls table). This makes the capability graph complete and
discoverable, and opens the door for user-defined numeric types that satisfy the same
capabilities.

### `Iterable<T>` capability

**Spec (Part III §12):** "A `for x in <expr>` is only valid in the interpreted context."

**Today:** the interpreter (`interp.rs:399-404`) hardcodes iteration over `Value::List`
and elaboration-time `Range`. A `Map<K,V>` or a user-defined collection cannot be iterated
even though it logically could be.

**Goal:** an `Iterable<T>` capability with a `fn next(self) -> Option<T>` method (or a
`fn iter(self) -> Iterator<T>`). The `for` loop would check `Iterable` instead of
hardcoding `Value::List`.

### Literal coercion rules

**Spec (Part I §6.1):** integer literals `0`/`1` serve as `Boolean`/`Quad`/`Natural`
depending on context.

**Today:** this is implicit in the typechecker widening table (same hardcoded `matches!`
block). The spec documents it but the compiler doesn't have a named mechanism for it.

**Goal:** express via the `From<T>` capability (above) or a dedicated `FromLiteral`
capability that documents which literal types coerce to which value types. This would
replace the ad-hoc integer-literal-as-Boolean rule with an explicit `impl
FromLiteral<Integer> for Boolean` in the prelude.

### Tuple types

**Spec (Part I §6.1):** tuples are listed as a value-layer collection — `(a, b, ...)`
with `.0`/`.1`/... indexing. Tuple **values** and indexing work; tuple **type
annotations** do not.

**Progress (2026-07-09, plugin-architecture branch):** the parser now parses tuple
types (`parse_type` has a `(` branch) and `ValueType::Tuple(Vec<ValueType>)` exists
(`net_type.rs`).

**Still open:** tuple type *resolution* — `resolve_type`/the typechecker have no
`Tuple` handling, so an annotation parses but is not checked against the value. Wire
resolution + checking, then test `fn foo() -> (Real, Natural)`, `var x : (Real, String)`,
`Vec<(Real, Real)>` (the bench-spec §12.4 sweep shape).

### Function references — passing named functions as arguments

**Spec (Part I §9.2):** "A function is a value: type `fn(T, U) -> R`."

**Today:** the `fn(T) -> R` type annotation **parses and resolves** — the grammar and
`ValueType::FnPtr` handle it. But the interpreter cannot **pass a named function** as an
argument. Writing `apply_op(my_func, 5.0)` where `my_func` is a top-level `fn` fails:
the interpreter resolves identifiers to values (`Value::Int`, `Value::Real`, etc.) but a
bare function name is not a `Value::Closure` — it's a `Callable::Function` that lives in
the registry, not in the value layer. Only lambdas (`|x| x * 2.0`) can be passed today,
because they evaluate to `Value::Closure` directly.

**Progress (2026-07-09, plugin-architecture branch):** landed as `Value::FnRef(String)` —
`eval_expr` on a bare `Expr::Ident` that resolves to a registered callable produces a
`FnRef`, and call sites (`interp.rs`) dispatch a local holding a `FnRef`/`Closure`
through the callable registry. **Still open:** confirm with a gate test
(`apply_op(my_func, 5.0)` end-to-end) and typecheck `FnRef` against the declared
`fn(T) -> R` annotation.

### Type inference for `var` — less verbosity

**Spec (Part III §1):** "`var name = expr;` may omit its type, inferred at interpretation
time — only valid in the interpreted context (bench)."

**Today:** in compiled contexts (`analog`/`digital`), a `var` requires an explicit type:
`var acc : Real = 0.0;`. Omitting the type is a hard error outside bench
(`behavior.rs:60-64`). In bench, the type is accepted but **ignored** — it's decorative,
not checked. There is no actual inference: the interpreter treats every value by its
runtime shape, and the typechecker doesn't infer from initializers.

**Goal:** proper type inference for initialized `var` declarations everywhere:
- `var x = 0.1;` → `Real` (literal inference)
- `var x = some_fn();` → return type of `some_fn` (call-site inference)
- `var x = a + b;` → type of `a` (binary-op inference)
- `var x = [1, 2, 3];` → `Vec<Natural>` (literal + element inference)

This eliminates the most common verbosity in PHDL without sacrificing type safety —
the type is still known at compile time, just not written by hand. `param`, ports, and
fields still require explicit types (their defaults/initializers may be absent).

**Lambda parameter Types.** Once type inference exists, lambda parameters should be
inferrable too: `|x| x * 2.0` should infer `x : Real` from the body, instead of
requiring the user to annotate every lambda parameter (today lambda params are
untyped and the interpreter handles them dynamically). This pairs with the function-
reference work above — when a lambda is passed as a `fn(T) -> U` argument, the
expected parameter types from the signature can drive inference.

### Discipline nature access by declared name

**Spec (Part I §10.1):** "the declared nature names are also available: `Temp(th)`,
`Pwr(th)`, etc."

**Today:** NOT properly implemented. The flattener (`jit/flatten.rs:472-475`)
hardcodes `"V"` as the only potential access and treats **everything else** as
`NatureKind::Flow`:

```rust
let nature_kind = match name.as_str() {
    "V" => NatureKind::Potential,
    _ => NatureKind::Flow,
};
```

So `Temp(th)` (a potential) is compiled as if it were a flow — silently wrong. `Pwr(th)`
(a flow) works by accident. The access name is never resolved against the discipline's
declared natures (`potential temp : Real; flow pwr : Real;`).

**Goal:** resolve the access name against the discipline's natures at lowering time.
When the flattener sees `Temp(th)`, it should look up `Temp` in `Thermal`'s declared
natures, find it's a `Potential`, and use `NatureKind::Potential`. This connects to the
`extern` declarations roadmap item — the accessors `V`, `I`, `Temp`, `Pwr` should be
declared as `extern fn` with signatures tied to their discipline, not hardcoded.

---

## `extern` declarations — explicit builtin contracts

### The problem

Today the compiler has three classes of builtins that are **invisible in source**:

1. **Intrinsic type+capability satisfaction.** `Real` satisfies `Add`, `Eq`, `Ord`,
   etc. — but there are no `impl` blocks. The satisfaction is hardcoded in the operator
   desugar pass. A user (or the IDE) reading the prelude sees `capability Add { fn
   add(self, o: Self) -> Self; }` but never sees *who implements it* or *what the
   contract guarantees*.

2. **Injected prelude items.** The prelude headers (`headers/*.phdl`) declare
   disciplines, bundles, enums, capabilities, and constants. But the compiler also
   injects intrinsic knowledge that lives **nowhere in source** — the math function
   table (`math.rs:46-72`, 25 fns), the analog operator registry (`analog_ops.rs`,
   21 operators), the `$`-syscall registry (`syscalls.rs`, 13 syscalls), the event
   registry (`event.rs`, 6 events). These are all implicit — a user has no way to
   discover their signatures, argument types, or semantics from PHDL source.

3. **Net type accessors.** `V(a,b)`, `I(a,b)`, `Temp(th)`, `Pwr(th)` — these are
   access functions tied to discipline natures, but their signatures are not declared
   anywhere in PHDL. The compiler generates them implicitly from the `potential`/`flow`
   declarations.

### The `extern` keyword

Introduce `extern` as a declaration modifier that tells the resolver: "the body of this
item is provided by the compiler; don't look for a source-level definition — but the
**signature is a real, checkable contract**."

```phdl
// The prelude declares the contract; the compiler provides the body.
extern fn sqrt(x: Real) -> Real;
extern fn ddt(x: Real) -> Real;
extern fn exp(x: Real) -> Real;
extern fn temperature() -> Real;       // $temperature without the $

// Intrinsic capability impls become visible:
extern impl Add for Real { fn add(self, o: Real) -> Real; }
extern impl Eq for Boolean { fn eq(self, o: Boolean) -> Boolean; }

// Discipline accessors are declared, not magic:
discipline Electrical {
    potential v : Real;
    flow i : Real;
}
extern fn V(a: Electrical, b: Electrical) -> Real;   // potential difference
extern fn I(a: Electrical, b: Electrical) -> Real;   // branch flow
```

### Benefits

- **IDE visibility.** Hovering `sqrt` in an editor shows the signature and doc comment
  from the `extern fn` declaration — today it shows nothing because the function isn't
  in any `.phdl` file.
- **Contract checking.** The type checker validates calls against the declared
  signature even for builtins. Today, a wrong-arity call to `ddt` might not be caught
  until codegen.
- **Discoverability.** `extern` declarations in the prelude serve as a living catalog
  of what the compiler provides. No need to cross-reference ROADMAP or code comments.
- **Extensibility.** Plugins register new `extern` items the same way — their
  signatures live in source, their bodies in the plugin.

### What this does NOT change

The `extern` keyword is purely a **declaration vs. definition** marker — like C's
`extern` or Rust's `extern "C"`. The runtime behavior of builtins is unchanged; the
compiler still dispatches to the same Rust implementations. `extern` just makes the
*contract* visible and checkable.

### Work

1. **Grammar:** `Item ::= ... | "extern" ExternItem` where `ExternItem` covers
   `FnDecl`, `ImplDecl`, and optionally `ModDecl` (for OSDI device-model stubs).
   **DONE for `fn` (2026-07-09, plugin-architecture branch).**
2. **Parser:** accept `extern` before `fn`/`impl`; the body is optional (signature-only).
   **DONE for `fn`** — `item.rs` eats `extern`, `FnDecl::parse_with_extern`, AST carries
   `is_extern`. `extern impl` still open.
3. **Elaborator:** register `extern fn` signatures in the callable registry; reject
   if a source-level body is also present (extern means "body is compiler-provided").
   **Still open** — nothing in `elab/` consults `is_extern` yet.
4. **Prelude migration:** move the math table, analog operators, syscalls, and events
   into `extern` declarations in the prelude headers. The compiler's internal tables
   cross-check against the declared signatures.
5. **LSP:** `extern` items are first-class symbols — goto-definition, hover, completion
   all work on them.

### Related: discipline accessors

The access functions `V(a,b)`, `I(a,b)`, and named natures (`Temp(th)`, `Pwr(th)`)
are currently compiler magic — generated from the `potential`/`flow`/`storage`
declarations. With `extern`, these could be declared explicitly:

```phdl
discipline Electrical {
    potential v : Real;
    flow i : Real;
}
// The compiler generates these from the nature declarations above:
extern fn V(a: Electrical, b: Electrical) -> Real;
extern fn I(a: Electrical, b: Electrical) -> Real;
```

Or better: the accessors could be **methods on the discipline** via an `impl`:

```phdl
impl Electrical {
    fn v(self, other: Electrical) -> Real;   // potential difference
    fn i(self, other: Electrical) -> Real;   // branch flow
}
```

This would replace the current free-function `V(a,b)`/`I(a,b)` with method syntax
`a.v(b)` / `a.i(b)`, and make the accessor signatures visible and checkable. The
free-function forms could remain as sugar.

---

## Plugin system — Part VI IMPLEMENTED (phases 0–5, 2026-07-10/11)

**Spec:** `docs/spec/part_vi_plugins.md` (Part VI, rewritten current 2026-07-11).
**Implementation plan:** `Plugin plan.md` (D1–D14 + delivery status).
All three backends (native dlopen, WASM/wasmtime, process JSON-RPC) share one
contract; the wire form **is the POM itself** (serde on the real
`Design`/`Value`, `pom/wire.rs` is protocol-only — D14, revised 2026-07-11);
staging conflicts are typed P0008; plugins live in the official monorepo
`~/Git/plugins` referenced via git deps with `subdir`. Builtin bench tasks
share the plugin task shape (`BenchTask::run(args, cx)`, `tasks.rs`).

**Still open:**
- **Artifact distribution** — prebuilt plugin binaries fetched from git
  releases per target triple (the host never builds plugin sources). Today a
  path/git plugin must have its `entry` artifact already built.
- **Wire-tier scripts** — capability-gated fs imports for WASM/process guests
  (declaring a script on those tiers is a loud load error today).
- **OSDI DeviceProvider netlist seam** — internal-node allocation for
  `@device(plugin = "osdi", …)` (see item 10 below).
- **`extract`/`.attach`/`.meta`** as spice-plugin bench tasks (G13).
- **`HookInput.solve` for swept analyses** — only `$op` carries node voltages;
  `$tran`/`$ac`/`$noise` hand plugins the analysis kind only.

Historical order (steps kept for reference; all but the noted leftovers done):

1. **POM project model** — hard prerequisite, see the section below.
2. **Manifest + discovery (§4, §5).** Parse `piperine-plugin.toml` into a permissions
   struct; `[plugins]` section in `Piperine.toml`; resolve into `target/plugins/<name>/`
   via the existing git/path resolver; `Piperine.lock` plugin entries with
   `manifest_hash`/`content_hash`. Errors: P0006 `BadManifest`, P0007 `HashMismatch`.
3. **TOFU (§3.2).** CLI approval prompt keyed by content hash, persisted to the
   lockfile; `--trust <file>` / `--no-trust` CI modes. Error: P0001 `Untrusted`.
4. **Registration contract (§6).** The `Plugin` trait: `manifest()`, `register()`
   (devices, attr schemas, bench tasks, scripts), seven no-op-default hooks. Start
   with the **native** backend (plain dlopen + one entry symbol — least new
   machinery), WASM (`wasmtime` + serialized POM views) second, out-of-process
   JSON-RPC last. The device ABI is Piperine's own `AnalogDevice`/`DigitalDevice`
   traits — never OSDI or any external model ABI (Plugin plan D13).
5. **Attribute schemas from plugins (§10).** Plugin-registered schemas join the
   `@attribute(schema=...)` registry; collision → P0003 `SchemaConflict`.
6. **Bench tasks from plugins (§6).** Plugin `BenchTask`s extend the
   `bench_task_implemented` allowlist at load time — this is the landing path for
   `extract`/`.attach`/`.meta` (G13 above).
7. **Device loading (§7).** `@device(plugin=…, type=…)` + `@port(name=…)` binding:
   `CircuitCompiler` detects the attribute, skips PHDL lowering, calls the plugin's
   `DeviceFactory` with the device spec (type id, attrs, port `NetRef` bindings,
   params). Solver sees a plain `Device`. Errors: P0004 `DeviceNotRegistered`.
8. **Lifecycle hooks (§8).** The seven hook points; `transform_design` mutates only
   through the staging handle (`set_param`/`add_instance`/`add_connection`), validated
   against the module table (no-netlist-magic). Alphabetical plugin order; conflicts →
   P0008 `StagingConflict`.
9. **Custom scripts (§9).** CLI dispatcher falls through to plugin-registered
   subcommands; capability-gated host context (`fs()`, `project()`, `spawn()`,
   `log()`); `piperine plugin list`. Error: P0009 `UnknownScript`.
10. **OSDI extraction (Plugin plan D13) — DONE (2026-07-10).** `solver/src/osdi/`
    moved to the external `~/Git/piperine-osdi` repo (loader/ffi/device/model +
    the openvaf-downloading build.rs + the full OSDI/cosim test corpus, 34
    tests green). The solver core dropped the `osdi` module, `build.rs`, and
    the `libloading` dependency; CLAUDE.md wording updated. `OsdiPlugin`
    registers the `@osdi` schema and an `Osdi::Device` factory.
    **Still open — DeviceProvider netlist seam:** OSDI setup allocates
    *internal* MNA nodes, but `PluginDeviceSpec` hands over already-connected
    terminal references only, so the `@device(plugin = "osdi", …)` PHDL
    binding fails loud (factory error explains). Extend the spec with a
    netlist handle (fresh-node allocation) to wire it; the `piperine_osdi`
    Rust API is fully functional meanwhile.

---

## POM project model — DONE (2026-07-10/11)

The POM carries a `Project` node (`pom/design.rs::Project` — name, version,
plugin names) populated during elaboration; `Design.project` is part of the
serialized surface. `piperine-project` resolves `[plugins]`
(path + git + `subdir` for the official monorepo) into local paths, the
lockfile records plugin entries with content hashes and TOFU trust, and
`PluginHost::load_for_project` anchors discovery/capabilities on the project
root. Remaining provenance ideas (per-item source package, dependency graph
reflection) folded into the section below as nice-to-haves — no longer
blocking anything.

<details><summary>Original text (for the provenance follow-ups)</summary>

## POM project model — prerequisite for Part VI (Plugins) [historical]

**Spec (Part VI):** plugins discover, load, and wire through a project model —
`Piperine.toml`, `Piperine.lock`, the resolver, plugin manifests, capability
enforcement. Today this lives entirely in `piperine-project` (a separate crate)
and the POM (`piperine-lang::pom::Design`) has no notion of a "project."

**Today:** the POM is a flat `Design` — modules, disciplines, bundles, enums,
capabilities, functions, instances. There is no concept of:
- Which file/package each item came from (erased by the resolver's text inlining).
- Project metadata (name, version, dependencies, plugins).
- The dependency graph (which package depends on which).
- Source provenance for diagnostics (which file/line produced this POM node).

**Problem:** Part VI (plugins) requires the host to know the project structure
to resolve plugin sources, apply TOFU, enforce capabilities, and wire
`@device` attributes to the right plugin. Without a project model in the POM,
the plugin system has no anchor point.

**Goal:** model the project in the POM. Two options:

1. **POM gains a `Project` node** — carries name, version, dependencies, plugin
   sources, lockfile state. Each item in the `Design` carries its source
   package. This makes the POM self-describing: a tool or plugin can reflect
   over the design AND the project in one graph.

2. **Merge `piperine-project` into `piperine-lang`** — the project crate is
   small (manifest parsing, git resolver, lockfile). Folding it into the lang
   crate eliminates a crate boundary and makes the project model available
   everywhere the POM is. The resolver becomes a module, not a separate crate.

Either way, the result is: every POM node knows where it came from, and the
project structure is queryable through the same reflection surface (Part IV)
that already serves the design.

**This is a hard prerequisite for Part VI (Plugins).** The plugin system
cannot be implemented without a project model to anchor plugin discovery,
TOFU, and capability enforcement.

</details>

---

## Extension / packaging (user-owned, deliberately out of agent scope)

VS Code extension productization, marketplace packaging, grammar/registry sync tests,
release/versioning story — see `editors/vscode/`.
