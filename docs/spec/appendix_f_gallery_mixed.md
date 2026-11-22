# Appendix F. Gallery: mixed-signal systems

This appendix is **informative**. It adds no rule. Each system combines reactions, definitions
and `analog` blocks; the walkthroughs show which instants are time points, which are jumps, and
what each analysis can and cannot say about the system.

**Models referenced.**

| Model | Defined in |
| --- | --- |
| `SampleHold` | §12.1 |
| `Comparator` | §13 |
| `Resistor`, `Capacitor`, `Switch` | §8, §11; declarations as assumed in Appendix D |
| `Dram1T1C`, `MemoryBank`, bundle `Cell` | §8.4, §7 |
| `Vco`, `RSwitch` | D.3, D.6 |
| `Tran`, `Keeper`, `Driven`, `Strength` | §22.3 |
| `Logic` | §22.1 |

**Guarded instants.** Claims marked "(guarded instant)" rest on §17.2.2: a reaction counts at a
scheduled time only if its event fires at `(t, 0)` (Appendix D).

## F.1 SAR ADC

**Purpose.** An 8-bit successive-approximation converter: a sample-and-hold, a resistive DAC, a
comparator located by the solver, and a binary search in a reaction.

```rust
/// 8-bit DAC: ideal level, ladder resistance, output switch.
module Dac8(in code: Bit<u8>, vout: Electrical) {
    vref: f64 = 1.0;
    rout: f64 = 1k;
    rsw: f64 = 200.0;
    signal src: Electrical;
    pub signal core: Electrical;
}

analog for Dac8 {
    v(src) +> vref * (code as f64) / 256.0;
    i(src, core) +> v(src, core) / rout;
    i(core, vout) +> v(core, vout) / rsw;
}

/// Binary search; cmp is high when the input is above the DAC.
module SarLogic(
    in clk: Bit,
    in start: Bit,
    in cmp: Bit,
    out code: Bit<u8> = 0x80,
    out done: Bit = false,
) {
    signal bit: Bit<u3> = 7;
    signal busy: Bit = false;
}

digital for SarLogic {
    on clk.rise() && start && !busy {
        busy <- true;
        bit <- 7;
        code <- 0x80;
        done <- false;
    }
    on clk.rise() && busy {
        let kept = if cmp {
            code
        } else {
            code & !(1u8 << bit)
        };
        if bit == 0 {
            code <- kept;
            done <- true;
            busy <- false;
        } else {
            code <- kept | (1u8 << (bit - 1));
            bit <- bit - 1;
        }
    }
}

module SarAdc(vin: Electrical, in clk: Bit, in start: Bit, out code: Bit<u8>, out done: Bit) {
    cpar: f64 = 50f;
    signal vh: Electrical;
    signal vdac: Electrical;
    signal cmp: Bit;
    let sh = SampleHold(vin, vh, start); // 12.1
    let dac = Dac8(code, vdac);
    let comp = Comparator(vh, vdac, cmp); // 13
    let logic = SarLogic(clk, start, cmp, code, done);
}

analog for SarAdc {
    i(dac.core) +> ddt(cpar * v(dac.core));
} // the parent's own branch on a public node
```

> **Resolution.** `rsw` is written `200.0`. The source model had the integer literal `200` as
> the default of an `f64` param, which §3.2.3 does not convert.

**Semantics walkthrough: structure.**

| Aspect | Trace |
| --- | --- |
| Discrete drives | `code` and `done` are driven by `logic` through its `out` ports, which supply the initial values. `SarAdc` passes its own `out` ports down and writes neither (§5.2). `cmp` is driven by `comp`; `held` by `sh`. |
| Branches | `Dac8`: a potential branch `(src, gnd)` and two flow branches. `SampleHold`: a potential branch `(vh, gnd)`. `SarAdc`: its own flow branch `(dac.core, gnd)`, attached to a public node of a child (§8.2, `pub` rule). |
| Loads | `Comparator` and `SampleHold` probe their inputs and draw no flow (§11 rule 4). `vdac` carries no flow, so `v(vdac) = v(dac.core)`. |
| Reading `code` | `Dac8` reads it directly: `code` is a `Bit<u8>` and holds `0x80` from `(0, 0)` (§3.3.6). |

**Semantics walkthrough: the operating point.**

