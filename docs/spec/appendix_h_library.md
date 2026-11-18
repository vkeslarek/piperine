# Appendix H. Value library

This appendix is normative. It is the closed catalogue of the library of the value language: the methods of the numeric types, `Option`, arrays and slices, ranges and iterators, and the traits and types a model may name without declaring them. Each entry is given with its signature and its meaning. §0.4 says the specification is complete; this appendix is where that holds for the library.

1. A method, trait or type that this appendix does not list, and that neither the model nor the standard library (§22) declares, does not resolve (`E0161`). The catalogue is closed.
2. The carrier and value methods of B.1 (`changed`, `prev`, `others`, `inflow`, `level`) and the functions a model declares are not in this appendix.
3. A method listed here has the behavior its section states, in every analysis and at elaboration alike (§3.9.7). A panic is a panic of §1 rule 11.
4. The signatures use the notation of §3 with four spellings that are not source syntax. `&self` says that the method reads its receiver and does not move it (§1.4.7). `impl Fn(A) -> R` stands for any function or closure that satisfies the bound `Fn(A) -> R` (§3.8.6). `Iterator<T>` stands for an iterator of elements of type `T` (H.7). `&str` is `&'static str` (§1.2.9).
5. A method with an unknown level in its receiver or in an argument is all-or-nothing (§3.3.2), unless its body is written with the control flow of §3.3.3: `filter`, `any`, `all` and `then` are, through their closures.

> **Resolution.** Without a list, the library of a language has no edge. A method a fence of this specification calls is a row below; the type checker is written against these rows.

> **Note (informative).** The names and signatures are those of the same methods of Rust's standard library, so a reader who knows them will find no surprise. The meanings are stated here; where MSL differs (integer overflow, the 64-bit `usize`), §0.4 lists the difference.

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

**Dynamic semantics**

1. `abs` is the absolute value. For the least value of `Int<N>`, whose magnitude does not fit, it wraps to that same value.
2. `min` and `max` are the lesser and the greater of the two.
3. `clamp` is `lo` when the value is below `lo`, `hi` when it is above `hi`, and the value otherwise.
4. `pow` is the value raised to `e`, modulo `2^N`. Any value to the power 0 is 1.
5. `rem_euclid` is the remainder `r` with `0 <= r < |d|` such that the value is `q · d + r` for an integer `q`. For the least value of `Int<N>` and `d = -1` it is 0.
6. `signum` is `-1`, `0` or `1` as the value is negative, zero or positive.
7. `checked_add`, `checked_sub` and `checked_mul` are `Some` of the mathematical result when it fits the type, and `None` otherwise.
8. `saturating_add`, `saturating_sub` and `saturating_mul` are the mathematical result when it fits the type, the greatest value of the type when it lies above, and the least when it lies below.

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

**Dynamic semantics**

1. `abs`, `floor`, `ceil`, `trunc`, `round`, `fract`, `min`, `max`, `clamp`, `signum`, `sqrt` and the three predicates are exact: each yields the value of the format that IEEE 754 defines. `floor` is the greatest integer not above the value, `ceil` the least not below, `trunc` the integer part toward zero. `round` is the nearest integer, a tie going away from zero. `fract` is the value minus its `trunc`.
2. `sqrt` is the correctly rounded square root. It is NaN for a value below zero, and `-0.0` for `-0.0`.
3. `min` and `max` are the lesser and the greater. When exactly one operand is NaN the result is the other operand.
4. `clamp` follows H.1 rule 3. It panics if `lo > hi` or if `lo` or `hi` is NaN. A NaN value stays NaN.
5. `signum` is `1.0` for a positive value, `+0.0` and `+inf` included, `-1.0` for a negative value, `-0.0` and `-inf` included, and NaN for NaN.
6. `exp`, `ln`, `log10`, `log2`, `powf`, `powi`, the trigonometric and hyperbolic functions, `atan2` and `hypot` are the mathematical functions of those names, in radians. The result is the mathematical value rounded to the format, with an error of at most one unit in the last place; within that bound it is implementation-defined. An implementation **MUST** give one result for one argument throughout a session (G.69).
7. Outside the domain of a function the result is NaN: `ln` and the logarithms of a value below zero, `asin` and `acos` of a magnitude above 1, `powf` of a negative base with an exponent that is not an integer. `ln(0.0)` is `-inf`. A result too large for the format is an infinity.
8. `y.atan2(x)` is the angle of the point `(x, y)`, from `-π` to `π`. `x.hypot(y)` is the square root of `x² + y²`, computed without overflow in the squares.
9. `rem_euclid` is the remainder `r = x - d · q`, with `q` the integer that makes `0 <= r < |d|`, rounded to the format; `d = 0.0` gives NaN.
10. `is_nan` is true for NaN. `is_infinite` is true for the two infinities. `is_finite` is true for every other value.

