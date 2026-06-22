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