| Aspect | Trace |
| --- | --- |
| `(0, 0)` commits | `code ← 0x80`, `done ← false`, `bit ← 7`, `busy ← false`, `held ← 0.0`, `cmp ← false` (§16 step 2). |
| Solve | `v(vh) = 0`. `v(src) = v(dac.core) = v(vdac) = vref / 2`; `cpar` carries no flow. Every node has a resistive path. |
| Reactions of `(0, 0)` | `v(vh, vdac)` is negative. `dn = above(−v(inp, inn))` fires through its `init()` arm (§22.2). `q <- up` commits false, the value `cmp` already has. |
| `clk` or `start` high at 0 | A carrier that starts high has not risen (§13 rule 6): no conversion starts at `(0, 0)`. A conversion starts at the first rise after it. |

**Semantics walkthrough: one conversion.**

| Instant | Trace |
| --- | --- |
| `start` rises at `(t_s, 0)` | A time point: the `SampleHold` reaction probes (§12.1.5, §17.2.2). It commits `held <- v(vin)`. |
| `(t_s, 1)` | `held` applies and `analog` reads it: a jump (§15 step 3.4). `vh` is the potential of a source with no state, so it steps. `cross` is compared across the jump (§13 rule 5). A crossing found there fires at `(t_s, 2)`, and `cmp` applies at `(t_s, 3)`. |
| First `clk` edge with `start` high | The first reaction fires: `busy`, `bit ← 7`, `code ← 0x80`, `done ← false`. The second does not: it reads the old `busy` (§12.1.1). |
| Each of the next eight edges | The second reaction reads `cmp` and `code`. It keeps or clears bit `bit`, and sets the next lower bit as the new trial. |
| One microstep later | `code` applies; `Dac8` reads it: a jump. `cpar · v(core)` is a state and stays (§15.2.1). `v(src)` steps, and the flow through `rout` with it. |
| Between edges | `v(vdac)` settles with time constant `rout · cpar`, 50 ps. If it passes `v(vh)`, the comparator's `cross` is located (§17.3) and `cmp` follows at the next microstep. |
| Last edge (`bit == 0`) | `code <- kept; done <- true; busy <- false`. `code` changes only if bit 0 was cleared. |
| Never together | `busy` separates the two reactions. If both guards held, source order would decide each level (§12.1.2). |

**Semantics walkthrough: time points and analyses.**

| Aspect | Trace |
| --- | --- |
| Time points per conversion | The `start` edge; each `clk` edge whose reaction can change `code` (guarded instant); each located comparator crossing. Up to nine commits change `code`, each a jump. |
| Timing requirement | `cmp` must be settled when the next edge reads it. The clock period must exceed several `rout · cpar`; the model does not check it. A `Setup` monitor on `cmp` does (§22.5). |
| Level-sensitive `start` | `SarLogic` reads `start` as a level, `SampleHold` as an edge. If `start` stays high after `done`, the next edge starts a new conversion of the same held sample. |
| `Dc` | The operating point above. `at`-free and delay-free, so reactions run at each sweep point (§16.1). |
| `Ac` | `held` and `code` are constant (§11.1): `vh` and `src` are potential sources of zero small-signal value. `vin` has no path to any output. The zero-order-hold response of `SampleHold` appears only in a periodic analysis (§11.1, §16.3). |
| `Noise` | No `noise` instance: the resistances are written as bare contributions. |
| Gradients | `held` is real and carries the sensitivity of `v(vin)` at the sampling instant. `code` is not real and acts through event times only (§23.3.4). |

## F.2 Charge-pump PLL

**Purpose.** A phase-frequency detector with a reset delay, a charge pump, a passive loop filter,
the VCO of D.3 and a divider. The top module is pure structure.

