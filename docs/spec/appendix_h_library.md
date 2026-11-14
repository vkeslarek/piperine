# Appendix H. Inherited library

This appendix is normative. It is the closed catalogue of the names a model takes from Rust's standard library: the methods of the numeric types, `Option`, arrays and slices, ranges and iterators, and the traits and types a model may name. §0.4 says everything else is Rust; this appendix draws the line for the library.

1. A method, trait or type of Rust's library that this appendix does not list does not resolve (`E0161`). The catalogue is closed.
2. The carrier and value methods of B.1 (`changed`, `prev`, `others`, `inflow`, `level`) and the functions a model declares are not in this appendix.
3. A method listed here has Rust's behavior, edition 2024, with the value semantics of §1. A panic is a panic of §1 rule 11.

> **Resolution.** Without a list, "everything not stated is Rust" admits `Vec`, threads and I/O, which §1 rule 2 removes. A method a fence of this specification calls is a row below; the type checker is written against these rows.

## H.1 Integer types

`uN`, `iN`, `UInt<N>`, `Int<N>`, `usize` and `isize`. Arithmetic wraps (§1).

| Name | Signature | Note |
| --- | --- | --- |
| `abs` | `fn abs(self) -> Self` | signed types only |
| `min` | `fn min(self, o: Self) -> Self` |  |
| `max` | `fn max(self, o: Self) -> Self` |  |
| `clamp` | `fn clamp(self, lo: Self, hi: Self) -> Self` | panics if `lo > hi` |
| `pow` | `fn pow(self, e: u32) -> Self` | wraps |
| `rem_euclid` | `fn rem_euclid(self, d: Self) -> Self` | panics if `d` is zero |
| `signum` | `fn signum(self) -> Self` | signed types only |
| `checked_add` | `fn checked_add(self, o: Self) -> Option<Self>` | `None` on overflow |
| `checked_sub` | `fn checked_sub(self, o: Self) -> Option<Self>` | `None` on overflow |
| `checked_mul` | `fn checked_mul(self, o: Self) -> Option<Self>` | `None` on overflow |
| `saturating_add` | `fn saturating_add(self, o: Self) -> Self` | clamps at the bounds |
| `saturating_sub` | `fn saturating_sub(self, o: Self) -> Self` | clamps at the bounds |
| `saturating_mul` | `fn saturating_mul(self, o: Self) -> Self` | clamps at the bounds |

## H.2 Float types

`fN` and `Float<N>` for `N` in {16, 32, 64, 128}.

| Name | Signature | Note |
| --- | --- | --- |
| `abs` | `fn abs(self) -> Self` |  |
| `sqrt` | `fn sqrt(self) -> Self` | NaN below zero |
| `powf` | `fn powf(self, e: Self) -> Self` |  |
| `powi` | `fn powi(self, e: i32) -> Self` |  |
| `exp` | `fn exp(self) -> Self` |  |
| `ln` | `fn ln(self) -> Self` |  |
| `log10` | `fn log10(self) -> Self` |  |
| `log2` | `fn log2(self) -> Self` |  |
| `sin` | `fn sin(self) -> Self` |  |
| `cos` | `fn cos(self) -> Self` |  |
| `tan` | `fn tan(self) -> Self` |  |
| `asin` | `fn asin(self) -> Self` |  |
| `acos` | `fn acos(self) -> Self` |  |
| `atan` | `fn atan(self) -> Self` |  |
| `atan2` | `fn atan2(self, x: Self) -> Self` |  |
| `sinh` | `fn sinh(self) -> Self` |  |
| `cosh` | `fn cosh(self) -> Self` |  |
| `tanh` | `fn tanh(self) -> Self` |  |
| `floor` | `fn floor(self) -> Self` |  |
| `ceil` | `fn ceil(self) -> Self` |  |
| `round` | `fn round(self) -> Self` |  |
| `trunc` | `fn trunc(self) -> Self` |  |
| `fract` | `fn fract(self) -> Self` |  |
| `min` | `fn min(self, o: Self) -> Self` |  |
| `max` | `fn max(self, o: Self) -> Self` |  |
| `clamp` | `fn clamp(self, lo: Self, hi: Self) -> Self` | panics if `lo > hi` |
| `signum` | `fn signum(self) -> Self` |  |
| `rem_euclid` | `fn rem_euclid(self, d: Self) -> Self` |  |
| `hypot` | `fn hypot(self, o: Self) -> Self` |  |
| `is_nan` | `fn is_nan(self) -> bool` |  |
| `is_finite` | `fn is_finite(self) -> bool` |  |
| `is_infinite` | `fn is_infinite(self) -> bool` |  |

## H.3 `bool`

| Name | Signature | Note |
| --- | --- | --- |
| `then` | `fn then<T>(self, f: impl Fn() -> T) -> Option<T>` | `Some(f())` when true |
| `then_some` | `fn then_some<T>(self, v: T) -> Option<T>` | `Some(v)` when true |

