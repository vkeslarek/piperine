# Part VII. Standard library

**Terms defined here:** standard library, strength, high impedance, four-state logic, converter, monitor, inertial delay.

## 22. Standard library

The **standard library** is the crate `std`, shipped with the implementation; its source files are normative, and each `rust` fence of this part is an excerpt of one of them.

1. The prelude (§3.4.2) re-exports every `pub` item of the namespaces of `std`. No `use` is needed.
2. A standard-library item has no semantics beyond its definition. Its behavior in each analysis follows from §16.2 applied to that definition.
3. A project **MAY** replace any item of this section by declaring an item of the same name in scope. The replacement is an ordinary item, with no privilege of the item it replaces. The core names of §14 are not replaceable (§14 rule 1).
4. Where this part gives a definition, the definition is normative. Where it gives only a signature and a contract, the item is declared `extern "intrinsic"` (§3 rule 10) and the contract is normative.
5. The per-analysis tables in this part are derived from the definitions. If a table and a definition disagree, the definition governs.

**Sources.**

| Section | Namespace | File |
| --- | --- | --- |
| §1 | `std::constants` | `constants.msl` |
| §4, §11 | `std::carriers` | `carriers.msl` |
| §22.1 | `std::values` | `values.msl` |
| §22.1 | `std::logic` | `logic.msl` |
| §22.2 | `std::analog` | `analog.msl` |
| §22.3 | `std::nets` | `nets.msl` |
| §22.4 | `std::convert` | `convert.msl` |
| §22.5 | `std::monitor` | `monitor.msl` |
| §22.6 | `std::io` | `io.msl` |

**Implemented by the engine.**

| Item | Entry | Reason |
| --- | --- | --- |
| `matches_bits` | §22.1.4 | reads the characters of a `str` |
| `logic` | §22.1.4 | reads the characters of a `str` |
| `table_model` | §22.2.15 | reads the characters of a `str` |
| `parse_table` | §22.6.4 | reads the characters of a `str` |
| `parse_hex` | §22.6.4 | reads the characters of a `str` |

> **Resolution.** The explicit model of unknowns (Appendix G.72) is stated here before the `std` files hold it: the fences of §22.1 (`Logic`, the operators, `cast`, `same`, `impl Bit`), of §22.3, and of the items that read a carrier directly are ahead of their files, and `std_excerpts` lists each until the file is brought to the text. Where the two differ, this part is the specification and the file is the defect.

> **Resolution.** An implementation **MAY** implement an item natively when the result is observably equal to the definition: same values, same events, same time points, same records.

> **Resolution.** The natures `Bit` (§4), `Electrical`, `Thermal` and `VSignal` (§4) and the module `Short` (§11) are prelude items. Their defining text is in the sections cited; they are not repeated here.

### Index

| Item | Kind | Entry | Built on |
| --- | --- | --- | --- |
| `Logic`, `LogicState` | value types | §22.1.1 | `Repr` |
| the operators of `Logic` | operator-trait `impl`s | §22.1.2 | Appendix H.10 |
| `cast` | `fn` | §22.1.3 | `Repr` |
| `same`, `matches_bits`, `logic` | `fn` | §22.1.4 | `encode`, `Logic` |
| `Bit::rise`, `Bit::fall` | `fn` on a nature | §22.1.5 | `changed`, `prev` |
| `init` | module, result `Event` | §22.1.6 | `changed` |
| `every` | module, result `Event` | §22.1.7 | `at` |
| `since` | module, result `f64` | §22.1.8 | `changed`, `now` |
| `State` | nature | §22.2.1 | — |
| `vt` | `fn` | §22.2.2 | `temperature` |
| `white_noise`, `flicker_noise`, `noise_table` | module, result `f64` | §22.2.3 | `noise`, `freq` |
| `limexp`; `pnjlim`, `fetlim`, `limvds`, `deltalim` | module; `fn` | §22.2.4 | `limit` |
| `above` | module, result `Event` | §22.2.5 | `cross`, `init` |
| `absdelta` | module, result `Event` | §22.2.6 | `cross`, `init` |
| `last_crossing` | module, result `f64` | §22.2.7 | `cross`, `now` |
| `idt` | module, result `f64` | §22.2.8 | `ddt`, `analysis` |
| `idtmod` | module, result `f64` | §22.2.9 | `idt`, `cross` |
| `slew` | module, result `f64` | §22.2.10 | `ddt`, `cross`, `above` |
| `transition` | module, result `f64` | §22.2.11 | `at`, `after`, `now`, `analysis` |
| `zi_nd` | module, result `f64` | §22.2.12 | `every`, `delay`, `settle` |
| `zi_np`, `zi_zd`, `zi_zp` | module, result `f64` | §22.2.13 | `zi_nd` |
| `laplace_nd`, `laplace_np`, `laplace_zd`, `laplace_zp` | module, result `f64` | §22.2.14 | `ddt`, `State` |
| `table_model` | `fn` | §22.2.15 | — |
| `db10`, `db20` | `fn` | §22.2.16 | — |
| `normal` | `fn` | §22.2.17 | — |
| `Strength`, `Driven` | value types | §22.3.1 | `Logic` |
| `Driven::strongest` | `fn` | §22.3.2 | `same` |
| `sum` | `fn` | §22.3.4 | — |
| `Tribuf`, `Pullup`, `Pulldown` | module | §22.3.5 | definitions |
| `Keeper` | module | §22.3.6 | `changed`, `others` |
| `Tran` | module | §22.3.7 | `others` |
| `Tgate` | module | §22.3.8 | `Tran` |
| `D2A` | module | §22.4.1 | `transition` |
| `A2D` | module | §22.4.2 | `above` |
| `Bidir` | module | §22.4.3 | `others`, `transition`, `above` |
| `AckWithin`, `Never`, `Stable`, `Setup`, `Hold`, `MinPulse` | module | §22.5.2–§22.5.7 | `require`, `since` |
| `inertial` | module, result `T` | §22.5.8 | `at`, `now`, `analysis` |
| `TribufDelayed` | module | §22.5.9 | `inertial` |
| `Severity`, `log` | value type, `fn` | §22.6.1 | `emit!` |
| `finish`, `fatal` | `fn` | §22.6.2 | `emit!` |
| `require`, `cover`, `target` | `fn` | §22.6.3 | `emit!` |
| `parse_table`, `parse_hex` | `const fn` | §22.6.4 | `include_str!` |

### Reading the per-analysis tables

Entries for items read by `analog` use the four solve columns of §16.2: equilibrium solve, transient step, jump solve, small-signal sweep.
Entries for items that act at instants use the three columns of the second table of §16.2.
A pure `fn` over values has no table: it is evaluated wherever it is called (§3).

### Errors

Codes owned by this part. Conditions that are instances of another part's rule are listed per entry by section only.

| Code | Condition | Phase |
| --- | --- | --- |
| `E0702` | pattern of `logic` or `matches_bits`: a character outside `0 1 x z`, or a length other than `N` | elaboration |
| `E0703` | `table_model` control string not of the form in §22.2.15 | elaboration |
| `E0704` | `table_model` table: abscissae not strictly increasing, or fewer points than the interpolation degree needs | elaboration |
| `E0705` | `table_model` input outside the table under extrapolation `E`: a panic (§1 rule 11) with its own code | run |
| `E0706` | `parse_table` or `parse_hex`: text not in the format of §22.6.4 | elaboration |

---

### 22.1 Values and `Bit`

#### 22.1.1 `Logic` and `LogicState`

`Logic<N>` is four-state logic on `N` levels: each level is 0, 1, X (not determined) or Z (not driven). It is an ordinary library struct; no rule of the language names it (§3.3.2). `Logic` alone is `Logic<1>`.

```rust
/// Four-state logic on N levels: 0, 1, X (not determined) and Z (not driven).
#[derive(Repr)]
pub struct Logic<const N: usize = 1> {
    pub binary: UInt<N>, // 1 where the level is 0 or 1
    pub value: UInt<N>, // the level where binary; where not, 0 is X and 1 is Z
}

#[derive(Repr, PartialEq)]
pub enum LogicState {
    X,
    L,
    H,
    Z,
}

impl<const N: usize> Default for Logic<N> {
    fn default() -> Logic<N> {
        Logic { binary: 0, value: 0 }
    }
}

impl<const N: usize> Logic<N> {
    fn ones(self) -> UInt<N> {
        self.binary & self.value
    }
    fn zeros(self) -> UInt<N> {
        self.binary & !self.value
    }
    pub fn is_high(self) -> bool {
        self.ones() == !0
    }
    pub fn is_low(self) -> bool {
        self.zeros() == !0
    }
    pub fn is_x(self) -> bool {
        self.binary == 0 && self.value == 0
    }
    pub fn is_z(self) -> bool {
        self.binary == 0 && self.value == !0
    }
    pub fn to_bits(self) -> Option<UInt<N>> {
        if self.binary == !0 {
            Some(self.value)
        } else {
            None
        }
    }
    pub fn at(self, k: usize) -> LogicState {
        let b = (self.binary >> k) & 1;
        let v = (self.value >> k) & 1;
        if b == 1 {
            if v == 1 {
                LogicState::H
            } else {
                LogicState::L
            }
        } else if v == 1 {
            LogicState::Z
        } else {
            LogicState::X
        }
    }
}
```

| `binary` | `value` | Level |
| --- | --- | --- |
| 1 | 0 | 0 |
| 1 | 1 | 1 |
| 0 | 0 | X |
| 0 | 1 | Z |

1. Level `k` of a `Logic<N>` is the pair of bit `k` of `binary` and of `value`, read by the table above. `Logic<N>` has the `Repr` width `2 * N`: levels `0..N` are the plane `binary`, levels `N..2 * N` the plane `value` (§2).
2. An all-zero value is X on every level, so the bytes of `Logic::default()` are all zero and `Logic<N>` implements `Default` with that value (Appendix H.8). `Logic::X`, `Logic::Z`, `Logic::L` and `Logic::H` are constants of the one-level `Logic`; a wider all-X value is `Logic::default()` and a wider all-binary one `Logic::from(b)`. A carrier of type `Bit<Logic<4>>` with no initial value starts at `xxxx` (§3.3.4).
3. `x.at(k)` gives the `LogicState` of level `k`, which a `match` covers with four arms. An out-of-range `k` is a panic (§1 rule 11).
4. `x.is_high()`, `x.is_low()`, `x.is_z()` and `x.is_x()` are true exactly when every level is 1, 0, Z or X respectively. They return a `bool`, so they decide control (§3.3.5).
5. `x.to_bits()` gives `Some` of the `UInt<N>` when every level is 0 or 1, and `None` when any level is X or Z. On a `Logic<1>`, `x.to_bit()` gives the `Option<bool>` by the same rule, and `x.to_volts(vdd)` gives `vdd` for 1, `0.0` for 0 and `vdd / 2.0` for X and for Z: a function that returns an `f64`, so `analog` accepts it as a read of the carrier (§3.3.6).
6. `Logic::from(b)` is total. A `UInt<N>` or a `bool` gives the `Logic` whose every level is the 0 or 1 of `b` (Appendix H.8 rule 8).
7. A `Logic` has no arithmetic and no ordering: `+`, `-` and `<` on one are `E0180`. A `Logic` is converted first (`to_bits()`), which gives an `Option`.
8. A `Logic` is a four-state value, not an IEEE 1164 nine-state one: a drive's strength is a field of the type driven (`Driven`, §22.3), not a state of the logic.

