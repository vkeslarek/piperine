# Appendix E. Gallery: digital

This appendix is **informative**. It adds no rule. Every model here uses discrete carriers only:
none has a node, a probe or an `analog` block, so none touches the solver.

**What every model of this appendix does in each analysis.** The rows follow from §16.2 and are
not repeated per model.

| Situation | Behavior | Rule |
| --- | --- | --- |
| `(0, 0)` | Every initial value, written or `Default::default()`, is committed; `changed()` fires for it. Definitions settle before reactions run. | §16 step 2, §13 rule 6 |
| Transient | Reactions run at their instants. No instant is a time point because of these models alone. | §17.2.2 |
| `Dc`, operating point of `Ac`/`Noise` | Reactions and undelayed commits run at the microsteps of `(0, ·)` and of each sweep point. A commit with `after d`, `d > 0`, never comes due. | §16.1.3 |
| `Dc` sweep | Discrete state carries from point to point. Initial values are committed at the first point only. | §16.1.2 |
| Small-signal sweep | No instant, no reaction, no carrier change. A discrete carrier has no response. | §16.2, §11.1 |
| `Noise` | No source. | §11.1 |
| `settle` | Fires at the operating point and at the end of the analysis, even with nothing analog. | §15 |

Types referenced: `Tx` (§2), `Stream<T>` (§7), `Logic` (§22.1), `Driven`, `Pullup` (§22.3).

## E.1 Flip-flop with asynchronous reset

**Purpose.** A register of any `Repr` type with an active-low asynchronous reset.

```rust
module Dff<T: Repr + Default>(in clk: Bit, in rst_n: Bit, in d: Bit<T>, out q: Bit<T>) {
    reset: T;
}

digital<T> for Dff<T> {
    on clk.rise() || rst_n.fall() {
        if !rst_n {
            q <- reset;
        } else {
            q <- d;
        }
    }
}
```

**Semantics walkthrough.**

| Aspect | Trace |
| --- | --- |
| Drives | `Dff` holds the one drive of `q`, written by commits (§5.2). `q` has no initial value and starts at `Default::default()` of `T`, which is why `T` is bound by `Default`. |
| Events | `a || b` fires when either fires; the reaction runs once even when both do (§13 rule 1). |
| Reset low from the start | `rst_n` starts low and `changed()` fires at `(0, 0)`, but `fall()` does not: `prev()` is the initial value, so a carrier that starts low has not fallen (§13 rule 6, §22.1). A model that must reset at the start writes `init()`. |
| Clock high from the start | `clk.rise()` does not fire at `(0, 0)`: a clock that starts high has not risen (§22.1). The first edge is the first change after the start. |
| Clock edge at `(t, n)` | The reaction reads `rst_n` and `d` as settled at microstep `n` (§12.1.1). The commit applies at `(t, n + 1)`. |
| `d` changing with the edge | A `d` committed for the same microstep as the edge is already applied when the reaction reads it. A `d` committed by a reaction of that microstep is seen at the next edge. |
| Reset released | `rst_n` rising fires neither event. `q` keeps `reset` until the next clock edge. |
| Clock edge under reset | `!rst_n` is true: `q <- reset`. |
| One path | `if !rst_n` takes exactly one arm: `rst_n` is a `bool` (§3.3.5). A reset input that may be X or Z is a `Bit<Logic>` read through `is_low()`. |

## E.2 Latches

**Purpose.** A level-sensitive latch as a reaction, and a set-reset latch as two delayed
definitions with no reaction.

```rust
module DLatch<T: Repr + Default>(in en: Bit, in d: Bit<T>, out q: Bit<T>) {}

digital<T> for DLatch<T> {
    on en.changed() || d.changed() {
        if en {
            q <- d;
        }
    } // transparent from time 0: the initial values are changes
}

/// Cross-coupled NOR gates with gate delay: pure structure.
module SrLatch(in s: Bit, in r: Bit, out q: Bit = false, out qn: Bit = true) {
    tpd: f64 = 10p where tpd > 0.0;
    q = !(r | qn) after tpd;
    qn = !(s | q) after tpd;
}
```

**Semantics walkthrough: `DLatch`.**

