## 11. System-task availability

| Task / form | analog | digital | bench | Meaning in bench |
|-------------|:------:|:-------:|:-----:|------------------|
| `$assert(cond, msg)` | ✓ | ✓ | ✓ **implemented** | fails the test/flow |
| `$info/$warn/$error/$fatal` | ✓ | ✓ | ✓ **implemented** | run log |
| `$display(args…)` | ✓ | ✓ | ✓ **implemented** (arguments rendered — scalars, tuples, lists, maps, options — joined by a space, no severity prefix) | print to the run output |
| math (`exp`, `abs`, …) | ✓ | ✓ | ✓ **implemented** | same |
| `$op(cfg)` | — | — | ✓ **implemented** (`$op()` and `$op(OpConfig { .solver = Solver { … } })`) | DC operating point → `OpResult` |
| `$tran(cfg)` | — | — | ✓ **implemented** (`TranConfig { .stop, .step /*0 = adaptive auto*/, .start /*delayed-start: solve from 0, record from .start*/, .solver }`; positional `(stop, step)` kept as convenience; `ic:` maps not yet) | transient → `Trace` |
| `$ac(cfg)` | — | — | ✓ **implemented** (`AcConfig { .fstart, .fstop, .points, .scale, .solver }`; `Oct` maps onto the solver's log sweep) | frequency sweep → complex `Trace` |
| `$noise(cfg)` | — | — | ✓ **implemented** (`NoiseConfig { .out = Net \| (Net, Net), .fstart, .fstop, .points, .scale, .solver }` — the spec's `out : Branch` field, a bare Net meaning `(net, gnd)` or a `(Net, Net)` pair; the positional `$noise(out, cfg)` alias is kept for one release) | `NoiseTrace.{psd,total}` |
| result `.v/.i` | — | — | ✓ **implemented** on `OpResult`, `Trace`, and the AC `Trace` (`Trace.i` recomputes a two-terminal device's current per step from the solved voltages — resistive via `eval_residual`, reactive via `dQ/dt` of `eval_charge`; ideal sources read the exact branch unknown; devices reading runtime state/vars fail loud) | measurement (§4, §6) |
| `Waveform` methods | — | — | ✓ **implemented**: `at/min/max/mean/rms/peak_to_peak/len/points/cross/rise_time/fall_time/fft`, `mag/phase/db` on `Waveform<Complex>`, and `map(f)` (a closure-taking method — the interpreter invokes the closure per sample; Real result stays `Waveform`, Complex result stays `ComplexWaveform`) | measurement (§6) |
| `select`, name/`.set` staging | — | — | ✓ **implemented**: bare-name staging (`sw.ctrl = 1`), `select("...").param = v` bulk staging (string-literal paths), and `select("...")` in *expression* position returning a `SelectionRef` (`len`/`labels`/field-read; staging via a held selection re-runs against the live design) | reflection + override |
| `extract`, `.attach`, `.meta` | — | — | ✓ *not yet implemented* | plugin annotations (extensibility spec) |
| `$write(path, …)` | — | — | ✓ **implemented** (CSV of lists/tuples/scalars) | emit artifacts |
| `$plot(w, title)` | — | — | ✓ *not yet implemented* | emit artifacts |
| `V(a,b)`/`I(a,b)` branch access | ✓ | — | ✗ | analog-only; bench uses a result object |
| `<+`, `<-`, `ddt`, `idt`, operators | ✓ | — | ✗ | measure, not contribute |
| `@` events, `posedge`/`cross` | ✓ | ✓ | ✗ | events belong to the solve |

There are no configuration setter tasks (`$option`/`$temperature(set)`/`$ic`/`$nodeset`) — that
configuration is fields of the analysis config bundle (§5.1). A task the toolchain does not
implement is a compile error in a bench, not a silent no-op — calling an unimplemented task fails
at elaboration, before any analysis ever runs.

The §5.1 config bundles (`Solver`, `OpConfig`, `TranConfig`, `AcConfig`, `NoiseConfig`) and the
`Scale`/`CrossDir` enums are **defined in the stdlib prelude** and consumed by the analyses; the
`Map` value type backs the `ic`/`nodeset` fields. Default parameter values on user-defined `fn`/method
signatures (Part I §9.1) are **implemented**: trailing params may carry a default
(`fn foo(x: Real, k: Real = 2.0)`), a call may omit them, and defaults are elaboration constants
honored by both the interpreter (bench/POM fns) and the IR inliner (analog fns used in contributions).

---

