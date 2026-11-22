# Explicit Unknowns Specification

**Approved**: 2026-11-21 - user: "Aceito os recomendados" (Q4: no separate strength trait, `impl Resolve<Driven>` carries it; Q5: Strength set B, five levels HighZ < Weak < Pull < Strong < Supply), after R1–R10, Q1–Q3 and the ABI (major 1, edited in place) the same day; D1–D21 stand.

## Decision (user, 2026-11-21)

"Não acho que unknown deveria ser algo implícito, deveria ser algo EXPLICITO. Se o param não sofreu drive e pode iniciar sem default, ele é Option. Se o registor inicia sem valor, caso ele seja Bit = Erro, caso ele seja Logic inicia com X em todos os bits (usando o Default -> tem Default, pode não ser iniciado, não tem default, tem que ser especificado). Esse é o ponto que entra na semantica do unknown que parece um 'caso específico' que eu odeio. Quem vai criar o modulo tem que entender que UInt pode ser unknown e f64 não, etc... ao invés de especificar Option<UInt> e Option<f64> ou Logic<64> ou até Option<Logic<64>> que pode ser inteiro None ou inteiro de 4 estados. [...] Mesmo que o uso de Bit em wires e drivers seja algo super normal, tudo tem que ser explícito."

"Bit é binary digit, não tem outros. Logic pode ser 0, 1, Z ou X. Merge bit a bit entre Logic e Bit é possível."

On resolved nets the user asked: "O resolve vai ser um método do nature ou um módulo atrelado?" (answered in Q1). On the Book: "Aproveita que precisa mudar o livro também".

### Answers (user, 2026-11-21, to the first version of this spec)

"Q1 -> Permitir impls em natures é uma boa... Ficaria abc : Electrical e poderia chamar abc.func() como uma criação de um tipo... Aí tira-se o campo e passa a exigir que nós discretos só podem ser ligados a multiplos outputs com a trait Resolve e podemos pensar em diversas outras possibilidades com trait. Q2. Faz sentido. Inclusive, se vamos exigir que a nature tenha o Resolve, podemos exigir que outra trait ResolveWithStrength (nao precisa ser esse nome btw) diga que o resolve recebe o Logic + Strength. Recomendo que resolve seja até genérico para Resolve<X: Repr>. O Strength, gostaria que colocasse na literatura quais valores são usados em diversos modelos para ver qual opção melhor. E gostaria de exercitar como seria ligar a um signal com strength. Outra coisa interessante que estou pensando: pq tem Wire? Pq eu simplesmente não declaro a Nature ABC com um resolve específico e uso o tipo dela como normal? Inclusive, o Resolve<X: Repr> poderia ser impl Resolve<Bit> for AndWire { ... } e deixaria fazer o drive de um bit nesse wire... Traits como CAPACIDADES. Q3. Sim, instabilidade é erro"

"ABI pode continuar major 1 pq não fizemos release ainda"

What they settle:

| Answer | Effect |
| ------ | ------ |
| Q1 = B, extended | A nature takes trait `impl`s. The `resolve` field and the contextual keyword `resolve` are removed. A discrete carrier takes more than one drive only when its nature implements `Resolve`. **Traits are capabilities** (D17). |
| `Resolve<X: Repr>` | The trait is generic over the driven type, so a module drives a plain `bool` into a wired-AND net (D18, D19). |
| "pq tem Wire?" | The library declares no net nature. A user declares a nature and gives it the capability (D20). |
| Q2 | Strengths are kept. Their set is Q5, with the literature the user asked for; whether they need a trait of their own is Q4. |
| Q3 = A | A loop of resolved drives that does not settle is the run error `E0511`. |
| ABI | The ABI stays MAJOR 1: it is unreleased and is edited in place (D13). |

The approved rules, binding:

| # | Rule |
| - | ---- |
| R1 | No value carries a hidden known flag. `bool`, `UInt<N>`, `Int<N>`, `Float<N>`, enums, structs, tuples and arrays are always known. |
| R2 | `Logic` is a standard-library type with four states, 0, 1, Z and X; `Logic<N>` is a vector of `N` of them. Its operators (`&`, `\|`, `^`, `!`, shifts, the bitwise operators between a `Logic` and a binary value) follow four-state tables and are ordinary library `impl`s, never language rules (CC-11). |
| R3 | Absence is `Option<T>`. `Option<Logic<64>>` is wholly `None` or a present four-state vector. |
| R4 | A carrier with no initial value is legal only when its type implements `Default`, and starts at `Default::default()`. `Logic` (X on every level), `Option` (`None`) and a type deriving `Default` whose fields all have it implement it. No primitive numeric type does (`bool`, `UInt`, `Int`, `Float`); the specification says this differs from Rust. A missing initial value on a type without `Default` is an elaboration error naming the carrier and the trait. |
| R5 | `Logic == Logic` returns `Logic` (X when either side has X or Z on a compared level). `if`, `while` and guards accept `bool` only, so deciding on a `Logic` is written: `match`, `.is_high()`, or `same(a, b)`. |
| R6 | A binary value converts to `Logic` totally (`From`); `Logic` to a binary value gives an `Option` (`None` for X or Z). The analog side reads a `Logic` only through a function; `bool`, integer and float carriers are read directly, with no `known()` gate. |
| R7 | Part I §3.3 as it stands is removed (the known flag, the literal `Unknown`, `level(k)`, `known`/`known_or`/`is_known`, the merge of effects under unknown control, `emit!`'s `maybe`, the unknown index, flag-total functions, the gate of §3.3.5), with what the events part built on it. §3.3 is rewritten as the explicit model. |
| R8 | Resolved nets: `resolve` is a pure function of the drives at one instant, never a module. It is the function of the trait `Resolve`, which a nature implements (answer to Q1). Z is a value a drive holds, conflicts give X. `resolve` stays instantiated for every drive count `0..=N`. |
| R9 | ABI: the per-value known byte goes; `Option` keeps its presence byte; `Logic` takes two bits per level. The ABI stays MAJOR 1 (user: "não fizemos release ainda"). |
| R10 | The feature `unknown-semantics-per-level` (its `KNOWN_LEVELS` flag, branch `feature/unknown-semantics-per-level`, never merged) is dropped. `GAPS.md` S6 and G6 close by this redesign. |

How the user's words map to the specification's names: the user's `Bit` value is `bool`, the value a `Bit` carrier holds (`nature Bit<T: Repr = bool>`, §4 rule 12); `UInt`, `Int` are `UInt<N>`, `Int<N>`; `f64` is `Float<64>`.

## Problem Statement

Part I §3.3 gives every level of every value a hidden known flag. A module author must know, without the type saying so, that a `u8` carrier may hold "no value", that an `if` on it runs both arms and merges them, that `analog` must read it through `known_or`, and that a record may arrive with `maybe = true`. The implementation carries that through every layer: `ValueType::Flagged` in the IR, one known byte per value in the ABI, a merge runtime in the generated code, `commit_with` in the device contract. It still disagrees with the text (`GAPS.md` S6: one flag per value where the text says one per level), and the feature planned to close that gap, `unknown-semantics-per-level`, would have doubled the storage of every carried value.

The user's decision removes the idea instead of finishing it. What a type can hold is what its declaration says: a binary type holds binary values, `Option<T>` says "may be absent", `Logic` says "may be X or Z". This feature amends the specification and the Book first (on `main`, as `value-semantics` did), then brings the front end, the IR, the plan, the ABI, the generated code, the engines, the host, the standard library, the examples and the tests to the amended text.

## Goals

- [ ] The specification and the Book state the explicit model, and no rule of either mentions a known flag, `Unknown`, `level`, `known_or`, `maybe` or a merge under unknown control.
- [ ] A model whose carriers all have initial values and binary types runs with the results it gave before; the 7 ngspice twins print the same bytes.
- [ ] The IR, the plan, the ABI, the generated code and the engines hold no flag, no merge and no `commit_with`.
- [ ] `Logic<N>` runs from the standard library's own source with the four-state tables of EU-08.
- [ ] `GAPS.md` S6 and G6 are closed, `unknown-semantics-per-level` is recorded as dropped, and MD-70 is recorded as superseded.

## Out of Scope

| Item | Where it goes |
| ---- | ------------- |
| Running a nature that implements `Resolve` (`others()`, relaxation from source), and `impl Trait for` a nature in the checker | `generics` (the `impl` and the bound) and `resolved-nets` B3 onward, with the amendments of design.md. This feature writes the specification, the Book and the library text |
| Turning other nature rules into capability traits | inventoried in design.md "Capabilities later"; none is built here |
| Bundles | `resolved-nets-bundles` B2 onward |
| Generic functions, `impl`, `self`, traits, generic structs | `generics` phase A. Phase B of this feature (EU-07 to EU-10) starts on the tree that has it |
| Integer, enum and `Option` params on the host | `params-and-io` B5, with the amendments of design.md |
| `Pfd3` and functions that return `Event` | `unknown-semantics-events` B5 |
| A nine-state logic type (IEEE 1164 `U`, `W`, `L`, `H`, `-`) | not asked; a strength is a field of the driven type (Q4, Q5) |
| Arithmetic on `Logic` (`+`, `-`, `<`) | not asked: a `Logic` is converted first (`to_bits()`), which gives an `Option` |
| The interpreter (`piperine-interp`) | frozen at the M1 analog subset; it loses the flag forms it lists as refused, nothing more |
| The language server | deferred by the user |

**Implicit-requirement dimensions.** Input validation: EU-02, EU-03, EU-04 (every removed name and every missing initial value has a diagnostic). Failure states: EU-11 AC 4 (a loop that does not settle), EU-04 AC 4 to 6. State-transition integrity: EU-13 (what a change is, what `prev()` holds before the first one). Data lifecycle: EU-04 (a carrier's value exists from `(0, 0)`). Observability: EU-15 (records and host cells). Migration: EU-20 (every source in the tree moves in the feature that refuses the earlier form). Idempotency, auth, rate limits, external dependencies: N/A because a compiler and a deterministic simulator have none. Concurrency and ordering: N/A because this feature changes no rule of §15; the order of commits and microsteps is untouched.

---

## Assumptions & Open Questions

The planner's decisions where the approved rules left a choice. Each is the simplest reading that follows CC-11 and the rules above, and each waits for the user's confirmation with the plan.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| D1: the shape of `Logic<N>` | A library struct of two integer planes, `pub struct Logic<const N: usize = 1> { pub binary: UInt<N>, pub value: UInt<N> }`. Level `k` is 0 when `binary` is 1 and `value` 0; 1 when both are 1; X when both are 0; Z when `binary` is 0 and `value` 1. `Logic` alone is `Logic<1>` | An ordinary struct: its `Repr` layout, its ABI layout and its copy follow from the rules every struct has. Two bits per level, packed (R9). All-zero is X, so `Default` is the zero value. `[Logic; N]` of an enum would cost one byte per level and need element loops for every operator | y - no objection, 2026-11-21 |
| D2: how an operator reaches a library type | Operator traits in Appendix H (new H.10), each a capability in the sense of D17 (a type has `&` when it implements `BitAnd`): `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr` with parameters `<Rhs = Self, Out = Self>`, `Not<Out = Self>`, and `PartialEq<Rhs = Self, Out = bool>`. `a & b` is `BitAnd::bitand(a, b)` of the one `impl` for the two operand types. The built-in types implement them with the meaning §1.5 already gives | R2 and R5 demand it: `&` on `Logic` and an `==` that returns `Logic` cannot be library `impl`s without it. A trait holds no type (§3.7.1), so the result type is a trait parameter | y - no objection, 2026-11-21 |
| D3: `From` | `trait From<T> { fn from(x: T) -> Self; }` joins H.8. `Logic<N>: From<UInt<N>>`, `Logic<1>: From<bool>` | R6 names `From`; the specification has no such trait today | y - no objection, 2026-11-21 |
| D4: which built-in types implement `Default` | `()`, `Option<T>` (`None`), a tuple and an array `[T; N]` when every part does. `bool`, `UInt<N>`, `Int<N>`, `Float<N>` and `str` do not. An enum still does not derive it; it may implement it by hand | R4; tuples and arrays follow the rule a derived struct follows | y - no objection, 2026-11-21 |
| D5: the code of a missing `Default` | `E0187` gains three conditions: a carrier with no initial value whose type does not implement `Default`; `#[derive(Default)]` on a type with a field that does not; `T::default()` on a type that does not. No new code | `E0187` is the code of "a type does not satisfy a trait"; the fewest codes | y - no objection, 2026-11-21 |
| D6: retired codes | `E0150`, `E0151`, `E0906`, `E0908` are retired and never reused. `E0604` loses its `maybe` condition | their rules are gone | y - no objection, 2026-11-21 |
| D7: `Unknown`, `level`, `known`, `known_or`, `is_known`, `high`, `merge` in a source | Names that do not resolve: `E0161` by the rule every unknown name follows. `Unknown` stops being a token. The diagnostic's help names what to write (an initial value, `Option<T>`, `Logic`) | no special case; the same choice as `Clone`/`Copy` in `value-semantics` D1 | y - no objection, 2026-11-21 |
| D8: the first commit and `prev()` | The initial value, written or `Default::default()`, is committed at `(0, 0)` and that commit is a change (§13 rule 6 kept: `changed()` fires there, `init()` rests on it). Before any later change `prev()` equals the initial value, as a param's already does | every carrier has a value from `(0, 0)`; there is nothing else `prev()` could hold | y - no objection, 2026-11-21 |
| D9: edges | On a `Bit`: `rise()` is `changed() && !prev() && value`, `fall()` its mirror. On a `Bit<Logic>`: `rise()` is a change that ends at 1 from 0, X or Z; `fall()` one that ends at 0. A carrier that starts high has not risen at `(0, 0)`. **This changes behavior**: today a `Bit` that starts high rises at `(0, 0)` (§22.1.5 rule 3) and models filter it with `&& x.prev().is_known()` | "Bit é binary digit": a rise is 0 to 1. The filter disappears with the flag; a model that wants an action at the start writes `init()` | y - no objection, 2026-11-21 |
| D10: `cast` | `cast<A: Repr, B: Repr>(a: A) -> Option<B>` is `B::decode(a.encode())`, an ordinary function | a `decode` that fails has no "unknown" to return | y - no objection, 2026-11-21 |
| D11: `same` | `same<T: Repr>(a: T, b: T) -> bool` is `a.encode() == b.encode()`: equal level by level. On `Logic` it compares the four states literally | R5; one generic definition, no rule for `Logic` | y - no objection, 2026-11-21 |
| D12: `tri` (re-derived) | `Tri<T>`, `tri` and the planned `Tristate<T>` leave the library. A bus with one driver at a time is a user nature whose `impl Resolve<Option<T>, V>` returns the `V` its author chooses for a conflict (the gallery shows one) | with `Resolve<X, V>` the result type is the author's, so the library needs no conflict type of its own | n |
| D13: ABI version (user) | MAJOR stays 1, symbol `piperine_abi_1`. The contract is unreleased and is edited in place until the first release: `Field` becomes `{ name, leaf, offset, count }`, `commit_with` and `commit_flag` are removed, the shipped layouts lose the known byte. MINOR resets to 0 | user: "ABI pode continuar major 1 pq não fizemos release ainda". MINOR counts what was appended to the major's base; the one thing minor 1 appended (`commit_with`) is removed and the base itself is rewritten, so nothing is appended. A compiled model in the cache is keyed by the contract's source, so one built before the edit is rebuilt, never loaded | y |
| D14: the `driving` field of a drive pin (abi.md D1, re-derived) | It stays, as a plain `BOOL` after the bytes of the **driven type `X`**: 0 is a drive absent from what `resolve` receives (relaxation step 1), 1 a present drive. The value pin has the layout of the nature's `value` type `V`. It is not Z and not `Strength::HighZ`: those are values a present drive holds | `resolve` is generic over `X`; a `bool` driven into a wired-AND net has no "not driving" value. Absence during relaxation is the engine's, R8's "every drive count `0..=N`" | n |
| D15: how the host prints | A `Logic<N>` cell is a pattern of `0`, `1`, `x`, `z`, highest level first (`10xz`), in tables, records and JSON Lines. `None` prints `None`. No cell is "unknown" | the Verilog convention; the answer Q2 of `unknown-semantics-per-level` had recommended | y - no objection, 2026-11-21 |
| D16: queue and phases | The amendment (spec and Book) merges to `main` first. Phase A (flags out of every layer) runs right after `composite-values` merges, before `generics`. Phase B (`Logic` from library source, operator traits) runs after `generics` phase A is on `main` | `Logic<N>` is a generic struct with trait `impl`s: it needs what `generics` builds. An interim intrinsic `Logic` would be a special case to delete later | y - no objection, 2026-11-21 |
| D17: traits are capabilities; a nature takes trait `impl`s | §3.1.9 and `E0134` are narrowed to bundles: `impl Tr for N` is accepted for a nature `N`. In it `self`, where a function has one, is a handle (§3.1.1), and the `impl` holds what the trait declares. A trait a nature implements is a **capability** of every carrier of that nature: a rule that used to ask "which kind of nature is this" asks "does it implement the trait". A generic parameter bounds a nature by its capabilities, `W: Nature + Resolve<Driven>` | user: "Permitir impls em natures é uma boa", "Traits como CAPACIDADES". What `E0134` protected: a trait function's `self` was assumed to be a value and a bound to stand for a value type. Handles (§3.1.1) and the `Nature` bound (§4 rule 11) already answer both | y - user |
| D18: the shape of `Resolve` | `pub trait Resolve<X: Repr, V: Repr = X> { fn resolve<const N: usize>(drives: [X; N]) -> V; }`. `X` is the type a module drives; `V` **MUST** be the nature's `value` type (`E0203`). `impl Resolve<bool> for AndWire` reads as the user wrote it. A nature may implement it for several `X` | user: "Resolve<X: Repr>", "impl Resolve<Bit> for AndWire". A trait holds no type (§3.7.1), so the result type is a parameter, as in D2; its default keeps the common case one argument long | n |
| D19: how a drive picks its `impl` | By the type of the value it drives. Every drive of one carrier has the same `X`; two driven types on one carrier are `E0225` naming both modules; a drive whose type no `impl` accepts is `E0180`; a literal that fits several is `E0181` | one `resolve` call combines one array type. Mixing would need a conversion nobody wrote | n |
| D20: the library's nets | No library net nature: `Wire`, `Tri<T>` and `SumNet` are removed. The library keeps `Strength`, `Driven`, the resolution function `Driven::strongest`, `sum`, and the modules `Tribuf`, `Pullup`, `Pulldown`, `Keeper`, `Tran`, `Tgate`, `Bidir`, `TribufDelayed`, each generic over `W: Nature + Resolve<Driven>`. A user writes `pub nature Sda { value: Driven }` and `impl Resolve<Driven> for Sda { .. }` | user: "pq tem Wire? Pq eu simplesmente não declaro a Nature ABC com um resolve específico e uso o tipo dela como normal?". A library module asks for a capability, never for a library net | n |
| D21: one drive or many | A carrier whose nature does not implement `Resolve` takes one drive; a second is `E0220`, whose text now names the nature and `Resolve`. A carrier whose nature implements it takes any number, zero included, and its value is always `resolve` of its present drives. Its ports take no direction, as a resolved nature's do today | user: "nós discretos só podem ser ligados a multiplos outputs com a trait Resolve". `E0220` already is "a second drive"; only its message changes | y - user |
| Q4: does strength need a trait of its own | **Recommended: no.** `Resolve<X, V>` already says it: a strength-aware nature is `impl Resolve<Driven> for Sda`, whose drives are a level and a strength. Alternative: a second trait `ResolveStrength<X, V = X>` whose function receives `[(X, Strength); N]` | See "Q4" below | y - user 2026-11-21 "Aceito os recomendados": no |
| Q5: the set of `Strength` | **Recommended: B, five values** `HighZ < Weak < Pull < Strong < Supply` (today's set). A: three (`HighZ`, `Weak`, `Strong`). C: the eight of IEEE 1364 with charge storage | See "Q5" below | y - user 2026-11-21 "Aceito os recomendados": B |

**Open questions:** none. Q1 to Q5 are answered (Q4 and Q5: user, 2026-11-21, "Aceito os recomendados").

### Q4: a second trait for strength?

The user offered "outra trait ResolveWithStrength ... diga que o resolve recebe o Logic + Strength" and, in the next sentence, "Recomendo que resolve seja até genérico para Resolve<X: Repr>". The second makes the first unnecessary, so the plan takes one trait and puts the choice in front of the user.

| | A: one trait (recommended) | B: a second capability |
| - | - | - |
| Written | `impl Resolve<Driven> for Sda { fn resolve<const N: usize>(d: [Driven; N]) -> Driven { Driven::strongest(d) } }` | `impl ResolveStrength<Logic> for Sda { fn resolve<const N: usize>(d: [(Logic, Strength); N]) -> (Logic, Strength) { .. } }` |
| A drive | `sda = Driven::pull(true);` | `sda = (Logic::H, Strength::Pull);` |
| What the language adds | nothing: the strength is a field of the driven type | a second trait the elaborator must look for, a rule for a nature that implements both, and the same information as a tuple instead of a struct |
| Other capabilities later | a nature that resolves any other pair (a level and a delay, a level and a source id) writes its own `X` the same way | each such pair would ask for one more trait |

No syntax marks a drive with a strength apart from its value, so in both options the strength travels in what is driven. B changes its spelling only.

### Q5: which strengths? The literature

Written from the standards and tool documentation as the planner knows them; task T7 checks each citation against its document before the text lands in Part VII.

| Model | Values | What they are for |
| ----- | ------ | ----------------- |
| Verilog, IEEE 1364-2005 §7.9 to §7.13; SystemVerilog, IEEE 1800-2017 §28.11 to §28.15 | Eight levels: `supply` (7), `strong` (6), `pull` (5), `large` (4), `weak` (3), `medium` (2), `small` (1), `highz` (0). `supply`, `strong`, `pull`, `weak` are drive strengths of gates and continuous assignments; `large`, `medium`, `small` are charge-storage strengths of `trireg` nets. Each level pairs with 0 or 1; a net may hold an ambiguous range | Switch-level MOS modeling: a resistive switch (`rtran`, `rnmos`) lowers a strength by a table; a `trireg` keeps charge when undriven; `wand`, `wor`, `tri0`, `tri1` are net types with fixed resolutions |
| SystemVerilog user-defined nettypes, IEEE 1800-2012 §6.6.7 | Any user type, with a resolution function named in the `nettype` declaration | The standard's own answer to "why a fixed set": the user declares the net type and its resolution. It is the closest precedent for D17 to D20 |
| VHDL `std_logic`, IEEE 1164 (`std_ulogic`: `U X 0 1 Z W L H -`) | Nine values: two strengths (forcing `X 0 1`, weak `W L H`), `Z`, `U` (never assigned) and `-` (don't care, for synthesis) | Gate-level and RTL: pull-ups and open-drain buses need exactly one weak level; `resolved` is an ordinary VHDL function attached to the subtype, another precedent for a user resolution |
| Verilog-AMS LRM 2.4, mixed-signal chapter (connect modules, driver access functions `$driver_count`, `$driver_state`, `$driver_strength`) | The Verilog eight, read per driver | A digital-to-analog connect module reads each driver's strength to pick an output resistance. The strength is a property of a drive that the analog side reads: what `others()` and `Driven.s` give here |
| XSPICE and ngspice digital nodes (ngspice manual, the XSPICE digital node type) | Twelve states: level `0`, `1`, `U` times strength `s` (strong), `r` (resistive), `z` (high impedance), `u` (undetermined) | Mixed-mode simulation: three real strengths are enough for gates, pull-ups and tri-states; `u` is the resolver's "cannot tell" |
| SystemC `sc_logic`, IEEE 1666 (`sc_signal_resolved`, `sc_signal_rv`) | Four values `0 1 Z X`, no strength; a fixed resolution table | System-level models: tri-state buses only |
| Logisim | A wire is 0, 1, floating or error; a "Pull Resistor" component gives a floating wire a level | Teaching: one implicit weak level, and a conflict is an error value shown in red |
| Digital (H. Neemann) | A value and a high-impedance mask per bit; pull-up and pull-down components; two active outputs that disagree stop the simulation as a short circuit | Teaching: no X at all, a conflict is an error |

What the survey shows: every model has "not driving"; every model that has pull-ups has at least one level below the normal drive; only Verilog goes past two real levels, and its extra ones exist for charge storage and switch networks; the two languages that let users declare nets (VHDL, SystemVerilog nettypes) make the resolution an ordinary user function, which is the model the user chose here.

| | A: three | B: five (recommended) | C: eight |
| - | - | - | - |
| Set | `HighZ < Weak < Strong` | `HighZ < Weak < Pull < Strong < Supply` | `HighZ < Small < Medium < Weak < Large < Pull < Strong < Supply` |
| Follows | IEEE 1164, XSPICE | Verilog's drive strengths | Verilog whole |
| Pull-up against an open-drain driver | yes | yes | yes |
| A keeper that a pull-up overrides | no: both are `Weak`, they contend | yes: `Weak` under `Pull` | yes |
| A rail that a gate cannot fight | no | yes: `Supply` | yes |
| `trireg` charge storage, resistive switches | no | no | yes, with the reduction table and decay rules to specify |
| Cost | the smallest; `Keeper` needs another idea | today's library set, no migration | a table of reductions, three levels no shipped module uses |

Because nets are user natures and `Resolve` is generic over `X` (D18, D20), the library's set is only what the shipped `Driven`, `strongest` and modules use. A user who needs the eight Verilog levels declares an enum and a nature; nothing in the language stops at five. That is why B is enough: it is the smallest set under which every module the library ships (`Pullup`, `Keeper`, `Tribuf`, `Tran`) keeps its meaning.

---

## User Stories

### P1: A value is what its type says ⭐ MVP

**User Story**: As a module author, I want every value of a binary type to be a value, so that I never write a gate around a read and never ask whether an `if` ran one arm or two.

**Why P1**: It is the decision itself (R1, R7).

**Acceptance Criteria** (EU-01, no value carries a flag):

1. WHEN a reaction, a function or an `analog` block reads a carrier whose value type is `bool`, `UInt<N>`, `Int<N>`, `Float<N>`, an enum, a struct, a tuple or an array of those THEN the system SHALL yield the carrier's value, with no method between the read and its use.
2. WHEN an `if`, a `while`, a `match`, a guard or a short-circuit operand decides on a carrier's value THEN the system SHALL run exactly one path, the one the value selects.
3. The IR SHALL hold no flagged value type, and the generated code SHALL hold no merge of two paths.

**Acceptance Criteria** (EU-02, the removed names do not resolve):

1. IF a source writes `Unknown` THEN the system SHALL report `E0161` at the name, with a help that names an initial value, `Option<T>` and `Logic`.
2. IF a source calls `level`, `known`, `known_or`, `is_known`, `high` or `merge` on a value THEN the system SHALL report `E0161` naming the method and the type.
3. WHEN an `emit!` names a field `maybe` THEN the system SHALL accept it as an ordinary field.
4. The system SHALL raise `E0150`, `E0151`, `E0906` and `E0908` nowhere.

**Acceptance Criteria** (EU-03, control takes `bool`):

1. IF the condition of an `if` or a `while`, a guard, or an operand of `&&` or `||` has the type `Logic<N>` or `Option<T>` THEN the system SHALL refuse it with `E0180` naming the type found and `bool`.
2. WHEN an `analog` block uses a `Logic` carrier through a function that returns `f64` or `bool` THEN the system SHALL accept the read.
3. IF an `analog` block uses a `Logic` value where an `f64` or a `bool` is required THEN the system SHALL refuse it with `E0180`.

**Independent Test**: `signal n: Bit<u8> = 3;` read in a reaction as `if n > 2 { .. }` and in `analog` as `v(out) +> n as f64;`, run from source: the reaction takes the one arm and the node sits at 3.0 V. The same model with `n.known_or(0)` is `E0161`.

### P1: A carrier starts with a value, or its type has a default

**User Story**: As a module author, I want a carrier with no initial value to be refused unless its type says what "no value yet" is, so that the start of every run is written in the source.

**Why P1**: R3 and R4.

**Acceptance Criteria** (EU-04, the initial value):

1. WHEN a signal, an `out` port or a drive has `= value` THEN the system SHALL commit that value at `(0, 0)`.
2. WHEN a carrier has no initial value and its value type implements `Default` THEN the system SHALL commit `Default::default()` at `(0, 0)`.
3. WHEN an element of an array of carriers was never committed THEN the system SHALL read it as the array's initial value for that element, without storage proportional to the declared length.
4. IF a signal, an `out` port, or a drive of a resolved carrier has no initial value and its value type does not implement `Default` THEN the system SHALL report `E0187` at the declaration, naming the carrier and `Default`.
5. IF a one-driver carrier has no drive and its value type does not implement `Default` THEN the system SHALL report `E0187` naming the carrier.
6. IF a top-level `in` port has no initial value, its value type does not implement `Default`, and the host has given it no value when an analysis starts THEN the system SHALL refuse the analysis with `E0187` naming the port.
7. IF a signal is driven by an undelayed definition, or an `out` port of a child instance gives its initial value (the one driver of the carrier), THEN the system SHALL NOT report `E0187` for the carrier on account of a missing initial value; a signal driven only by a delayed definition is not exempt. *Amended (AFK default, to show the user): added after the approval of AC 4, which it narrows; the user's rule stands (a value that has a Default may start without one, one that has none must be specified): the definition or the child's port specifies it, once, from `(0, 0)`. An undelayed definition forbids an initial value on the signal (`E0338`), so without this exemption a `bool` carrier with a definition could be written neither way.*

**Acceptance Criteria** (EU-05, which types have a default):

1. The system SHALL give `Default` to `()`, to `Option<T>` (`None`), to `Logic<N>` (X on every level), to a tuple and an array whose parts all have it, and to a struct that derives it.
2. IF `#[derive(Default)]` is written on a struct with a field whose type does not implement `Default` THEN the system SHALL report `E0187` naming the field and its type.
3. IF `T::default()` is called for `bool`, `UInt<N>`, `Int<N>`, `Float<N>` or `str` THEN the system SHALL report `E0187` naming the type.

**Acceptance Criteria** (EU-06, absence is `Option`):

1. WHEN a param of type `Option<T>` is not bound by its instance THEN the system SHALL give it `None`.
2. WHEN a carrier of type `Bit<Option<T>>` has no initial value THEN the system SHALL start it at `None`, and a reaction SHALL read it with `match`, `if let` or the methods of Appendix H.3.
3. WHEN a carrier holds `Option<Logic<N>>` THEN its value SHALL be `None` or `Some` of a vector of `N` four-state levels, and no third form.

**Independent Test**: `signal en: Bit;` is `E0187` naming `en` and `Default`; `signal en: Bit = false;` and `signal last: Bit<Option<u8>>;` elaborate, and a reaction prints `last` as `None` at `(0, 0)`.

### P1: `Logic` is a library type with four states

**User Story**: As a module author, I want a type that says "this may be X or Z", with operators that follow the four-state tables, so that uncertainty is in the declaration of what carries it.

**Why P1**: R2, R5, R6. Phase B: it needs `generics`.

**Acceptance Criteria** (EU-07, the type):

1. The standard library SHALL declare `Logic<const N: usize = 1>` as a struct of two `UInt<N>` planes, `binary` then `value`, with the encoding of D1, and SHALL compile it from its own source with no rule of the language naming it.
2. The system SHALL give `Logic<N>` the `Repr` width `2 * N`: levels `0..N` the plane `binary`, levels `N..2 * N` the plane `value`.
3. WHEN `logic(p)` is called with a pattern of `0`, `1`, `x` and `z`, one character per level, highest level first, THEN the system SHALL give the `Logic<N>` with those states; IF the pattern holds another character or another length THEN the system SHALL report `E0702`.
4. WHEN `x.at(k)` is called on a `Logic<N>` THEN the system SHALL give the `LogicState` (`X`, `L`, `H`, `Z`) of level `k`, which a `match` covers with four arms.

**Acceptance Criteria** (EU-08, the operators):

1. WHEN `a & b` is evaluated on two `Logic<N>` THEN each level SHALL be 0 when either operand's level is 0, 1 when both are 1, and X otherwise.
2. WHEN `a | b` is evaluated on two `Logic<N>` THEN each level SHALL be 1 when either operand's level is 1, 0 when both are 0, and X otherwise.
3. WHEN `a ^ b` is evaluated on two `Logic<N>` THEN each level SHALL be the exclusive or when both levels are 0 or 1, and X otherwise.
4. WHEN `!a` is evaluated on a `Logic<N>` THEN each level SHALL be 1 for 0, 0 for 1, and X for X and for Z.
5. WHEN `a << k` or `a >> k` is evaluated on a `Logic<N>` THEN the levels SHALL move by `k` with their states, and the levels shifted in SHALL be 0.
6. WHEN `&`, `|` or `^` has a `Logic<N>` on one side and a `UInt<N>` on the other, or a `Logic<1>` and a `bool`, in either order, THEN the system SHALL give the `Logic<N>` of the same table with the binary operand converted by `From`.
7. IF an arithmetic operator or an ordering comparison has a `Logic` operand THEN the system SHALL refuse it with `E0180`.

**Acceptance Criteria** (EU-09, comparing and deciding):

1. WHEN `a == b` is evaluated on two `Logic<N>` THEN the result SHALL be a `Logic<1>`: X when any level of either operand is X or Z, otherwise 1 when every level is equal and 0 when not; `a != b` SHALL be its `!`.
2. WHEN `same(a, b)` is called on two `Logic<N>` THEN the result SHALL be the `bool` that is true exactly when every level holds the same one of the four states in both.
3. WHEN `x.is_high()`, `x.is_low()`, `x.is_z()` or `x.is_x()` is called on a `Logic<N>` THEN the result SHALL be the `bool` that is true exactly when every level is 1, 0, Z or X respectively.
4. IF `#[derive(PartialEq)]` is written on a struct with a `Logic` field THEN the system SHALL report `E0187` naming the field.

**Acceptance Criteria** (EU-10, conversions and the analog side):

1. WHEN `Logic::from(b)` is called with a `UInt<N>` or a `bool` THEN the system SHALL give the `Logic` whose every level is the 0 or 1 of `b`.
2. WHEN `x.to_bits()` is called on a `Logic<N>` THEN the system SHALL give `Some` of the `UInt<N>` when every level is 0 or 1, and `None` when any level is X or Z; `x.to_bit()` on a `Logic<1>` SHALL give the `Option<bool>` by the same rule.
3. WHEN `x.to_volts(vdd)` is called on a `Logic<1>` THEN the system SHALL give `vdd` for 1, `0.0` for 0 and `vdd / 2.0` for X and for Z, and an `analog` block SHALL accept it as a read of the carrier.

**Independent Test**: `signal bus: Bit<Logic<4>>;` starts as `xxxx`; after `bus <- logic("10xz") & Logic::from(0b1111u4);` the host prints `10xx`, `(bus == bus)` prints `x`, `same(bus, bus)` is `true`, and `bus.to_bits()` is `None`.

### P1: Resolved nets, history and events in the explicit model

**User Story**: As a module author, I want drives, changes and edges to be defined on values only, so that a wire's X is a state its type has and an edge is a transition I can read.

**Why P1**: R7's "re-derive what survives" and R8.

**Acceptance Criteria** (EU-11, capabilities, drives and `Resolve`; the specification text, run by `generics` and `resolved-nets`):

1. The specification SHALL state that a nature may implement a trait, that such a trait is a capability of its carriers, that `self` in such an `impl` is a handle, and that `impl Trait for` a bundle alone remains `E0134`.
2. The specification SHALL declare `Resolve<X: Repr, V: Repr = X>` with the one function `fn resolve<const N: usize>(drives: [X; N]) -> V`, SHALL state that `V` is the nature's `value` type (`E0203` otherwise), and SHALL hold no `resolve` field and no `resolve` keyword.
3. The specification SHALL state that a carrier takes more than one drive only when its nature implements `Resolve`, and that a second drive of any other carrier is `E0220`, whose diagnostic names the nature and `Resolve`.
4. The specification SHALL state that a drive picks its `impl` by the type of the value driven, that two driven types on one carrier are `E0225`, and that a driven type no `impl` accepts is `E0180`.
5. The specification SHALL state that `resolve` receives the present drives as an array of plain values, in instance-path order, instantiated for every count from zero to the number of drives.
6. The specification SHALL state that a drive with no initial value starts at `Default::default()` of the driven type and is a drive from `(0, 0)` on, that its absence is `E0187` (EU-04 AC 4), and that a carrier of a resolving nature needs no initial value of its own.
7. The specification SHALL state that in a relaxing loop every drive of the loop starts each relaxation (each microstep it runs) absent from what `resolve` receives, the passes of one relaxation building on the previous pass, and that absence is not a value of the type.
8. The specification SHALL state that a loop that still moves after its bounded passes stops the run with `E0511`, which names the loop's drives.

**Acceptance Criteria** (EU-12, the library's part in nets; the specification text, run by `resolved-nets`):

1. The specification SHALL define `Strength` with the set Q5 decides (by default `HighZ < Weak < Pull < Strong < Supply`), `Driven { v: Logic, s: Strength }`, and `Driven::strongest`: a `HighZ` drive is left out, the strongest drive wins, equal strongest drives give their level when they agree and X at that strength when they do not, and no drive gives `Driven::Z`.
2. The specification SHALL declare no `Wire`, `Tri` or `SumNet` nature, and SHALL declare `Tribuf`, `Pullup`, `Pulldown`, `Keeper`, `Tran`, `Tgate`, `Bidir` and `TribufDelayed` generic over `W: Nature + Resolve<Driven>`.
3. The specification's gallery SHALL hold four worked nets, each with its nature, its `impl` and its module code: a plain `bool` driven into a wired-AND nature, an open-drain line with a pull-up, a tri-state bus with two drivers and a keeper, and a conflict of equal strengths that reads X.

**Acceptance Criteria** (EU-13, changes, `prev()` and edges):

1. WHEN a carrier's value in a microstep differs from its value in the previous microstep in at least one level THEN the system SHALL count a change; a commit of the value the carrier holds SHALL NOT be one.
2. WHEN the initial value is committed at `(0, 0)` THEN the system SHALL fire `changed()` for that carrier there.
3. WHILE a carrier has had no change after `(0, 0)` the system SHALL give its initial value for `prev()`.
4. WHEN a `Bit` goes from `false` to `true` THEN `rise()` SHALL fire, and WHEN it goes from `true` to `false` THEN `fall()` SHALL fire.
5. WHILE a `Bit` holds the initial value it was given at `(0, 0)` the system SHALL fire neither `rise()` nor `fall()` for it.

**Acceptance Criteria** (EU-14, what the events part keeps):

1. WHEN a reaction listens to `settle` THEN the system SHALL run it once per time point after the instant is quiet, and SHALL hold and land its commits, as MD-74 items 1 and 4 say.
2. WHEN an event is read as a `bool` in a guard or a reaction body THEN the system SHALL give whether it fired in this microstep (`fired(k)`), a `bool`.
3. WHEN the guard of a reaction is false THEN the system SHALL NOT run the reaction, and no reaction SHALL run "as a maybe".

**Independent Test** (the specification part is read; the run part is): `signal clk: Bit = true;` with `on clk.rise() { n <- n + 1; }` and a clock that toggles three times after `(0, 0)`: `n` counts the rises after the start and not the initial value.

### P1: Records, the host and the device contract hold values only

**User Story**: As a host author and as a device author, I want a record, a cell and a pin to hold exactly the bytes of their type, so that no reader checks a flag before it reads.

**Why P1**: R9; EU-01 cannot hold while the contract below it carries the flag.

**Acceptance Criteria** (EU-15, records and cells):

1. The system SHALL lay a stream's record out as its declared fields only, with no `maybe` field.
2. WHEN a `require` record arrives THEN the host SHALL report a pass or a fail, and no third verdict.
3. WHEN a `fatal` record arrives THEN the run SHALL end.
4. WHEN the host prints a `Logic<N>` cell THEN it SHALL print `N` characters of `0`, `1`, `x`, `z`, highest level first, in tables, records and JSON Lines.
5. WHEN the host prints an `Option` cell THEN it SHALL print `None` or the value, and no cell SHALL print as unknown.

**Acceptance Criteria** (EU-16, the device contract, still major 1):

1. The device library SHALL export `piperine_abi_1`, with `MAJOR` 1 and `MINOR` 0, and WHEN the cache holds a compiled model built against the earlier contract THEN the system SHALL rebuild it and SHALL NOT load it.
2. The ABI's `Field` SHALL be `{ name, leaf, offset, count }`, and the shipped layouts SHALL be `BIT` `{ v: BOOL @0 }` of size 1 and alignment 1, `BYTE` `{ v: U8 @0 }` of size 1, and `F64` `{ v: F64 @0 }` of size 8 and alignment 8.
3. The `Host` SHALL offer `commit` and no `commit_with`, and the ABI SHALL define no `commit_flag`.
4. WHEN a pin carries `Option<T>` THEN its layout SHALL be one `BOOL` presence field followed by the layout of `T`, and a param that may be "not given" SHALL be such a pin.
5. WHEN a pin carries `Logic<N>` THEN its layout SHALL be two fields, `binary` at offset 0 and `value` at the size of `UInt<N>`, each with the leaf of `UInt<N>`, the whole of twice that size and of that alignment.
6. WHEN a drive pin of a resolved net is laid out THEN it SHALL be the layout of the driven type followed by one `driving: BOOL` field, 0 for a drive absent from the resolver and 1 for a present one.

**Independent Test**: a hand-written device in `crates/piperine-abi/tests` with a `BIT` pin of one byte binds and commits under `piperine_abi_1`; a cache entry keyed by the earlier contract is rebuilt.

### P1: The specification and the Book say it first

**User Story**: As a reader, I want the specification and the Book to teach the explicit model before the code changes, so that the text is the authority the code is brought to.

**Why P1**: the precedent of `value-semantics`; the user asked for the Book in the same move.

**Acceptance Criteria** (EU-17, the specification):

1. The specification SHALL hold, in Part I §3.3, the explicit model: R1 to R6 as numbered rules, with the statement that no primitive numeric type implements `Default` and that this differs from Rust.
2. The specification SHALL hold the four-state tables of EU-08 and EU-09 in Part VII §22.1, beside the library source that implements them.
3. The specification SHALL contain, outside Appendix G's record of this change, none of: "known flag", the literal `Unknown`, `level(`, `known_or`, `is_known`, "flag-total", "under unknown control", `maybe = true`.
4. Appendix G SHALL hold an entry for this amendment that lists what was removed, and SHALL mark G.29, G.46 and G.57 as closed by it.
5. `abi.md` SHALL describe the contract as EU-16 states it, with P3, P5, C7, D1 and its weak point 6 rewritten or removed, and SHALL say that the ABI is unreleased and changes in place until the first release.
6. Appendix H SHALL hold the `Default` rule of D4, `From`, the parameters of `PartialEq`, and the operator traits of D2.

**Acceptance Criteria** (EU-18, the Book):

1. The Book's chapter "Values in hardware" SHALL teach, in place of "Known and unknown", "Catching an unknown with `emit!`" and "Analog code has no unknowns": binary types and `Logic`, `Option` for absence, `Default` deciding whether a carrier may start without a value, `Logic == Logic` giving a `Logic`, `if` taking a `bool`, `same`, the conversions, and the analog side reading a `Logic` through a function.
2. The Book SHALL state, in its list of rules that differ from other languages a reader may know, that no primitive numeric type has a default value, in its own words and without the phrase "same as Rust".
3. The Book SHALL contain no listing and no sentence that uses `Unknown`, `known_or`, `known()`, `is_known`, `level(`, `high()`, `maybe`, or the gate of an analog read.
4. The Book SHALL teach nets through natures the reader declares: that a trait is a capability, `impl Resolve<X> for` a nature, and a section "Driving with strength" that works the four nets of EU-12 AC 3; it SHALL name no library `Wire`.
5. WHEN a Book listing cannot run yet THEN `tests/book/inventory.tsv` SHALL name its `GAPS.md` row, and its test SHALL fail loud when the listing starts to run or stops matching.
6. The Book's "Device ABI" chapter SHALL describe the contract of EU-16 and say it is unreleased.

**Acceptance Criteria** (EU-19, the records):

1. `.specs/STATE.md` SHALL hold MD-78 with the text of "MD entry" below, and SHALL mark MD-70 as superseded by it.
2. `GAPS.md` SHALL close S6 and G6 by this redesign, SHALL hold one silent row for "the implementation still carries the known flag" until phase A lands, and SHALL NOT hold that row after it.
3. `.specs/features/unknown-semantics-per-level/spec.md` SHALL open with a line that says the feature is dropped, by whom and when.

### P1: The tree is written in the explicit model, and nothing else moves

**User Story**: As a maintainer, I want the library, the examples, the fixtures and the tests in the explicit form, with every number unchanged, so that the removal can land and no guard carries an exception.

**Acceptance Criteria** (EU-20, the tree):

1. The system SHALL hold no `.msl` file under `crates/piperine-lang/std/`, `examples/`, `tests/` and `crates/*/tests/` that uses a removed name, other than a test that asserts its refusal.
2. WHEN a test pinned a behavior of unknown values THEN it SHALL be replaced by the test of the criterion that supersedes it, and the commit SHALL name both.
3. WHEN `std` is built THEN the system SHALL report no diagnostic, and every `rust` fence of Part VII SHALL be consecutive lines of its `std` file.

**Acceptance Criteria** (EU-21, parity):

1. WHEN the parity targets run THEN the 7 ngspice twins SHALL print the bytes they printed before, with `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` unedited.
2. WHEN `examples/buck` runs THEN its targets SHALL pass with the waveform asserts unchanged, and only the asserts on a known byte or a `maybe` field SHALL be rewritten.
3. WHEN the integrated suites run THEN `cargo test -p piperine --test integrated` and `cargo test -p piperine-cli --test integrated` SHALL pass.

---

## Edge Cases

- WHEN `Logic::default()` is asked for any `N` THEN it SHALL be X on every level, the value whose bytes are all zero.
- WHEN `a & b` has a Z level in `a` and a 0 level in `b` THEN the level SHALL be 0; with a 1 in `b` it SHALL be X.
- IF a module drives a `Logic` into a carrier whose nature implements `Resolve<bool>` only THEN the system SHALL refuse the drive with `E0180` naming the nature and the driven type.
- WHEN a nature implements `Resolve` and exactly one module drives its carrier THEN the value SHALL still be `resolve` of that one drive.
- IF `Logic<4> & Logic<8>` is written THEN the system SHALL refuse it with `E0180`.
- WHEN a struct derives `Default` and holds only `Option` and `Logic` fields THEN a carrier of it MAY have no initial value.
- IF `signal x: [Bit; 8];` has no initial value THEN the system SHALL report `E0187` once, naming `x`.
- WHEN a definition names some levels of a carrier and not others (§8.2) THEN the levels no definition names SHALL hold the carrier's initial value.
- WHEN a `const` is declared with a `Logic` value THEN a generation condition that needs a `bool` SHALL take it only through `is_high()`, `same` or a `match`.
- IF a hand-written `decode` returns `None` THEN the caller SHALL receive that `None`; no carrier is changed by it.

---

## Inventory: what is removed or reworked

### Specification (`docs/spec/`), amended in batches B1 and B2

| File | Lines that mention the model today | What changes |
| ---- | ---------------------------------- | ------------ |
| `part_i_values.md` | 75 | §3.1 (the table, §3.1.9, `E0134` narrowed to bundles), §3.7.12 and the `E0134` row of §3.7 (D17); §3.3 rewritten whole; §1 rule 13 (a panic under unknown control); §2 opening, rules 7 to 11 (levels carry no flag, `decode`'s `None` is returned, the all-zero rule); §1.5 and §1.8 mentions of unknown operands and control; §3.7.5's example `impl<T: Repr> Known for T` |
| `part_ii_carriers.md` | 24 | §4 rule 15; §5.1.1, §5.1.5, §5.1.6, §5.1.8, the §5.1 examples; §5.2.8, §5.2.12; the §5.3 table, §5.3.4 to §5.3.8, §5.3.11, §5.3.12; §6 rule 16; `E0187` in the §5.1 table. §4 whole for `Resolve`: the grammar line of `resolve`, the kinds table, rules 1, 2, 6, 8 and 15, `E0203`; §5.2 (the definitions of one-driver and resolved carrier, §5.2.1, §5.2.2, `E0220`, new `E0225`) |
| `part_iii_structure.md` | 16 | §8.1 (a level no definition names), §8.2.3 step 4 and its error row, §8.2.28, §8.2.30, §8.3.18, §8.5 (a foreign simulator and unknown inputs), the two `known_or` examples |
| `part_iv_behavior.md` | 26 | §11 rule 13 and its resolution, §12.1.19, the `SampleHold` example, `clk.prev().is_known()`, §13 rules 1, 5, 6 and 16, `at` and `cross` with unknown arguments, the §14 tables (`x.level(k)`), `E0150` in §14's table |
| `part_v_execution.md` | 12 | §16 step 2 (the first commit), §17.3 rule 5 and its resolution |
| `part_vi_io.md` | 7 | §19 rules 1 and 11, the `maybe` field, `E0604`, the unknown `pass` |
| `part_vii_stdlib.md` | 80 | §22.1.1 `Known` (removed), §22.1.2 `merge` (removed), §22.1.3 `cast`, §22.1.4 `same`, `matches_bits`, `logic`, §22.1.5 `impl Bit`, new §22.1 units for `Logic`, `LogicState` and the tables; §22.3.1 to §22.3.8; §22.4 converters and §22.5 monitors where they read a flag; the index and the error table |
| `part_viii_host.md` | 6 | §23.1 rule 9, §23.2 rules 3 and 4, §23.3 rule 4g |
| `appendix_a_lexical.md` | 15 | A.3 removed; `E0906`, `E0908` retired; the `IDENT` and literal productions |
| `appendix_b_grammar.md` | 6 | row 21 and the count of literals, the placement matrix row, the `literal` production; row 15 (`resolve`) and the contextual-keyword lists of A.1 and B lose `resolve`; the `nature_field` production |
| `appendix_c_implementation.md` | 6 | C.1 row of `signal`, C.2 "unknown control", C.2.1 "loop iterations under unknown control" |
| `appendix_d_gallery_analog.md`, `appendix_e_gallery_digital.md`, `appendix_f_gallery_mixed.md` | 14, 22, 16 | every listing and note that reads a flag or leaves a carrier without a value (E.5's unknown index, F.4's undriven, contended and kept) |
| `appendix_g_scope.md` | 13 | new entry G.72; G.29, G.46, G.57 closed |
| `appendix_h_library.md` | 2 | H rule 5 (all-or-nothing methods) removed; H.8 `Default`, `PartialEq`, `From`; new H.10 operator traits; H.9 unchanged (`Logic` is a library type, §22.1) |
| `index.md` | 7 | the concept rows and the glossary rows for known, unknown, flag-total, merge |
| `.specs/features/device-abi/abi.md` | 20 | edited in place, still major 1: §2 entry 7, §3 `Field` and constants (`MINOR` 0), P3, P5, the `TRACE` empty value, C7, D1, §11 examples, §12 versioning (unreleased: changes in place until the first release), §14 point 6, §15 rows 6 and 7 |

### The Book (`docs/book/content.html`, 140 lines), amended in batch B3

"Values in hardware" (the three sections named in EU-18 AC 1); "Language basics" and "Your own types" (`Option`, the `Default` row, the list of differing rules); "Traits and generics" (the `Default` row at line 1199, operator traits); "Natures and carriers" and "Digital behavior" ("Many drivers: resolved nets", the alarm, `Pfd3`); "Analog behavior" and "Mixed-signal" (`known_or`, `high()`, the sample-and-hold, `on settle` prose that mentions unknown); "Inside a simulation"; "Core and standard library" ("`std::values`: unknown values, bit patterns and clocks"); "Reading errors" (`E0150`, `E0151`, `E0906`); "The command line" and "Cheat sheet"; "Device ABI", "The solver", "From source to solver"; `docs/book/build.py` (`Unknown` in `CONSTS`). Held by `tests/book/*.rs`, `tests/book/inventory.tsv` (27 rows name the model) and `crates/piperine-cli/tests/book.rs`.

### `GAPS.md`

S6 and G6 (closed by redesign); the stdlib line of §1 (`known`, `known_or`, `is_known`, `high`); L3, L5 and L24 (the clauses "an index that may be unknown", "a loop under unknown control", which travel with nobody now); L58 (`merge`, `cast`, `same`, `Logic`); L59 (its clause on an `Option` whose presence may be unknown is moot; the non-literal closure and `expect` message clauses stay open); the conformance rows of I §3 to §3.3, II §4 to §5 and VI §19 to §21; the planned-work row "Unknown semantics"; the Book rows digital#7, natures#3, core-std#6.

### Code

| Layer | Files | What goes or changes |
| ----- | ----- | -------------------- |
| Lexer, parser, syntax tree | `crates/piperine-lang/src/msl/lexer.rs`, `ast.rs`, `parser/pat.rs`, `parser/entry.rs` | the `Unknown` token and literal |
| Checker | `crates/piperine-lang/src/types/flag_total.rs` (deleted), `analog.rs`, `intrinsic.rs`, `carrier.rs`, `expr.rs`, `record.rs`, `result.rs` | `E0150`, `E0151`, flag-total functions, the intrinsic methods, `maybe`; the `Default` rule |
| Elaborator | `crates/piperine-lang/src/elab/known.rs` (deleted), `consts.rs`, `compound.rs`, `lower.rs`, `intrinsics.rs`, `events.rs`, `variants.rs`, `selectors.rs`, `records.rs`, `derivatives.rs`, `carriers.rs` | the analysis of unknown-ness, the merge lowering, the unknown index, initial values |
| IR | `crates/piperine-ir/src/model.rs`, `typing.rs`, `check.rs`, `print.rs`, `deps.rs` | `ValueType::Flagged` and its rules |
| Plan | `crates/piperine-plan/src/digital.rs`, `plan.rs`, `derive.rs` | the known byte of every layout, the `maybe` field, `guard_may_be_unknown` |
| Generated code and its runtime | `crates/piperine-transpile/src/rt/flag.rs` (deleted), `rt/control.rs`, `rt/commits.rs`, `rt/real.rs`, `rt.rs`, `print/value.rs`, `print/shape.rs`, `print/cross.rs`, `print/digital.rs`, `print/reaction.rs`, `print/analog.rs`, `print/site.rs`, `print/contribution.rs`, `shape_check.rs` | flagged values, the merge of paths, `commit_with` |
| ABI | `crates/piperine-abi/src/layout.rs`, `raw.rs`, `validate.rs`, `caller.rs`, `device.rs`, `thunk.rs`, `shape.rs`, `verify.rs` | `Field.known`, `HAS_KNOWN`, `layout::Merge`, `commit_with`, `commit_flag`; `MINOR` 0, major unchanged |
| Engines | `crates/piperine-digital-solver/src/queue.rs`, `run.rs`, `settle.rs`, `store.rs`; `crates/piperine-solver/src/core/bind.rs`, `protocol/transient.rs` | the `MERGE` and `MAYBE` commits, relaxation's zeroing of a known byte, layout equality |
| Host and CLI | `crates/piperine-api/src/request.rs`, `verdict.rs`, `recorder.rs`, `record.rs`, `table.rs`, `probe.rs`, `plot.rs`, `backend.rs`; `crates/piperine-cli/src/cli.rs`, `render/json.rs`, `render/record.rs`, `commands/run.rs` | unknown cells, the unknown verdict, `maybe`; `Logic` and `None` cells |
| Interpreter | `crates/piperine-interp/src/walk.rs` | the flag forms it lists as refused |
| Standard library | `crates/piperine-lang/std/values.msl`, `carriers.msl`, `nets.msl`, `convert.msl`, `monitor.msl`, `analog.msl`, `prelude.msl`; new `logic.msl` (phase B) | `Known`, `merge`, `level`; `rise`, `fall`, `high`; `nets.msl` without `Wire`, `Tri`, `SumNet` (D20); `Logic` |

### Tests and goldens that pin unknown behavior (rewritten, never silently deleted)

| Target | Fate |
| ------ | ---- |
| `crates/piperine-lang/tests/types_unknowns.rs` | becomes `types_explicit.rs` |
| `crates/piperine-lang/tests/elab_unknowns.rs` | becomes `elab_explicit.rs` |
| `crates/piperine-parity/tests/source_unknowns.rs` and `tests/unknowns/*.msl` (15 models) | become `source_explicit.rs` and `tests/explicit/*.msl` |
| `crates/piperine-lang/tests/elab_values.rs`, `elab_digital.rs`, `elab_mixed.rs`, `elab_structure.rs`, `elab_results.rs`, `types_carriers.rs`, `types_events.rs`, `types_structure.rs`, `types_results.rs`, `msl_parser.rs`, `msl_lexer.rs`, `msl_errors.rs`, `pom_resolve.rs`, `pom_classify.rs`, `std_library.rs`, `std_galleries.rs`, `std_excerpts.rs`, `spec_text.rs`, `spec_codes.rs` | the cases that name a flag are replaced, case by case |
| `crates/piperine-ir/tests/digital.rs`, `amendments.rs`, `support/digital.rs`; `golden/buck/*.txt` and `golden/buck/full/*.txt` | flagged types leave; the goldens are regenerated once, in the task that removes `Flagged` |
| `crates/piperine-plan/tests/digital_shape.rs`, `amended_shape.rs` | layouts without the known byte |
| `crates/piperine-transpile/tests/reaction.rs`, `amended.rs`, `analog_merge.rs`, `pid.rs`, `golden/` | the merge tests are replaced by one-path tests; the goldens move once |
| `crates/piperine-abi/tests/layouts.rs`, `validate.rs`, `caller.rs`, `contract.rs` | the contract edited in place |
| `crates/piperine-digital-solver/tests/relax.rs`, `instant.rs`, `queue.rs`, `bind.rs`, `functions.rs`, `support/devices.rs` | plain `driving`, no `MERGE` |
| `crates/piperine-solver/tests/bind.rs`, `helpers/abi_mixed.rs`, `environment.rs`, `protocol_transient.rs`, `host_writes.rs`, `host_set.rs`, `live_params.rs`, `step_observer.rs`, `probe_selection.rs`; `crates/piperine-analog-solver/tests/support/devices.rs` | layouts of the hand-written devices |
| `crates/piperine-api/tests/session.rs`, `records.rs`, `probes.rs`, `analyses.rs`; `crates/piperine-cli/tests/run.rs`, `docs.rs` | cells, verdicts, `maybe` |
| `crates/piperine-parity/tests/source_events.rs`, `source_catalogue.rs`, `source_fixes.rs`, `buck_control.rs`, `src/scenario/discrete.rs`, `src/scenario/run.rs` and the models under `tests/{digital,events,fixes,structure,natures,run_errors,buck}/` | models given initial values; the buck's known-byte asserts rewritten |
| `crates/piperine-lang/tests/fixtures/` (`elab/buck.txt`, `gallery/SampleHold.msl`, `preformat/**`) | regenerated from the rewritten sources |
| `examples/buck/src/{control,dpwm,power,supervisor,top}.msl`, `benches/startup.msl`; `tests/e2e/integrated/src/{lib,pi_loop,state_machine,threshold}.msl`; `crates/piperine-cli/tests/projects/pump_tolerance` | `known_or(d)` becomes the plain read on a carrier given the initial value `d` implied; `high()` becomes the plain read; `&& x.prev().is_known()` is deleted (D9) |
| The 7 twins (`tests/e2e/twins`, `crates/piperine-cli/tests/twins.rs`, `crates/piperine-parity/tests/twins.rs`) | byte-identical; never edited |

### Decisions superseded

| Decision | Verdict |
| -------- | ------- |
| MD-70 (`unknown-semantics`: the merge in generated code, `commit_with`, `maybe`, `E0150`/`E0151`) | superseded whole by MD-78 |
| MD-64 (the IR's value forms; unknown-ness as an elaboration analysis) | superseded in the part "unknown-ness as an elaboration analysis" |
| MD-74 (`unknown-semantics-events`) | stands: `settle`, `fired(k)`, `limexp` and its parity carry no flag. Only §13 rule 1's "a guard that may be unknown runs the reaction as a maybe", which its plan code implements, goes |
| MD-76 (`value-semantics`) | stands, but its D7 changes home: the const-generic signature `fn<const N: usize>([T; N]) -> T` is now the function of the trait `Resolve<X, V>`, no longer a function a nature field names |
| `resolved-nets` B1 and B2 on `main` (a nature with a `resolve` field; `E0203` as a signature check) | reworked by its B3 onward under design.md "Changes to `resolved-nets`" |
| The orchestrator's ruling of 2026-11-15 (`ir-rust` F-B4: one flag per value, provisional) | void |

### MD entry (to be appended to `.specs/STATE.md`; the next free number on `origin/main` at `8b5596ed` is MD-78. `composite-values` may take a number at its merge: the second to merge renumbers)

> **MD-78: unknown is explicit; no value carries a flag.** `explicit-unknowns` (2026-11-21, user: "Não acho que unknown deveria ser algo implícito, deveria ser algo EXPLICITO").
> 1. **A value is what its type says.** Every value of `bool`, `UInt<N>`, `Int<N>`, `Float<N>`, an enum, a struct, a tuple or an array is known. Part I §3.3's known flag, the literal `Unknown`, `level`, `known`/`known_or`/`is_known`, the merge under unknown control, `maybe` and the gate of an analog read are removed from the language.
> 2. **Uncertainty and absence are types.** `Logic<N>` is a library struct of two planes with the states 0, 1, X, Z; its operators are library `impl`s of the operator traits of Appendix H.10. Absence is `Option<T>`.
> 3. **A carrier starts with a value or with its type's `Default`.** No primitive numeric type implements `Default`; a missing initial value on such a type is `E0187`.
> 4. **The device ABI stays major 1 and is edited in place.** It is unreleased (user: "ABI pode continuar major 1 pq não fizemos release ainda"), so no compatibility is promised until the first release; `MINOR` returns to 0. No known byte, no `commit_with`; `Option` is a presence byte and a payload, `Logic<N>` two planes, a drive pin the driven value and a plain `driving` byte.
> 5. **Traits are capabilities.** A nature takes trait `impl`s (`E0134` is narrowed to bundles). A discrete carrier takes more than one drive only when its nature implements `Resolve<X, V = X>`; the `resolve` field and keyword are removed, and the library declares no net nature (`Wire`, `Tri`, `SumNet` leave). A loop of resolved drives that does not settle is `E0511`.
> 6. **`unknown-semantics-per-level` is dropped**, and `GAPS.md` S6 and G6 close by this redesign.
>
> It supersedes MD-70 whole and MD-64 in its analysis of unknown-ness; it moves MD-76's D7 into the trait. MD-74 stands. **Scope:** `docs/spec`, `docs/book`, `crates/piperine-{lang,ir,plan,transpile,abi,digital-solver,solver,api,cli,parity}`, `examples/`, `tests/`.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| EU-01 | P1: a value is what its type says | Tasks | Pending |
| EU-02 | P1: a value is what its type says | Tasks | Pending |
| EU-03 | P1: a value is what its type says | Tasks | Pending |
| EU-04 | P1: a carrier starts with a value, or its type has a default | Tasks | Pending |
| EU-05 | P1: a carrier starts with a value, or its type has a default | Tasks | Pending |
| EU-06 | P1: a carrier starts with a value, or its type has a default | Tasks | Pending |
| EU-07 | P1: `Logic` is a library type with four states | Tasks | Pending |
| EU-08 | P1: `Logic` is a library type with four states | Tasks | Pending |
| EU-09 | P1: `Logic` is a library type with four states | Tasks | Pending |
| EU-10 | P1: `Logic` is a library type with four states | Tasks | Pending |
| EU-11 | P1: resolved nets, history and events | Tasks | Pending |
| EU-12 | P1: resolved nets, history and events | Tasks | Pending |
| EU-13 | P1: resolved nets, history and events | Tasks | Pending |
| EU-14 | P1: resolved nets, history and events | Tasks | Pending |
| EU-15 | P1: records, the host and the device contract | Tasks | Pending |
| EU-16 | P1: records, the host and the device contract | Tasks | Pending |
| EU-17 | P1: the specification and the Book say it first | Tasks | Pending |
| EU-18 | P1: the specification and the Book say it first | Tasks | Pending |
| EU-19 | P1: the specification and the Book say it first | Tasks | Pending |
| EU-20 | P1: the tree is written in the explicit model | Tasks | Pending |
| EU-21 | P1: the tree is written in the explicit model | Tasks | Pending |

**Coverage:** 21 total, 21 mapped to tasks (tasks.md), 0 unmapped.

---

## Sources

- The user, 2026-11-21 (the decision and the two questions quoted at the top).
- `docs/spec/` at `8b5596ed`: `part_i_values.md` §2, §3.3, §3.7, §3.8; `part_ii_carriers.md` §4, §5; `part_iii_structure.md` §8.2.3; `part_vii_stdlib.md` §22.1, §22.3; `appendix_h_library.md` H.8; `appendix_a_lexical.md` A.3.
- `.specs/features/device-abi/abi.md` §3 (P3, P5), §6 (C7), §9 (D1), §12 (versioning).
- `.specs/STATE.md` MD-64, MD-70, MD-74, MD-76, MD-77.
- `.specs/features/value-semantics/` (the precedent: the amendment first, then the code in three moves).
- `.specs/features/unknown-semantics/`, `unknown-semantics-events/`, `unknown-semantics-per-level/`, `resolved-nets/`, `resolved-nets-bundles/`, `params-and-io/`; `generics` on `origin/feature/generics` at `a9ac90db`; `composite-values` on `origin/feature/composite-values` at `b2fa6861`.
- `GAPS.md` S6, G6, L3, L5, L21, L24, L58, L59.
- `.specs/features/code-clarity/spec.md` (CC-11: no special case in the language).
- For Q5: IEEE 1364-2005 §7, IEEE 1800-2017 §6.6.7 and §28, IEEE 1164, IEEE 1666, the Verilog-AMS LRM 2.4 mixed-signal chapter, the ngspice manual's XSPICE digital node type, the Logisim and Digital documentation. Cited from the planner's knowledge; T7 verifies each.

---

## Success Criteria

- [ ] `grep` finds none of the removed names in `docs/spec` (outside G.72), `docs/book`, `std`, `examples` or a fixture.
- [ ] `grep` finds no `Flagged`, `HAS_KNOWN`, `commit_with`, `MAYBE` or `maybe` in `crates/*/src`.
- [ ] The 7 twins' bytes are unchanged and every buck target passes.
- [ ] Every Independent Test runs from source on the Rust backend.