```rust
impl Logic {
    pub const X: Logic = Logic { binary: 0, value: 0 };
    pub const Z: Logic = Logic { binary: 0, value: 1 };
    pub const L: Logic = Logic { binary: 1, value: 0 };
    pub const H: Logic = Logic { binary: 1, value: 1 };
    pub fn to_bit(self) -> Option<bool> {
        self.to_bits().map(|b| b == 1)
    }
    pub fn to_volts(self, vdd: f64) -> f64 {
        match self.to_bit() {
            Some(true) => vdd,
            Some(false) => 0.0,
            None => vdd / 2.0,
        }
    }
}
```

> **Resolution.** Two integer planes keep `Logic<N>` an ordinary struct: its `Repr` layout, its ABI layout and its copy follow from the rules every struct has, at two bits per level. `[Logic; N]` of an enum would cost one byte per level and need an element loop in every operator.

#### 22.1.2 Operators on `Logic`

Each operator is the function of an operator-trait `impl` (Appendix H.10), written in the library with ordinary bit operations on the two planes.

```rust
impl<const N: usize> BitAnd for Logic<N> {
    fn bitand(self, r: Logic<N>) -> Logic<N> {
        let one = self.ones() & r.ones();
        Logic { binary: one | self.zeros() | r.zeros(), value: one }
    }
}

impl<const N: usize> BitOr for Logic<N> {
    fn bitor(self, r: Logic<N>) -> Logic<N> {
        let one = self.ones() | r.ones();
        Logic { binary: one | (self.zeros() & r.zeros()), value: one }
    }
}

impl<const N: usize> BitXor for Logic<N> {
    fn bitxor(self, r: Logic<N>) -> Logic<N> {
        let both = self.binary & r.binary;
        Logic { binary: both, value: (self.value ^ r.value) & both }
    }
}

impl<const N: usize> Not for Logic<N> {
    fn not(self) -> Logic<N> {
        Logic { binary: self.binary, value: !self.value & self.binary }
    }
}

impl<const N: usize> Shl for Logic<N> {
    fn shl(self, k: usize) -> Logic<N> {
        Logic { binary: !(!self.binary << k), value: self.value << k }
    }
}

impl<const N: usize> Shr for Logic<N> {
    fn shr(self, k: usize) -> Logic<N> {
        Logic { binary: !(!self.binary >> k), value: self.value >> k }
    }
}

impl<const N: usize> PartialEq<Logic<N>, Logic> for Logic<N> {
    fn eq(self, r: Logic<N>) -> Logic {
        if self.binary != !0 || r.binary != !0 {
            Logic::X
        } else {
            Logic::from(self.value == r.value)
        }
    }
    fn ne(self, r: Logic<N>) -> Logic {
        !self.eq(r)
    }
}

impl<const N: usize> From<UInt<N>> for Logic<N> {
    fn from(b: UInt<N>) -> Logic<N> {
        Logic { binary: !0, value: b }
    }
}

impl From<bool> for Logic {
    fn from(b: bool) -> Logic {
        Logic { binary: 1, value: b as u1 }
    }
}

impl<const N: usize> BitAnd<UInt<N>, Logic<N>> for Logic<N> {
    fn bitand(self, r: UInt<N>) -> Logic<N> {
        self & Logic::from(r)
    }
}

impl<const N: usize> BitAnd<Logic<N>, Logic<N>> for UInt<N> {
    fn bitand(self, r: Logic<N>) -> Logic<N> {
        Logic::from(self) & r
    }
}

impl BitAnd<bool, Logic> for Logic {
    fn bitand(self, r: bool) -> Logic {
        self & Logic::from(r)
    }
}

impl BitAnd<Logic, Logic> for bool {
    fn bitand(self, r: Logic) -> Logic {
        Logic::from(self) & r
    }
}

impl<const N: usize> BitOr<UInt<N>, Logic<N>> for Logic<N> {
    fn bitor(self, r: UInt<N>) -> Logic<N> {
        self | Logic::from(r)
    }
}

impl<const N: usize> BitOr<Logic<N>, Logic<N>> for UInt<N> {
    fn bitor(self, r: Logic<N>) -> Logic<N> {
        Logic::from(self) | r
    }
}

impl BitOr<bool, Logic> for Logic {
    fn bitor(self, r: bool) -> Logic {
        self | Logic::from(r)
    }
}

impl BitOr<Logic, Logic> for bool {
    fn bitor(self, r: Logic) -> Logic {
        Logic::from(self) | r
    }
}

impl<const N: usize> BitXor<UInt<N>, Logic<N>> for Logic<N> {
    fn bitxor(self, r: UInt<N>) -> Logic<N> {
        self ^ Logic::from(r)
    }
}

impl<const N: usize> BitXor<Logic<N>, Logic<N>> for UInt<N> {
    fn bitxor(self, r: Logic<N>) -> Logic<N> {
        Logic::from(self) ^ r
    }
}

impl BitXor<bool, Logic> for Logic {
    fn bitxor(self, r: bool) -> Logic {
        self ^ Logic::from(r)
    }
}

impl BitXor<Logic, Logic> for bool {
    fn bitxor(self, r: Logic) -> Logic {
        Logic::from(self) ^ r
    }
}
```

The four-state tables, per level. In the operand columns and rows, X stands for X or Z; a result is never Z.

| `&` | 0 | 1 | X |
| --- | --- | --- | --- |
| **0** | 0 | 0 | 0 |
| **1** | 0 | 1 | X |
| **X** | 0 | X | X |

| `\|` | 0 | 1 | X |
| --- | --- | --- | --- |
| **0** | 0 | 1 | X |
| **1** | 1 | 1 | 1 |
| **X** | X | 1 | X |

| `^` | 0 | 1 | X |
| --- | --- | --- | --- |
| **0** | 0 | 1 | X |
| **1** | 1 | 0 | X |
| **X** | X | X | X |

| `!` | Result |
| --- | --- |
| 0 | 1 |
| 1 | 0 |
| X, Z | X |

1. `a & b` is 0 on a level where either operand is 0, 1 where both are 1, and X otherwise. `a | b` is 1 where either is 1, 0 where both are 0, and X otherwise. `a ^ b` is the exclusive or where both levels are 0 or 1, and X otherwise. `!a` is 1 for 0, 0 for 1, and X for X and for Z. A Z level in one operand and a 0 in the other gives 0 under `&`; with a 1 it gives X.
2. `a << k` and `a >> k` move the levels by `k` with their states, and the levels shifted in are 0: `Shl` and `Shr` shift both planes and set the shifted-in levels of `binary` to 1.
3. `&`, `|` and `^` with a `Logic<N>` on one side and a `UInt<N>` on the other, or a `Logic<1>` and a `bool`, in either order, give the `Logic<N>` of the same table with the binary operand converted by `From`. Each is one more `impl` of one line, `impl<const N: usize> BitAnd<UInt<N>, Logic<N>> for Logic<N>`, `impl<const N: usize> BitAnd<Logic<N>, Logic<N>> for UInt<N>`, and the `bool` pair for `Logic<1>`; likewise for `BitOr` and `BitXor`.
4. `a == b` on two `Logic<N>` is a `Logic<1>`: X when any level of either operand is X or Z, otherwise 1 when every level is equal and 0 when not. `a != b` is its `!`. Operands of different widths (`Logic<4> & Logic<8>`) are `E0180`.
5. `#[derive(PartialEq)]` on a struct with a `Logic` field is `E0187` naming the field: the derived `==` would have to return a `bool` and a `Logic` does not give one.
6. A model decides on a `Logic` with `same(a, b)`, `x.is_high()`, a `match` on `x.at(k)` or `x.to_bit()` (§3.3.5).

```rust,fragment
signal bus: Bit<Logic<4>>;                       // starts xxxx
on clk.rise() {
    bus <- logic("10xz") & Logic::from(0b1111u4); // 10xx
    let eq = bus == bus;                         // x
    let s = same(bus, bus);                      // true
}
```

#### 22.1.3 `cast`

```rust
/// The same levels under another type of the same width, or `None` when `B` rejects them.
pub fn cast<A: Repr, B: Repr>(a: A) -> Option<B> where A::WIDTH == B::WIDTH {
    B::decode(a.encode())
}
```

1. The result is what `B::decode` returns for the levels of `a`. A `decode` that fails gives `None`; the caller receives it and no carrier is changed by it (§2 rule 9).
2. A width mismatch is the false `where` of `cast`: `E0127` (§3 rule 8), naming `cast::<A, B>` and both types. (`E0701` is retired: `cast` is an ordinary generic function, so its own `where` is the check, and the language has no second code for it.)
3. A definition between carriers of equal width and different types is written with `cast` and the `Option` unwrapped where the types cannot fail (§8.2.6).

```rust,fragment
signal sh: [Bit; 8] = [false; 8];
byte = cast(sh).unwrap();   // byte: Bit<u8>; sh[0] is the least significant level
```

#### 22.1.4 `same`, `matches_bits`, `logic`

```rust
/// Equal level by level: on a `Logic`, the four states are compared literally.
pub fn same<T: Repr>(a: T, b: T) -> bool {
    a.encode() == b.encode()
}

pub extern "intrinsic" fn matches_bits<const N: usize>(s: Logic<N>, pattern: str) -> bool;

pub extern "intrinsic" fn logic<const N: usize>(pattern: str) -> Logic<N>;
```

| Function | Result |
| --- | --- |
| `same(a, b)` | `true` when every level of `a` and `b` holds the same one of its states. On a `Logic`, X equals X and Z equals Z but X is not Z. |
| `matches_bits(s, p)` | `true` when every level of `s` is the state the corresponding character of `p` names |
| `logic(p)` | the `Logic<N>` whose levels are the states the characters of `p` name |

1. `same` differs from `==` on a `Logic` (§22.1.2 rule 4): `==` is a `Logic` and X when a compared level is X or Z; `same` is a `bool` and is true for two Z levels.
2. A pattern has one character per level, highest level first, each of `0`, `1`, `x`, `z`. Any other character, or a length other than `N`, is `E0702`.
3. `logic("10xz")` is the `Logic<4>` with levels 3 to 0 equal to 1, 0, X, Z.

> **Resolution.** `same` is its definition. `matches_bits` and `logic` read the characters of a `str`, which no definition in the language can do: they are `extern "intrinsic"` and the contract of rules 2 and 3 is normative. The leftmost pattern character is the highest level (`N − 1`), as in a Verilog literal.

#### 22.1.5 `impl Bit`