| Aspect | Trace |
| --- | --- |
| `(0, 0)` | The initial values of `en` and `d` are changes (§13 rule 6). With `en` high the reaction commits `q <- d`: the latch is transparent from time 0. |
| Transparent | Each change of `d` while `en` is high reaches `q` one microstep later. |
| Closing | `en` falling runs the reaction with `en` low: no commit. `q` keeps the last value committed. |
| `en` falls as `d` changes | Both are applied before the reaction reads (§12.1.1). `en` reads low, so the new `d` is not taken. The outcome is defined, not a race. |

**Semantics walkthrough: `SrLatch`.**

| Aspect | Trace |
| --- | --- |
| Drives | Each output has one drive: a delayed definition, whose `= value` is its first commit (§5.2, §8.2.4). |
| No loop | Each drive depends on the other through an `after`. Without it the two definitions form a loop through one-driver carriers, an elaboration error (§8.2.3). |
| `(0, 0)`, `s = r = 0` | `q ← 0`, `qn ← 1` are committed. The definitions commit `q <- !(0 \| 1) = 0` and `qn <- !(0 \| 0) = 1` for `tpd`. The state is consistent and nothing changes. |
| Set | `s` rises at `t`: `qn <- 0` applies at `t + tpd`, then `q <- 1` at `t + 2·tpd`. |
| Both high, then released | With `s = r = 1`, both outputs are 0. Released together at `t`, both go to 1 at `t + tpd` and to 0 at `t + 2·tpd`: an oscillation of period `2·tpd`, as in the ideal circuit. |
| Glitches | Delays are transport (§12.1.4). A pulse on `s` shorter than `tpd` propagates. An inertial gate is `inertial` (§22.5). |
| Where time does not advance | Neither commit comes due (§16.1.3). The latch keeps its initial values whatever `s` and `r` are, through a whole `Dc` sweep (G.32). |

## E.3 Pipelines

**Purpose.** Reads see the past (§12.1.1), so one reaction describes a whole pipeline, in any
statement order.

```rust
module MacPipe(
    in clk: Bit,
    in a: Bit<i16>,
    in b: Bit<i16>,
    in valid_in: Bit,
    out acc: Bit<i40> = 0,
    out valid_out: Bit = false,
) {
    signal a1: Bit<i16> = 0;
    signal b1: Bit<i16> = 0;
    signal v1: Bit = false;
    signal p2: Bit<i32> = 0;
    signal v2: Bit = false;
}

digital for MacPipe {
    on clk.rise() {
        a1 <- a;
        b1 <- b;
        v1 <- valid_in; // stage 1
        p2 <- (a1 as i32) * (b1 as i32);
        v2 <- v1; // stage 2
        if v2 {
            acc <- acc + p2 as i40;
        }
        valid_out <- v2; // stage 3
    }
}

module Delay<T: Repr + Default, const N: usize>(in clk: Bit, in d: Bit<T>, out q: Bit<T>) where N >= 1 {
    signal r: [Bit<T>; N];
    q = r[N - 1];
}

digital<T, N> for Delay<T, N> {
    on clk.rise() {
        r[0] <- d;
        for k in 1..N {
            r[k] <- r[k - 1];
        }
    }
}
```

> **Resolution.** `where N >= 1` is added to `Delay` (§8.1, const-generic constraint). With
> `N == 0` the definition `q = r[N - 1]` has no element to read.

**Semantics walkthrough: `MacPipe`.**

| Aspect | Trace |
| --- | --- |
| One snapshot | Every right-hand side reads values from the start of the microstep; all commits apply together at the next (§12.1.1). Statement order does not matter here: no level is written twice. |
| Latency | Operands captured at edge 1 are multiplied at edge 2 and accumulated at edge 3. `acc` and `valid_out` show the result after edge 3. |
| Reading its own output | `acc` is an `out` port and is read inside the module like any of its carriers (§8.1). |
| Start | `a1`, `b1` and `p2` are 0 and `v1`, `v2` are false: `acc` is not written until a valid operand has travelled the pipeline. |
| Width | `acc + p2 as i40` is `acc + (p2 as i40)`; the sum wraps at 40 levels (§1). |

**Semantics walkthrough: `Delay`.**

| Aspect | Trace |
| --- | --- |
| Drives | `r` is written element by element by one reaction; `q` is a definition over `r[N − 1]` (§5.2). |
| The loop | The `for` in the reaction is a value-level loop with a constant bound. Every `r[k − 1]` is the old value, so the array shifts by one. |
| Output | `q` settles in the microstep where `r[N − 1]` applies, before that microstep's reactions (§8.2.1). |
| Start | `q` holds `Default::default()` of `T` until `N` edges have passed. |

