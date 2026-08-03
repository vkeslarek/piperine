# codegen-simplification — measured shrinkage (T14, CGS-12)

**Requirement**: CGS-12 (AC 26–28). Measurement-only task, no source change.

## Per-file caps (spec AC 26)

| File | T1 baseline | Cap | Actual (post-delegation) | Status |
|---|---|---|---|---|
| `crates/piperine-codegen/src/device/analog/operators.rs` | 242 | ≤ 100 | **95** | ✅ met |
| `crates/piperine-codegen/src/device/analog/events.rs` | 188 | ≤ 140 | **160** | ❌ missed by 20 |
| `crates/piperine-codegen/src/device/analog/limits.rs` | 170 | ≤ 130 | **153** | ❌ missed by 23 |
| `crates/piperine-codegen/src/emit/analog_expr.rs` | 618 | ≤ 500 | **495** | ✅ met |
| `crates/piperine-codegen/src/resolve/pom/analog_ops.rs` | 358 | ≤ 330 | **375** | ❌ missed by 45 |

(T1 baseline column verified against `git show eadd5a2:<path> \| wc -l` for
all five files — matches spec.md's "Now" column exactly.)

## Crate-wide net deletion (spec AC 27)

- T1 baseline (`git ls-tree -r eadd5a2 crates/piperine-codegen/src`, every
  `.rs` file summed): **16 670 lines**.
- Current (`find crates/piperine-codegen/src -name '*.rs' | xargs wc -l`):
  **16 474 lines**.
- **Net removed: 196 lines.** Required: ≥ 300. **Missed by 104 lines.**

## Reason for the three missed caps and the missed net-deletion floor (spec AC 28)

Per AC 28, this is reported plainly, not fixed by moving code to another
file. Three per-file caps and the workspace-wide floor were sized on the
assumption that deleting a formula/state implementation and replacing it
with a call/wrapper would net out close to the size of what was deleted.
In practice each of the three missed files kept (or grew) a body of code
that is not the delegated family's *logic* but is real, load-bearing
adapter/mapping code the delegation itself requires:

- **`events.rs` (+20 over cap)**: keeps everything that is *not* the fire
  decision — the timer period table, the `CompiledTrigger` →
  `signal::events::Trigger` mapping (`to_signal_cross_dir`), and the
  `@initial`/fired-event variable-action dispatch (design.md's own scope
  for this file, unchanged by delegation). The file's job was always wider
  than "hold the detector," so the 188 → 160 shrink is real but bounded by
  that wider scope, not by the detector alone.
- **`limits.rs` (+23 over cap)**: T6's `SPEC_DEVIATION` (recorded, user-
  approved) added `Limiter::advance_precomputed` plumbing and two
  accessors (`seeds()`/`restore_seeds`) needed for checkpoint pack/unpack —
  code the original sketch did not anticipate, documented in-file because
  the two-step construction order (`seed()` after `@initial`) is load-
  bearing and the file's own header calls that split out explicitly.
- **`analog_ops.rs` (+45 over cap, and now *larger* than its own T1
  baseline)**: T8 replaced the hand-rolled segment/slope/validation code
  with a call into `signal::Table::with_mode` plus `points()`/
  `segment_slope()`, which is shorter — but the file holds `Ddt`/`Idt`/
  `IdtMod`/`Ddx`/`Delay`/`Slew`/`Table`/`Laplace` (eight operators total,
  `table` is one of eight), so the family this feature delegates is a
  fraction of the file's scope, and the loud-error mapping (`err_from`)
  plus doc comments explaining the delegation split (why validation lives
  in `signal::Table`, why `const_array`'s failure path is untouched) added
  lines the pre-delegation version did not carry.

The net-deletion floor (196 of 300 required) is the aggregate of the same
effect across all five delegations (T2–T10): each replaced a formula/state
body with a thinner call-through, but every one of them also added a
mapping layer, an adapter type (`SlottedOperator`, `LimitFn` + `mod
wrappers`), or explanatory doc comments justifying the split — code that
is new, not moved, and that the original 300-line target did not budget
for.

**This is not proposed to be fixed by moving code to another file** (AC 28
is explicit that doing so would not count as meeting the cap). The three
per-file misses and the net-deletion shortfall are reported here for the
user to decide whether to renegotiate the caps, accept the shortfall as
the honest cost of the adapter code delegation required, or scope a
follow-up cleanup pass — no decision is made unilaterally in this commit.

## Gate

`cargo build --workspace && cargo test --workspace`: **0 failed**, 276
`test result: ok` blocks, zero rustc warnings. `tests/ngspice_validation.rs`
(33/33), `tests/host_capability_parity.rs` (4/4, `table_nonlinearity`
unmodified), `tests/delegation_guard.rs` (2/2, all five witnesses proven
able to fail per T12's commit message), `crates/piperine-signal/tests/
purity.rs` (2/2) all pass.