```rust
impl Bit {
    /// A change that ends at true: false to true.
    pub fn rise(self) -> Event {
        self.changed() && !self.prev() && self
    }
    /// A change that ends at false: true to false.
    pub fn fall(self) -> Event {
        self.changed() && self.prev() && !self
    }
}

impl Bit<Logic> {
    /// A change that ends at 1 from 0, X or Z.
    pub fn rise(self) -> Event {
        self.changed() && self.is_high() && !self.prev().is_high()
    }
    /// A change that ends at 0 from 1, X or Z.
    pub fn fall(self) -> Event {
        self.changed() && self.is_low() && !self.prev().is_low()
    }
}
```

| Previous → current | `rise()` | `fall()` |
| --- | --- | --- |
| false → true | fires | — |
| true → false | — | fires |

On a `Bit<Logic>`: a rise is a change that ends at 1 from 0, X or Z; a fall is a change that ends at 0 from 1, X or Z.

1. `rise` and `fall` are functions returning `Event` (§3). They keep no state; `self` is a handle (§3.1).
2. A `Bit` that starts high has not risen at `(0, 0)`: the initial value is committed there and is a change (§13 rule 6), but `prev()` equals the initial value (§5.3.8), so `!prev()` is false. A model that wants an action at the start writes `init()` (§22.1.6).
3. Inside a reaction, `clk.rise()` used as a `bool` says whether it fired in this microstep (§13 rule 2).
4. A `Bit` is read in `analog` and as a condition directly, as the `bool` it holds (§3.3.6).
5. These methods are defined on `Bit<bool>` and on `Bit<Logic>`. They do not exist on `Bit<T>` for other `T`.

| At instants | Where time does not advance | Transient | Small-signal sweep |
| --- | --- | --- | --- |
| `rise()`, `fall()` | at the microsteps of `(0, ·)` and of each sweep point | at every instant | none |

#### 22.1.6 `init`

Fires once per analysis, at `(0, 0)`.

```rust
pub module init() -> Event {
    signal up: Bit = true;
    up.changed()
}
```

1. Each use of `init()` is one instance with its own carrier `up` (§8.3.3).
2. `up` is committed once, at `(0, 0)` (§16 step 2). That commit is its only change (§13 rule 6).
3. In a sweep `init()` fires at the first sweep point only (§16.1.2).
4. In an analysis run at a transient's last instant `init()` does not fire (§16.4).
5. Reactions on `init()` read the first equilibrium solution (§16 step 4).
6. `init()` is restricted to an analysis by a guard: `init() && analysis(Transient)`.

| At instants | Where time does not advance | Transient | Small-signal sweep |
| --- | --- | --- | --- |
| `init()` | once, at `(0, 0)` of the first point | once, at `(0, 0)` | none |

#### 22.1.7 `every`

Fires at `t0` and every `period` after.

```rust
pub module every(t0: f64, period: f64) -> Event {
    signal k: Bit<u64> = 0;
    let tick = at(t0 + k as f64 * period);
    tick
}

digital for every {
    on tick {
        k <- k + 1;
    }
}
```

1. The `n`-th firing is at `t0 + n · period`, rounded to a tick (§15), for `n = 0, 1, …`.
2. Each firing commits `k`, which re-arms the `at` (§13 rule 4).
3. `t0` and `period` are value inputs. A change of a carrier or param they read re-arms the `at` with the current `k`.
4. `every` never fires where time does not advance (§16.1.3).
5. A continued transient keeps `k` and the armed time (§16.4). The sequence continues.

Consequences of §13 rule 4, stated for reference:

| Arguments | Behavior |
| --- | --- |
| `t0 == 0` | the first firing is at `(0, 1)` |
| `t0 < 0` | the first time is past; the instance never fires |
| `period < 0` | fires once at `t0`, then never |
| `period == 0` | re-arms at the current time each microstep; the instant never goes quiet, which is the error of §15 |

| At instants | Where time does not advance | Transient | Small-signal sweep |
| --- | --- | --- | --- |
| `every(t0, period)` | never | at `t0 + n · period` | none |

#### 22.1.8 `since`

Time since a carrier last changed.

```rust
pub module since<T: Repr>(in s: Bit<T>) -> f64 {
    signal t: Bit<f64> = -inf;
    now() - t // a plain real, so analog may read it (§3.3.6)
}

digital<T> for since<T> {
    on s.changed() {
        t <- now();
    }
}
```

1. The result is `inf` while `s` has never changed, and `now()` minus the time of the last change after.
2. The record of a change applies one microstep after the change (§12.1). A reaction that fires on `s.changed()` reads the time since the change before it.
3. At `(0, 0)` the result is `inf`: the first commit of `s` is not yet recorded.
4. `s` is an `in` port of a one-driver nature. `since` does not take a resolved carrier or a node.
5. The result reads `now()`. It **MUST NOT** be read in a definition (§8.2.5).
6. The result is a plain real. `analog` **MAY** read it.

| Result | Equilibrium solve | Transient step | Jump solve | Small-signal sweep |
| --- | --- | --- | --- | --- |
| `since(s)` | `0 − t`: `inf` before the first change, `0` after one at `(0, ·)` | `now() − t` | `now() − t` | as the equilibrium solve |

In an analysis run at a transient's last instant `now()` is that instant (§16.2), in every column.

---

### 22.2 Analog functions and modules

#### 22.2.1 `State`

A unitless nature for states of order one.

```rust
pub nature State {
    potential: Value { access: pos, abstol: 1u },
    flow: Rate { access: rate, abstol: 1n },
}
```

1. `pos(y)` is the potential probe and `rate(y)` the flow probe of a `State` node.
2. A row written as `rate(y) +> residual` carries an equation tested against `abstol` 1 n (§17.1).
3. A user state **SHOULD** be normalized to order one before it is placed on a `State` node (§4 rule 4).
4. `State` is the default nature of `idt`, `idtmod`, `slew`, `zi_nd` and `laplace_*`.

#### 22.2.2 `vt`

```rust
pub fn vt() -> f64 {
    K_B * temperature() / Q
}
```

1. `vt()` is the thermal voltage at the instance's temperature, `dtemp` included (§8.5).
2. It reads only environment that is constant during an analysis. It is callable everywhere, definitions included (§14).

#### 22.2.3 `white_noise`, `flicker_noise`, `noise_table`

```rust
pub module white_noise(psd: f64, name: Option<str>) -> f64 {
    noise(psd, name)
}

pub module flicker_noise(k: f64, exp: f64, name: Option<str>) -> f64 {
    noise(k / freq().powf(exp), name)
}

pub module noise_table<const N: usize>(name: Option<str>) -> f64 {
    table: [(f64, f64); N];
    noise(table_model(freq(), table, "1C"), name)
}
```

| Module | Power spectral density |
| --- | --- |
| `white_noise(psd, name)` | `psd` |
| `flicker_noise(k, exp, name)` | `k / f^exp`, with `f = freq()` |
| `noise_table(name) { table: … }` | piecewise linear in frequency through the `(frequency, psd)` pairs |

1. Each use is one instance, hence one noise source (§11.1). Two uses are independent. One instance read in two contributions is fully correlated.
2. `name` groups sources in reports. It has no effect on correlation.
3. `psd` is read only where the source is a source (§16.2). `flicker_noise` is never evaluated at `freq() == 0`.
4. The implicit param `m` multiplies noise power (§8.5).

> **Resolution.** `table` is a param, an array passed to `table_model` by value (§1.4.6). The definition is `noise(table_model(freq(), table, "1C"), name)`: linear interpolation, the end values held outside the table (VAMS 4.6.4.3).

| Result | Equilibrium solve | Transient step | Jump solve | Small-signal sweep |
| --- | --- | --- | --- | --- |
| all three | 0; `psd` not read | 0; not read | 0; not read | 0 in `Ac`; in `Noise` a source of the density above |

#### 22.2.4 `limexp` and the limiting functions

```rust
pub module limexp(x: f64) -> f64 {
    limit(x, |new, old| deltalim(new, old, 1.0)).exp()
}

pub fn pnjlim(new: f64, old: f64, nvt: f64, vcrit: f64) -> f64 {
    if new > vcrit && (new - old).abs() > 2.0 * nvt {
        if old > 0.0 {
            let arg = (new - old) / nvt;
            if arg > 0.0 {
                old + nvt * (2.0 + (arg - 2.0).ln())
            } else {
                old - nvt * (2.0 + (2.0 - arg).ln())
            }
        } else {
            nvt * (new / nvt).ln()
        }
    } else if new < 0.0 {
        let arg = if old > 0.0 {
            -old - 1.0
        } else {
            2.0 * old - 1.0
        };
        if new < arg {
            arg
        } else {
            new
        }
    } else {
        new
    }
}

pub fn fetlim(new: f64, old: f64, vth: f64) -> f64 {
    let hi = (2.0 * (old - vth)).abs() + 2.0;
    let lo = (old - vth).abs() + 1.0;
    let vtox = vth + 3.5;
    let delv = new - old;
    if old >= vth {
        if old >= vtox {
            if delv <= 0.0 {
                if new >= vtox {
                    if -delv > lo {
                        old - lo
                    } else {
                        new
                    }
                } else if new > vth + 2.0 {
                    new
                } else {
                    vth + 2.0
                }
            } else if delv >= hi {
                old + hi
            } else {
                new
            }
        } else if delv <= 0.0 {
            if new > vth - 0.5 {
                new
            } else {
                vth - 0.5
            }
        } else if new < vth + 4.0 {
            new
        } else {
            vth + 4.0
        }
    } else if delv <= 0.0 {
        if -delv > hi {
            old - hi
        } else {
            new
        }
    } else if new <= vth + 0.5 {
        if delv > lo {
            old + lo
        } else {
            new
        }
    } else {
        vth + 0.5
    }
}

pub fn limvds(new: f64, old: f64) -> f64 {
    if old >= 3.5 {
        if new > old {
            if new < 3.0 * old + 2.0 {
                new
            } else {
                3.0 * old + 2.0
            }
        } else if new < 3.5 {
            if new > 2.0 {
                new
            } else {
                2.0
            }
        } else {
            new
        }
    } else if new > old {
        if new < 4.0 {
            new
        } else {
            4.0
        }
    } else if new > -0.5 {
        new
    } else {
        -0.5
    }
}

pub fn deltalim(new: f64, old: f64, delta: f64) -> f64 {
    new.clamp(old - delta, old + delta)
}
```

| Function | Bounds |
| --- | --- |
| `pnjlim` | the step of a junction voltage above `vcrit`, logarithmically in units of `nvt` |
| `fetlim` | the step of a gate voltage relative to the threshold `vth` |
| `limvds` | the step of a drain-source voltage |
| `deltalim` | the step to at most `delta` in either direction |

1. Each limiting function is a pure `fn(new, old) -> f64`, passed to `limit` directly or in a closure (§18).
2. A limiting function changes the Newton path, never the answer (§18 rule 5).
3. `limexp(x)` equals `x.exp()` at convergence. Per iteration its argument moves by at most 1.0.
4. `limexp` is one `limit` instance per use. It vetoes convergence while active (§17.1.3).
5. For differentiation `limit` is transparent (§18 rule 3): the derivative of `limexp(x)` is `limexp(x)`.