## H.3 `bool`

| Name | Signature | Note |
| --- | --- | --- |
| `then` | `fn then<T>(self, f: impl Fn() -> T) -> Option<T>` | `Some(f())` when true |
| `then_some` | `fn then_some<T>(self, v: T) -> Option<T>` | `Some(v)` when true |

**Dynamic semantics**

1. `b.then(f)` is `Some(f())` when `b` is true and `None` otherwise. `f` is called only when `b` is true.
2. `b.then_some(v)` is `Some(v)` when `b` is true and `None` otherwise. `v` is an argument and is always evaluated.

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

**Dynamic semantics**

1. `is_some` is true for a `Some`, `is_none` for `None`.
2. `unwrap` and `expect` yield the payload of a `Some` and panic on `None`. The diagnostic of `expect` carries `msg`.
3. `unwrap_or(d)` yields the payload of a `Some`, and `d` on `None`; `d` is always evaluated. `unwrap_or_else(f)` yields `f()` on `None` and calls `f` only then.
4. `map(f)` is `Some(f(v))` for `Some(v)` and `None` for `None`. `and_then(f)` is `f(v)` for `Some(v)` and `None` for `None`.
5. `a.or(b)` is `a` when `a` is a `Some`, and `b` otherwise.

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

**Dynamic semantics**

1. `len` is the number of elements, and `is_empty` is `len() == 0`. For an array `[T; N]`, `len` is `N`.
2. `iter` yields a borrow of each element, from index 0 upwards (H.7).
3. `contains(x)` is true when some element equals `*x` under `==`.
4. `first` and `last` are `Some` of a borrow of element 0 and of the last element, and `None` when there is no element.
5. `get(i)` is `Some` of a borrow of element `i`, and `None` when `i >= len()`. It never panics; `xs[i]` does (§1.5.25).

## H.6 Ranges

`a..b` and `a..=b`. A range is also an iterator (H.7).

| Name | Signature | Note |
| --- | --- | --- |
| `contains` | `fn contains(&self, x: &T) -> bool` | `a..b`, `a..=b` |
| `is_empty` | `fn is_empty(&self) -> bool` |  |

**Dynamic semantics**

1. `(a..b).contains(x)` is `a <= *x && *x < b`, and `(a..=b).contains(x)` is `a <= *x && *x <= b`. A range without one bound (`a..`, `..b`, `..=b`) tests the bound it has, and `..` contains every value. With a float bound the comparisons are those of §1.5.23, so no range contains NaN.
2. `(a..b).is_empty()` is `!(a < b)`, and `(a..=b).is_empty()` is `!(a <= b)`.
3. As an iterator, `a..b` over an integer type yields `a`, `a + 1`, …, `b - 1`, and `a..=b` yields `a` through `b`. An empty range yields nothing. Ranges over a float type and ranges without a lower or an upper bound are not iterators.

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

**Static semantics**

