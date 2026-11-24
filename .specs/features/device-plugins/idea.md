# Device plugins — idea

Status: **idea, much later** (user, 2026-11-24: "coloca uma feature para criar o esquema de plugins,
também para bem depois"). No spec yet. `arduino-simulator` depends on it.

## Goal

Let a device that is not written in MSL — an MCU emulator, a co-simulated block, a compiled model
from another tool — be distributed and installed as a **plugin**: a separately built library that
implements the device ABI, lives in its own repository and package, and is placed in an MSL design
like any module.

## Why a plugin boundary

- **Licences.** Piperine is MIT. A plugin built on GPL code (e.g. simavr) can itself be GPL-3.0
  when it ships separately: Piperine stays MIT; whoever distributes both together distributes the
  combination under the GPL, which MIT allows; using it obliges nothing. (Not legal advice.)
- **Reach.** Emulators and foreign models (AVR, ESP32 via QEMU, RISC-V, FPGA, OSDI-like models)
  plug in without touching the core.
- **One contract.** A third-party plugin and an official one (e.g. our own MIT AVR core) follow the
  same ABI, so a model swaps one for the other without changing.

## What exists already

- The device ABI (`.specs/features/device-abi/abi.md`, crate `piperine-abi`): a `cdylib` exporting
  `piperine_abi_1`; `piperine-transpile/src/load.rs` opens such a library (tied today to the
  transpiler's cache and manifest).
- MSL `extern "cosim" module` and `#[link(...)]` parse and reflect (Part III §8.5), but nothing
  lowers or loads them.

## Gaps to close (from the arduino-simulator proof of concept, 2026-11-24)

1. MSL `extern "cosim"` / `#[link]` → find, load and bind an external ABI library through the
   normal `Session` / CLI path (today only `CircuitBuilder::device` injects a device).
2. Packaging: how a plugin is declared, fetched, versioned, trusted and found (manifest, the
   project's dependency resolver, release assets per platform).
3. A text / bytes param (e.g. a firmware path or image); `ParamValue` lacks it.
4. `HORIZON` must be the end of the tentative step (abi.md R5) even when a `cross` reaches the
   device, so an emulator can run ahead (today it is clamped to the instant → polling).
5. Devices whose state lives outside the ABI's private pins (no snapshot) need a clear contract:
   `IRREVERSIBLE`, non-rewindable sessions, or a snapshot hook.
6. The host's sink should filter a device's internal events (e.g. `wake`) from real changes, and
   carry a device's own event time where it differs from the accepted instant.
7. Exported stock devices for tests and examples (R, C, diode with limiting, sources), today only in
   test helpers.
8. Relation to the existing `piperine-plugin` crate (native dlopen + embedded Python, manifest,
   TOFU trust, permissions): reuse its loading/trust/packaging or define the device-plugin path
   beside it — to be decided.

## Depends on / blocks

- Builds on the device ABI and the host (`Session`).
- **Blocks:** `arduino-simulator` (`.specs/features/arduino-simulator/discussion.md`) and
  `esp32-simulator` (`.specs/features/esp32-simulator/idea.md`).
