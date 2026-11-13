# MSL stack audit (spec + parser + POM)

**Auditor**: independent sub-agent, read-only. **Commit read**: probes started at `11b8f98a`, every finding below was re-run at `ad6a954f` ("msl-pom done - proofs green, verification round 2 pass"). `cargo test -p piperine-lang` is green there (15 targets, 0 failed).
**Method**: a throwaway crate outside the repo linked to `piperine-lang` by path; about 330 hand-written adversarial sources (lexical edges, every B.3 production and its mutations, recovery orders, multi-file `mod` layouts, globs and shadowing, generation, templates, bundles, natures) run through `parse`, `print` (round trip and idempotence), `Design::build` (twice, for determinism), `check()`, a JSON round trip and `to_ast`; every `rust` fence of `docs/spec` built against the test prelude and against the spec's own stdlib text; release-mode scaling runs. Verifier findings 1-10 of `msl-pom/verification.md` are not repeated.

## Verdict

The stack is solid where it was tested: no panic in the parser, the printer round-trips every clean input I wrote, `check()` and determinism held on all of them, lexical scoping (`let` shadowing, binders, closures, nested items, `crate`/`super`/`self`, multi-file layouts, cycles) is right, and the spec's error-code tables are internally consistent (no code used but undefined, none defined twice). It is not yet "redondinho". Three things block the next stage: the spec's own models do not resolve (`str`, `gnd`, `Nature`, `m`, `dtemp` are in no table and no source, so valid text gets `E0161`); an identifier pattern that names a constant or a unit variant (`None`, `Dc`, a `const`) is modelled as a fresh binding, which silently changes what a `match` means; and there is no compilable standard library, so no test can say "a gallery model builds with zero diagnostics". Behind those sit a parser that accepts several things the grammar forbids (any expression as a commit target, `signal` in `analog` and reactions, `pub` on a behavior block, `{ r }` shorthand), rejects a few the spec prescribes (`Int< -1 >`, `use a::{self}`, doc comments before statements), and a spec whose "everything else is Rust" clause is wider than what the parser takes and than what a type checker can be written against.

## Summary

| Layer | BLOCKER | MAJOR | MINOR |
| --- | --- | --- | --- |
| Spec (`docs/spec`) | 1 | 6 | 7 |
| Parser (`msl/`) | 0 | 4 | 9 |
| POM (`pom/`, `docs/pom`) | 2 | 8 | 9 |
| **Total (46)** | **3** | **18** | **25** |

A finding that spans layers is counted in the layer where the fix starts.

## BLOCKER

### A-01 (POM) Core names of B.1 are in no table: valid models get `E0161`
- **Where**: `crates/piperine-lang/src/pom/builtin.rs:41` (`BUILTINS`); spec B.1 rows 26, 52, 53, 54; §1 (`&'static str`); §6 rule 11; §8.5.3 (G.54).
- **Evidence**: `const S: &'static str = "x";` → `E0161 cannot find 'str'`. `module M<N: Nature>(a: E, g: N) { let r = R(a, gnd); let t = m * dtemp; }` → `E0161` on `Nature`, `gnd`, `m`, `dtemp`, and port `g` is `PortKind::Unknown`. Over the spec corpus: 13 false `E0161` on `str` and 8 on `gnd`. The test prelude hides `Nature` behind a stand-in `pub trait Nature {}` (`tests/fixtures/pom/prelude.msl:7`); `Repr` is built in, `Nature` is not, though B.1 lists both as language traits. `gnd` cannot be declared in the language at all (no item form yields a node of every continuous nature).
- **Fix**: add rows `str` (Type), `Nature` (Trait), `gnd` (new kind `Node`), `m` and `dtemp` (new kind `ImplicitParam`, visible only inside a module body and its blocks). Drop the `Nature` stand-in from the fixture. Classify `N: Nature` by the built-in row.

