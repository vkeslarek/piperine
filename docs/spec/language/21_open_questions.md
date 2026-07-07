## 14. Open questions

- **Sweep sugar** — a `$dc(param, from, to, step)` convenience vs. the `for` idiom (no-bloat).
- **Waveform algebra scope** — which measures are built-in `Waveform` methods vs. library
  functions over `points()`/`fft()`.
- **Node reference type — resolved for milestone 1.** `.v(a, b)`/`.i(a, b)` take bare names,
  resolved against the bench's module POM (§3) into `Net`/`Instance` handles; only exposed
  top-level nets and instance ports are addressable (encapsulation holds — a device-internal node
  that never reaches a port is not nameable from a bench). `.i(a, b)` is defined as the *unique*
  two-terminal instance whose ports connect exactly to nets `a` and `b`; the instance-port form
  (`.i(resistor.p, resistor.n)`) is preferred and always unambiguous. A device-internal current
  with no MNA branch unknown (no ideal-source `<-`) is recomputed from the solved terminal
  voltages via the device's own residual, not read as a separate solver variable; a two-terminal
  match with more than one candidate instance is a fail-loud error, not a guess.
- **Default-argument ordering** — confirming trailing-only defaults (no keyword-argument calls) is
  enough, or whether named arguments at call sites are wanted (they would generalize `.name =`).
- **Override addressing** — milestone 1 stages by bare instance label within the bench's own
  module (`sw.ctrl = 1` stages against instance `sw`); hierarchical dotted paths into a nested
  DUT (`select("//dac/rseg[3]")`-style) are `select`'s job once it exists, not bare-name staging's.