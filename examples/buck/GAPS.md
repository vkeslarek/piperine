# Gaps found while writing the buck converter

What the design wanted and the language or its front end could not say directly. Each entry
names the construct wanted, the spec section that needs it, and what the model does instead.
None of these is hidden: the model says the closest legal thing, and the comments point here
where it matters.

## G1. A fixed-point number type

- **Wanted:** a Q-format type such as `Fix<i32, 4>` whose arithmetic aligns binary points and
  whose `>>` back to an integer is a typed conversion, as VHDL's `sfixed` or SystemVerilog
  fixed-point libraries give.
- **Needed by:** SPEC §6.3 (the Q4 PID: gains, the three terms, the duty word).
- **Instead:** plain `i32` carriers and constants with the scale in their names (`KP_Q4`), the
  shift as a named constant (`Q_SHIFT`), and the scaling documented in SPEC §6.3. The language's
  value types are a closed list (spec Part I §1) and have no fixed-point kind; a `struct` with
  operator methods would not help, since an `impl` of `Add` is not in the inherited library
  (Appendix H).

## G2. A ranged integer type

- **Wanted:** an integer constrained to a range, such as VHDL's `integer range 0 to 168`, for
  the setpoint code.
- **Needed by:** SPEC §7.1 (codes 0 to 168; a larger write saturates).
- **Instead:** `Bit<u8>` and an explicit `d.min(SETPOINT_MAX)` at the one place the register is
  written. Nothing checks the range at the other readers.

## G3. Appendix H methods on a carrier receiver stay unresolved (closed: now reported)

- **Wanted:** `oc_count.saturating_add(1)` on a `Bit<u4>` carrier, inside a reaction. Spec Part I
  §1 rule 1 and Appendix H H.1 make `saturating_add` a method of every integer value, and a
  reaction reads a carrier as its value.
- **Needed by:** SPEC §8.2, §8.4 and §8.6 (the consecutive-sample counters of the supervisor).
- **What happened:** the POM resolver left such a call `Unresolved` with no diagnostic.
- **Closed by `msl-digital` (2026-11-16) as a reported gap, not a silent one:** the type checker
  types every method call through its catalogue, and a method with no row for its receiver's
  type is refused, `P0001` naming the method and the type (`saturating_add` on `u4`). Nothing is
  left unresolved without a diagnostic. `saturating_add` itself has no row yet.
- **Instead:** the counters stay written as compare-and-increment
  (`if c < N { c + 1 } else { N }`), which reads as clearly.

## G4. The meaning of a float-to-narrow-integer `as`

- **Wanted:** a statement of what `x as u10` does for an `f64` outside 0 to 1023. Spec Part I
  §1 rule 1 says "`as` truncates or extends, as in Rust"; Rust saturates float-to-integer casts
  to the target type, but `u10` is not a Rust type, and "truncates" could also be read as
  wrapping modulo 2^10.
- **Needed by:** SPEC §4.1 (the ADC code).
- **Instead:** the ADC clamps to 0.0 to 1023.0 before the cast, so the result is the same under
  either reading.
- **Provisional rule (ir-rust, 2026-11-15):** the transpiler saturates to the N-bit range and
  maps NaN to 0, as Rust does for its own integer types. This is a default chosen to unblock
  `ir-rust`, not spec text: Part I §1 rule 1 still needs a sentence that fixes it.

## G5. A shift by N or more bits

- **Wanted:** a statement of what `x << s` and `x >> s` do on a `UInt<N>`/`Int<N>` when
  `s >= N`. Rust panics in debug and masks the shift amount in release; neither is acceptable
  for a language whose arithmetic wraps the same way in every build (Part I §1 rules 5 and 6).
- **Needed by:** SPEC §6.3 (the Q4 shifts of the PID) only in principle; the model's shift
  amounts are constants below the width.
- **Provisional rule (ir-rust, 2026-11-15):** the mathematical result truncated to N bits, so
  `<<` by N or more gives 0, `>>` on an unsigned value gives 0, and `>>` on a signed value gives
  the sign fill. Spec text still needed.

