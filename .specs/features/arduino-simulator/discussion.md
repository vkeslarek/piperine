# Arduino simulator — discussion before planning

Status: **open for discussion** (user + coordinator), 2026-11-24. No spec yet. When the questions
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