```rust
module Pfd(in fref: Bit, in fdiv: Bit, out up: Bit = false, out dn: Bit = false) {
    treset: f64 = 50p;
}

digital for Pfd {
    on fref.rise() {
        up <- true;
    }
    on fdiv.rise() {
        dn <- true;
    }
    on (up.changed() || dn.changed()) && up && dn {
        up <- false after treset;
        dn <- false after treset;
    }
}

module ChargePump(in up: Bit, in dn: Bit, vc: Electrical) {
    icp: f64 = 100u;
}

analog for ChargePump {
    i(vc) +> if dn {
        icp
    } else {
        0.0
    } - if up {
        icp
    } else {
        0.0
    };
}

module Divider<const N: u32>(in clk: Bit, out q: Bit = false) where N >= 2 && N % 2 == 0 {
    signal cnt: Bit<u32> = 0;
}

digital<N> for Divider<N> {
    on clk.rise() {
        if cnt == N / 2 - 1 {
            cnt <- 0;
            q <- !q;
        } else {
            cnt <- cnt + 1;
        }
    }
}

/// Pure structure.
module Pll(in fref: Bit, out clk: Bit) {
    r: f64 = 10k;
    c1: f64 = 10p;
    c2: f64 = 1p;
    signal up: Bit;
    signal dn: Bit;
    signal fdiv: Bit;
    signal vc: Electrical = 0.0;
    signal mid: Electrical = 0.0;
    let pfd = Pfd(fref, fdiv, up, dn);
    let cp = ChargePump(up, dn, vc);
    let rz = Resistor(vc, mid) { r: r };
    let cz = Capacitor(mid, gnd) { c: c1 };
    let cp2 = Capacitor(vc, gnd) { c: c2 };
    let vco = Vco(vc, clk); // D.3
    let div = Divider::<8>(clk, fdiv);
}
```

> **Resolution.** `where N >= 2 && N % 2 == 0` is added to `Divider` (§8.1, const-generic
> constraint). `N / 2 - 1` wraps for `N < 2`, and an odd `N` would divide by `N − 1`.

**Semantics walkthrough: the operating point.**

| Aspect | Trace |
| --- | --- |
| Drives | `up`, `dn` by `pfd`; `fdiv` by `div`; `clk` by `vco`. Each driver supplies the initial value (§5.2). |
| Branch signs | `i(vc)` is the flow from `vc` to `gnd` through the pump (§6). `dn` adds `icp` out of `vc`; `up` subtracts it, charging the filter. |
| No DC path | `vc` and `mid` see capacitors and a current source only. Both have `= 0.0` and are held through the transient's operating point (§16). |
| Holds | Two holds to `gnd` and a resistor between them. The resistor is a flow branch, so no loop of potential contributions exists. |
| `(0, 0)` | All discrete initial values are committed. `up.changed()` and `dn.changed()` fire (§13 rule 6); the guard `up && dn` is false. The VCO's `above` fires (D.3). |
| `(0, 1)` | `clk ← true`. `clk.rise()` runs the divider: `cnt <- 1`. The divider counts the edge at time 0. |
| `fref` high at 0 | `fref.rise()` does not fire at `(0, 0)` (§13 rule 6): `up` stays low until the first rise after the start. |
| First step | The holds are removed. Both nodes carry `ddt` states, so the release is not a jump (§16). |

**Semantics walkthrough: the detector.**

| Situation | Trace |
| --- | --- |
| `fref` leads | `fref.rise()` at `(t1, 0)`; `up` applies at `(t1, 1)`: a jump, the pump flow steps by `icp`. `c2 · v(vc)` and `c1 · v(mid)` stay (§15.2.1). |
| `fdiv` follows at `t2` | `dn` applies at `(t2, 1)`. Its `changed()` fires there with `up && dn` true: the third reaction commits both resets for `t2 + treset`. |
| Overlap | For `treset` both are high and the pump flow is `icp − icp = 0`. |
| Reset | At `(t2 + treset, 0)` both fall. `changed()` fires; the guard is false. |
| Both edges in one microstep | Both commits apply at `(t, 1)`. The third reaction fires once (§13 rule 1) and schedules one reset. |
| Edge on the reset tick | The reset applies at `(t, 0)`. The edge's reaction runs at `(t, 0)` and its `up <- true` applies at `(t, 1)`. No two commits meet (§12.1.2), and the edge counts. |
| Edge while the reset is pending | `up` is already high. `up <- true` comes from another statement and does not cancel the reset (§12.1.4). Both outputs fall when it is due: the edge is lost, as in a real detector's dead zone. |

**Semantics walkthrough: time points and analyses.**