> **Resolution.** `deltalim(new, old, delta)` is `new.clamp(old − delta, old + delta)`. `pnjlim`, `fetlim` and `limvds` are the ngspice functions of the same names (Appendix C.2) written in MSL, branch by branch; `pnjlim` has the forward-bias step of `DEVpnjlim` (for `vold > 0`, with `arg = (vnew − vold)/vt`: `vold + vt·(2 + ln(arg − 2))` when `arg > 0`, else `vold − vt·(2 + ln(2 − arg))`; for `vold <= 0`, `vt·ln(vnew/vt)`) and its reverse-bias branch (a `vnew < 0` below `−vold − 1`, or below `2·vold − 1` when `vold <= 0`, is raised to that bound). Their exact step is not observable at convergence and is not normative.

| Result | Equilibrium solve | Transient step | Jump solve | Small-signal sweep |
| --- | --- | --- | --- | --- |
| `limexp(x)` | bounded per iteration; `exp(x)` at convergence | the same | the same | `exp(x)` at the operating point, as a gain |

#### 22.2.5 `above`

A rising crossing that is also right at the operating point.

```rust
pub module above(e: f64, ttol: Option<f64>, etol: Option<f64>) -> Event {
    cross(e, ttol, etol) || (init() && e > 0.0)
}
```

1. `above(e)` fires when `cross(e)` fires (§13 rule 5).
2. `above(e)` also fires at `(0, 0)` when `e > 0.0` in the first equilibrium solution.
3. `ttol` and `etol` are those of `cross` (§17.3). Left out, they are `None`.
4. A falling crossing is `above(-e)`.
5. Each use is one instance, holding one `cross` and one `init`.

| At instants | Where time does not advance | Transient | Small-signal sweep |
| --- | --- | --- | --- |
| `above(e)` | at `(0, 0)` of the first point when `e > 0`; then when `e` rises through 0 between successive solves or sweep points | at `(0, 0)` when `e > 0`; then located in time; also across a jump and across the rounds of the operating point | none |

#### 22.2.6 `absdelta`

Fires at the start, and each time `e` has moved by `delta` since it last fired.

```rust
pub module absdelta(e: f64, delta: f64) -> Event {
    signal last: Bit<f64> = 0.0;
    let moved = init() || cross(e - last - delta) || cross(last - delta - e);
    moved
}

digital for absdelta {
    on moved {
        last <- e;
    }
}
```

1. The first firing is `init()`. Its reaction commits `last <- e`, read at the first solution.
2. Later firings are located crossings of `e` through `last + delta` upward or `last − delta` downward.
3. Each firing re-seats `last`, which re-forms both crossing expressions.
4. `delta` **SHOULD** be positive. With `delta <= 0` each expression is positive once `e` has drifted from `last`. No rising crossing is then guaranteed.
5. The reaction probes, so each firing is a time point (§17.2).

| At instants | Where time does not advance | Transient | Small-signal sweep |
| --- | --- | --- | --- |
| `absdelta(e, d)` | at the first point; then when `e` has moved by `d` between successive solves or sweep points | at `(0, 0)`; then located in time | none |

#### 22.2.7 `last_crossing`

Time of the last rising crossing of `e`; `-1.0` before the first.

```rust
pub module last_crossing(e: f64) -> f64 {
    signal t: Bit<f64> = -1.0;
    t // a plain real, so analog may read it (§3.3.6)
}

digital for last_crossing {
    on cross(e) {
        t <- now();
    }
}
```

1. The result is a discrete value: piecewise constant, changing one microstep after each located crossing.
2. The recorded time is the tick the crossing was rounded up to (§15).
3. Only rising crossings are recorded. The falling ones are `last_crossing(-e)`.
4. A crossing at the operating point is not recorded: `cross` needs two solutions (§16).
5. When `analog` reads the result, each update is a jump (§15.2). The jump moves no state.

| Result | Equilibrium solve | Transient step | Jump solve | Small-signal sweep |
| --- | --- | --- | --- | --- |
| `last_crossing(e)` | constant; `-1.0`, or `0.0` after a crossing between solves | constant during the step | constant | constant: no response |

#### 22.2.8 `idt`

The time integral of `x`, on a node whose nature gives the state its tolerance.

```rust
pub module idt<S: Nature = State>(x: f64, ic: Option<f64>, reset: Option<bool>) -> f64 {
    signal y: S;
    potential(y) // the result
}

analog<S> for idt<S> {
    let hold = reset.unwrap_or(false) || (ic.is_some() && analysis(Static));
    if hold {
        potential(y) +> ic.unwrap_or(0.0);
    } else {
        flow(y) +> ddt(potential(y)) - x;
    }
}
```

| Argument | Meaning | Left out |
| --- | --- | --- |
| `x` | the integrand | required |
| `ic` | the value held in an equilibrium solve and while `reset` is true | no initial condition |
| `reset` | while true, the result is held at `ic`, or `0.0` | never reset |

Static semantics:

1. The branch `(y, gnd)` is a switch branch (§11 rule 2). Its selector is `hold`.
2. `reset` **MUST NOT** depend on a probe (§11 rule 3). It may read params, discrete carriers and the environment.
3. A discrete carrier read in `reset` or `ic` passes through `level` (§3.3.5).
4. `ic` and `x` are value inputs. They **MAY** probe and are differentiated (§8.3.1).
5. An instance **MAY** read its own result in `x`: `let y = idt(f(y));` is `y' = f(y)` (§8.3.5).
6. `S` selects the tolerances: the potential `abstol` of `S` for the state, its flow `abstol` for the row.

Dynamic semantics:

| Result | Equilibrium solve | Transient step | Jump solve | Small-signal sweep |
| --- | --- | --- | --- | --- |
| `ic` given, `reset` false | `ic` | `ic + ∫ x dt` | the integral is a state and is kept (§15.2.1) | `x / jω` |
| no `ic`, `reset` false | free; the row is `x = 0` | the equilibrium value `+ ∫ x dt` | kept | `x / jω` |
| `reset` true | `ic`, or `0.0` | `ic`, or `0.0` | forced to `ic`: a state made to jump (§15.2.2) | the response of `ic`; zero when `ic` is constant |

7. With no `ic`, the equilibrium row `x = 0` has a solution only inside a feedback loop. Otherwise the solve fails, naming the instance (§8.3).
8. When `reset` falls, integration continues from the held value (§15.2.1).
9. `idt` follows VAMS 4.5.4 with no rule of its own.

```rust,fragment
analog for Pi { v(outp) +> k1 * v(inp) + k2 * idt(v(inp), 0.0); }
```

```rust,fragment
let absorbed = idt::<Heat>(pwr(t), 0.0, reset: ph != Phase::Boiling);
```

#### 22.2.9 `idtmod`

The integral of `x` wrapped into `[lo, lo + modulus)`, with `lo = offset.unwrap_or(0.0)`.

```rust
pub module idtmod<S: Nature = State>(x: f64, ic: Option<f64>, modulus: f64, offset: Option<f64>) -> f64 {
    signal base: Bit<Option<f64>>; // where the integral was last seated; None before the first wrap
    signal seat: Bit = false; // true for the one microstep that seats it
    let lo = offset.unwrap_or(0.0);
    let start = lo + (ic.unwrap_or(0.0) - lo).rem_euclid(modulus);
    let acc = idt::<S>(x, base.unwrap_or(start), reset: seat);
    acc
}

digital<S> for idtmod<S> {
    on cross(acc - lo - modulus) {
        base <- Some(acc - modulus);
        seat <- true;
    } // over the top
    on cross(lo - acc) {
        base <- Some(acc + modulus);
        seat <- true;
    } // under the bottom
    on seat.rise() {
        seat <- false;
    }
}
```

1. `ic` is not trailing, so it is always written: a value, or `None` (§3.2.2).
2. The initial value is `start`: `ic`, or `0.0`, wrapped into the range.
3. A wrap takes three microsteps of one instant:

| Microstep | Applied | Effect |
| --- | --- | --- |
| `n` | — | a `cross` fires; its reaction commits `base` and `seat <- true` |
| `n + 1` | `base`, `seat` high | `idt` forces its node to `base`: a jump (§15.2.2); `seat.rise()` commits `seat <- false` |
| `n + 2` | `seat` low | a second jump; the integral continues from `base` (§15.2.1) |

4. The seated value lies inside the range. Neither `cross` fires again at that instant.
5. The state never grows, so its tolerances stay meaningful for any run length.
6. The result is discontinuous at a wrap. Edges **SHOULD** be taken from offset crossings of the result, not from the wrap (Appendix D.7).
7. `modulus` is not checked. With `modulus == 0.0`, `start` is not finite and the solve fails (§17.1).

| Result | Equilibrium solve | Transient step | Jump solve | Small-signal sweep |
| --- | --- | --- | --- | --- |
| `idtmod(x, ic, modulus, offset)` | `start`, or the last seated `base` | the wrapped integral | kept, except when seated | `x / jω` |

```rust,fragment
let ph = idtmod(f0 + kvco * v(vctl), 0.0, 1.0, -0.5);    // phase in [-0.5, 0.5)
```

#### 22.2.10 `slew`

Follows `x` with a bounded rate of change.

```rust
#[derive(Repr, PartialEq)]
enum Slewing {
    Track,
    Up,
    Down,
}

pub module slew(x: f64, rise: Option<f64>, fall: Option<f64>) -> f64 {
    signal y: State;
    signal mode: Bit<Slewing> = Slewing::Track;
    let (r, f) = (rise.unwrap_or(inf), fall.unwrap_or(inf));
    let dx = ddt(x);
    pos(y)
}

analog for slew {
    match mode {
        Slewing::Track => {
            pos(y) +> x;
        },
        Slewing::Up => {
            rate(y) +> ddt(pos(y)) - r;
        },
        Slewing::Down => {
            rate(y) +> ddt(pos(y)) + f;
        },
    }
}

digital for slew {
    on above(dx - r) && mode == Slewing::Track {
        mode <- Slewing::Up;
    } // x runs away upward
    on above(-dx - f) && mode == Slewing::Track {
        mode <- Slewing::Down;
    }
    on (cross(pos(y) - x) || cross(x - pos(y))) && mode != Slewing::Track {   // caught up
        mode <- if dx > r { Slewing::Up } else if dx < -f { Slewing::Down } else { Slewing::Track };
    }
}
```

| Argument | Meaning | Left out |
| --- | --- | --- |
| `x` | the input; a value input, so it **MAY** probe (§8.1) | required |
| `rise` | the largest upward rate, a positive number | no bound |
| `fall` | the largest downward rate, given as a positive number | no bound |

1. In `Track` the result equals `x`. In `Up` it rises at `rise`; in `Down` it falls at `fall`.
2. The pattern is that of §11.2: a switch branch selected by a `Bit`, flipped by located crossings. The result is exact.
3. Entering a limited mode is a jump in which `pos(y)` is a state and stays (§15.2.1).
4. When the output catches up with `x`, the reaction re-selects the mode from the current `dx`.
5. `slew` follows VAMS 4.5.9.

