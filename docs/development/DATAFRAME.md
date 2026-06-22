# DataFrame — a typed, analysis-independent result container

## Why

Today an analysis hands back `AnalysisResult` — an *unordered* `HashMap<String,
VectorData>` of named vectors, wrapped per-vector as `Signal`. That's fine for
"probe one signal," but it loses structure the moment you want to *analyze the
data*: column order, dtypes, which column is the sweep axis, and — crucially —
the **dimensionality** (a 2-D DC sweep, a Monte-Carlo batch, AC over corners).

`DataFrame` is one internal type that captures all of it, **independent of which
analysis produced it**. The bet: pay the complexity once, here, so everything
downstream gets simple:

- **Data analysis** in Piperine — select / filter / derive / aggregate columns
  with the same API regardless of `op`/`tran`/`ac`/Monte-Carlo.
- **Optimization loops** — read a DataFrame, compute a cost, alter, re-run.
- **Export** — column-oriented contiguous arrays map cleanly to numpy / polars /
  pandas via PyO3, and to CSV / plotting, with no per-analysis special-casing.

This is a deliberate tradeoff: concentrate the complexity in one well-shaped type
instead of scattering it across every consumer. That's the right place for it.

## The type (Rust)

Column-oriented, one dtype per column, all columns the same length — the layout
that exports to numpy/polars without a reshape.

```rust
/// A single typed column. Contiguous so it maps 1:1 to a numpy array.
pub enum Column {
    Real(Vec<f64>),
    Complex(Vec<(f64, f64)>),
    Int(Vec<i64>),
    Str(Vec<String>),
    Bool(Vec<bool>),            // masks from comparisons
}

pub struct DataFrame {
    names:   Vec<String>,       // column order
    columns: Vec<Column>,       // parallel to `names`, equal length
    /// Columns that form the (possibly multi-dimensional) index/axes:
    /// `["time"]` for tran, `["frequency"]` for ac, `["v1","v2"]` for a 2-D DC
    /// sweep, `["run","time"]` for Monte Carlo. Logical N-D, physical 2-D.
    index:   Vec<usize>,
    meta:    Meta,              // analysis kind, plot name, per-column units
}
```

- **Handle semantics** (like `ArrayObj`): wrap as `Arc<Mutex<DataFrame>>` inside a
  `Value::ExternObject`, so passing a frame around shares storage and derived
  columns are cheap.
- **Multi-dimensionality is tidy/long, not a tensor.** Extra *index columns*
  encode the axes. A DC sweep `v1 × v2` is columns `[v1, v2, v(out), …]` with
  `index = [0, 1]`. This stays 2-D in memory (numpy-friendly) while being N-D in
  meaning, and it is exactly the shape pandas/polars want.

## How analyses produce it

Every analysis already collects an `AnalysisResult`. A `DataFrame::from_result`
builder orders the columns, infers dtypes (real vs complex), and tags the index
column by `AnalysisKind` (`time`/`frequency`/the swept source). So:

```
$tran / $ac / $dc / $noise / …  →  AnalysisResult  →  DataFrame
```

Result objects expose it: `tran_res.frame()` → `DataFrame`. Monte-Carlo / corner
loops `concat` per-run frames, adding a `run`/`corner` index column — giving one
long DataFrame for the whole batch, analysis-type-agnostic.

## Piperine surface (target)

```verilog
DataFrame df = $tran(1e-9, 1e-6).frame();

// access
Signal vout = df["v(out)"];          // string indexing
Signal t    = df.index();            // the axis column(s)
real n      = df.nrows();

// vectorized math → new Series (no manual loops)
Signal power = df["v(out)"] * df["i(out)"];
Signal mask  = df["v(out)"] > 1.0;   // boolean Series

// transform / select / filter
DataFrame hi = df.filter(df["v(out)"] > 0.9);
DataFrame sub = df.select('{"time", "v(out)"}');
df = df.with_column("p", power);

// reduce
real e = df["p"].integral();
real pk = df["v(out)"].max();

// export
df.to_csv("run.csv");
```

Core methods: `cols()`, `nrows()/ncols()/shape()`, `index()`, `col(name)` /
`df[name]`, `select(names)`, `filter(mask)`, `with_column(name, series)`,
`slice(lo,hi)`, `head(n)`, `concat(other)`, `groupby(index)`, `to_csv(path)`,
column reductions, and a tabular `Display` for `$display(df)`.

## What the language must support (the ergonomics work)

The DataFrame *type* is straightforward Rust. Making it **ergonomic in Piperine**
needs language features we don't have yet. Ordered by leverage:

### 1. String / key indexing — `df["v(out)"]`  *(small, required)*