1. An iterator is a value that yields a finite sequence of elements, in order, once. It comes from `iter()` of an array or a slice (H.5), from an integer range (H.6), or from an adaptor below. Its type cannot be named; it can be bound by a `let`, passed to a `for` (§1.8.9) and be the receiver of the methods of this section.
2. `map`, `filter`, `zip`, `enumerate` and `rev` are adaptors: each yields a new iterator. `fold`, `sum`, `count`, `any` and `all` are consumers: each reads the sequence and yields a value.
3. `rev` is allowed on `iter()` of an array or a slice, on an integer range, and on a `map`, `enumerate`, `zip` or `rev` of those. `sum` requires an integer or a float element type.

**Dynamic semantics**

4. An adaptor does nothing by itself. Its closure runs when a consumer or a `for` asks for an element, once per element, in the order of the sequence.
5. `map(f)` yields `f(x)` for each element `x`. `filter(f)` yields the elements for which `f` is true; `f` receives a borrow.
6. `a.zip(b)` yields the pairs `(x, y)` of the elements at the same position, and ends when either ends.
7. `enumerate` yields `(k, x)` with `k` counting from 0, of type `usize`. `rev` yields the same elements from the last to the first.
8. `fold(init, f)` starts with `init` and replaces it by `f(acc, x)` for each element; its value is the last `acc`. For an empty sequence it is `init`.
9. `sum` adds the elements with `+`, from the first to the last, starting from zero: integers wrap (§1 rule 6) and floats round at each addition. `count` is the number of elements.
10. `any(f)` is true when `f` is true for some element; it stops at the first one. `all(f)` is true when `f` is true for every element; it stops at the first for which it is false. On an empty sequence `any` is false and `all` is true.

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

**Static semantics**

1. `Clone`: a value of the type can be duplicated, and `x.clone()` yields the duplicate. `Copy`: the type is copied wherever it is read (§1.4.6); it requires `Clone`.
2. `PartialEq` gives `==` and `!=`. Derived, two values are equal when they are the same variant and their fields are equal, field by field. `Eq` adds nothing a model calls: it states that every value equals itself, and requires `PartialEq`.
3. `PartialOrd` gives `<`, `<=`, `>` and `>=`. Derived, a struct orders by its fields in declaration order, the first difference deciding; an enum orders by the discriminant of the variant (§1.3.7), then by the fields. It requires `PartialEq`. `Ord` adds nothing a model calls: it states that any two values are ordered, and requires `Eq` and `PartialOrd`.
4. `Default` gives `T::default()`. Derived on a struct, each field has its own default. The defaults of the built-in types are `false`, zero, `None`, `()`, and the tuple or array of defaults. An enum does not derive `Default`.
5. `Debug` and `Hash` add nothing a model calls. A type may state them, and a bound may require them.
6. A derive on a struct or an enum requires every field to implement the trait (§2 rule 3 for `Repr`). A derive on a generic type holds for the arguments that implement it.
7. The built-in types implement these traits:
    - `bool`, `UInt<N>`, `Int<N>` and `()`: all nine;
    - `Float<N>`: `Clone`, `Copy`, `PartialEq`, `PartialOrd`, `Default` and `Debug`;
    - `&str`: `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Debug` and `Hash`;
    - a tuple, an array `[T; N]` and an `Option<T>`: each trait that every part implements;
    - a function pointer: `Clone`, `Copy` and `Debug`.

> **Resolution.** A float has no `Eq`, `Ord` or `Hash` because NaN is not equal to itself (§1.5.23). A type with a float field therefore derives `PartialEq` and `PartialOrd` only.

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

The unit type, tuples, arrays, slices, borrows and function pointers are written with the type syntax of §1.2 and have no name. A range, an iterator and a closure have types that cannot be written (§1.5.11, H.7, §1.2.11). `Event` (§13), `Analysis` (§11.1) and the natures and bundles are the types of Parts II to IV.