## E.4 State machine with an enum

**Purpose.** A UART transmitter whose state is an `enum` with a payload, carried by one `Bit<Tx>`.

```rust
/// Tx is the enum of section 2.
module UartTx(
    in clk: Bit,
    in baud_tick: Bit,
    in start: Bit,
    in byte: Bit<u8>,
    out txd: Bit = true,
    out busy: Bit,
) {
    signal st: Bit<Tx> = Tx::Idle;
    signal sh: Bit<u8> = 0;
    busy = st != Tx::Idle;
}

digital for UartTx {
    on clk.rise() {
        match st {
            Tx::Idle => if start {
                sh <- byte;
                st <- Tx::Start;
            },
            _ if !baud_tick => {},
            Tx::Start => {
                txd <- false;
                st <- Tx::Data { n: 0 };
            },
            Tx::Data { n } => {
                txd <- (sh & 1) == 1;
                sh <- sh >> 1;
                st <- if n == 7 {
                    Tx::Stop
                } else {
                    Tx::Data { n: n + 1 }
                };
            },
            Tx::Stop => {
                txd <- true;
                st <- Tx::Idle;
            },
        }
    }
}
```

**Semantics walkthrough.**

| Aspect | Trace |
| --- | --- |
| Drives | `txd`, `st`, `sh` by commits; `busy` by a definition (§5.2). `busy` has no initial value: an undelayed definition drives it, and its first evaluation at `(0, 0)` is its first value (§8.2.29). |
| `(0, 0)` | `st ← Idle`, `sh ← 0`, `txd ← true` are committed. `busy` settles to false and its `changed()` fires (§13 rule 6). |
| Accepting a byte | An edge in `Idle` with `start` high commits `sh <- byte` and `st <- Start`. `busy` rises in the microstep where `st` applies (§8.2.1). |
| Pacing | In every other state an edge without `baud_tick` matches the guarded arm and commits nothing. |
| Frame | Successive ticks commit the start bit, eight data bits least significant first, and the stop bit. `n` counts 0 to 7 in a `u3`. |
| End of frame | The tick in `Stop` commits `txd <- true` and `st <- Idle` together. `busy` falls as the stop bit begins. |
| Stop bit length | A new byte accepted at once still waits in `Start` for the next tick before pulling `txd` low. The stop bit lasts one full bit. |
| Encoding | Behavior does not depend on the tag encoding (§2 rule 2). |

## E.5 Memory

**Purpose.** A synchronous RAM as an array of carriers, with read-before-write.

```rust
module Ram(
    in clk: Bit,
    in we: Bit,
    in addr: Bit<u8>,
    in wdata: Bit<u32>,
    out rdata: Bit<u32> = 0,
) {
    signal mem: [Bit<u32>; 256] = [0; 256]; // one carrier per word, materialized on first write (§5.1)
}

digital for Ram {
    on clk.rise() {
        if we {
            mem[addr] <- wdata;
        }
        rdata <- mem[addr]; // read-before-write
    }
}
```

**Semantics walkthrough.**

| Aspect | Trace |
| --- | --- |
| Drives | `Ram` drives each word of `mem` and `rdata` by commits. A commit to `mem[addr]` writes that element's levels only (§12.1). |
| Read-before-write | `mem[addr]` on the right reads the word as it was at the start of the microstep (§12.1.1). A write and a read of one address at one edge return the old word. |
| Storage | A word exists from its first commit (§5.1.3). A word never written reads the array's initial value, 0, and costs no storage (§5.1.8). |
| Index | `addr` is a `u8`: an index is a plain integer, and an out-of-range index is a panic (§1 rule 11). |
| Asynchronous read | A read port without a clock is a definition, `rdata = mem[addr];`, in place of the commit (§8.2). |

## E.6 Stream register slice

**Purpose.** One pipeline stage of a valid/ready stream. It registers `valid` and `data`; `ready`
passes through combinationally.

