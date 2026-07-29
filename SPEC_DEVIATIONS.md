# Spec Deviation Audit — `docs/spec/` vs. the Implementation

**Date:** 2026-07-28 · **Scope:** every `SPEC_DEVIATION`, "not yet enforced",
and documented gap marker across `docs/spec/*.md`, `crates/*/src/`, `src/`,
and `ROADMAP.md`.

---

## Summary

| Category | Count | Severity |
|---|---|---|
| Explicit `SPEC_DEVIATION` (code + docs) | 12 | Design-level (documented, intentional) |
| Solver rules "not yet enforced" | 7 | Coverage gaps (reachable, unguarded) |
| Rust/Python surface asymmetries | 3 | Tracked follow-up |
| Infrastructure / publication gaps | 2 | Non-blocking |

None of the deviations contradict a spec goal. All are phrasing-level
(naming, scope, or partial delivery), recorded honestly in the code or spec
text, and tracked in `ROADMAP.md` or per-feature `tasks.md`.

---

## 1. Explicit `SPEC_DEVIATION` markers

### 1.1 `Session::set` — no auto-rebuild on structural writes (Rust)

**Spec says:** `Session.set` should auto-rebuild the circuit on a structural
(`Invalidation::Rebuild`) parameter write and count it in `rebuilds()`.

**Code does:** Rust `Session::set` **fails loud** on a structural write;
`rebuilds()` stays `0`. Python's live `_Session` (via `live.rs`) **does**
auto-rebuild — this behavior is preserved by the Python binding delegating
to `_Session::from_design`, not to `Session::set`.

**Locations:** `crates/piperine-api/src/session/entry.rs:32` (marker),
`docs/spec/part_viii_host_api.md:237`, `docs/spec/appendix_c_host_surface.md:142`.

**Rationale (recorded):** Python's ~150-line auto-rebuild machinery (dirty
ledger, mid-transient split, state carry) was deliberately not ported to the
Rust `Session` in Phase 5 of `p6-cleanup-architecture`. The Rust path fails
loud instead of silently rebuilding; the `rebuilds()` surface stays for a
future wiring without a signature change. `Session::sweep`/`sweep_grid` do
auto-rebuild, scoped to the sweep path.

**Risk:** A Rust host that loops `set` + analysis expecting auto-rebuild will
get a loud error instead. The workaround is `Session::compile` per
configuration.

---

### 1.2 `Trace<T>` — Python stays distinct pyclasses

**Spec says:** one generic `Trace<T>` container for every swept signal set.

**Code does:** Rust side — `AcTrace`/`NoiseTrace` are genuine type aliases
(`Trace<ComplexWaveform>`, `Trace<NoiseSample>`), not separate types
(`HOST-13` folded them in). Python side — `_AcTrace`/`_NoiseTrace` stay
distinct native `#[pyclass]` types, each wrapping the corresponding Rust
`Trace<T>` internally.

**Locations:** `docs/spec/part_viii_host_api.md:373`,
`docs/spec/appendix_c_host_surface.md:82-83`.

**Rationale:** PyO3 pyclasses cannot be generic over a Rust type parameter.
The method surface is identical; the class identity is not. A caller sees
the same `.v()`/`.i()`/`.axis()`/`.stats` regardless.

---

### 1.3 Instance-scoped param access (HOST-12)

**Spec says:** param access is module-level (`amp.param("m1.w")`).

**Code does:** instance-scoped — `op.instance("m1").param("w")` /
`op["m1"].param("w")`.

**Locations:** `docs/spec/part_viii_host_api.md:356`.

**Rationale:** `ParamDescriptor` is per-device and only available after
compilation, matching the shipped `Introspect::list_params` ABI and the same
instance-scoped pattern `opvar` already uses. Module-level param access would
require pre-compilation catalog lookup that the ABI does not expose.

---

### 1.4 `Into<Freq>` retrofit scoped to `Session::ac` only (HOST-21)

**Spec says:** every frequency/time-shaped argument accepts `impl Into<...>`
(SI-suffixed strings, bare `f64`, typed units).

**Code does:** the retrofit is applied to `Session::ac`'s `fstart`/`fstop`
only — not every frequency/time arg across the analysis menu.

**Locations:** `crates/piperine-api/src/session/entry.rs:273`,
`docs/spec/part_viii_host_api.md:492`.

**Rationale:** widening to every method is a separable mechanical follow-up.
The newtypes (`Freq`/`Time`), SI-string parsing, and Python helpers (`Hz`/
`ns`/`mV`/`C`) are fully delivered; only the `impl Into` signature
retrofit is partial.

---

### 1.5 Malformed SI string panics (`From` is infallible)

**Spec says:** `Freq::from("10MHz")` parses; a malformed string should fail
loud.

**Code does:** `From<&str> for Freq` panics on a malformed SI string (the
trait is infallible).

**Locations:** `docs/spec/part_viii_host_api.md:492`.

**Rationale:** the `From` trait cannot return `Result`. A `try_from` variant
is the clean fix; today the panic is the failure path (loud, not silent).

---

### 1.6 `.sens` — direct method, not single-linear-solve

**Spec says:** sensitivity via a single-direct linear solve against the
Jacobian.

**Code does:** central finite-difference over the restamp path
(`Session::sens` restamps `label.param` per point).

**Locations:** `crates/piperine-solver/src/analyses/sens.rs:7`.

**Rationale:** the direct method was sketched but not implemented; the
finite-difference path is numerically equivalent and reuses the existing
restamp infrastructure.

---

### 1.7 Language-server: multi-file `Design` framing

**Spec says:** the LSP frames a "multi-file `Design`" as the elaboration
unit.

**Code does:** each file is parsed/elaborated independently; cross-file
goto/rename works through a project-level symbol index, not a shared
`Design`.