`Expr::Index` currently evaluates only integer indices (`.get(i)`). Extend it so a
non-integer key dispatches to a string getter (`col`/`get`). Same change makes
associative arrays `aa["key"]` work — shared with Phase 9.

### 2. Operator overloading on objects — `df["a"] + df["b"]`, `s > 1.0`  *(medium, highest leverage)*

This is the single biggest ergonomics win: vectorized Series arithmetic and
comparison masks instead of hand-written `foreach` loops. `eval_binary_op` today
only handles `Real`/`Integer`/`String`; when either operand is an `ExternObject`
it should dispatch to a method on `ExternClass` (e.g. `binary_op(op, rhs)` /
`__add__`). Then `Signal`/`Column` implement element-wise ops returning a new
Series, and comparisons return a `Bool` Series usable by `filter`.

### 3. Lambdas / `with` iterator clause — `df.map(x -> x*2)`, `q.find() with (item > 2)`  *(large, unlocks general transforms)*

Generic `filter`/`map`/`apply`/derived columns need a way to pass an expression
parameterized by a row/element. Piperine has no closures. Two routes:
- SystemVerilog `with (item …)` clause on methods (matches the array-locator
  spec in `SYSTEMVERILOG_FEATURES.md §12.5`), or
- arrow lambdas `x -> expr`.
Either needs: parser support, a callable `Value`, and interpreter eval in a child
scope. High effort, but it's what makes column transforms feel like pandas. Until
then, #2 (vectorized ops + mask `filter`) covers most real cases without lambdas.

### 4. Row slicing — `df[10:100]`, `s[0:50]`  *(small–medium)*

`Expr::PartSelect` (`a[lo:hi]`) already parses but the interpreter rejects it.
Map it to `.slice(lo, hi)` on frames/series.

### 5. Field-access sugar — `df.time`, `s.values`  *(small, optional)*

`obj.member` with no parens currently can't reach a getter. Allow a dotted path
whose head is an `ExternObject` and whose tail takes no args to call a zero-arg
method. Pure sugar over `df["time"]` / `s.values()`.

### 6. Tuple / multi-assignment — `(n, m) = df.shape();`  *(small, optional)*

Nice for shape/destructuring. Workaround today: `df.shape()` returns an array and
index it.

### 7. Tabular display & to-string — `$display(df)`  *(small)*

A `Display` that prints a head/tail table. Helps every debugging session.

## Export path (later, not blocking)

Because columns are contiguous single-dtype `Vec`s:
- **CSV** — trivial, from `names` + `columns`.
- **PyO3** — each `Column` → a numpy array (`f64`/`i64`/complex); the frame → a
  dict-of-arrays or a `polars`/`pandas` DataFrame. The `index` set maps to a
  pandas `MultiIndex`. No per-analysis code.
- **Plotting** — hand `index` + value columns to any plotter downstream.

Keeping the dtype-per-column invariant is what makes all three free.

## Recommended order

1. **#1 string indexing** + `DataFrame` type + `from_result` + `to_csv` + display
   — usable frames you can inspect and dump.
2. **#2 operator overloading** → vectorized `Signal`/`Column` math + mask `filter`
   — the ergonomics inflection point; most analysis becomes loop-free.
3. **#4 slicing**, **#5 field sugar**, **#6 tuples** — polish.
4. **#3 lambdas / `with`** — general transforms; do once the simpler wins land.
5. Monte-Carlo `concat` + `groupby`, then the PyO3 bridge.

This slots into the [ROADMAP](ROADMAP.md): the type and #1/#2/#4 belong with the
data/results work (Phases 4–7); #3 and assoc-arrays are Phase 9 language items.
DataFrame is the through-line that makes all of them pay off together.

---

# Implementation refinement (build this)

Implementation-ready spec, PHASE-style. Grounded in the current code. Build in the
order of §7. Each step is independently shippable and testable with a `MockBackend`
(assert on values, no real simulator).

## 0. What already routes for free

- **String indexing `df["v(out)"]`** needs **no interpreter change**.
  `Expr::Index(base, idx)` already evaluates `base` and calls
  `obj.call_method("get", &[idx])` (interpreter.rs `Expr::Index` arm). So `df["col"]`
  arrives at `DataFrameObj::call_method("get", [String("col")])`. The DataFrame's
  `get` just branches on the arg: `String → column (Signal)`, `Integer → row`.
- **Indexed assignment `df["new"] = sig`** already routes to `call_method("set",
  [idx, value])` (the `Stmt::Assign` Index branch). DataFrame `set` adds/replaces a
  column when the key is a string.
