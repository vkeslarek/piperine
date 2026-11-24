# Arduino simulator — discussion before planning

Status: **postponed, much later** (user, 2026-11-24: "coloca lá para depois, bem depois"). Open for discussion when it comes back. No spec yet. When the questions
below are settled, Fable writes `spec.md`, `design.md` and `tasks.md` from this file.

## Goal

Run a real Arduino firmware (an AVR program compiled by `avr-gcc` / `arduino-cli`) inside a Piperine
circuit: the MCU's pins drive and read the circuit, the circuit's voltages reach its ADC, and the
user sees `Serial` output and the waveforms together.

## What already exists

- The device ABI was designed with this case in mind: `.specs/features/device-abi/abi.md` §11.7
  "An AVR emulator: an external device with its own time base". It sketches the pins (`pad_in`,
  `pad_out`, `adc0`, `reset`), the firmware as a structural param, a `cpu` state pin, the `wake`
  event that advances the core to a time, pad writes at exact times (`LATER` commits), interrupts
  from pad changes, and the ADC sampled from a continuous pin.
- That sketch assumes an in-house `Cpu` type. No emulator library was ever chosen in the repo.

## The emulator: options

| Option | What it is | For | Against |
| --- | --- | --- | --- |
| A. In-house Rust AVR core | Write the ATmega328P core (instruction set, timers, ADC, USART, pin change / external interrupts) as a Rust crate | Fits the ABI exactly (state pin = plain bytes, own time base, no `unsafe`, no deps — the `piperine-numbers` rules); licence-clean; cycle timing under our control | The most work (~130 instructions + peripherals) |
| B. Wrap simavr | Mature C AVR simulator (many cores, peripherals, GDB) | Fast to a first result; well tested | GPL-3.0 (would contaminate distribution); C dependency + FFI; its own event loop must be inverted to fit "advance to a time" |
| C. Port avr8js | Wokwi's TypeScript AVR core (MIT) | Clean licence, compact, readable; good reference | A port is still a rewrite; JS → Rust |

Coordinator's recommendation: **A**, using simavr and/or avr8js only as **test oracles** (run the
same firmware there, compare pin traces and cycle counts) — nothing linked.

## Decided so far (user, 2026-11-24)

- **Base: avr8js (MIT)** — copied and adapted into our own Rust core, keeping its copyright and
  licence notice on the ported files; shaped to extend to other cores.
- **Facts are free: datasheet distillation.** Per core, a fact sheet (registers, bits, pinout,
  vectors, instruction cycles, peripheral behaviour) is distilled from the Microchip datasheets /
  instruction-set manual and simavr's documentation and observable behaviour. The core is
  implemented from those fact sheets and avr8js — never from simavr's code. The distillation
  may read simavr's source, but it extracts only MCU data and MCU behaviour (register maps,
  pinouts, vectors, timings, what a peripheral does) — never its code structure, names or
  organisation (user, 2026-11-24). simavr is used only
  as a black-box test oracle (run the same firmware, compare pins and cycles); never linked,
  never committed.
- **Proof of concept done (not committed, GPL):** blink on simavr co-simulated with a Piperine
  transient through a `HostElement`; 5 edges exactly on the 500 ms grid. Gaps found: host devices
  are analog-only (the pin was a Norton source, no digital net/driver), only breakpoints to see
  the future, no stock host R/D/V, no pin inputs/ADC/UART hooks, simavr sleep uses wall time,
  operating point sees the pin before the AVR ran.

- **Our own emulator, no bridge** (user, 2026-11-24): no GPL bridge process (WebSocket/socket to a
  simavr shim) — "Vamos seguir por criar o nosso emulador... vai dar trabalho mas compensa demais".
  simavr stays a local, uncommitted black-box oracle only.

## Proof of concept result (2026-11-24, discarded after recording)

A throwaway prototype (simavr linked locally, never committed, deleted afterwards) co-simulated a
real ATmega328P firmware (`avr-gcc`, 16 MHz) with a Piperine transient for 3 s: D13 blink, D9 PWM
(Timer1) into a 10 kΩ / 1 µF RC, ADC0 sampling the RC node every 250 ms, D2 reading a 1 Hz sine
through a Schmitt input mirrored onto D7, and UART TX at 9600 baud. Two integrations:

| | Device ABI (`piperine-abi`, in-process `CircuitBuilder::device`) | `HostElement` (old host path) |
| --- | --- | --- |
| wall time for 3 s | 74.6 s | 9.9 s |
| accepted steps | 1.9 M | 192 k |
| PWM duty | 25.390 % (976.56 Hz) | 25.391 % |
| `filt` mean / ripple | 1.2695 V (= hand RC) / 97.8 mV (hand 96.7) | 1.2696 V / 96.9 mV |
| D2 crossing error vs analytic | < 0.015 µs (solver locates it) | ~µs |
| D2 → D7 delay | 4.000 µs (one wake quantum) | 0.9–1.7 µs |
| ADC vs node voltage at the sample | 0 counts 10/11, −1 once | 0–1 (−3 once) |