```rust
module Slice<T: Repr + Default>(in clk: Bit, rx: flip Stream<T>, tx: Stream<T>) {
    signal full: Bit = false;
    signal buf: Bit<T>;
    tx.data = buf;
    tx.valid = full;
    rx.ready = !full || tx.ready;
}

digital<T> for Slice<T> {
    on rx.beat(clk) {
        buf <- rx.data;
        full <- true;
    }
    on clk.rise() && !(rx.valid && rx.ready) && tx.ready {
        full <- false;
    }
}
```

**Semantics walkthrough.**

| Aspect | Trace |
| --- | --- |
| Directions | `tx` is the producer view: `Slice` drives `tx.valid` and `tx.data` and reads `tx.ready`. `rx` is flipped: it reads `rx.valid` and `rx.data` and drives `rx.ready` (§7). |
| Drives | Three definitions, one per driven bundle field; `full` and `buf` by commits. Each carrier has one drive (§5.2). |
| `(0, 0)` | `full ← false`. `tx.valid` settles to false. `rx.ready` is true: `!full` is true, so `tx.ready` is not read. `buf` and `tx.data` hold `Default::default()` of `T`. |
| Events | `rx.beat(clk)` is `clk.rise() && rx.valid && rx.ready`, a function returning `Event` on the bundle (§7, §13). The second event is a guarded `clk.rise()` (§13 rule 1). |
| Input beat | `buf <- rx.data; full <- true`. |
| Output beat only | `full` and `tx.ready` with no input beat: the second reaction commits `full <- false`. |
| Both in one edge | `tx.ready` makes `rx.ready` true, so the input beat happens. The first reaction replaces `buf`; the second is excluded by its guard. `full` stays true: full throughput. |
| Stalled | `full` and not `tx.ready`: `rx.ready` is false, no reaction fires. |
| Exclusive guards | The two reactions never fire together, so no level of `full` is written twice. If they did, source order would decide (§12.1.2). |
| Chains | `rx.ready` depends on `tx.ready`. A chain of slices is a chain of definitions that settles in one microstep, in dependency order (§8.2.1). |

## E.7 Open-drain interrupt line

**Purpose.** Several sources share one active-low line through a resolved nature the model
declares. No module knows how many drivers there are.

```rust,fragment
pub nature IrqLine { value: Driven }

impl Resolve<Driven> for IrqLine {
    fn resolve<const N: usize>(drives: [Driven; N]) -> Driven {
        Driven::strongest(drives)
    }
}

module IrqSource(in req: Bit, irq_n: IrqLine) {
    irq_n = if req {
        Driven::strong(false)
    } else {
        Driven::Z
    };
}

module IrqController(in clk: Bit, irq_n: IrqLine, out pending: Bit = false) {}

digital for IrqController {
    on clk.rise() {
        pending <- irq_n.v.is_low();
    }
}

module Board(in clk: Bit, in req0: Bit, in req1: Bit, out pending: Bit) {
    signal irq_n: IrqLine;
    let pu = Pullup(irq_n); // wins only while every source is released
    let s0 = IrqSource(req0, irq_n);
    let s1 = IrqSource(req1, irq_n);
    let ic = IrqController(clk, irq_n, pending);
}
```

**Semantics walkthrough.**

| Aspect | Trace |
| --- | --- |
| Drives | `irq_n` has three drives: `pu`, `s0`, `s1`. `IrqController` never writes it and holds no drive (§5.2). `pending` is driven by `ic`, which supplies its initial value. `irq_n` needs no initial value of its own: its nature implements `Resolve` (§5.2.3). |
| Resolution | The value is `Driven::strongest` of the three drives, in instance-path order (§4 rule 2). `strongest` does not depend on the order (§22.3.2). |
| `(0, 0)` | Each definition makes its first commit. The resolved value settles in the same microstep, and `irq_n.changed()` fires (§15 step 3.2). |
| Idle | Both sources drive `Driven::Z`. The pull-up wins: `irq_n.v` is 1. |
| A request | One source drives `strong(false)`. Strong beats pull: `irq_n.v` is 0. A second request changes nothing. |
| Sampling | At a clock edge the controller reads the resolved value settled in that microstep and commits `pending <- irq_n.v.is_low()` (§12.1.1). |
| Adding a source | One more `IrqSource` anywhere `irq_n` is visible. Nothing counts drivers. |
| No pull-up | With every source released the line is `Driven::Z`, whose level is Z (§22.3). `is_low()` is false, so `pending` stays low: the model says what an undriven line means. |