- **Method calls / chaining** (`df.filter(...).to_csv(...)`) already work via the
  ExternObject dispatch.

Two real language changes remain: **operator overloading** (§3) and **slicing** (§4).

## 1. The Rust type

`crates/piperine-interpreter/src/extern_types.rs` (new `DataFrameObj`, alongside
`SignalObj`/`ArrayObj`). Column-oriented, one dtype per column, handle semantics
(`Arc<Mutex<…>>`) like `ArrayObj`.

```rust
#[derive(Debug, Clone)]
pub enum Column {
    Real(Vec<f64>),
    Complex(Vec<(f64, f64)>),
    Int(Vec<i64>),
    Str(Vec<String>),
    Bool(Vec<bool>),     // comparison masks
}

impl Column {
    fn len(&self) -> usize { /* match arms */ }
    fn as_reals(&self) -> Vec<f64> { /* Complex→magnitude or re; Bool→0/1; Str→0 */ }
}

#[derive(Debug)]
pub struct DataFrame {
    names:   Vec<String>,   // column order
    columns: Vec<Column>,   // parallel to names, equal length
    index:   Vec<usize>,    // which columns are the index/axes (e.g. [time])
}

#[derive(Debug)]
pub struct DataFrameObj { df: std::sync::Mutex<DataFrame> }

impl DataFrameObj {
    pub fn new(df: DataFrame) -> Value { Value::ExternObject(std::sync::Arc::new(Self { df: Mutex::new(df) })) }
}
```

## 2. Producing a frame from an analysis

Add `from_result(&AnalysisResult) -> DataFrame` and expose it as a `.frame()` method
on `AnalysisHandleObj` (extern_types.rs). The result API is unchanged; DataFrame is
opt-in.

```verilog
DataFrame df = $tran(1e-9, 1e-6).frame();
```

`from_result`: order columns with the scale/index first (`time`/`frequency`/`sweep`
if present, else first key), convert each `VectorData::Real → Column::Real`,
`Complex → Column::Complex`; set `index = [0]` (the scale column). Column order from
`AnalysisResult.vectors` is otherwise insertion/sorted — sort the non-index names for
determinism.

> Monte-Carlo (later): a `concat(other)` method appends rows and adds a `run`/`corner`
> index column → one long frame for a batch.

## 3. Operator overloading — the one core language change

`eval_binary_op` (interpreter.rs) currently errors on `ExternObject` operands. Add
dispatch to an ExternClass hook.

**`crates/piperine-interpreter/src/value.rs`** — extend the trait (default errors, so
existing ExternClasses are unaffected):

```rust
pub trait ExternClass: std::fmt::Debug + Send + Sync {
    fn type_name(&self) -> &str;
    fn call_method(&self, method: &str, args: &[Value]) -> Result<Value, String>;
    /// Binary operator with another value. `op` is "+","-","*","/","%","**",
    /// "<","<=",">",">=","==","!=","&&","||". `self_on_left` is false when the
    /// object is the right operand (`scalar * signal`). Default: unsupported.
    fn binary_op(&self, op: &str, _other: &Value, _self_on_left: bool) -> Result<Value, String> {
        Err(format!("operator `{op}` not supported on {}", self.type_name()))
    }
}
```

**`eval_binary_op`** — before the final `Err` arm:

```rust
(Value::ExternObject(o), rhs) => o.binary_op(binop_str(op), &rhs, true).map_err(InterpreterError::Other),
(lhs, Value::ExternObject(o)) => o.binary_op(binop_str(op), &lhs, false).map_err(InterpreterError::Other),
```

with a `binop_str(&BinOp) -> &'static str` map. (Put these two arms *before* the
catch-all; keep all existing scalar arms first.)

**`SignalObj::binary_op`** (extern_types.rs) — element-wise, broadcasting a scalar:

- `Signal op Signal` → new `SignalObj` (wrapping a fresh `VectorData::Real`),
  element-wise; lengths must match.
- `Signal op scalar` (real/int) → broadcast the scalar over every element.
- Arithmetic (`+ - * / % **`) → numeric Signal.
- Comparisons (`< <= > >= == !=`) → a **mask**: emit a Signal whose data is `1.0/0.0`
  per element (a 0/1 Real Signal works everywhere; a dedicated Bool is optional).
- Respect `self_on_left` for `-`, `/`, `%`, `**` (non-commutative).

(`SignalObj` already holds a `name`, `data: VectorData`, `result: Arc<AnalysisResult>`.
The op result is a detached Signal — give it a synthetic name like `"<expr>"` and a
shared empty/clone scale via the same `result` so `.integral()` still finds time.)

