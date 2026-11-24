# ESP32 simulator on Renode — idea

Status: **idea, much later** (user, 2026-11-24: "Escreve uma nova feature com o Renode aí"). No spec
yet. Depends on `device-plugins` (`.specs/features/device-plugins/idea.md`); shares the sync design of
`arduino-simulator` (`.specs/features/arduino-simulator/discussion.md`).

## Goal

Run a real ESP32 firmware (ESP-IDF / Arduino-ESP32 / Zephyr build) co-simulated with a Piperine
circuit: the MCU's GPIOs, PWM (LEDC), ADC and DAC meet the circuit through the device ABI, the
circuit's voltages reach its ADC and inputs, and the user sees UART output and waveforms together.

## Why Renode (and not our own core)

- An ESP32 is too large to write in-house (dual core, Xtensa LX6/LX7 or RISC-V for the C series,
  caches, FreeRTOS, many peripherals, radio). Unlike the AVR, the realistic path is a mature
  emulator behind a plugin.
- **Renode** (Antmicro) is built for co-simulation: its own virtual time advanced in quanta,
  peripherals as models with read/write callbacks, address hooks, and an external co-simulation API
  (used with Verilator). Its licence is understood to be MIT — **to confirm**.
- Alternative kept for comparison: Espressif's QEMU fork (more complete ESP32 peripherals, official,
  integrated with `idf.py`; GPL, deterministic time only with `-icount`, GPIO/ADC models to modify).

## What to verify first (open, from web search 2026-11-24)

- Which ESP32 variants Renode simulates today: Xtensa support landed in Renode 1.13; a tlib proposal
  added ESP32 / ESP32-S2 / ESP32-S3 configurations; ESP32-C3 (RISC-V) is unconfirmed.
- How complete its ESP32 peripheral models are (GPIO matrix, LEDC, ADC, DAC, timers, UART, RMT,
  I2C/SPI) — gaps may need our own Renode peripheral models (C#), shipped with the plugin.
- Renode's licence and the licence of any ESP32 platform files.

## Synchronisation (same scheme as arduino-simulator)

- **MCU → circuit:** a GPIO/LEDC/DAC register write is a callback at its virtual instant → a commit
  at that exact time; the emulator does not stop.
- **Circuit → MCU, pull on read:** a read of the GPIO input register, an ADC conversion or a
  comparator status runs a callback; the plugin pauses Renode there, Piperine's solver catches up to
  that virtual instant, and the value comes back with a **validity window** (until when it surely
  cannot change); reads inside the window use the cached value without a sync.
- **Input-driven interrupts** (GPIO interrupt, touch, ADC thresholds): Piperine injects them at the
  crossing instant; while one is armed the emulator runs at most to the end of that input's window.
  Renode's state save/load exists but is costly, so prefer conservative lookahead over rollback.
- **Quanta:** Renode advances time in quanta; the plugin sets them from Piperine's horizon so a
  quantum never crosses a known Piperine event.

## Gaps on the Piperine side

Everything `device-plugins` lists (MSL `extern "cosim"` → library loading, text/bytes params for
the firmware image, `HORIZON` run-ahead, devices without snapshots, sink filtering), plus: a plugin
that drives an external process or runtime (Renode is .NET) — in-process hosting vs a child process
with a socket, and its cost per sync.

## Questions for the user (when it comes back)

1. First target: ESP32 (Xtensa) or ESP32-C3 (RISC-V)?
2. Peripherals in the first cut: GPIO, LEDC PWM, ADC, DAC, UART — and later I2C/SPI, RMT, touch?
3. Firmware input: an ELF/bin built by ESP-IDF or arduino-cli; do we accept only that?
4. Renode first, or keep QEMU as the first target if Renode's ESP32 models turn out too partial?
5. Electrical fidelity of a pin (ideal logic vs an output stage with resistance, pull-ups, 3.3 V).