| Result | Equilibrium solve | Transient step | Jump solve | Small-signal sweep |
| --- | --- | --- | --- | --- |
| `slew(x, rise, fall)` | `x`: `ddt` is 0, so the mode stays `Track` | `x`, or a ramp at the bound | continuous when the mode changes | the response of `x` in `Track`; zero in `Up` or `Down` |

#### 22.2.11 `transition`

Ramps a piecewise-constant input: a delay `td`, then a straight edge of `tr` (up) or `tf` (down).

```rust
pub module transition(in x: Bit<f64>, td: Option<f64>, tr: Option<f64>, tf: Option<f64>) -> f64 {
    signal xd: Bit<f64> = 0.0; // x after the delay
    signal y0: Bit<f64> = 0.0; // output where the current edge began
    signal y1: Bit<f64> = 0.0; // where it ends
    signal t0: Bit<f64> = 0.0; // when it began
    signal dt: Bit<f64> = 0.0; // its duration; 0.0 at rest
    xd = x after td.unwrap_or(0.0);
    let (a, b, s, d) = (y0, y1, t0, dt);
    let y = if analysis(Static) {
        x
    } else if d > 0.0 {
        a + (b - a) * ((now() - s) / d).clamp(0.0, 1.0)
    } else {
        b
    };
    y
}

digital for transition {
    on at(t0 + dt) && dt > 0.0 {
        dt <- 0.0;
    } // the far corner
    on x.changed() && analysis(Static) {
        y0 <- x;
        y1 <- x;
    } // no delay to wait out, no ramp: at rest on x
    on xd.changed() && !analysis(Static) {                    // written last: wins a tie (§12.1.2)
        let to = xd;
        y0 <- y; y1 <- to; t0 <- now();
        dt <- if to > y { tr.unwrap_or(0.0) } else if to < y { tf.unwrap_or(0.0) } else { 0.0 };
    }
}
```

| Argument | Meaning | Left out |
| --- | --- | --- |
| `x` | the input, an `in` port: a discrete expression (§8.1) | required |
| `td` | transport delay before each edge starts | `0.0` |
| `tr` | duration of a rising edge | `0.0`: a step |
| `tf` | duration of a falling edge | `0.0`: a step |

Static semantics:

1. `x` is bound to an `in` port. The argument is an anonymous definition and **MUST NOT** probe (§8.1). `transition(v(a), …)` is an error at the call.
2. An expression over carriers passed as `x` is read directly (Appendix D.4).
3. The result reads `now()`. It **MUST NOT** be read in a definition (§8.2.5).

Dynamic semantics:

4. A change of `x` reaches `xd` after `td`, by transport delay (§12.1.4).
5. Each change of `xd` starts an edge from the current output to the new value.
6. A change that arrives mid-ramp starts from the current output.
7. `xd` starts at `0.0`, so the first value of `x` arrives `td` later, with a ramp from `0.0`.
8. The state carriers `y0`, `y1`, `t0` and `dt` start at `0.0`: the output is at rest at `0.0`.
9. Both corners of every edge are changes of carriers that `analog` reads. The solver lands on them (§17.2).
10. In an equilibrium solve the result is the input (VAMS 4.5.8). The first edge leaves from there with no ramp.
11. The seed tests `analysis(Static)`, not `init()`: an input committed at `(0, 1)` is still followed (§22.2.12).
12. A transient continued under §16.4 keeps `xd`, `t0` and `dt`. An edge in progress finishes.

| Result | Equilibrium solve | Transient step | Jump solve | Small-signal sweep |
| --- | --- | --- | --- | --- |
| `transition(x, td, tr, tf)` | `x` | the ramp at `now()`, or the resting value | the value at `now()`; a corner moves nothing | constant: no response (§11.1) |

```rust,fragment
let ramp = transition(vref * (code as f64) / 256.0, 0.0, tedge, tedge);
```

#### 22.2.12 `zi_nd`

A discrete-time filter sampled every `period`:
`y[n] = Σ num[k]·x[n−k] − Σ den[k]·y[n−1−k]`.

```rust
pub module zi_nd<const N: usize, const D: usize>(x: f64, period: f64) -> f64 {
    num: [f64; N];
    den: [f64; D]; // den[k] is the coefficient of y[n−1−k]
    signal xs: [Bit<f64>; N] = [0.0; N]; // xs[k] = x[n−1−k]
    signal ys: [Bit<f64>; D] = [0.0; D]; // ys[k] = y[n−1−k]
    signal vout: Bit<f64> = 0.0; // the current sample y[n]
    signal y: State;
    let sampled = analysis(Transient) && !analysis(Static);
    pos(y)
}

digital<N, D> for zi_nd<N, D> {
    on settle && analysis(Static) {          // the history of a steady input, read by the first step (§15)
        vout <- pos(y);
        for k in 0..N { xs[k] <- x; }
        for k in 0..D { ys[k] <- pos(y); }
    }
    on every(period, period) {
        let acc = num[0] * x;
        for k in 1..N {
            acc += num[k] * xs[k - 1];
        }
        for k in 0..D {
            acc -= den[k] * ys[k];
        }
        vout <- acc;
        xs[0] <- x;
        for k in 1..N {
            xs[k] <- xs[k - 1];
        }
        ys[0] <- acc;
        for k in 1..D {
            ys[k] <- ys[k - 1];
        }
    }
}

analog<N, D> for zi_nd<N, D> {
    if sampled {
        pos(y) +> vout;
    } else {
        let acc = 0.0;
        for k in 0..N {
            acc += num[k] * delay(x, k as f64 * period);
        }
        for k in 0..D {
            acc -= den[k] * delay(pos(y), (k + 1) as f64 * period);
        }
        rate(y) +> pos(y) - acc; // the row of y: y = Σ num·x(t−kT) − Σ den·y(t−(k+1)T)
    }
}
```

> **Resolution.** The sample carrier is named `vout` here. The design names it `out`, which is a reserved word (Appendix A.1).

> **Resolution.** At each placement, `period` **MUST** be a discrete expression that is constant during an analysis. `at` (§13 rule 11) and `delay` without `max` (§14.6.2) require it, per instance, after value inputs are substituted.

1. The transfer function is `H(z) = Σ num[k]·z^−k / (1 + Σ den[k]·z^−(k+1))`. The leading denominator coefficient is 1 and is not written.
2. `num` and `den` are params, given in a param block: `zi_nd(v(inp), 1u) { num: [0.5, 0.5], den: [] }`.
3. `N` and `D` are const generics, inferred from the arrays. `num` and `den` are not structural; `N` and `D` are.
4. The branch `(y, gnd)` is a switch branch selected by `sampled`, which is constant during a solve.
5. While a transient runs, the filter is sampled state. Samples are taken at `period, 2·period, …`.
6. Each sample changes `vout`, which `analog` reads: a time point and a jump (§17.2). The reaction probes `x` there.
7. In every other solve the same transfer is written with `delay` (§11.1).
8. The sampled state is seeded by the operating point's `settle`. The seed reads the settled solution, so `t = 0` is consistent.
9. The seed tests `analysis(Static)`, not `init()`. A flip-flop held in reset commits its `q` at `(0, 1)`, after `init()` has fired.
10. Around a periodic trajectory the linearization uses the `delay` form (§16.3). This is intended.
11. `zi_nd` follows VAMS 4.5.12.

| Result | Equilibrium solve | Transient step | Jump solve | Small-signal sweep |
| --- | --- | --- | --- | --- |
| `zi_nd(x, period)` | `H(1)·x`: every `delay` is identity | `vout`, piecewise constant | `vout`, after the sample is applied | `H(exp(jω·period))·x` |

The behavior of `zi_nd` in a `Dc` analysis run at a transient's last instant is open (Appendix G.33).

#### 22.2.13 `zi_np`, `zi_zd`, `zi_zp`

Discrete-time filters given by roots. Each converts its roots to coefficients at elaboration and places one `zi_nd`.

```rust
/// True when each complex root has its conjugate as often as itself.
const fn conjugate_closed<const N: usize>(roots: [(f64, f64); N]) -> bool {
    roots.iter().all(
        |r| r.1 == 0.0 || roots.iter().filter(|c| c.0 == r.0 && c.1 == -r.1).count() == roots.iter().filter(|c| c.0 == r.0 && c.1 == r.1).count(),
    )
}

/// Coefficients, ascending in `q = z^−1`, of the product of `1 − ρ·q` over the roots.
const fn poly_q<const N: usize>(roots: [(f64, f64); N]) -> [f64; N + 1] {
    let c = [(0.0, 0.0); N + 1];
    c[0] = (1.0, 0.0);
    for n in 0..N {
        let (a, b) = roots[n];
        for k in (1..=n + 1).rev() {
            let (pr, pi) = c[k - 1];
            c[k] = (c[k].0 - (a * pr - b * pi), c[k].1 - (a * pi + b * pr));
        }
    }
    let re = [0.0; N + 1];
    for k in 0..=N {
        re[k] = c[k].0;
    }
    re
}

/// The same product without its leading 1: the `den` of `zi_nd` (§22.2.12 rule 1).
const fn den_q<const D: usize>(poles: [(f64, f64); D]) -> [f64; D] {
    let c = poly_q(poles);
    let d = [0.0; D];
    for k in 0..D {
        d[k] = c[k + 1];
    }
    d
}

pub module zi_zp<const N: usize, const D: usize>(x: f64, period: f64) -> f64 {
    zeros: [(f64, f64); N] where conjugate_closed(zeros);
    poles: [(f64, f64); D] where conjugate_closed(poles);
    zi_nd(x, period) { num: poly_q(zeros), den: den_q(poles) }
}

pub module zi_zd<const N: usize, const D: usize>(x: f64, period: f64) -> f64 {
    zeros: [(f64, f64); N] where conjugate_closed(zeros);
    den: [f64; D];
    zi_nd(x, period) { num: poly_q(zeros), den }
}

pub module zi_np<const N: usize, const D: usize>(x: f64, period: f64) -> f64 {
    num: [f64; N];
    poles: [(f64, f64); D] where conjugate_closed(poles);
    zi_nd(x, period) { num, den: den_q(poles) }
}
```

1. A root is a pair `(real, imaginary)`. A root `ρ` contributes the factor `1 − ρ·z^−1` (VAMS 4.5.12). The imaginary parts of the product cancel when the roots are conjugate-closed, so the coefficients are the real parts.
2. A complex root **MUST** appear with its conjugate. Otherwise the `where` of the param is false: `E0317`.
3. `num` and `den` follow the convention of `zi_nd` (§22.2.12 rule 1).
4. Behavior in every analysis is that of the `zi_nd` placed.

> **Resolution.** Roots are params, so the conversion to coefficients is an elaboration constant (§8.2).

#### 22.2.14 `laplace_nd`, `laplace_np`, `laplace_zd`, `laplace_zp`

Continuous-time filters `H(s)`, with one internal `State` node per pole.