### A-02 (POM) An identifier pattern that names a constant or unit variant becomes a new binding
- **Where**: `pom/lower/body.rs` (pattern lowering), `pom/resolve/names.rs:370`.
- **Evidence**: `fn f(o: Option<u8>) -> u8 { match o { None => 0, Some(x) => x } }` → `dump()` shows `Binding None 38..42`. Same for `const ZERO: u8 = 0; match x { ZERO => 1, other => other }` (`Binding ZERO`), `use E::*; match e { A => .., B => .. }` (`Binding A`, `Binding B`), `match a { Dc => {}, Tran => {} }` in `analog` (`Binding Dc`), and a unit struct `U`. Expected (Rust rule, inherited by §0.4): the pattern is a path to the constant, variant or unit struct; the first arm is not a catch-all. No diagnostic, no reference recorded on the constant.
- **Fix**: give `PatKind::Ident` a `res: Resolution` slot. Resolution sets it when the plain name (no `mut`, `ref`, `@`) resolves to a const, unit variant, unit struct or the built-ins `None`/`Dc`/`Tran`/`Ac`/`Noise`/`Static`; the binding declaration is then not entered in scope and `Binding::is_binding()` answers false (the node stays, so the "resolution adds links only" rule holds). Add the reference to the index. Test all five cases.

### A-03 (Spec) There is no compilable standard library, and the spec's own prelude text is not a prelude
- **Where**: Part VII (51 `rust` fences), §4 examples (`part_ii_carriers.md:133-143`), §3.4.2, G.38.
- **Evidence**: built against the test prelude, 46 of 95 spec fences report `E0161` (126 in total: `pos`, `rate`, `require`, `strong`, `Strength`, `parse_table`, `since`, `cast`, `inertial`, ...). Concatenating every fence of Parts I, II and VII as one source resolves all of that except A-01 and A-09, so the text exists; but used as a prelude it exports almost nothing, because `nature Bit`, `Wire`, `Electrical`, `Thermal`, `VSignal`, `State` are written without `pub` (as a prelude: 153 `E0161` on `Bit`, 79 on `Electrical`). No test asserts that a gallery model builds clean: `pom_corpus` checks invariants and losslessness only.
- **Fix**: extract the stdlib into real files (`crates/piperine-lang/std/*.msl`), `pub` on every exported item, generated from or checked against the Part VII fences; add a test "every gallery fence of App. D/E/F builds with zero diagnostics against it" (models referenced across fences supplied by a small table). Until that test exists, resolution is proven only on fixtures written for it.

## MAJOR

### A-04 (Parser) A commit takes any expression as its target
- **Where**: `msl/parser/expr.rs:689-715` (`Tail::Commit` after a full expression); B.3.2 `place ::= path ( '[' expr ']' | '.' IDENT )*`.
- **Evidence**: `on e { -q <- 1; a + b <- 2; f(q) <- 3; q.x() <- 4; q[1..2] <- 5; *q <- 6; q as u8 <- 7; (q) <- 8; }` → zero diagnostics; the AST holds `Commit { place: Unary(Neg, q) }`, `Binary(Add, a, b)`, a call. A definition is strict (`f(q) = 1;`, `(q) = 1;` are `E0900`). In the POM those commits exist but are writers of nothing (`q.writers()` counts only `q[0] <- ..`), silently.
- **Fix**: validate the left side as a `place` when the tail is `<-` (same check as the definition) and report `E0900` "expected a place: a carrier, `x[i]` or `x.f`". Decide `(q)` once for both forms (see A-39).

### A-05 (Parser) `signal` inside `analog` and inside a reaction is accepted and becomes a declaration there
- **Where**: `msl/parser/check.rs:308-311` (only `Ctx::Fn` is reported); §10 rule 9, B.2.1.
- **Evidence**: `analog for M { signal s: E; }` and `digital for M { on a.changed() { signal t: E; } }` → no diagnostic; `dump()` shows `Signal s` under the `Behavior` and `Signal t` under the `Reaction`. `Module::signals()` does not list them, so the POM holds a signal that belongs to no module.
- **Fix**: report `E0404` for `StmtKind::Signal` in `Ctx::Analog`, `Ctx::Reaction` and `Ctx::Closure` (`E0121` stays for `fn`).

