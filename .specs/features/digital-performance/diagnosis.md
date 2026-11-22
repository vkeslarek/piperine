# Digital Performance - Measured Diagnosis

Measured 2026-11-22 on `origin/main` 754784a1 (the hidden known flag in the generated digital code) and,
for comparison, on `origin/feature/explicit-unknowns` 2cd962bd (the flag removed, T31 to T33; B7 still in
progress there). Digital-only designs, no analog unknown anywhere, compiled backend (`rust`), harness in
release (`fat` LTO; the generated design library is built by `rustc -C opt-level=2` either way). Machine:
AMD Ryzen 5 4600H, 12 threads, `clocksource = hpet`, shared with other build lanes (load 10 to 18 for the
whole session): wall times move by up to 2x between runs, so every figure below is the **best** of two to
four runs of the same case (see §7 for how to reproduce). No solver code was changed.

## 1. Verdict

**Piperine's digital engine is 175x (a 64-bit counter) to 560 000x (10 000 flops) slower per clock cycle
than Verilator on the same logic, and it does not scale: the cost per flop grows with the number of flops,
and the memory of a digital-only run grows with its length until it fails.** The generated code is 1 to 8 %
of the time. The rest is the engine's per-call, per-commit and per-microstep machinery, built for exactness
and rewindability of an event-driven mixed-signal instant, never for throughput.