```rust
/// Coefficients, ascending in `s`, of the product of `1 − s/ρ`; a root at zero gives the factor `s`.
const fn poly_s<const N: usize>(roots: [(f64, f64); N]) -> [f64; N + 1] {
    let c = [(0.0, 0.0); N + 1];
    c[0] = (1.0, 0.0);
    for n in 0..N {
        let (a, b) = roots[n];
        let zero = a == 0.0 && b == 0.0;
        let m = a * a + b * b;
        let (ia, ib) = if zero {
            (0.0, 0.0)
        } else {
            (a / m, -b / m)
        }; // 1/ρ
        for k in (1..=n + 1).rev() {
            let (pr, pi) = c[k - 1];
            c[k] = if zero {
                (pr, pi)
            } else {
                (c[k].0 - (ia * pr - ib * pi), c[k].1 - (ia * pi + ib * pr))
            };
        }
        if zero {
            c[0] = (0.0, 0.0);
        }
    }
    let re = [0.0; N + 1];
    for k in 0..=N {
        re[k] = c[k].0;
    }
    re
}

pub module laplace_nd<const N: usize, const D: usize>(x: f64) -> f64 where D >= 1 && N <= D {
    num: [f64; N];
    den: [f64; D] where den[D - 1] != 0.0;
    signal w: [State; D - 1]; // w[k]: the k-th derivative of W, where den(s)·W = x
    let top = if D > 1 {
        ddt(pos(w[D - 2]))
    } else {
        x / den[0]
    };
    let y = (0..N).map(
        |k| num[k] * if k + 1 < D {
            pos(w[k])
        } else {
            top
        },
    ).sum();
    y
}

analog<N, D> for laplace_nd<N, D> {
    for k in 0..D - 1 {
        if k + 2 < D {
            rate(w[k]) +> ddt(pos(w[k])) - pos(w[k + 1]);
        }
    }
    if D > 1 {
        let acc = den[D - 1] * top - x;
        for j in 0..D - 1 {
            acc += den[j] * pos(w[j]);
        }
        rate(w[D - 2]) +> acc; // the row of the last state: den(s)·W = x
    }
}

pub module laplace_zp<const N: usize, const D: usize>(x: f64) -> f64 where N <= D {
    zeros: [(f64, f64); N] where conjugate_closed(zeros);
    poles: [(f64, f64); D] where conjugate_closed(poles);
    laplace_nd(x) { num: poly_s(zeros), den: poly_s(poles) }
}

pub module laplace_zd<const N: usize, const D: usize>(x: f64) -> f64 where N < D {
    zeros: [(f64, f64); N] where conjugate_closed(zeros);
    den: [f64; D];
    laplace_nd(x) { num: poly_s(zeros), den }
}

pub module laplace_np<const N: usize, const D: usize>(x: f64) -> f64 where N <= D + 1 {
    num: [f64; N];
    poles: [(f64, f64); D] where conjugate_closed(poles);
    laplace_nd(x) { num, den: poly_s(poles) }
}
```

1. In the `nd` form, `H(s) = Σ num[k]·s^k / Σ den[k]·s^k`, coefficients in ascending powers of `s`.
2. A root `ρ` contributes the factor `1 − s/ρ`. A root at zero contributes the factor `s` (VAMS 4.5.11).
3. A complex root **MUST** appear with its conjugate. The leading denominator coefficient of `laplace_nd` **MUST** be non-zero (`den[D - 1] != 0.0`): `den: [1.0, 0.0]` would give `H(s) = s`, improper, with fewer poles than the `D - 1` states of rule 5. A zero leading coefficient (so also a denominator that is identically zero) is an error. Each is a false `where` of the param: `E0317`.
4. `laplace_nd` requires `D >= 1` and `N <= D`: a proper transfer function. A placement that breaks it is `E0304`.
5. The filter is realized with `ddt` on internal `State` nodes, in the controllable canonical form. Its states obey the jump rule (§15.2.1). With `D == 1` there is no state and the result is `num[0] * x / den[0]`.
6. `x` is a value input: it **MAY** probe and is differentiated.
7. Applied to a noise result, the filter is `H(jω)·n` (§11.1).

> **Resolution.** The transfer function and the number of states are normative; the controllable canonical form is the definition above.

| Result | Equilibrium solve | Transient step | Jump solve | Small-signal sweep |
| --- | --- | --- | --- | --- |
| `laplace_*(x)` | `H(0)·x`; with a pole at zero the output is free, as for `idt` without `ic` | the response of `H(s)` to `x` | states kept | `H(jω)·x` |

#### 22.2.15 `table_model`

Interpolates a table of samples.

```rust
pub extern "intrinsic" fn table_model<const N: usize>(x: f64, table: [(f64, f64); N], control: str) -> f64;
```

1. `table` holds `(abscissa, value)` pairs with strictly increasing abscissae. Otherwise `E0704`.
2. `control` is one interpolation character, then zero, one or two extrapolation characters (VAMS 9.21).

| Position | Character | Meaning |
| --- | --- | --- |
| interpolation | `1` | linear |
| | `2` | quadratic spline |
| | `3` | cubic spline |
| | `D` | the closest sample, no interpolation |
| extrapolation | `C` | constant: the end value |
| | `L` | linear, with the end slope |
| | `E` | error: `E0705` when `x` is outside the table |

3. One extrapolation character applies to both ends. Two apply to the low end, then the high end. None is `L` at both.
4. Any other control string is `E0703`.
5. The function is pure. Inside `analog` it is differentiated through (§3 rule 1); its derivative is that of the interpolant.
6. `E0705` is a panic (§1 rule 11): it fails the analysis. The diagnostic **MUST** name the instance and the call.
7. A table is usually a `const` read from a file (§20, §22.6.4).
8. The end conditions of the splines `2` and `3` are implementation-defined and **MUST** be documented (G.64).

> **Resolution.** `table_model` reads the characters of `control`, so it is `extern "intrinsic"` (§22) and rules 1 to 8 are its contract. It covers one dimension, which is every use in this specification. The control-string table is that of VAMS 9.21 restricted to one dimension.

```rust,fragment
const IV: [(f64, f64); 64] = parse_table(include_str!("diode_iv.tbl"));
analog for MeasuredDiode { i(a, k) +> table_model(v(a, k), IV, "3E"); }   // cubic, error outside
```

#### 22.2.16 `db10`, `db20`

```rust
pub fn db10(x: f64) -> f64 {
    10.0 * x.log10()
} // a power ratio

pub fn db20(x: f64) -> f64 {
    20.0 * x.abs().log10()
} // an amplitude ratio
```

1. Both are pure functions over values, callable anywhere.
2. `db10` of a non-positive number and `db20(0.0)` are not finite. In `analog` that fails the solve (§17.1).

> **Resolution.** `db20` takes the magnitude, so a sign inversion does not make a gain undefined (`target("gain_db", db20(v(vo) / v(vi)) - 40.0)`, §19).

#### 22.2.17 `normal`

One step of a pseudo-random generator with a normal distribution.

```rust
/// One step of SplitMix64: a uniform real in [0, 1) and the next state.
fn uniform(rng: u64) -> (f64, u64) {
    let s = rng + 0x9E3779B97F4A7C15;
    let z = (s ^ (s >> 30)) * 0xBF58476D1CE4E5B9;
    let z = (z ^ (z >> 27)) * 0x94D049BB133111EB;
    let z = z ^ (z >> 31);
    ((z >> 11) as f64 / 9007199254740992.0, s)
}

pub fn normal(rng: u64, mean: f64, sigma: f64) -> (f64, u64) {
    let (a, s1) = uniform(rng);
    let (b, s2) = uniform(s1);
    let z = (-2.0 * (1.0 - a).ln()).sqrt() * (2.0 * PI * b).cos();
    (mean + sigma * z, s2)
}
```

1. `normal` is pure: the sample and the next state are functions of the arguments alone.
2. The first component is the sample. The second is the next generator state.
3. With `sigma == 0.0` the sample is `mean`.
4. The state is kept by the caller, in a signal. A model's random sequence is therefore a function of its seed param.

> **Resolution.** The generator is the one written above: SplitMix64 for two uniform draws and Box-Muller for the sample (`1.0 - a` is in `(0, 1]`, so the logarithm is finite). Integer arithmetic wraps (§1 rule 6).

```rust,fragment
on clk.changed() {
    let (j, next) = normal(rng, 0.0, jitter);
    rng <- next;
    clk <- !clk after period / 2.0 + j;
}
```

---

### 22.3 Strengths and ready-made net modules

High impedance, strengths and resolution by strength are a value type and an `impl Resolve` (§4 rule 8, §5.2). None is a language feature, and the library declares **no net nature**: a model declares the nature of its net and gives it the capability (§3.7.13). This section holds what such a nature needs, a type for what is driven, a resolution function, and the modules that drive.

#### 22.3.1 `Strength`, `Driven`

```rust
/// How hard a drive holds its level. Verilog's drive strengths (IEEE 1364), without charge storage.
#[derive(Repr, PartialEq, PartialOrd)]
pub enum Strength {
    HighZ,
    Weak,
    Pull,
    Strong,
    Supply,
}

/// What a module drives into a net that resolves by strength: a level and how hard it is held.
#[derive(Repr)]
pub struct Driven {
    pub v: Logic,
    pub s: Strength,
}

impl Driven {
    pub const Z: Driven = Driven { v: Logic::Z, s: Strength::HighZ }; // not driving
    pub fn strong(v: bool) -> Driven {
        Driven { v: Logic::from(v), s: Strength::Strong }
    }
    pub fn pull(v: bool) -> Driven {
        Driven { v: Logic::from(v), s: Strength::Pull }
    }
    pub fn weak(v: bool) -> Driven {
        Driven { v: Logic::from(v), s: Strength::Weak }
    }
    pub fn supply(v: bool) -> Driven {
        Driven { v: Logic::from(v), s: Strength::Supply }
    }
}

impl Default for Driven {
    fn default() -> Driven {
        Driven::Z
    }
}
```

1. A **strength** is a value of `Strength`. The order is `HighZ < Weak < Pull < Strong < Supply`.
2. **High impedance** is the value `Driven::Z`: strength `HighZ`, level Z. It is the drive of a module that is not driving. A drive that is `HighZ` is a drive that is present; a drive that is absent is not received by `resolve` at all (§4 rule 15).
3. `Driven` reads as a `Logic` through `.v` and as a `Strength` through `.s`. A reader tells *undriven* (`Driven::Z`), *contended* (X at a strength) and *kept* (a `Weak` level) apart by `.v` and `.s` (Appendix F.4).
4. A strength is a field of the type driven, never a state of the logic and never a second trait: a nature that resolves by strength is `impl Resolve<Driven> for N`, and a nature that resolves any other pair (a level and a delay, a level and a source) declares its own driven type the same way.

#### 22.3.2 `Driven::strongest`

```rust
impl Driven {
    /// The strongest drive wins; equal strongest drives that disagree give X. A HighZ drive is left out.
    pub fn strongest<const N: usize>(d: [Driven; N]) -> Driven {
        d.iter().filter(|x| x.s != Strength::HighZ).fold(Driven::Z, |a, x| {
            if x.s > a.s {
                x
            } else if x.s < a.s {
                a
            } else if same(a.v, x.v) {
                a
            } else {
                Driven { v: Logic::X, s: a.s }
            }
        })
    }
}
```