| Aspect | Trace |
| --- | --- |
| Time points | Rising `fref` and `fdiv` edges whose reactions can change `up` or `dn` (guarded instant); reset times; every VCO edge and wrap, which are located crossings (§17.2 table). |
| VCO edges | An edge that only advances the divider's counter is a time point anyway: it is located. It is not a jump. |
| Jumps | Changes of `up` and `dn`, and the two jumps of each VCO wrap (D.3). |
| Loop frequency | `vc` starts at 0 V: the VCO runs at `f0`, 1 GHz, and `fdiv` at 125 MHz. |
| `Dc` | Singular, naming `vc` and `mid`: the truth about a charge-pump filter (§16). |
| `Ac`, `Noise` | Their operating point fails the same way, also at a transient's last instant (§16.4.2). The loop is closed through discrete carriers and event times, so its response needs a periodic analysis (§16.3, G.30). |
| Gradients | Edge times move with `kvco`, `f0`, `icp` and the filter params through the located VCO crossings (§23.3.2). |

## F.3 DRAM array with a swappable cell

**Purpose.** An array whose cell is a template param. A behavioral cell replaces the
charge-based cell of §8.4 without touching the array.

```rust
/// Behavioral cell with the ports of Dram1T1C (§8.4), for fast runs.
module BehavioralCell(c: Cell) {
    vdd: f64 = 1.0;
    ron: f64 = 2k;
    signal bit: Bit = false;
}

analog for BehavioralCell {
    if c.wl {
        i(c.bl) +> (v(c.bl) - if bit {
            vdd
        } else {
            0.0
        }) / ron;
    }
}

digital for BehavioralCell {
    on c.wl.fall() {
        bit <- v(c.bl) > vdd / 2.0;
    }
}

module SenseAmp(bl: Electrical, vref: Electrical, in en: Bit, out q: Bit = false) {}

digital for SenseAmp {
    on en.rise() {
        q <- v(bl, vref) > 0.0;
    }
}

module Dram<const ROWS: u32, const COLS: u32>(in wl: [Bit; ROWS], in pre: Bit, in sense: Bit, out q: [Bit; COLS]) {
    cbl: f64 = 100f;
    vpre: f64 = 0.5;
    signal bl: [Electrical; COLS];
    signal vref: Electrical;
    pub let bank = MemoryBank::<_, ROWS, COLS>(wl, bl) { cell: Dram1T1C { cs: 30f } }; // 8.4
    for c in 0..COLS {
        let cb = Capacitor(bl[c], gnd) { c: cbl };
        let pc = RSwitch(bl[c], vref, pre); // D.6
        let sa = SenseAmp(bl[c], vref, sense, q[c]);
    }
}

analog<ROWS, COLS> for Dram<ROWS, COLS> {
    v(vref) +> vpre;
}
```

> **Resolution.** The store reaction of `BehavioralCell` listens to `c.wl.fall()`. A word line
> that starts low has not fallen at `(0, 0)` (§13 rule 6), so the declared `bit = false` is
> kept; no guard is needed.

**Semantics walkthrough: structure.**

| Aspect | Trace |
| --- | --- |
| Template | `cell: C` with `C: Fn(Cell)` holds for every module whose port list is `(Cell)` (§8.4). The param is structural (§8.1.2). |
| Swap | `BehavioralCell {}` in place of `Dram1T1C { cs: 30f }` changes every cell. `MemoryBank` is untouched. |
| Bundle | `Cell { wl: wl[r], bl: bl[c] }` is a bundle literal (§7). `wl` is an `in` field: the cell reads it. `bl` is a node. |
| Generation | Two nested `for`s place `ROWS × COLS` cells; the `let` makes a two-dimensional array (§8.2). `Dram`'s own `for` places three instances per column. |
| Paths | A storage node is probed as `bank.cells[3][5].sn`: `bank`, `cells` and `sn` are all `pub` (§8.2, `pub` rule). The host can read any path regardless (§21). |
| Drives | Each `q[c]` is driven by one `SenseAmp`. `vref` has one potential branch, from `Dram`. |

**Semantics walkthrough: `Dram1T1C` in the array.**