How the ABI run was wired: every part a hand-written `piperine_abi::device::Device` in Rust (AVR,
pin output stage as a 5 V Norton behind 25 Ω, Schmitt input via two `CROSS` watches, sine, LED, R, C
from the solver's test helpers), bound with `CircuitBuilder::device` and nets named by `Port::Net`;
the real solver ran it (analog Newton/TR-BDF2, event engine dispatching `START`, `wake` re-armed
with `REPLACE`, pad edges as timed commits, the ADC `sample` event reading the accepted continuous
solution, UART as a `RECORD` pin). No cdylib, no MSL, no Session/CLI.

Gaps the real feature must close:
1. **`HORIZON` forbids run-ahead** when an input of the emulator is reached by a `CROSS` watch
   (`plan.rs::crossing_reach` sets it to the instant itself; abi.md R5 says "end of the tentative
   step" — the implementation is stricter than the text). The emulator then polls (4 µs quantum →
   750 k wakes, 1.9 M steps). Needs run-ahead to the next visible action with exact interruption
   on an input change (and/or snapshot + rollback).
2. **No path from MSL to an external ABI library:** `extern "cosim"` parses and reflects, but the
   transpiler, plan and `Session::open` ignore it; `CircuitBuilder::device` is the only entry.
3. **Emulator state outside the shape** (behind a `Mutex`) forces `IRREVERSIBLE` and makes the
   session non-rewindable; our own core should keep its state in the private state pin (R3/F1).
4. **No text/bytes param** for the firmware path (`ParamValue` lacks it).
5. **The sink mixes internal `wake` changes with real pad changes**; observations carry the
   accepted instant, not the emulator's own edge time (UART stamps 1–2 µs late).
6. **ADC needs the emulator to expose conversion start** (the `sample` event at the start of the
   sample-and-hold, read from the accepted solution).
7. **No exported stock devices** (R/C/diode helpers are test-only; the helper diode has no
   limiting).
8. simavr quirk (not ours): a PORTB read-modify-write re-raises PB1's IRQ and glitched OC1A.

Result page: https://claude.ai/artifact/BRoyB1LUJ4kVX1k3QCeZ39

## Idea on standby: an external GPL plugin first (user, 2026-11-24)

Final discussion on standby. Piperine is MIT (GPL-compatible), so an AVR plugin built on simavr can
be GPL-3.0 if it lives in its own repository and package, never in the Piperine repo or binary:
Piperine stays MIT; whoever distributes Piperine + the plugin together distributes the combination
under the GPL (MIT allows it); using it obliges nothing. The plugin boundary already exists (the
device ABI: a `cdylib` exporting `piperine_abi_1`, `piperine-transpile/src/load.rs`; the
`piperine-plugin` crate: native dlopen, manifest, trust). Possible path: (1) external GPL plugin over
simavr through the ABI — many MCUs at once; (2) later our own MIT core (avr8js base + datasheet
distillation) as the official plugin behind the same contract; simavr becomes its test oracle;
(3) the same mechanism opens other emulators (ESP32 via QEMU, RISC-V, FPGA). Missing pieces are the
PoC gaps (MSL `extern "cosim"` → library loading, text/bytes param, `HORIZON` run-ahead, sink
filtering). Not legal advice.

## Analog → MCU without lockstep (coordinator note, 2026-11-24)

MCU → circuit is cheap: the core runs ahead to its next visible action and commits it at its exact
time. Circuit → MCU forces lockstep only if the core cannot know when an input will change. Plan:
1. Inputs reach the core only as discrete events — a pin threshold crossing (located exactly by the
   solver's `cross`), an ADC sample (requested by the core at a time it knows), a comparator flip —
   never as a continuous read.
2. **Optimistic run-ahead with rollback** (time-warp): the core snapshots its state (our own core:
   registers + SRAM + peripherals, a few KB, copied cheaply) and runs to the horizon; if an input
   event arrives at t before where it ran, it restores the snapshot ≤ t, applies the input at t,
   re-runs, and replaces its already committed future outputs (`REPLACE` commits).
3. **Skip the rollback when the input was not observed:** the core logs the instants it read each
   pin (`PINx` read, armed interrupt); if nothing read the changed pin after t, the input is applied
   without re-running.
4. **Conservative lookahead** where possible: the analog side's event prediction gives an earliest
   possible next crossing; the core may run safely up to min(next own action, that bound).
5. ABI fix needed: `HORIZON` must be the end of the tentative step (abi.md R5), not the current
   instant, when a `cross` reaches the device — the PoC's 4 µs polling came from that.

## Questions for the user

1. **Board / core scope.** Start with the Uno (ATmega328P, 16 MHz) only? Later: Mega (ATmega2560),
   ATtiny85, then other families (ESP32 is not AVR — a different core entirely).
2. **Peripherals in the first cut.** GPIO + pin-change/external interrupts, Timer0/1/2 (`millis`,
   `delay`, PWM), ADC, USART (`Serial`). Leave I2C (TWI), SPI, EEPROM, watchdog, sleep modes for
   a second phase?
3. **Firmware input.** Accept `.hex` and `.elf` built elsewhere; optionally run `arduino-cli` when
   installed to build a `.ino`? (We would not ship a toolchain.)
4. **How a model uses it.** A stdlib module, e.g. `ArduinoUno(d0..d13, a0..a5, gnd, vcc) { firmware:
   "blink.hex" }`, with digital pins as resolved nets (the MCU is one driver; a pull-up is another)
   and an analog side per pin (output stage as a source + resistance; input as a threshold /
   ADC sample)? Or only the bare `Atmega328p` with ports, and the board as a library module on top?
5. **Electrical fidelity of a pin.** Ideal logic (0/1/Z) to the digital side only, or a real output
   stage (Vcc/0 V through ~25 Ω, input leakage, pull-up ~35 kΩ) so an LED + resistor or an RC
   filter on a PWM pin behaves physically?
6. **`Serial`.** Output as a record stream the host prints (`--stream serial`), input from a host
   file/string? A "serial monitor" in the CLI?
7. **Timing.** Cycle-accurate instruction timing (each instruction's cycle count, timers in
   lock-step)? Speed target: real time (16 M cycles per simulated second) is the usual bar.
8. **Debugging.** GDB remote stub (step the firmware while the circuit runs) now or later?
9. **Queue.** Plan now, build after spec-closure (item 13) as the user asked the queue to stop
   there — or build it alongside?