| Drives | `strongest` |
| --- | --- |
| none, or all `HighZ` | `Driven::Z` |
| one strongest drive | that drive |
| several of the greatest strength, same level | that level, that strength |
| several of the greatest strength, levels differ | X, that strength |

1. The result does not depend on the order of the drives. `strongest([x, x])` equals `x`.
2. A conflict is a value: two `Strong` drives that disagree give `Driven { v: Logic::X, s: Strength::Strong }` and nothing stops. A loop of drives that never settles is an instability, which is an error (`E0511`, §8.2.3).
3. A net nature uses it as its `impl`: `impl Resolve<Driven> for Sda { fn resolve<const N: usize>(drives: [Driven; N]) -> Driven { Driven::strongest(drives) } }`.
4. `w.others()` of such a nature is `strongest` over every drive but this module's own (§5.2).

#### 22.3.3 Why five strengths

The set of `Strength` is the smallest under which every module of this section keeps its meaning: a pull-up that an open-drain driver overrides, a keeper that a pull-up overrides (`Weak` under `Pull`), and a rail that a gate cannot fight (`Supply`). Because a net is a nature the model declares and `Resolve` is generic over the driven type, the set is only what the shipped `Driven`, `strongest` and modules use; a model that needs more declares its own enum and nature.

The literature, as far as it could be confirmed against its document:

- IEEE 1364-2005 defines eight strength levels, `supply`, `strong`, `pull`, `large`, `weak`, `medium`, `small` and `highz`; the Verilog-AMS LRM 2.4 (§9.22.3, `$driver_strength`, Figure 9-3) maps them to the coding of IEEE 1364-2005 subclauses 7.10 and 7.11, and lets a connect module read the strength of each driver of a signal (§9.22.1 to §9.22.3). `large`, `medium` and `small` are the strengths of charge storage; the five of this section are Verilog's drive strengths without it.
- IEEE 1800 §6.6.7 declares user-defined net types whose resolution function the user names; that is the closest precedent for a net nature with a user `impl Resolve`.
- IEEE 1164 (`std_ulogic`) has nine values: three forcing, three weak, `Z`, `U` and `-`, and a resolved subtype whose resolution is an ordinary function. It has one weak level, enough for a pull-up and no more.
- The ngspice manual (§8.6.1, the XSPICE digital node type) has twelve values, the states 0, 1 and U each at a strength strong, resistive, high impedance or undetermined; its `real` node type (§8.6.2) resolves by summing, as `sum` below does.

Of these, only Verilog passes two real levels, and its extra three exist for charge storage and resistive switches, which no module here models.

#### 22.3.4 `sum`

A real-valued net whose drivers add: the function of the `impl Resolve<f64>` of a summing nature the model declares.

```rust
pub fn sum<const N: usize>(d: [f64; N]) -> f64 {
    d.iter().sum()
}
```

1. The drives are added in the order of their instance paths (§4 rule 2). The result is a function of the source.
2. With no drive the value is `0.0`.

#### 22.3.5 `Tribuf`, `Pullup`, `Pulldown`

Each module takes a net of any nature that implements `Resolve<Driven>`: `W: Nature + Resolve<Driven>`.

```rust
pub module Tribuf<W: Nature + Resolve<Driven>>(w: W, in d: Bit, in oe: Bit) {
    w = if oe { Driven::strong(d) } else { Driven::Z };
}

pub module Pullup<W: Nature + Resolve<Driven>>(w: W) {
    w = Driven::pull(true);
}

pub module Pulldown<W: Nature + Resolve<Driven>>(w: W) {
    w = Driven::pull(false);
}
```

| Module | Drive |
| --- | --- |
| `Tribuf`, `oe` high | `Driven::strong(d)` |
| `Tribuf`, `oe` low | `Driven::Z` |
| `Pullup` | `Driven::pull(true)`, constant |
| `Pulldown` | `Driven::pull(false)`, constant |

1. Each module holds one drive of its net, a definition (§8.2).
2. A pull wins only while every stronger drive is released (Appendix E.7).

#### 22.3.6 `Keeper`

A bus keeper (Verilog `trireg`): holds the last driven level, weakly.

```rust
pub module Keeper<W: Nature + Resolve<Driven>>(w: W) {
    signal last: Bit<Logic> = Logic::X;
    w = Driven { v: last, s: Strength::Weak };
}

digital for Keeper {
    on w.changed() {
        let seen = w.others().v; // what the others drive, without this keeper
        if seen.to_bit().is_some() {
            last <- seen; // remember only a 0 or a 1
        }
    }
}
```

1. The keeper records the level the other drives give whenever the net changes and that level is 0 or 1.
2. Its own drive is `Weak`, and `others()` leaves it out, so it never records itself.
3. `last` starts at X. Until a 0 or a 1 has been seen the keeper drives a weak X: the model says it has never been driven, which a `bool` could not.

#### 22.3.7 `Tran`

A bidirectional pass gate (Verilog `tranif1`).

```rust
pub module Tran<W: Nature + Resolve<Driven>>(a: W, b: W, in en: Bit) {
    a = if en { b.others() } else { Driven::Z };
    b = if en { a.others() } else { Driven::Z };
}
```

1. While `en` is high, each side is driven with what the rest of the other side drives.
2. `others()` excludes this gate's own drive. The gate does not latch itself.
3. Two nets joined by `Tran(a, b, true)` carry one value. They stay two carriers, each with its own `changed()`.
4. Gates in parallel, in a ring or in a mesh form loops among drives of resolved nets. They settle by relaxation (§8.2.3).
5. In such a loop strengths only rise from pass to pass. `Driven::strongest` reaches its fixed point within the passes §8.2.3 gives.

#### 22.3.8 `Tgate`

A CMOS transmission gate at switch level: `tranif1` and `tranif0` in parallel.

```rust
pub module Tgate<W: Nature + Resolve<Driven>>(a: W, y: W, in s: Bit, in sn: Bit) {
    let n = Tran(a, y, s);
    let p = Tran(a, y, !sn);
}
```

1. The gate conducts while `s` is high or `sn` is low.
2. The two `Tran`s form a loop among drives of `a` and `y`. It settles by relaxation (§22.3.7 rule 4).

`Bidir` (§22.4.3) and `TribufDelayed` (§22.5.9) are the other modules of this kind.

#### Per-analysis behavior of §22.3

Every item of §22.3 is discrete. Its behavior is the first row of the second table of §16.2.

| At instants | Where time does not advance | Transient | Small-signal sweep |
| --- | --- | --- | --- |
| definitions, `resolve`, `others()`, the reaction of `Keeper` | at the microsteps of `(0, ·)` and of each sweep point | at every instant | none |

---

### 22.4 Converters

A **converter** is a module with at least one discrete port and one node, which carries a signal from one kind to the other.
Discrete carriers and nodes never connect, and no converter is inserted implicitly (§6.1).

#### 22.4.1 `D2A`

`Bit` to node: a level through a resistance, with finite edges.

```rust
pub module D2A(in d: Bit, a: Electrical, vdd: Electrical) {
    rout: f64 = 100.0;
    tr: f64 = 20p;
    let ramp = transition(
        if d {
            1.0
        } else {
            0.0
        },
        0.0,
        tr,
        tr,
    );
}

analog for D2A {
    i(a) +> (v(a) - ramp * v(vdd)) / rout;
}
```

| `d` | Target potential of `a` |
| --- | --- |
| high | `v(vdd)` |
| low | `0` |

A model whose input is a `Logic` writes its own converter with `to_volts` (§22.1.1 rule 5): X and Z then read as mid-rail.

> **Resolution.** `rout` defaults to `100.0`. The design writes `100`, an integer literal, which is a type error for an `f64` param (Appendix A.2).

1. The output is a source of `ramp · v(vdd)` behind `rout`.
2. `ramp` moves between levels in `tr`, by `transition` (§22.2.11). Both corners are time points.
3. `vdd` is probed only. `D2A` draws no flow from it.

| In `analog` | Equilibrium solve | Transient step | Jump solve | Small-signal sweep |
| --- | --- | --- | --- | --- |
| `ramp` | the level of `d` | the ramp | the value at `now()` | constant |
| the contribution | a conductance `1/rout` from `a` to the target | the same | the same | `1/rout` at `a`; gain `ramp` from `v(vdd)` |

#### 22.4.2 `A2D`

Node to `Bit`, with hysteresis; right from the operating point.

```rust
pub module A2D(a: Electrical, out d: Bit = false) {
    vth: f64 = 0.6;
    hyst: f64 = 50m;
}

digital for A2D {
    on above(v(a) - vth - hyst / 2.0) {
        d <- true;
    }
    on above(vth - hyst / 2.0 - v(a)) {
        d <- false;
    }
}
```

1. `d` goes high when `v(a)` rises through `vth + hyst/2`, and low when it falls through `vth − hyst/2`.
2. `d` starts `false`. `above` corrects it at `(0, 0)` when `v(a)` already exceeds the upper threshold.
3. `A2D` has no `analog` block. It loads the node with nothing.
4. Each crossing is located (§17.3) and is a time point.

| At instants | Where time does not advance | Transient | Small-signal sweep |
| --- | --- | --- | --- |
| the two reactions | at `(0, 0)`; then when a threshold is passed between successive solves or sweep points | located in time | none: `d` is constant |

#### 22.4.3 `Bidir`

A pad: one resolved net, one node.

```rust
pub module Bidir<W: Nature + Resolve<Driven>>(d: W, a: Electrical, vdd: Electrical) {
    rout: f64 = 100.0;
    tr: f64 = 20p;
    vth: f64 = 0.6;
    hyst: f64 = 50m;
    signal seen: Bit = false; // the node, thresholded
    let driving = d.others().s != Strength::HighZ;
    d = if driving {
        Driven::Z
    } else {
        Driven::strong(seen)
    };
    let ramp = transition(d.others().v.to_volts(1.0), 0.0, tr, tr);
}

analog for Bidir {
    if driving {
        i(a) +> (v(a) - ramp * v(vdd)) / rout;
    } // released: no contribution, zero flow
}

digital for Bidir {
    on above(v(a) - vth - hyst / 2.0) {
        seen <- true;
    }
    on above(vth - hyst / 2.0 - v(a)) {
        seen <- false;
    }
}
```

| The rest of the net | Pad drives the node | Pad drives the net |
| --- | --- | --- |
| drives (`others().s != HighZ`) | as `D2A`, from `others().v` (X and Z read as mid-rail) | `Driven::Z` |
| has released | no contribution: zero flow (§11 rule 4) | `Driven::strong(seen)` |

1. `driving` is a `bool` over a discrete carrier. It is a legal selector of the branch `(a, gnd)` (§11 rule 3).
2. A change of `driving` is read by `analog`: a time point and a jump.
3. A pad faces a driver, not another pad. Two `Bidir`s alone on one net each drive because the other does not.
4. That loop has no fixed point: the run stops with `E0511`, naming the loop's drives (§8.2.3).

Per-analysis behavior is that of `D2A` while `driving`, and of `A2D` for `seen`.

---

### 22.5 Monitors

A **monitor** is a module that watches carriers and reports a temporal property with `require`. No syntax is needed beyond reactions.

