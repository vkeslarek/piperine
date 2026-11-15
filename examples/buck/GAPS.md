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

## G3. Appendix H methods on a carrier receiver stay unresolved

- **Wanted:** `oc_count.saturating_add(1)` on a `Bit<u4>` carrier, inside a reaction. Spec Part I
  §1 rule 1 and Appendix H H.1 make `saturating_add` a method of every integer value, and a
  reaction reads a carrier as its value.
- **Needed by:** SPEC §8.2, §8.4 and §8.6 (the consecutive-sample counters of the supervisor).
- **What happens:** the POM resolver (`crates/piperine-lang/src/pom/resolve/members.rs`) links
  a method on a carrier receiver to the carrier built-ins, the `impl`s of its nature and of a
  declared value type, and blanket trait impls; an inherited primitive method of the value is
  none of these, so the call is left `Unresolved`, **with no diagnostic**. It is a front-end
  gap, not a language gap: a type checker would link it.
- **Instead:** the counters are written as compare-and-increment
  (`if c < N { c + 1 } else { N }`), which resolves and reads as clearly. The test target
  `buck_example.rs` asserts that no member of the package stays unresolved, so a regression of
  this kind is caught rather than hidden.

## G4. The meaning of a float-to-narrow-integer `as`

- **Wanted:** a statement of what `x as u10` does for an `f64` outside 0 to 1023. Spec Part I
  §1 rule 1 says "`as` truncates or extends, as in Rust"; Rust saturates float-to-integer casts
  to the target type, but `u10` is not a Rust type, and "truncates" could also be read as
  wrapping modulo 2^10.
- **Needed by:** SPEC §4.1 (the ADC code).
- **Instead:** the ADC clamps to 0.0 to 1023.0 before the cast, so the result is the same under
  either reading.