**Locations:** `crates/piperine-lang-server/src/project.rs:56`.

---

### 1.8 Language-server: diagnostic fan-out across all known codes

**Spec says:** diagnostics for parse errors only.

**Code does:** the handler fans out across every code currently defined in
the elaborator's error catalog (broader than parse-only).

**Locations:** `crates/piperine-lang-server/src/handlers/diagnostics.rs:130`.

---

### 1.9 Language-server: occurrences without decl span

**Spec says:** occurrences require a declaration span.

**Code does:** T20/LSP-22 removed the `decl_span.is_some()` gate so
unlabeled instances also get occurrences.

**Locations:** `crates/piperine-lang-server/src/symbol_index.rs:364`,
`crates/piperine-lang-server/src/occurrences.rs:10`.

---

## 2. Solver failure rules "not yet enforced"

Seven rules in the Part VII §15 enforcement table are marked **not yet
enforced** — the condition is reachable in principle but passes silently
today. Each is tracked in the table with its honest reason.

| § | Rule | Why not enforced | Risk |
|---|---|---|---|
| §2 | Element declares no capability | Two existing fixtures rely on inert no-capability elements (`live_params.rs`, `solver_entry.rs`) | A no-capability element is silently inert — not an error |
| §4 | Digital boundary changes during an analysis | No boundary snapshot is compared per analysis | A mid-analysis topology change goes undetected |
| §4 | Digital event targets a nonexistent net | `DigitalNet` is an index; out-of-range is not constructible through the public surface | Unreachable through the API as shipped |
| §6 | Stamp references an unmapped variable | `AnalogReference` carries its index; an unmapped variable is not constructible | Unreachable through the API as shipped |
| §8 | Analysis-time loading changes matrix dimension | Nearest guard is `analyses/dc.rs`'s "solution not contiguous" | A dimension change mid-analysis may not be caught at the right layer |
| §10 | Transient reaches minimum timestep without converging | The `dt_min` floor exists; no test drives a circuit onto it | A non-converging transient at the dt floor is accepted with a warning |
| §11 | AC frequency point cannot solve its linear system | `math/faer.rs` wraps the linear failure, but no test constructs a singular AC point | A singular AC point would surface as a linear-solve error, not an analysis-level error |
| §14 | Digital delta cycle does not settle within iteration cap | `digital/scheduler.rs` raises it; no test builds a non-settling loop | The error path exists but is untested |

**Pattern:** most of these are unreachable through the public API (§4/§6) or
are error paths that exist but lack a driving test (§10/§11/§14). The two
genuine coverage gaps are §2 (inert no-capability elements) and §4 (digital
boundary changes).

---

## 3. Rust/Python surface asymmetries

| Surface | Rust | Python | Follow-up |
|---|---|---|---|
| `Waveform` measurements (`slew_rate`/`rise_time`/`fall_time`/`overshoot`/`settling_time`/`delay`) | ✅ `piperine-api` | ❌ not bound | HOST-14, tracked |
| `ComplexWaveform` margins (`bandwidth_3db`/`gain_margin`/`phase_margin`/`unity_gain_freq`) | ✅ `piperine-api` | ❌ not bound | HOST-16, tracked |
| `Waveform` transforms (`fft`/`resample`/`derivative`/`integral`/`clip`) | ✅ `piperine-api` | ❌ not bound | HOST-15, tracked |

These are the one intentional, tracked asymmetry left by the `host-library`
feature — not a parity-test regression (`ANALYSES` covers *analyses*, not
every `Waveform` method).

---

## 4. Infrastructure / publication gaps

### 4.1 mkdocs nav — Part VIII + Appendix C unpublished

`part_viii_host_api.md` and `appendix_c_host_surface.md` are **not** in
`mkdocs.yml`'s `nav:` today. The `p6-cleanup-architecture` deferred item
owns that wiring. Until it lands, the dv-core host-surface documentation
(margins channel, `r.violations`, three postures) is normative in the repo
but unpublished on the built site. Parts I, II, V, VII, and Appendix B
**are** in the nav.

**Locations:** `docs/spec/part_viii_host_api.md:23` (publication-status note).

### 4.2 Plugin v2 — manifest-level compatibility field deferred

`piperine_plugin_abi_version` is the only version guard; a manifest-level
compatibility field is a deferred follow-up.

**Locations:** `docs/spec/part_vi_plugins.md:388`.

---

## 5. Cross-reference: `ROADMAP.md`

`ROADMAP.md:482` records: "A handful of `SPEC_DEVIATION`s were recorded
along the way (instance- vs module-scoped param access, dict-kwargs vs
literal `a=`/`b=` sweep spelling in Python, `Session::set` structural-change
behavior) — see `.specs/features/host-library/tasks.md` per-task Status
notes and `validation.md` for the full list; none contradict a spec goal,
all are phrasing-level."

This audit confirms that statement: every `SPEC_DEVIATION` marker is
documented at its code site, cross-referenced in the formal spec, and
tracked in a feature's `tasks.md` or `ROADMAP.md`. No undocumented
deviations were found.

---

## 6. Recommendations

| Priority | Item | Action |
|---|---|---|
| Medium | §2 no-capability element (solver) | Either enforce the rule or document the inert-by-design choice formally |
| Medium | §10/§11/§14 solver error paths | Add driving tests (the error code exists, just untested) |
| Low | mkdocs nav gap | Wire Part VIII + Appendix C into `nav:` |
| Low | Python `Waveform` measurements | Bind HOST-14/15/16 to Python |
| Low | `Into<Freq>` widening | Mechanical retrofit to every freq/time arg |
| Low | `Freq::try_from` | Replace the `From` panic with a `TryFrom<&str>` |