#### 22.5.1 Rules common to all monitors

1. A monitor only reports. It never writes what it watches (§5.2).
2. Each check is a record on the `checks` stream (§22.6.3), written from a reaction (§19 rule 4).
3. A condition is a `bool`, so a check is a pass or a failure (§19 rule 1).
4. A model that turns a violation into X reads the monitor's output: `q <- if viol { Logic::X } else { Logic::from(d) };`. The injection is opt-in and visible in the netlist.
5. No monitor fires at `(0, 0)`. Each reads its history there before the first commit is recorded; `since` is still `inf`.
6. From then on `Setup` and `MinPulse` measure from that first commit, as IEEE 1364 timing checks do.
7. Where time does not advance, a monitor runs at the microsteps of `(0, ·)` and of each sweep point. In a small-signal sweep it does not run.

| Monitor | Check name | Property |
| --- | --- | --- |
| `AckWithin` | `ack_within_n` | every `req` is acknowledged within `n` clock cycles |
| `Never` | `never` | `bad` is never true at a clock edge |
| `Stable` | `stable` | `d` does not change while `en` is high |
| `Setup` | `setup` | `d` was stable for `ts` before each rising edge |
| `Hold` | `hold` | `d` holds for `th` after each rising edge |
| `MinPulse` | `min_pulse` | no pulse on `d` is shorter than `w` |

#### 22.5.2 `AckWithin`

```rust
pub module AckWithin(in clk: Bit, in req: Bit, in ack: Bit) {
    n: u32 = 4;
    signal waiting: Bit<u32> = 0; // cycles since an unanswered req; 0 is idle
}

digital for AckWithin {
    on clk.rise() {
        if ack {
            waiting <- 0;
        } else if waiting > 0 || req {
            require("ack_within_n", waiting < n);
            waiting <- waiting + 1;
        }
    }
}
```

1. `waiting` counts the rising edges since an unanswered `req`.
2. The check fails at the first edge where `waiting` has reached `n` with no `ack`.

#### 22.5.3 `Never`

```rust
pub module Never(in clk: Bit, in bad: Bit) {}

digital for Never {
    on clk.rise() {
        require("never", !bad);
    }
}
```

1. `bad` is sampled at each rising edge, with the value before the edge's own commits (§12.1.1).

#### 22.5.4 `Stable`

```rust
pub module Stable<T: Repr>(in en: Bit, in d: Bit<T>) {}

digital<T> for Stable<T> {
    on d.changed() && en && !en.changed() {
        require("stable", false);
    }
}
```

1. A change of `d` while `en` is high is a violation.
2. The microstep in which `en` itself changes does not count.

#### 22.5.5 `Setup`

```rust
pub module Setup<T: Repr>(in clk: Bit, in d: Bit<T>, out viol: Bit = false) {
    ts: f64 where ts > 0.0;
    let age = since(d);
}

digital<T> for Setup<T> {
    on clk.rise() {
        require("setup", age >= ts);
        viol <- age < ts;
    }
}
```

1. At each rising edge the time since `d` last changed **MUST** be at least `ts`.
2. A `d` that changes in the microstep of the edge was stable before it: `since` has not yet recorded that change (§22.1.8 rule 2).
3. `viol` is high from one microstep after a violating edge until one after the next clean edge.

#### 22.5.6 `Hold`

```rust
pub module Hold<T: Repr>(in clk: Bit, in d: Bit<T>) {
    th: f64 where th > 0.0;
    let high_for = since(clk); // clk high: its last change was a rise
}

digital<T> for Hold<T> {
    on d.changed() && clk {
        require("hold", !clk.changed() && high_for >= th);
    }
}
```

1. A change of `d` while `clk` is high **MUST** come at least `th` after the rise.
2. A change in the edge's own microstep is a violation.
3. `Hold` assumes `th` is shorter than the high phase of `clk`.
4. The initial value of `d` is committed at `(0, 0)`, where `clk` has not yet risen (§22.5.1 rule 5), so it is not a violation.

#### 22.5.7 `MinPulse`

```rust
pub module MinPulse(in d: Bit) {
    w: f64 where w > 0.0;
    let width = since(d);
}

digital for MinPulse {
    on d.changed() {
        require("min_pulse", width >= w);
    }
}
```

1. Each change of `d` **MUST** come at least `w` after the previous one.
2. A `d` that changes again before `w` has passed since time 0 is a short pulse (§22.5.1 rule 6).

#### 22.5.8 `inertial`

An **inertial delay** passes a change of its input only once the input has been stable for the delay.

```rust
pub module inertial<T: Repr + Default>(in x: Bit<T>, tpd: f64) -> T {
    signal y: Bit<T>;
    signal seen: Bit<f64> = 0.0;
    y
}

digital<T> for inertial<T> {
    on x.changed() {
        seen <- now();
        if analysis(Static) {
            y <- x;
        }
    }
    on at(seen + tpd) {
        y <- x;
    } // re-armed by every change: only the last one survives
}
```

1. The result follows `x` once `x` has been stable for `tpd`. A pulse shorter than `tpd` does not pass.
2. Every change of `x` re-arms the `at` (§13 rule 4). Only the last pending change survives.
3. `tpd` is a value input. It **MAY** depend on the value being delayed.
4. The result is generic over `T`. It delays a `Bit` or the drive of a resolved net alike.
5. The result reads a discrete carrier only. It **MAY** be read in a definition (§8.2.5).
6. In an operating point there is no "before" to wait out: the result follows `x` at the next microstep.
7. The result starts at `Default::default()` of `T` (§3.3.4), which is why `T` is bound by `Default`: for `Driven` that is `Driven::Z`.

| At instants | Where time does not advance | Transient | Small-signal sweep |
| --- | --- | --- | --- |
| `inertial(x, tpd)` | follows `x` one microstep later, also through a `Dc` sweep | at `(0, ·)` follows `x`; afterwards `tpd` after `x` last changed | none |

This is the delay to use for logic that must follow a sweep (Appendix G.32).

#### 22.5.9 `TribufDelayed`

A tristate buffer with Verilog's inertial gate delay, slower to release than to drive.

```rust
pub module TribufDelayed<W: Nature + Resolve<Driven>>(y: W, in d: Bit, in oe: Bit) {
    ton: f64 = 100p;
    toff: f64 = 300p;
    y = inertial(
        if oe {
            Driven::strong(d)
        } else {
            Driven::Z
        },
        if oe {
            ton
        } else {
            toff
        },
    );
}
```

1. The drive of `y` is the definition's value, delayed inertially by `ton` while `oe` is high and by `toff` otherwise.
2. The first argument is an anonymous definition on the `in` port of `inertial` (§8.1).

---

### 22.6 Output and files

Functions over `emit!` (§19) and `include_str!` (§20). None is core, and there are no macros to define.

#### 22.6.1 `Severity`, `log`

```rust
#[derive(Repr, PartialEq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

pub fn log(severity: Severity, msg: str) {
    emit!("log", severity = severity, msg = msg);
}
```

1. `log` writes a record to the `log` stream of the calling instance (§19 rule 2).
2. `log` is callable wherever `emit!` is: reactions and `analog` (§3).
3. From `analog`, a `log` record is a trace. It is written at every evaluation, rejected iterations included (§19 rule 3).
4. A message with values in it is a record: `emit!("log", msg = "overrange", vin = v(inp));`. There is no format-string syntax.
5. The simulator's own records go to the same stream (§19 rule 2).

#### 22.6.2 `finish`, `fatal`

```rust
pub fn finish() {
    emit!("sim", finish = true);
}

pub fn fatal(msg: str) {
    emit!("sim", finish = true, failed = true, msg = msg);
}
```

1. Both write to the reserved global stream `sim`. The analysis ends at the instant of the record (§19 rule 5).
2. That instant is the last time point (§17.2.2).
3. `fatal` also marks the run as failed, with a message.
4. Both are callable from reactions only. A call from `analog` is an elaboration error (§19 rule 4).
5. Whether a run ended by `finish` can be continued is open (Appendix G.35).

#### 22.6.3 `require`, `cover`, `target`

```rust
/// A condition that must hold: a pass or a failure.
pub fn require(name: str, pass: bool) {
    emit!("checks", kind = "require", name = name, pass = pass);
}

/// A condition worth reaching; the host counts the hits.
pub fn cover(name: str, hit: bool) {
    emit!("checks", kind = "cover", name = name, pass = hit);
}

/// A number to keep at or above zero: an objective or a constraint of an optimization (§23.2).
pub fn target(name: str, margin: f64) {
    emit!("checks", kind = "target", name = name, margin = margin);
}
```

| Function | `kind` | Fields | Read by the host as |
| --- | --- | --- | --- |
| `require(name, pass)` | `"require"` | `name`, `pass` | a pass or a failure |
| `cover(name, hit)` | `"cover"` | `name`, `pass` | a hit count |
| `target(name, margin)` | `"target"` | `name`, `margin` | a margin, met at or above zero; the last record of a run is kept |

1. All three write to the reserved global stream `checks`. They are callable from reactions only (§19 rule 4).
2. A check is an ordinary reaction. It runs when its event fires and reads what that reaction reads (§15).
3. A record is its declared fields (§19 rule 1): a `require` is a pass or a fail, with no third verdict.
4. A reduction over an analysis is an explicit signal. Its `target` is re-emitted as it goes; the host keeps the last record (§19).
5. `target` margins are the objectives and constraints of an optimization (§23.2).
6. In a small-signal sweep no reaction runs, so no check is written (§16.2).

#### 22.6.4 `parse_table`, `parse_hex`

```rust
pub const extern "intrinsic" fn parse_table<Row, const N: usize>(text: str) -> [Row; N]; // Row: f64 or a tuple of f64

pub const extern "intrinsic" fn parse_hex<T, const N: usize>(text: str) -> [T; N]; // T: an integer type
```

1. Both are `const fn`s from text to an array. They run at elaboration; a run never reads a file (§20).
2. The array's element type and length are those of the `const` being initialized.
3. `parse_table` reads one row per line, columns separated by whitespace. Each row has one number per component of `Row`.
4. `parse_hex` reads the `$readmemh` format: hexadecimal words separated by whitespace.
5. Text that does not fit the `const`'s type, in its rows or in their number, is the elaboration error of §20.
6. Text that is not in the format at all is `E0706`.

> **Resolution.** The signatures are schematic: the design gives only "from text to an array whose length is the `const`'s type". The element type is taken from the `const` as well.

> **Resolution.** In `parse_table`, blank lines are skipped and `#` starts a comment that runs to the end of the line. Numbers are the language's real literals, SI suffixes included.

> **Resolution.** In `parse_hex`, `_` inside a word, `//` and `/* */` comments and `@address` records are accepted as in `$readmemh`. A digit `x` or `z` is `E0706`: the elements are integers, which hold no X or Z. An index the text does not write is a misfit under rule 5.

```rust,fragment
const IV:  [(f64, f64); 64] = parse_table(include_str!("diode_iv.tbl"));   // whitespace columns
const ROM: [u32; 1024]      = parse_hex(include_str!("boot.hex"));         // $readmemh format
```