### A-06 (Parser) `Int< -1 >` does not parse, against A.5 rule 4
- **Where**: `msl/parser/ty.rs:33` (`generic_args`); A.5 rule 4 ("write `Int< -1 >` or `Int<{ -1 }>`").
- **Evidence**: `module M(a: Bit<Int< -1 >>) {}` → `E0900 unexpected '-', expected '{', literal, type or '>'`. `Int<{ -1 }>` parses.
- **Fix**: accept `'-' literal` as a const generic argument, as Rust does. Add both spellings to `msl_parser`.

### A-07 (Parser) Doc comments and attributes before a statement, contribution, commit, reaction, definition, generation or nature field are parse errors with misleading text
- **Where**: `msl/parser/expr.rs` (block statements), `msl/parser/msl.rs` (module items, digital items, nature fields); A.4, B.3.1 item 1 (`stmt` is Rust's, which takes outer attributes).
- **Evidence**: `analog for M { /// gate current\n i(a) +> 1.0; }` → `E0900 unexpected identifier 'i', expected '#', 'const', 'enum', ...`. `fn f() { /// x\n let y = 1; }` → `unexpected 'let', expected '#', 'const', ...`. `digital for M { /// clock edge\n on e { } }` → `E0434` over the reaction plus `E0900`. `module M() { /// drive\n s = x; }` → `unexpected '=', expected ':'` then a spurious `E0903` on the next `for`. `nature N { /// level\n value: bool }` → `E0900`. A `///` at end of file → `unexpected end of input, expected '#', 'const', ...`.
- **Fix**: accept `attrs` before every statement form, `on`, definition, generation and `nature_field` (see decision D9 for which of them reach the metadata of §21); a dangling doc gets its own message.

### A-08 (POM) `use a::{self, K}` is not resolved
- **Where**: `pom/resolve/imports.rs:235` (`"self" if segments.len() > 1`); §3.4.4 ("Rust's rules for paths, `use`").
- **Evidence**: `mod a { pub const K: f64 = 1.0; } use a::{self, K}; const Z: f64 = a::K + K;` → `E0161 cannot find 'self'`. With `use a::{self as aa, n::{self, J as JJ}};` the names `aa` and `n` are then unknown (`E0161` on `aa::K`, `n::J`). Side effect: `Path::text()` of a use leaf returns `a::{self as aa, n::{self, J` (the span runs through the braces).
- **Fix**: a `self` leaf imports the namespace named by its prefix (under its `as` name). Give each leaf path the span of its own segments, not the hull.

### A-09 (POM) An associated item through a generic bounded by a built-in trait is a false `E0161`
- **Where**: `pom/resolve/names.rs` (associated segment after a `GenericParam`); Part VII `cast` (`part_vii_stdlib.md:135`).
- **Evidence**: `fn cast<A: Repr, B: Repr>(a: A) -> B where A::WIDTH == B::WIDTH { B::decode(a.encode()).unwrap() }` → `E0161` on `WIDTH` (twice) and on `decode`. `u8::WIDTH` is `Deferred(ReceiverTypeUnknown)` and `T::W` with a user trait resolves, so only the built-in bound fails. The stdlib's own text triggers it.
- **Fix**: give the `Repr` row its members (`WIDTH`, `encode`, `decode`, `level`) or answer `Deferred` when a bound of the generic is a built-in trait.

### A-10 (Spec + POM) Two items of one name in a namespace: no rule, no code, no diagnostic
- **Where**: spec has `E0330` (module members) and `E0143` ("overload", functions only); nothing else. `pom/resolve/scopes.rs:61-76`.
- **Evidence**: `module M() {} module M() {} struct M; const M: f64 = 1.0; mod a {} mod a {}` → zero diagnostics; `M { x: 1.0 }` then resolves to the first module. Also silent: `use a::K; const K: f64 = 3.0;` and `use a::*; use b::*;` with `K` in both (first wins).
- **Fix**: one new code in §3.4 ("a name declared twice in one namespace, per type/value table; an import that collides with an item; a name reachable through two globs when used") reported by the scopes stage, both declarations kept. Decision D5.

### A-11 (POM) A design with an overflowing float literal cannot be read back from its JSON
- **Where**: `msl/lexer.rs:569-573` (`parse_f64`), `ast::LitKind::Float { value: f64 }`.
- **Evidence**: `const B: f64 = 1e999;` → no diagnostic, `value = inf`; `serde_json::to_string(&design)` writes `null` and `from_str` fails with `invalid type: null, expected f64`. Breaks POM-15 (C94) for a source the front end accepts.
- **Fix**: report a literal that overflows `f64` at lex time (decision D12), and serialize the float from its `raw` text, never as a bare `f64`.

### A-12 (POM) `Design::nature(name)` returns the prelude's item when the project shadows it; the collections disagree about prelude files
- **Where**: `pom/reflect/mod.rs:338-369`.
- **Evidence**: prelude `pub nature Bit { value: bool }`, root `nature Bit { value: u8 }`: `design.nature("Bit")` is the prelude one, while the port type `Bit`, `design.find("Bit")` and `design.module("Buf")` (same setup) all give the root one. §22 rule 3 lets a project replace a stdlib item, so this is the common case. `modules()` and `natures()` include prelude files; `structs()`, `functions()`, `namespaces()` exclude them. `qualified_name()` is `Buf` for both the prelude and the root module.
- **Fix**: one precedence everywhere (root files first, as `module()`), one inclusion rule with an explicit `*_all()` or a `SourceFile::is_prelude` filter, and add `enums()`, `traits()`, `consts()`, `impls()`, `struct_named(name)`, `function(name)`.

### A-13 (POM) No way to walk a body without matching raw storage
- **Where**: `pom/reflect/body.rs:93-149`; `Function::body() -> &Block`, `Reaction::body() -> &Block`, `Instance::param_bindings() -> &[FieldInit]`.
- **Evidence**: `Expr` has `kind()`, `enclosing()`, `target()` and nothing else: no `children()`, no `parent()`, no visitor. "Every call in this function", "the statement this expression sits in", "the arguments of this call as handles" all need a hand-written recursion over `ExprKind`/`StmtKind`/`Block` with raw ids. `Struct` and `Enum` have no `impls()` or `method(name)` (only `Nature::impls`), `Trait` has no implementors, an instance of a core module (`cross`, `ddt`) answers `module() == None` with no `builtin()` accessor.
- **Fix**: `Expr::children()`, `Expr::parent()`, `Stmt::children()`, `Decl::exprs()` (pre-order), a `Block` handle, `Instance::builtin()`, `Struct::impls()`/`Enum::impls()`/`Trait::impls()`, handles for `Arg` and `FieldInit` (`name`, `value`, `target`).

### A-14 (POM) Diagnostics the POM adds use `E0161` where the spec assigns other codes
- **Where**: `pom/resolve/classify.rs` (messages `no param`, `no port`, `no parameter`, `no field`).
- **Evidence**: `R(a, b) { zz: 1.0 }` → `E0161 no param 'zz' in 'R'` (spec: `E0320`). `g(1.0, d: 3.0)` → `E0161 no parameter 'd' in 'g'` (spec: `E0140`). `R(a, b, q: b)` → `E0161 no port 'q'` (spec: `E0140`, and §8.1.9 lets only optional value inputs be named). A bundle literal with an unknown field is `E0263`. `E0161` is "a path, `use` or `mod` that does not resolve".
- **Fix**: use the spec's codes, or state in `docs/pom/resolution.md` that the POM reports under `E0161` what elaboration re-reports precisely (and then two diagnostics will exist for one error).

### A-15 (POM) An access call resolves to a quantity of another nature
- **Where**: `pom/resolve/classify.rs:416` (`probe_callee`).
- **Evidence**: natures `E` (`access: v`, `i`) and `Th` (`access: v`, `pwr`), port `t: Th`: `i(t)` → callee resolved to `E.I`, class `Probe`, no diagnostic. §4 rule 10 resolves by the nature of the first argument, and `Th` declares no `i` (`E0206`).
- **Fix**: when the first argument's nature is known and declares no such access, leave the callee `Unresolved` (and report, or defer to `E0206`); never fall back to the first quantity of that name.

### A-16 (POM) A method on an array of carriers resolves to the element nature's `impl`
- **Where**: `pom/resolve/members.rs:132` (`receiver` looks through arrays).
- **Evidence**: `signal arr: [Bit; 2];` with `impl Bit { fn rise(self) -> Event }`: `arr.rise()` → `Decl(Fn impl Bit.rise)`, same as `arr[0].rise()`. §5 (`E0211`) forbids a carrier method on an array.
- **Fix**: look through the array only after an index; `arr.m()` stays `Unresolved`/`Deferred`.

### A-17 (Spec) "Everything else is Rust" is wider than the grammar the parser implements, and B.3.6 does not list the difference
- **Where**: §0.4, B.3.1 item 1, B.3.3 item 6, B.3.6.
- **Evidence** (each is rejected by the parser, none is listed as removed): `type T = f64;` and associated types; function-pointer types (`f: fn(f64) -> f64`, although §4 rule 8 gives `resolve` the type `fn(&[T]) -> T`); `impl Fn(f64) -> f64` in argument position; `where T: Bound` on `fn`/`impl`/`struct`/`trait`; qualified paths `<T as Tr>::X`; `pub(crate)`/`pub(super)`; `&self`, `mut x` and pattern parameters; `..5` patterns; `use ::a`; inner attributes; `!` and raw-pointer types. The messages mislead: `fn(f64) -> f64` as a type, `type`, `static`, `mut x` all give `E0903 '...' is a reserved keyword and cannot be used as an identifier`; `fn h(&self)` gives `unexpected '&'`.
- **Fix**: make B.3.6 the complete list of what is removed (decision D1), give each a targeted message ("type aliases are not part of the language", "write `self`, not `&self`").

### A-18 (Spec) The inherited Rust library surface is unbounded
- **Where**: §0.4 ("the standard methods of the value types"), §1 rule 1.
- **Evidence**: the spec's own code calls `unwrap_or`, `clamp`, `exp`, `powf`, `sqrt`, `rem_euclid`, `abs`, `max`, `is_some`, `contains`, and also `.iter().fold(..)`, `.iter().filter(..).count()`, `.sum()`: iterators and closure traits. Nothing says which methods, traits (`Iterator`, `IntoIterator`, `PartialOrd`, `Default`, ...) and types (`Range`, slices) exist. A type checker and a conformance test cannot be written against "Rust". The POM marks all of it `Deferred(ReceiverTypeUnknown)`.
- **Fix**: a closed catalogue (new appendix): per value type, the methods and associated consts; the traits a model may name in bounds and derives; the iterator adaptors. Decision D2.

### A-19 (Spec) `(0.1..=10.0).contains(&n)` contradicts §1 rule 2
- **Where**: `part_iii_structure.md:119`, `part_v_execution.md:686` vs §1 rule 2 and `E0102` ("`&` other than a shared borrow of an array, slice or string in a function parameter").
- **Evidence**: the example borrows an `f64` in an argument. The parser accepts it; a checker written from §1 rejects the spec's own param idiom.
- **Fix**: decision D7.

### A-20 (Spec) May a behavior block rename the module's generics?
- **Where**: §10 rule 6 and its Resolution ("bound positionally"), B.3.3 item 14.
- **Evidence**: `module M<T>(..)` with `analog<X> for M<X> { .. }` → the POM reports `E0403`; yet in a mismatching list it links names positionally (`digital<N, T> for M<N, T>` on `M<T, const N>`: `N` → generic `T`). "Positionally" reads as renaming allowed; "the generic names of the module" reads as not.
- **Fix**: decision D6; then make the POM's link and its diagnostic agree.

### A-21 (Spec) Where a module, a nature and a behavior block may be written
- **Where**: B.2.1 note 3 ("items at file scope only"), §8 rule 1 ("an item of the file namespace"), §3.4.1 (a namespace is a `mod` item or a file), `E0401` ("no module of the same file").
- **Evidence**: `mod a { nature N {..} module M() {} analog for M {} }` parses and binds with no diagnostic; `digital for a::M` written at the root, or in a sibling `mod b` as `super::a::M`, also binds (same file). Whether that is intended is not stated.
- **Fix**: say "namespace scope (a file or an inline `mod`)" in B.2.1 note 3 and §8 rule 1, and say whether a block may sit in another namespace of the same file than its module (recommended: yes, the rule is per file).

## MINOR

| Id | Layer | Where | Evidence (observed vs expected) | Correction |
| --- | --- | --- | --- | --- |
| A-22 | Parser | `msl/parser/msl.rs:360-380` | `pub analog for M { }` parses with no diagnostic; `behavior_block` has no `pub` | reject with `E0900` and a help line |
| A-23 | Parser | param block and template field lists | `R(p, n) { r }` and `R { r }` parse, and `print` rewrites them to `{ r: r }`; G.14 says the long form only is in force, and the authored shape is lost | reject in param blocks and templates until G.14 is decided, or keep a `shorthand` flag in the AST |
| A-24 | Parser | module body items | `q <- 1;` in a module body → `E0907 '<-' inside an expression`; `i(a) +> 1.0;` → `E0900 unexpected '('`. Both are well-formed statements in the wrong place (`E0404` wording) | report `E0404` with help ("a definition is `q = ..;`; a commit goes in a reaction") |
| A-25 | Parser | `msl/parser/entry.rs:328` | `E0910` fires on flat chains: a 300-term sum, 255 chained method calls, a 255-arm `else if`, 255 nested `Option<`, while 256 parentheses pass. Generated code hits this | count nesting only; parse left-associated chains iteratively (decision D8) |
| A-26 | Parser | item recovery | `module M<const N: u32>() where N > 0, N < 5 { }` → a spurious `E0900` at the `>` of the generics (recovery restarts at `const`) before the real one. `fn f() { let a = ; foo(; let c = 3; }` → the error at `;` is lost, the valid `let`s are reported "expected item", and `f` becomes four `Error` items. One bad port (`in a: ,`) turns the whole module into an `Error` item, so every use of it is `E0161`/`E0401` | resync on closing delimiters, not on item keywords inside `<..>`/`(..)`; recover a port list per port |
| A-27 | Parser | lexer | invalid escapes are never checked (`"\q"` is clean) and the cooked value is not available; a raw string is stored cooked, so `print` turns `r#"a"b\c"#` into `"a\"b\\c"`; an integer literal that fits no type (`99999999999999999999999999999999999999999999`) is clean | validate escapes (`E0900`), keep the raw spelling, add `Lit::str_value()`/`int_value()`/`suffix()` |
| A-28 | Parser | messages | `static X: f64 = 1.0;` → "`static` is a reserved keyword and cannot be used as an identifier" (B.3.6 removes `static` items: say so, name `const`); `let r#out = 1;` → `unexpected '#'` (say raw identifiers are not supported); `let y: f64 = 1.0;` in a module body → generic `E0900` (B.3.5 forbids the annotation: say so) | targeted messages |
| A-29 | Parser | leniency | accepted at parse and left to nobody: `nature A { }` (grammar needs one field), `signal g: (Bit, Bit);`, `signal h: &Bit;` (`sigtype` is a path or an array), `extern "C" fn a(x: f64) -> f64 { x }`, `fn a();` at file scope, `let x = { i(a) +> 1.0; 2.0 };` (a contribution inside a block-valued expression) | parse errors for the first three; codes for the rest (A-34, A-35) |
| A-30 | POM | `pom/index.rs:44` | a parent's drive of a child's public carrier is not a writer of it: `c.w = true;` and `c.w <- true;` leave `C.w.writers()` empty (the `Field` node does resolve to `C.w`) | index writers by the resolved member as well as by the root |
| A-31 | POM | `Generation::items()` | for an `if` it returns the first arm only; easy to misuse | return none for `if` or all arms' items; keep `arms()` |
| A-32 | POM | scopes stage | member tables are quadratic: one module with 2000 signals builds in 42 ms, with 8000 in 487 ms (release); `Design::module(name)` and `Module::signal(name)` are linear scans (220 µs per lookup at 4000 modules) | sorted name tables (`BTreeMap`, still deterministic) |
| A-33 | POM | `Design: Deserialize` | a design read from JSON is not validated; handles then `unreachable!`/index-panic on bad ids | `Design::from_json` that runs `check()` and fails cleanly |
| A-34 | Spec | §3 rule 2 | no rule or code for an `extern "C" fn` with a body or a plain `fn` without one | add to `E0125` or a new row |
| A-35 | Spec | B.3.4 | silent on a `contribution`/`commit` inside a block used as a value (`let x = { .. +> ..; 2.0 };`, `let y = if c { q <- 1; 2 } else { 3 };`) | allow only where the block is itself a statement, or state that it is allowed |
| A-36 | Spec | `E0121` vs `E0404` | `+>`/`<-`/`on` in a `fn` matches both rows (§3 rule 6; §10 rule 8 "nor inside a closure or a `fn`"). The parser picks `E0121` for `fn`, `E0404` for a closure | state the precedence in both tables |
| A-37 | Spec | A.2, §1 rule 7 | a float literal out of `f64` range and an integer literal out of range have no rule (Rust denies both by lint, which the language does not have) | decision D12 |
| A-38 | Spec | B.3.3 item 10, §8.5.7 | "checked after parsing" but `E0370` is an elaboration code; `extern "cosim" module X(in a: Bit) -> f64 { signal s: Bit; s = a; 1.0 }` is clean through parse and POM | say which phase; the POM can report it (purely structural) |
| A-39 | Spec + docs | B.3.2 `place`, `docs/pom/mapping.md` (`place` row), `reflection.md` (`Signal::writers`) | the POM docs list `(s)` as a place; the grammar has no parenthesized place; the parser rejects `(q) = 1;` and accepts `(q) <- 1;` | pick one; recommended: no parentheses |
| A-40 | Spec | §3.4, resolution | a glob that brings one name from two namespaces, and a `use` that collides with a local item, are undefined (see A-10) | cover in the new duplicate-name rule |
| A-41 | POM docs | `docs/pom/mapping.md` | maps the B.3 productions, not the 65 constructs of B.1: named arguments (`Arg::name`), SI literal (`LitKind::Float.raw`), `Unknown`, value input, result, bundle direction, `Fn(P)` bound, `pub` on body items, ABI strings, `gnd`, `m`, `dtemp` have no row | add a B.1 table with one row per construct and a test like `every_b3_production_is_mapped` |
| A-42 | POM tests | `tests/fixtures/pom/prelude.msl`, `pom_corpus.rs` | the prelude holds stand-ins (`Nature`, `Logic`, `init`, `State`) and no test builds a gallery model with zero diagnostics; no negative test for a commit place; no serde test with a non-finite or huge literal; no test for an identifier pattern naming a constant | add with A-01, A-02, A-03, A-04, A-11 |
| A-43 | POM | `Path::text()` on use leaves | see A-08: `a::{self as aa, n::{self, J` | leaf spans |
| A-44 | POM | built-in instances | `cross(x)`, `ddt(x)` are `CallClass::Instance { module: None, via: None }`; the only way to know which primitive is to resolve the callee path by hand | `Instance::builtin() -> Option<BuiltinId>` |
| A-45 | POM | struct patterns | `match s { S { a, q } => a }` with no field `q`: silent, while `S { a: 1.0, c: 2.0 }` reports `no field 'c'` | same treatment for both |
| A-46 | Parser | `msl/parser/check.rs:47-70` | `E0143` ("overload") is the only duplicate-name check and uses a message about signatures; it fires for `fn a() {} fn a(x: f64) {}` but two `const`s or two modules pass (A-10) | fold into the namespace rule; keep `E0143` for the message "functions are not overloaded" |

## Spec decisions needed

| # | Question | Recommendation |
| --- | --- | --- |
| D1 | Which Rust forms are in the language? (A-17) | Make B.3.6 exhaustive. Keep out: type aliases, associated types, trait-bound `where`, `impl Trait`, qualified paths, restricted `pub(..)`, labels, pattern and `mut` parameters, `&self`. Take in: function-pointer types `fn(T) -> U` (needed to write the type §4 rule 8 names) and a negative literal as a const generic argument (A-06) |
| D2 | Which Rust library methods, traits and types exist? (A-18) | A closed catalogue appendix: numeric methods of `f64`/integers, `Option` combinators, arrays and slices (`len`, `iter`, indexing), ranges (`contains`, iteration), a short list of iterator adaptors (`map`, `filter`, `fold`, `sum`, `count`, `zip`, `enumerate`, `rev`, `any`, `all`), derivable traits. Everything else is `E0161` |
| D3 | Are `gnd`, `Nature`, `m`, `dtemp`, `str` core names or prelude source? (A-01) | Core (built-in table). `gnd` cannot be written in the language; B.1 already counts all of them as additions. Reword §6 rule 11 ("a core name") |
| D4 | Is the stdlib a normative source? (A-03) | Yes: ship `std/*.msl`, make Part VII's fences excerpts of it, `pub` on exports, and gate on "galleries build clean" |
| D5 | Duplicate names in a namespace, colliding imports, ambiguous globs (A-10, A-40) | One new `E016x` in §3.4, Rust's rules: one name per table per namespace; an explicit item or `use` beats a glob; two globs are an error only when the name is used |
| D6 | Generic names of a behavior block: must they equal the module's? (A-20) | Yes, same names in the same order (what the POM enforces); reword the Resolution from "bound positionally" to "repeated" |
| D7 | `&` in argument position (A-19) | Allow a shared borrow of any value as a call argument (read-only, no lifetime escapes) and narrow `E0102` to the rest; the alternative is to rewrite the two examples as `n >= 0.1 && n <= 10.0` |
| D8 | What does `E0910` count? (A-25) | Nesting only (delimiters, blocks, unary and type nesting). Flat binary chains, method chains and `else if` chains are not nesting; give them a separate, larger bound or none |
| D9 | Where may doc comments and attributes be written? (A-07) | Everywhere Rust takes an outer attribute, plus reactions, definitions, generation and nature quantities. Docs on a quantity and on a reaction go to the metadata of §21; docs on a statement are kept in the tree and otherwise ignored |
| D10 | Identifier patterns (A-02) | Confirm Rust's rule in Part I: a plain identifier pattern that resolves to a constant, unit variant or unit struct is that path, not a binding |
| D11 | Phase and code of structural checks the POM can already do (A-14, A-38) | Let the POM report `E0320`, `E0140`, `E0370`, `E0201` (repeated or mixed nature fields) under the spec's codes; they need no types |
| D12 | Out-of-range literals (A-11, A-37) | A parse error (`E0900` row, or a new `E0909`): a float literal whose nearest `f64` is infinite; an integer literal that exceeds its suffix type, or 128 bits without a suffix |
| D13 | Where items may sit (A-21) | "Namespace scope": files and inline `mod`s; a block and its module in the same file, any namespace of it |
| D14 | Codes for misplaced statements (A-24, A-36) | `E0404` for `<-`, `+>`, `on`, `signal` in any block or body that is not theirs, a module body included; `E0121` only for a `fn` body, and say it wins there |
| D15 | G.15 raw identifiers | Decide now: accept `r#name` for the seven added keywords. `out`, `on`, `after`, `signal`, `module` are everyday port and net names, and the planned netlist import (G.8) will meet them |
| D16 | G.14 field-init shorthand (A-23) | Decide now, since the parser already takes it: accept `{ r }` in param blocks and templates as in Rust struct expressions, and keep the authored form in the AST |
| D17 | `(s)` as a place (A-39) | No: a place is a path with indexes and fields; reject parentheses in both definitions and commits |