`Column::binary_op` / `DataFrameObj` operator support is optional in v1 — `df["a"]`
already yields a `Signal`, so `df["a"] * df["b"]` is **Signal × Signal**, covered by
the above. That is the whole point: operate on columns, not the frame.

## 4. Slicing — `df[lo:hi]`, `s[lo:hi]`

`Expr::PartSelect(base, lo, hi)` is parsed but the interpreter rejects it
(interpreter.rs). Replace the error with a dispatch:

```rust
Expr::PartSelect(base, lo, hi) => {
    let recv = self.eval_expr(base, scope)?;
    let l = self.eval_expr(lo, scope)?;
    let h = self.eval_expr(hi, scope)?;
    match recv {
        Value::ExternObject(o) => o.call_method("slice", &[l, h]).map_err(InterpreterError::Other),
        other => Err(/* type error */),
    }
}
```

`SignalObj::slice(lo, hi)` → a new Signal over `data[lo..=hi]`. `DataFrameObj::slice`
→ a new frame with each column row-sliced. (Inclusive or half-open: pick **half-open
`[lo, hi)`** to match Rust/`values()`; document it.)

## 5. DataFrameObj methods (`call_method`)

| Method | Returns | Notes |
|--------|---------|-------|
| `get(key)` | Signal / row | `String`→column Signal; `Integer`→a 1-row frame (or error v1) |
| `set(key, sig)` | void | string key adds/replaces a column (length must match) |
| `cols()` | string[] (RealVec of indices? no) | column names — return `Value::ExternObject(ArrayObj of Str)` or a string list |
| `nrows()` / `ncols()` | integer | |
| `shape()` | int[] | `[nrows, ncols]` (until tuples land, §6 design doc) |
| `index()` / `scale()` | Signal | the (first) index column |
| `select(names)` | DataFrame | subset; `names` is an array of strings |
| `with_column(name, sig)` | DataFrame | derived column (alias of string `set`, returns self) |
| `filter(mask)` | DataFrame | keep rows where mask Signal != 0 |
| `slice(lo, hi)` | DataFrame | §4 |
| `head(n)` | DataFrame | first `n` rows |
| `to_csv(path)` | void | write `names` header + rows (uses backend? no — std fs) |
| reductions on a column | via `df["c"].max()` etc. | delegate to Signal |

`filter(mask)`: `mask` is a Signal (length = nrows) of 0/1 from a comparison (§3).
Keep row `i` where `mask[i] != 0`. Apply to every column → new frame.

`to_csv`: plain `std::fs::write`. No simulator involvement — it's host-side I/O.

## 6. Tabular display

Implement `Display` (or a `to_string()` method) printing a head/tail table so
`$display(df)` is useful. `$display` calls `Value::to_string()` (fmt::Display on
`Value`) — make `Value::ExternObject` print `obj` via a `display()` ExternClass hook
or a `to_string` method the formatter calls. Minimal v1: a `df.show()` method that
returns a formatted `String`.

## 7. Build order

1. **§1 type + §2 `.frame()` + §5 core methods (`get`/`cols`/`nrows`/`ncols`/`index`/
   `to_csv`) + §0 string-index (free)** — inspectable, dumpable frames.
   Tests: build a frame from a mock `AnalysisResult`, `df["v(out)"].max()`,
   `df.nrows()`, `df.to_csv` round-trip.
2. **§3 operator overloading + `SignalObj::binary_op` + `filter`** — the inflection:
   `df["p"] = df["v(out)"] * df["i(out)"]`, `df.filter(df["v(out)"] > 0.9)`.
   Tests: `(a+b)`, `a*2.0`, `a > thr` mask, `filter`.
3. **§4 slicing**, **§6 display**, `select`/`head`/`with_column`.
4. Later: Monte-Carlo `concat`/`groupby`; lambdas (`filter(x -> …)`) when closures
   land (Phase 9); PyO3 export.

## 8. Tests (`tests/e2e_dataframe_test.rs`, MockBackend)

- `$tran(...).frame()` → `df.nrows()`, `df.ncols()`, `df.cols()` correct.
- `df["v(out)"]` is a Signal; `.max()/.rms()` match.
- vectorized: `df["v(out)"] * 2.0`, `df["a"] + df["b"]` → Signal with expected values.
- mask + filter: `df.filter(df["t"] > 1e-6).nrows()`.
- `with_column` / `select` / `slice` / `head` shapes.
- `to_csv` writes a parseable file (header + N rows).

## 9. Docs to update

- `docs/lang/stdlib.md` — a `DataFrame` section (methods table, handle semantics,
  `result.frame()`), and note vectorized Signal operators.
- Cross-link from `docs/ngspice/analyses.md` (results → frames).