| Aspect | Trace |
| --- | --- |
| Operating point | Word lines low: the access branch has `g = 0` and carries no flow. Each `bl[c]` reaches `vref` through an `RSwitch`, on or off: 0.5 V. Each `sn` is held at 0 V, and its leak agrees (§16). |
| `(0, 0)` | `SenseAmp` starts with `q` false until the first `sense` edge. If `sense` starts high, `en.rise()` does not fire (§13 rule 6). |
| Precharge | `pre` high: `bl` follows `vref` through `ron`. `pre` low: `bl` floats on `cbl`, leaking through `roff`. |
| Word line rises | `analog` of `COLS` cells reads it: a time point and a jump. The switch is resistive, so `cs · v(sn)` and `cbl · v(bl)` stay (§15.2.1). The flows step. |
| Charge sharing | `sn` and `bl` approach each other with time constant near `ron · cs · cbl / (cs + cbl)`, about 0.1 ns. The bit line moves by `(v(sn) − vpre) · cs / (cs + cbl)`. |
| Sensing | `sense` rises: a time point, since the reaction probes (§12.1.5). It commits `q[c] <- v(bl, vref) > 0.0`, read from the solution before that microstep (§15 table). |
| Word line falls | A jump with nothing forced. The cell keeps the shared level, not its old one: the read is destructive. |
| Limits of the model | The array has no write driver and no restore. Every `sn` starts at 0 V and reads as 0. A write path is one more branch per bit line. |
| `Dc` | Solvable: every node has a resistive path (`rleak`, `roff`). |
| `Ac` | Linearized with the word lines as found at the operating point (§11.1). |
| `Noise` | No `noise` instance. |

**Semantics walkthrough: `BehavioralCell`.**

| Aspect | Trace |
| --- | --- |
| Branch kind | The branch `(c.bl, gnd)` gets a flow contribution or none. No contribution is zero flow (§11 rule 4). The kind never changes, so this is not a switch branch (§11 rule 2). |
| Word line high | The cell drives the bit line toward `vdd` or 0 V through `ron`. |
| Word line falls, `(t, n)` | The reaction runs before the step to `t` is solved and probes the solution of the previous time point (§15 step 3.3): the bit line as the cell and any writer left it. It commits `bit`. |
| `(t, n + 1)` | `bit` applies. Nothing is pending, so the step that arrives at `t` is solved (§15 step 3.4). It reads both changes: the fall of the word line removes the cell's flow, and `bit`, which `analog` reads statically, moves nothing with the word line low. |
| Time points | Every word-line change that `analog` reads; every fall, since the reaction probes. |

## F.4 Four resolved nets

**Purpose.** A discrete net that takes several drivers is a nature that the model declares and
gives the capability `Resolve` (§4 rule 8, §5.2). The library declares no net nature. Each of
the four nets below has its nature, its `impl`, its module code and a table of outcomes, and
none is analog.

### F.4.1 A plain `bool` into a wired-AND net

**Purpose.** No strength and no `Logic`: the `impl` says that a driver that is missing reads
high, which is the pull-up.

```rust,fragment
pub nature AndWire { value: bool }

impl Resolve<bool> for AndWire {
    fn resolve<const N: usize>(drives: [bool; N]) -> bool {
        drives.iter().all(|d| d)
    }
}

module IrqSource(irq_n: AndWire, in pending: Bit) {
    irq_n = !pending; // drives a plain bool
}

module Board(in a: Bit, in b: Bit) {
    signal irq_n: AndWire; // no value of its own: it is resolve of its drives
    let s0 = IrqSource(irq_n, a);
    let s1 = IrqSource(irq_n, b);
}
```

| `a` | `b` | `irq_n` |
| --- | --- | --- |
| false | false | true |
| true | false | false |
| true | true | false |
| (no `IrqSource` instance) | | true: `resolve` of an empty array |

| Aspect | Trace |
| --- | --- |
| Drives | `s0` and `s1` each hold one drive of `irq_n`, of the type `bool`. `irq_n` needs no initial value of its own (§5.2.3). |
| Resolution | `all` of the present drives: low when either source pulls it low, high when both release it or nobody drives it. |
| One drive | With one source the value is still `resolve` of that one drive (§5.2.10). |
| A drive of another type | A third module that drove a `Logic` into `irq_n` is `E0180` naming `AndWire` and `Logic`: the nature has no `Resolve<Logic>` (§5.2.15). |

### F.4.2 An open-drain line with a pull-up

**Purpose.** The pull-up is a module, and the line shows Z when it is missing.