## H.4 `Option`

| Name | Signature | Note |
| --- | --- | --- |
| `is_some` | `fn is_some(&self) -> bool` |  |
| `is_none` | `fn is_none(&self) -> bool` |  |
| `unwrap` | `fn unwrap(self) -> T` | panics on `None` (§1 rule 11) |
| `expect` | `fn expect(self, msg: &str) -> T` | panics on `None` |
| `unwrap_or` | `fn unwrap_or(self, d: T) -> T` |  |
| `unwrap_or_else` | `fn unwrap_or_else(self, f: impl Fn() -> T) -> T` |  |
| `map` | `fn map<U>(self, f: impl Fn(T) -> U) -> Option<U>` |  |
| `and_then` | `fn and_then<U>(self, f: impl Fn(T) -> Option<U>) -> Option<U>` |  |
| `or` | `fn or(self, o: Option<T>) -> Option<T>` |  |

## H.5 Arrays and slices

`[T; N]` and `&[T]`.

| Name | Signature | Note |
| --- | --- | --- |
| `len` | `fn len(&self) -> usize` |  |
| `is_empty` | `fn is_empty(&self) -> bool` |  |
| `iter` | `fn iter(&self) -> Iterator<&T>` | an iterator, see H.7 |
| `contains` | `fn contains(&self, x: &T) -> bool` | `T: PartialEq` |
| `first` | `fn first(&self) -> Option<&T>` |  |
| `last` | `fn last(&self) -> Option<&T>` |  |
| `get` | `fn get(&self, i: usize) -> Option<&T>` | `None` past the end |

## H.6 Ranges

`a..b` and `a..=b`. A range is also an iterator (H.7).

| Name | Signature | Note |
| --- | --- | --- |
| `contains` | `fn contains(&self, x: &T) -> bool` | `a..b`, `a..=b` |
| `is_empty` | `fn is_empty(&self) -> bool` |  |

## H.7 Iterator adaptors

The ten adaptors and consumers below, on an array's `iter()` and on a range.

| Name | Signature | Note |
| --- | --- | --- |
| `map` | `fn map<U>(self, f: impl Fn(T) -> U) -> Iterator<U>` |  |
| `filter` | `fn filter(self, f: impl Fn(&T) -> bool) -> Iterator<T>` |  |
| `fold` | `fn fold<A>(self, init: A, f: impl Fn(A, T) -> A) -> A` |  |
| `sum` | `fn sum(self) -> T` | numeric `T` |
| `count` | `fn count(self) -> usize` |  |
| `zip` | `fn zip<U>(self, o: Iterator<U>) -> Iterator<(T, U)>` |  |
| `enumerate` | `fn enumerate(self) -> Iterator<(usize, T)>` |  |
| `rev` | `fn rev(self) -> Iterator<T>` | arrays and ranges |
| `any` | `fn any(self, f: impl Fn(T) -> bool) -> bool` |  |
| `all` | `fn all(self, f: impl Fn(T) -> bool) -> bool` |  |

## H.8 Traits

The traits a model may name in a bound or a derive. `Repr` (§2), `Nature` (§4) and `Fn` (§8.4) are the language's own and are not listed here. A name inside `#[derive(..)]` resolves like a name in a bound: it must be one of these traits, `Repr`, or a trait the project declares; any other name (an unknown word, a type, a value) is `E0161` at the name.

| Trait | Written as | Note |
| --- | --- | --- |
| `Clone` | `#[derive(Clone)]`, `T: Clone` |  |
| `Copy` | `#[derive(Copy)]`, `T: Copy` |  |
| `PartialEq` | `#[derive(PartialEq)]`, `T: PartialEq` | `==`, `!=` |
| `Eq` | `#[derive(Eq)]`, `T: Eq` |  |
| `PartialOrd` | `#[derive(PartialOrd)]`, `T: PartialOrd` | `<`, `<=`, `>`, `>=` |
| `Ord` | `#[derive(Ord)]`, `T: Ord` |  |
| `Default` | `#[derive(Default)]`, `T: Default` |  |
| `Debug` | `#[derive(Debug)]`, `T: Debug` |  |
| `Hash` | `#[derive(Hash)]`, `T: Hash` |  |

## H.9 Types

The types a model may name besides the carriers of Part II and the types it declares.

| Type | What it is | Defined in |
| --- | --- | --- |
| `bool` | the boolean type | §1 |
| `UInt<N>` | unsigned integer of `N` levels; `uN`, `usize` | §1 rule 5 |
| `Int<N>` | signed integer of `N` levels; `iN`, `isize` | §1 rule 5 |
| `Float<N>` | IEEE 754 float; `fN` | §1 |
| `Option<T>` | an optional value | §1, §3.2 |
| `str` | the type of `&'static str` | §1 |