Recommendation, argued in §6: **act now on a small `digital-performance` feature limited to the
scalability defects and the per-call overhead (levers L1 to L3 of §5)**, which are local to
`piperine-digital-solver`, `piperine-abi` and the owner's digital-only path, change no semantics and
promise 10x to 100x+ on the cases measured, and **defer the Verilator-style compiled evaluation of
synchronous domains (L4 to L6) to a later, designed feature**, after `explicit-unknowns` has landed
(it changes the generated code's shape) and after L1 to L3 have shown what the event engine costs when
it is done well.

Four findings, in order of severity:

1. **A digital-only run keeps everything until its end** (§3). With no analog unknown the owner's
   transient takes one step from 0 to the stop time (`protocol/transient.rs:312`, `None => stop_tick`):
   `Digital::advance` runs every instant of the run under the one checkpoint of that step, so the store's
   and the queue's journals (`store.rs:183`, `queue.rs` `Undo::Taken` clones each due list) and every
   observation (`Obs`, a `String` net name and a `Vec<u8>` per change, a `String` instance path per
   commit) are held until the stop, then delivered at once. Measured: 3.7 KiB per cycle for a counter,
   about 1.2 KiB per flop update for 100 flops; 3 M cycles of the counter under an 8 GB cap abort with
   `memory allocation of 1073741824 bytes failed`; an uncapped 20 M-cycle run reached 20 GB in three
   minutes and had to be killed. A host that streams records receives nothing until the run ends. This
   alone rules out long digital runs, whatever the speed.
2. **The reaction path is quadratic in the reactions that fire together** (§2.2, §4): `flops`
   costs 4.6 us per flop update at 100 flops, 14 us at 1000 and 195 us at 10 000 (one clock cycle of
   10 000 flops: 1.9 s). Causes, all in `piperine-digital-solver`: the `HORIZON` fixed point of
   `run_reactions` (`run.rs:699`) calls `earliest_other` (`run.rs:640`) for every reaction against every
   other's commits on every pass (O(J^2) per pass), and re-calls each reaction once more with the capped
   horizon whenever another reaction of the microstep commits before its horizon, which happens at every
   clock edge (the oscillator's next edge and every register's next-microstep commit cap each other), so
   each register's reaction is called more than once per edge, whether or not it reads `HORIZON`; `Queue::push` (`queue.rs:99`) walks
   every pending commit of every due instant to apply transport (C3) on each push, O(pending) per
   commit; `check_canonical` looks a net's layout up with `DigitalPlan::net_layout` (`plan.rs:489`), a
   linear search over every instance's pins, once per applied net.
3. **Every device call costs about 1 to 2 us at best (up to 75 us where finding 2 bites), of which the generated code is a few percent** (§2.1, §4):
   per discrete-function call the solver side computes the cell lengths with `cell_lens` (a walk over
   the shape building a `Vec`) twice, `Handle::eval` twice more, the thunk twice more: six shape walks
   and six `Vec`s for one call; plus a 16-byte-aligned `Vec` per input and per output cell, a
   `Vec<&[u8]>` of each, a seed and a jac `Vec` (empty), a `Vec` of pointers each way, a `Mutex` lock, a
   `catch_unwind`, a `to_vec` of each written value and a `Vec<Out>`. A reaction call adds a clone of
   the instance's pin table, a `Vec<String>` of every pin name, a `Vec<Statement>`, a `Planned` with a
   `Vec<u8>` per commit, a trampoline per commit, then an `Observation::Commit` with a cloned instance
   path per commit and a `Pending` with its own `Vec<u8>` in the queue. 36 to 47 % of all samples end in
   `malloc`/`free`.
4. **Per microstep, the engine walks the whole design** (§2.1): `run_functions` steps through the
   entire levelized order checking a dirty flag per function (`run.rs` `walk`), `fired_reactions`
   (`run.rs:459`) scans every reaction of every instance, and `microsteps` (`instant.rs:166`) scans every
   net for fired and boundary changes. This is cheap per item but it is O(design) per microstep, paid
   even when one net moved; with event-driven calls this costly it is hidden, after L2 it would show.

## 2. Numbers

### 2.1 Throughput

Each case is MSL generated by `crates/piperine-api/tests/digital_bench.rs` on a 100 MHz `Osc` (two
instants per cycle). "Calls/cycle" counts the device calls of one cycle from the design: every `on
<event>` head is a discrete function of its instance (it computes the event's bit) evaluated at both
edges, so a register clocked by `clk.rise()` costs two function calls and one reaction call per cycle; a
definition (`y = ...`) is one function call when its input moved. "ns/call" is the wall of a cycle divided
by its calls (reactions and functions together). Peak memory is the whole process's (`VmHWM`) at the end
of that case's run, and grows with the cycles run (finding 1). Verilator: §4.

| case | calls/cycle (react + fn) | main: cycles/s | main: ns/cycle | main: ns/call | explicit: cycles/s | explicit: ns/cycle | explicit: ns/call | peak MiB (main) | cycles run | Verilator ns/cycle | main / Verilator |
|---|---|---|---|---|---|---|---|---|---|---|---|
| counter64 | 3 + 4 | 95,202 | 10,504 | 1,501 | 94,331 | 10,601 | 1,514 | 743 | 200,000 | 59.9 | 175x |
| fsm | 3 + 4 | 78,082 | 12,807 | 1,830 | 74,438 | 13,434 | 1,919 | 838 | 200,000 | 47.8 | 268x |
| pipeline10 | 13 + 34 | 15,927 | 62,786 | 1,336 | 16,918 | 59,107 | 1,258 | 953 | 50,000 | 42.0 | 1,495x |
| pipeline100 | 103 + 304 | 1,818 | 550,003 | 1,351 | 1,795 | 557,000 | 1,369 | 795 | 5,000 | 76.3 | 7,208x |
| pipeline1000 | 1003 + 3004 | 73 | 13,730,296 | 3,427 | 69 | 14,526,185 | 3,625 | 784 | 500 | 890.1 | 15,426x |
| flops100 | 103 + 204 | 2,176 | 459,466 | 1,497 | 2,035 | 491,307 | 1,600 | 2,357 | 20,000 | 94.5 | 4,862x |
| flops1000 | 1003 + 2004 | 72 | 13,962,779 | 4,643 | 64 | 15,690,053 | 5,218 | 1,163 | 1,000 | 630.1 | 22,160x |
| flops10000 | 10003 + 20004 | 1 | 1,946,752,146 | 64,877 | 0 | 2,253,259,248 | 75,091 | 311 | 20 | 3,460.0 | 562,645x |
| chain10 | 3 + 14 | 47,619 | 21,000 | 1,235 | 48,950 | 20,429 | 1,202 | 382 | 50,000 | 66.8 | 314x |
| chain100 | 3 + 104 | 9,706 | 103,031 | 963 | 9,469 | 105,613 | 987 | 219 | 5,000 | 201.9 | 510x |
| chain1000 | 3 + 1004 | 1,116 | 896,327 | 890 | 1,098 | 910,592 | 904 | 206 | 500 | 1,858.3 | 482x |
| dpwm | 5 + 8 | 28,280 | 35,361 | 2,720 | 29,143 | 34,314 | 2,640 | 1,741 | 200,000 | 68.8 | 514x |

Reading the table:

- **Per call** the engine costs 0.9 to 1.8 us in the cases without many reactions at one edge (counter,
  FSM, chains, small pipelines), 2.7 us in the DPWM (seven to nine commits per edge); at this CPU's 3 to
  4 GHz that is 3 000 to 7 000 machine cycles per call. Verilator spends 0.3 to
  2 ns per flop or per combinational stage of these designs.
- **Scaling, functions**: the chain is linear: its marginal cost is about 0.9 us per definition
  (`chain100` to `chain1000`). Scaling, reactions: the flops are not (finding 2): the per-flop cost
  grows 3x from 100 to 1 000 flops and 14x from 1 000 to 10 000; the pipeline's per-stage cost grows
  2.5x from 100 to 1 000 stages (its stages are registers).
- **Fixed cost per cycle**: about 10 us for the counter, i.e. two instants of seven calls. Verilator's
  fixed cost (its `eval()` scheduling loop, called twice per cycle by the testbench) is 40 to 60 ns.
- **DPWM** (`examples/buck/src/dpwm.msl` verbatim, the `Never` monitor included, 100 MHz): 35 us per
  cycle, 28 000 cycles per second, against 69 ns per cycle in Verilator (500x). The buck's DPWM runs at
  200 MHz: a millisecond of the buck's digital side alone is 200 000 cycles, 7 s of engine time even
  with no analog step at all.
- **Explicit unknowns** (`feature/explicit-unknowns`): within 6 % of `main` on every case but the
  flops (11 to 15 %, from one or two runs each, inside this machine's noise). The hidden known flag costs a few generated instructions per value; the generated code is 1 to
  8 % of the time (§3), so removing the flag cannot move the engine's figures, and does not.
- **Compile time** (not part of the run): 0.1 to 1 s for the small designs once cached, 6 to 13 s for
  10 000 flops; one cold compile of the 10 000-flop design under the heaviest load of the session took
  419 s (not reproduced, not investigated).

### 2.2 Scaling of the reaction path

| flops | ns per flop update (main, best) | growth per 10x flops |
| --- | --- | --- |
| 100 | 4 600 | - |
| 1 000 | 14 000 | 3.0x |
| 10 000 | 195 000 | 13.9x |

The ratio to Verilator per cycle: 100 flops 4 900x, 1 000 flops 22 000x, 10 000 flops 560 000x.

## 3. Profile

`perf` is refused (`perf_event_paranoid = 3`). The profile is gdb stack sampling: the bench runs under
`gdb` (symbols loaded before it starts, `gdb-add-index` on a copy of the binary), a shell loop sends
`SIGUSR1` every 0.1 s, gdb stops on it without passing it on, prints every thread's stack and continues
(`profile/prof.sh`). 805 samples over four cases on `main` (about +-3 points per row). Each sample goes to
the category of its innermost frame that matches one (`profile/split.py`), so a `malloc` reached from
`Queue::take_due` counts as queue; the last row gives the share whose leaf is the allocator, across rows.

| Where (by innermost owner) | counter64 (218) | chain1000 (210) | dpwm (157) | flops1000 (220) |
| --- | --- | --- | --- | --- |
| Generated code (`design::*`, its runtime `rt`) | 2.8 % | 8.1 % | 7.6 % | 0.9 % |
| Per-call marshalling, solver side (`call_function`, `call_reaction`: buffers, pin clones, name `Vec`s) | 23.9 % | 19.5 % | 16.6 % | 9.5 % |
| ABI frame (`Handle::eval`/`react`, thunks, `Mutex`, `catch_unwind`, commit trampolines) | 21.1 % | 21.0 % | 15.9 % | 6.4 % |
| Shape walks per call (`cell_lens`, `validate`) | 10.1 % | 16.7 % | 14.6 % | 7.7 % |
| Scheduler and queue (`Queue::push`/`take_due`, `fired_reactions`, `HORIZON` fixed point, dag walk) | 11.5 % + 4.6 % `run_reactions` | 1.0 % | 7.6 % + 1.9 % | 55.9 % + 2.3 % |
| Commits applied to the store (`apply_due`, `write_net`, `check_canonical`/`net_layout`) | 6.4 % | 8.6 % | 10.2 % | 11.4 % |
| Observations and their delivery (`Obs`, `Delivery::collect`/`flush`, adapter `drain`) | 11.9 % | 21.4 % | 12.7 % | 2.3 % |
| Journal (marks, rewinds, releases of store and queue) | 1.8 % | 3.3 % | 9.6 % | 1.4 % |
| Instant loop and protocol (`microsteps` net scans, `BTreeSet`s, `advance`) | 6.0 % | 0.5 % | 3.2 % | 2.3 % |
| **Leaf in `malloc`/`free`/`realloc`** (spread over the rows above) | **42.7 %** | **35.7 %** | **47.1 %** | **15.9 %** |

In flops1000, `earliest_other` alone is 40.9 % inclusive and `Queue::push` 14.5 % (`Pending::removed_by`
4.5 %).

The time split asked for, folded into five:

| | counter64 | chain1000 | dpwm | flops1000 |
| --- | --- | --- | --- | --- |
| Generated code | 3 % | 8 % | 8 % | 1 % |
| Call protocol (marshalling + ABI frame + shape walks) | 55 % | 57 % | 47 % | 24 % |
| Scheduler and queue | 16 % | 1 % | 10 % | 58 % |
| Commits, observations and records, journal | 20 % | 33 % | 33 % | 15 % |
| Instant loop and owner protocol | 6 % | 1 % | 3 % | 2 % |
| (of all of it, allocation) | 43 % | 36 % | 47 % | 16 % |

## 4. Against Verilator

Verilator 5.043 is installed (oss-cad-suite), so the same designs were built in Verilog and timed directly
(`verilator/gen.py`, `verilator/run.sh`; nothing was installed): `--cc --exe --build -O3 --x-assign fast
--x-initial fast --no-assert`, C++ at `-O2`, single-threaded (no `--threads`), best of three runs of about
a second each. Every register reaches an output, so nothing is optimized away; the 10 000 flops are a
generate loop of 1-bit module instances, not one vector.

What the comparison is and is not:

- **Two-state against four-valued.** Verilator is two-state (`--x-assign fast`); `main`'s values carry
  a known flag. The explicit-unknowns column removes that difference in the generated code and changes
  nothing measurable (§2.1): the gap is not the value model.
- **Clock.** Ours is an `Osc` module in simulated time: each edge is a commit through the event queue,
  an instant with its microsteps, a reaction and its event function. Verilator's testbench toggles `clk`
  in a C++ loop and calls `eval()` twice per cycle: cycle-based, no time wheel, no delay. A Verilog
  `always #5 clk = ~clk` under `--timing` would add Verilator's own timing machinery; it was not
  measured. The clock is 3 of the counter's 7 calls per cycle, so it is part of our fixed cost, not of
  the scaling.
- **Single thread on both sides.**
- **Semantics we keep and Verilator does not**: rewindable instants (the owner protocol's checkpoints),
  observations of every change, transport (C3) per statement, `HORIZON`, `MERGE` commits under unknown
  control, records. Part of that has a real cost; §5 separates what does not need to cost what it does.

Per element, Verilator spends about 0.35 ns per 1-bit flop (`flops10000`), 0.85 ns per registered
32-bit stage with its definition (`pipeline1000`), 1.8 ns per 32-bit combinational stage (`chain1000`),
on a fixed 40 to 60 ns per cycle. These are measured here, on this machine, so no published figure is
used as the reference.

The ratio per cycle is in the table of §2.1: 175x to 270x for a few calls per cycle (counter, FSM), 300x
to 500x for the DPWM and the chains, and 1 500x to 560 000x for the register-heavy cases, growing with
size.

### How far the engine is from Verilator's model

| Verilator | Piperine today |
| --- | --- |
| One static schedule per trigger, computed at compile time | A levelized order of functions (`dag.rs`) walked dynamically per microstep with a dirty flag per function; reactions found per microstep by scanning every instance |
| Whole design fused into a few C++ functions over a flat struct of native integers | One `extern "C"` call per function and per reaction, each through a shape-described frame, byte buffers built per call |
| No per-event bookkeeping: a nonblocking assignment is a store to a shadow variable | Every commit is a heap `Pending` in a `BTreeMap` queue with transport (C3) applied by scanning, observed with `String`s, journaled for rewind |
| Change detection only on triggers (clock edges) | Change tracking on every net, `PREVIOUS` kept, readers dirtied per write |
| Evaluation of comb logic until settled, settled by construction (levelized) | Same idea (levelized, cycles through `RELAX`), but with per-call overhead three orders above the logic's own cost |

The levelized order is there; what separates the two is that our order is interpreted per microstep over
generic, rewindable, observed byte-net machinery, and Verilator's is compiled into straight-line code.
"Ordena em topo order e reavalia até convergir" is what `dag.rs` + `walk` already do; the cache benefit the
user expects needs the second half, the fusion.

## 5. Levers, ranked by expected gain

Estimated, not measured (no solver code changed): each row says what removes which measured share.

| # | Lever | What it removes | Expected gain | Size |
| --- | --- | --- | --- | --- |
| L1 | **Bound a digital-only run's memory**: step a transient with no unknown in bounded windows (an advance to `min(stop, now + window)`, or release the journal between instants when nothing can retake them: with no analog side no instant is ever rewound), and deliver observations as instants finish | Finding 1: the O(run) memory and the end-of-run delivery (6 to 21 % of the time is `Delivery::flush`/`collect` at the end) | Correctness of long runs (from "fails at a few million cycles" to constant memory); 5 to 20 % of time | S |
| L2 | **Remove the quadratic paths**: index pending commits by `(writer, statement)` for C3 instead of scanning; precompute each net's layout at bind; compute the `HORIZON` caps in one pass (sort the others' earliest dues once) and re-call only reactions whose shape reads `HORIZON`; name observations by `NetId`, not by a name scan (`adapters.rs:273` scans every net name per fired event) | Finding 2: 58 % of flops1000, all the super-linear growth | 3x at 1 000 flops, 20x to 60x at 10 000, and the per-flop cost flat in size | S to M |
| L3 | **An allocation-free call path**: cell lengths and offsets computed once at bind (no `cell_lens` per call, no per-call validation of a shape that bind already validated); per-instance input/output buffers reused, or pointers straight into the store (it is already 16-byte aligned); no pin-table clone, no pin-name `Vec<String>`; commits written into a reused arena; observations only for nets someone watches (probes, records, the analog boundary), by id; the `Mutex` and `catch_unwind` per call reconsidered (one guard per instant) | Finding 3: the 47 to 57 % call protocol and most of the 36 to 47 % allocation | 5x to 10x on every case (from 1 to 2.7 us per call to a few hundred ns) | M |
| L4 | **Per-microstep work proportional to activity**: a worklist of dirty functions in level order instead of walking the whole order; reactions found from the fired event nets' readers instead of scanning every instance; a changed-net list instead of a scan of every net | Finding 4 (small today, dominant after L2 and L3 for large sparse designs) | 1x today; up to 10x for large designs with little activity per edge | S |
| L5 | **Static scheduling of synchronous domains + fusion (the Verilator model)**: at elaboration, recognize the reactions of one clock edge that only read registered state and commit at the next microstep with no delay; compile the domain into one generated function over a flat state struct (two-phase: compute every next value, then commit), followed by its levelized definitions as straight-line code; the engine calls it once per edge and sees one device with one commit list (or none, when nothing outside watches) | The per-call cost itself, the queue for intra-domain commits, the per-net observation | 20x to 100x beyond L1 to L4 for register-heavy designs: the remaining gap to Verilator | L (IR, transpile, ABI: a domain kernel; exactness against the event engine as oracle) |
| L6 | **Change tracking vs full re-evaluation per domain**: once L5 exists, a domain whose state mostly moves every cycle (datapaths, pipelines) re-evaluates in full; a sparse one (FSMs, control) keeps event-driven tracking. A per-domain choice, measured | Wasted re-evaluation or wasted tracking | 1x to 3x per domain | M, after L5 |
| L7 | **Per-instant parallel partitions**: the functions of one level and the reactions of one microstep are independent by construction (R2: one snapshot) and could run in parallel | Wall per instant on many cores | Negative today: a call costs 2 us and an instant has a handful of calls, so thread handoff (1 to 10 us) is larger than the work. Worth it only after L5, on domains of thousands of flops, with coarse partitions (Verilator's `--threads` needs large designs to gain too) | L, after L5 |

Change tracking vs full re-eval today: our engine already does change tracking (a function runs only when
a read moved); its cost is per call, not per check, so tracking is not what is slow. Full re-evaluation
becomes the better choice only when a call costs a few nanoseconds, i.e. after L5.

## 6. Act now or later

**Now: L1, L2 (and L4 with them, it is small), then L3**, as one `digital-performance` feature. Reasons:

- L1 is a defect, not an optimization: a long digital-only run cannot complete, and the user's stated
  target is big circuits that are mostly digital. Every other lever is moot while memory grows with time.
- L2 is a defect for the same target: 10 000 flops on one clock cost 2 s per cycle today, and the
  growth is super-linear, so "big" is exactly where it breaks.
- L1 to L4 stay inside `piperine-digital-solver`, `piperine-abi`'s caller/thunk and the owner's
  digital-only stepping; they change no semantics and no generated code, and the existing digital tests
  plus the buck are their oracle. They also help the mixed buck: its instant run at every accepted step
  was 11 % of a transient step (`transient-performance` diagnosis §3), all of it this machinery.
- Expected outcome: 10x to 100x on the measured cases, flat per-flop cost, constant memory. That still
  leaves us roughly 20x to 100x behind Verilator on register-heavy logic, which is the honest boundary of an
  event-driven engine with a call per process.

**Later: L5 and L6 (the Verilator model), then L7.** Reasons:

- L5 is a design feature: it needs the elaborator or the plan to identify synchronous domains, a new
  kind of generated kernel, and an ABI contract for it, plus the event engine kept as its exactness
  oracle (records, `HORIZON`, transport and unknown merges must give the same observations).
- `explicit-unknowns` is still in flight and changes the generated digital code; fusion written against
  `main` would be written twice.
- Its gain can only be judged once L3 has shown what an event-driven call costs when done well: if a
  call drops to ~100 ns, a 1 000-flop domain is 100 us per cycle, and whether that is enough depends on
  the sizes the user actually simulates, which this diagnosis does not know.
- L7 is last: parallelism multiplies whatever per-instant cost exists; today that cost is overhead.

## 7. Reproducing

```sh
# Piperine (release; one case: DIGITAL_BENCH=<prefix>; cycles: DIGITAL_BENCH_CYCLES=<n>)
cargo test --release -p piperine-api --test digital_bench -- --nocapture
# Verilator twins (needs verilator and a C++ compiler on PATH; installs nothing)
.specs/features/digital-performance/verilator/run.sh /var/tmp/<work>
# Profile one case (gdb; ~220 samples at 0.1 s)
cp target/release/deps/digital_bench-<hash> /tmp/b && gdb-add-index /tmp/b
DIGITAL_BENCH=counter DIGITAL_BENCH_CYCLES=2000000 .specs/features/digital-performance/profile/prof.sh 220 raw.txt /tmp/b --nocapture
.specs/features/digital-performance/profile/fold.sh raw.txt > fold.txt
python3 .specs/features/digital-performance/profile/split.py fold.txt
.specs/features/digital-performance/profile/incl.sh fold.txt | head -60
```

Cap the memory (`ulimit -v`) of any long run: finding 1 makes it grow by kilobytes per cycle.
The explicit-unknowns column came from a detached worktree of `origin/feature/explicit-unknowns` at
2cd962bd with the same bench file, the FSM's `phase.known_or(Phase::Idle)` and `ticks.known_or(0)`
written `phase` and `ticks` (the explicit model has no `known_or` there); the worktree was removed after.