```rust,fragment
pub nature Sda { value: Driven }

impl Resolve<Driven> for Sda {
    fn resolve<const N: usize>(drives: [Driven; N]) -> Driven {
        Driven::strongest(drives)
    }
}

module OpenDrainPin(sda: Sda, in pull_low: Bit) {
    sda = if pull_low { Driven::strong(false) } else { Driven::Z };
}

module I2cBus(in m: Bit, in s: Bit) {
    signal sda: Sda;
    let master = OpenDrainPin(sda, m);
    let slave = OpenDrainPin(sda, s);
    let rp = Pullup(sda); // Driven::pull(true)
}
```

| `master` | `slave` | `rp` | `sda` |
| --- | --- | --- | --- |
| Z | Z | pull 1 | 1 at `Pull` |
| strong 0 | Z | pull 1 | 0 at `Strong` |
| strong 0 | strong 0 | pull 1 | 0 at `Strong` |
| Z | Z | (no `Pullup` instance) | `Driven::Z`: `sda.v.is_z()` |

| Aspect | Trace |
| --- | --- |
| Drives | Three drives of `sda`: `master`, `slave`, `rp`, in instance-path order. `Pullup` is generic over `W: Nature + Resolve<Driven>` and accepts `Sda` because the nature implements the capability (§22.3.5). |
| Strength | A pull wins only while every stronger drive is released: `Strong` beats `Pull`, and `Pull` beats nothing. |
| Without the pull-up | With both pins released the line is `Driven::Z`. A reader that decides on it writes how: `sda.v.is_z()`, or `sda.v.to_volts(vdd)` in a converter, which reads Z as mid-rail. |
| Analog side | The analog pull-up resistor of a mixed model is the same line read from `analog`: `i(vdd, pad) +> v(vdd, pad) / rp` beside `v(pad) +> sda.v.to_volts(v(vdd))` in a converter module (`Bidir`, §22.4.3). |

### F.4.3 A tri-state bus with two drivers and a keeper

**Purpose.** Two buffers share one line, and a keeper remembers the last level that was driven.

```rust,fragment
pub nature TriBus { value: Driven }

impl Resolve<Driven> for TriBus {
    fn resolve<const N: usize>(drives: [Driven; N]) -> Driven {
        Driven::strongest(drives)
    }
}

module Bus(in d0: Bit, in oe0: Bit, in d1: Bit, in oe1: Bit) {
    signal line: TriBus;
    let a = Tribuf(line, d0, oe0);
    let b = Tribuf(line, d1, oe1);
    let k = Keeper(line); // §22.3.6
}
```

| `oe0` | `oe1` | `line` |
| --- | --- | --- |
| true | false | `d0` at `Strong` |
| false | true | `d1` at `Strong` |
| false | false | the keeper's last level at `Weak`; before any buffer has driven, X at `Weak` |

| Aspect | Trace |
| --- | --- |
| Drives | `a`, `b` and `k`. `k.others()` is `strongest` of the two buffers, without the keeper's own drive (§5.2.11). |
| Keeper memory | `last` starts at X (a `Bit<Logic>`): the model says it has never been driven, which a `bool` could not. A reaction remembers a level only when `others()` gives a 0 or a 1. |
| Before any drive | `line` reads X at `Weak`: `line.v.is_x()`. |
| No loop | The keeper's drive reads `last`, written by a reaction, and `others()` leaves its own drive out: no relaxation is needed (§8.2.3). |

### F.4.4 A conflict of equal strengths

**Purpose.** A conflict is a value; an instability is an error.

Both buffers of F.4.3 enabled, `d0` true and `d1` false: two `Strong` drives that disagree, so
`Driven::strongest` gives `Driven { v: Logic::X, s: Strength::Strong }`.

| `d0` | `d1` | `line` |
| --- | --- | --- |
| true | false | X at `Strong` |
| true | true | 1 at `Strong` |

| Aspect | Trace |
| --- | --- |
| A conflict | A reader sees `line.v.is_x()`; `line.v.to_bit()` is `None`; nothing stops. The keeper does not record it (it remembers only a 0 or a 1). |
| An instability | A loop of drives through `others()` with no reaction and no `after` that never stops changing, such as two modules that each drive the inverse of what the other drives, is not a conflict. After its bounded passes the run stops with `E0511`, naming the two drives (§8.2.3). |

A typed bus that allows one driver at a time is the same pattern with the author's own result
type, which is why the library has no `Tri`:

```rust,fragment
#[derive(Repr, PartialEq)]
pub enum Data {
    Free,
    Byte(u8),
    Contended,
}

pub nature DataBus { value: Data }

impl Resolve<Option<u8>, Data> for DataBus {
    fn resolve<const N: usize>(drives: [Option<u8>; N]) -> Data {
        drives.iter().fold(Data::Free, |a, x| match (a, x) {
            (y, None) => y,
            (Data::Free, Some(b)) => Data::Byte(b),
            _ => Data::Contended,
        })
    }
}
```

Here the driven type is `Option<u8>` and the value a reader sees is `Data`: `Resolve<X, V>`
with two types (§4 rule 8).

## F.5 Charge sharing through an ideal switch

**Purpose.** The jump rule on its smallest example: an ideal switch closes between two
capacitors at different potentials.

```rust
module ShareCharge(in close: Bit) {
    pub signal a: Electrical = 1.0; // held at 1 V through the transient's operating point
    pub signal b: Electrical = 0.0; // held at 0 V
    let c1 = Capacitor(a, gnd) { c: 1p };
    let c2 = Capacitor(b, gnd) { c: 3p };
    let sw = Switch(a, b, close); // 11: a short while close is high
}
```

> **Resolution.** `a` and `b` are internal public nodes with `= value`. The source model took
> them as ports and stated "a at 1 V", "b at 0 V" in comments only. A node with no DC path
> starts a transient from its `= value` (§16), so the initial charge is now part of the model.

**Semantics walkthrough.**

| Aspect | Trace |
| --- | --- |
| Branches | Two capacitor branches to `gnd`. `Switch` has a switch branch `(a, b)`: potential while `close` is high, flow otherwise (§11 rule 2). The selector is a discrete carrier, so the nodes are not merged and the branch keeps its flow unknown (§11 rule 3). |
| Operating point | `close` must be low. The switch is open; neither node has a DC path; both are held at their `= value` (§16). |
| `close` high at 0 | The holds on `a` and `b` and the short between them close a loop of potential contributions. The solve is singular and names them (§16). |
| First step | The holds are removed; `1p · v(a)` and `3p · v(b)` are the states. Nothing moves until the switch closes. |
| `close` rises at `(t, 0)` | The commit applies before the step to `t` is solved; that step reads `close`: a jump (§15 step 3.4). |

At that instant the step that arrives at `t` reads `close` (§15.2). It is a TR-BDF2 step
(Appendix C.2). With `i` the switch flow at the end of its trapezoidal stage and `k > 0` that
stage's weight on the new charge, its node rows are below; nothing moved before `t`, so no
previous derivative enters them:

| Row | Equation | Source |
| --- | --- | --- |
| node `a` | `k · 1p · (v(a) − 1) + i = 0` | `ddt` as the trapezoidal stage discretizes it |
| node `b` | `k · 3p · (v(b) − 0) − i = 0` | the same |
| switch | `v(a) − v(b) = 0` | the constraint that forces the jump |

| Step | Trace |
| --- | --- |
| The step | Adding the node rows removes `i` and `k`: `1p · (v − 1) + 3p · v = 0`, so the stage ends at 0.25 V. The second stage, backward Euler over the whole step from the charges of `t − h` (§15.2.6), has the same form and conserves the same sum. So `v(a) = v(b) = 0.25 V`: the right values are those of the first step that reads the jump. |
| Conservation | The cut that the switch closes holds 1 pC before and `4p · 0.25 = 1 pC` after (§15.2.2). |
| No solve of its own | The change has no solve at an unchanged `t`; the step to `t` is the solve that reads it (§15.2). |
| Restart | The step to `t` is an ordinary step, tested for truncation error; the step after it reads the change, is tested with the first-order estimate, drops the previous derivative and runs its trapezoidal stage as backward Euler (§15.2.6). |
| Afterwards | `a` and `b` float together at 0.25 V. The transient is determined by the states (§15.2.1). |
| `Dc` | Singular with the switch open or closed: no node has a DC path. Nothing is held outside a transient's operating point (§16). |
| `Ac`, `Noise` | Their operating point fails the same way. |
| Gradients | Across the jump the sensitivities of the states are carried by the step that reads it, as the states are (§15.2.10, §23.3.2). |
