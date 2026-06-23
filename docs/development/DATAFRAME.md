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
}
```

- **Handle semantics** (like `ArrayObj`): the wrapper struct holds a
  `Mutex<DataFrame>`, and `Value::ExternObject(Arc<dyn ExternClass>)` supplies the
  `Arc`. So passing a frame around shares storage and derived columns are cheap —
  *exactly* the `ArrayObj` shape (`struct ArrayObj { items: Mutex<Vec<Value>> }`),
  not a separate `Arc<Mutex<…>>` field.
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

Core methods (**implemented**): `cols()`, `nrows()/ncols()/shape()`, `index()`,
`df[name]`, `select(names)`, `filter(mask)`, `with_column(name, series)`,
`slice(lo,hi)`, `head(n)`, `concat(other)`, `to_csv(path)`, `show()`.

Signal reductions (**implemented**): `max()`, `min()`, `mean()`, `rms()`,
`peak_to_peak()`, `integral()`, `bandwidth_3db()`, `phase_margin()`, `at(x)`,
`sigma()`/`std()`, `yield_(threshold, op)`.

Deferred: `groupby(index)` (Phase 6 MC aggregation), lambdas/`with` (Phase 9),
PyO3 export (separate effort).

## What the language must support (the ergonomics work)

The DataFrame *type* is straightforward Rust. Making it **ergonomic in Piperine**
needs language features we don't have yet. Ordered by leverage:

### 1. String / key indexing — `df["v(out)"]`  *(already free — see §0)*

`Expr::Index` already evaluates `base`, then `call_method("get", &[idx])`. A
string key arrives at `DataFrameObj::get` unchanged. Same path makes associative
arrays `aa["key"]` work — shared with Phase 9.

### 2. Operator overloading on objects — `df["a"] + df["b"]`, `s > 1.0`  *(medium, highest leverage)*

The single biggest ergonomics win: vectorized Series arithmetic and comparison
masks instead of hand-written `foreach` loops. `eval_binary_op` today handles only
scalar pairs; when either operand is an `ExternObject` it must dispatch to a new
`ExternClass::binary_op`. Then `SignalObj` implements element-wise ops returning a
new Signal, and comparisons return a 0/1 mask Signal usable by `filter`.

### 3. Lambdas / `with` iterator clause — `df.map(x -> x*2)`  *(large, Phase 9)*

Generic `filter`/`map`/`apply` parameterized by a row/element. Piperine has no
closures. Until they land, #2 (vectorized ops + mask `filter`) covers most real
cases without lambdas.

### 4. Row slicing — `df[10:100]`, `s[0:50]`  *(small)*

`Expr::PartSelect(base, lo, hi)` parses but the interpreter rejects it. Map it to
`call_method("slice", &[lo, hi])`.

### 5. Field-access sugar — `df.time`, `s.values`  *(optional, deferred)*

`obj.member` with no parens reaching a zero-arg getter. Pure sugar over
`df["time"]`. Not built in v1.

### 6. Tuple / multi-assignment — `(n, m) = df.shape();`  *(optional, deferred)*

Workaround today: `df.shape()` returns an `int[]`, index it.

---

# Implementation refinement (build this)

Implementation-ready spec. Every code anchor below is a real file + symbol in the
current tree. Build in the order of §7. Each step is independently shippable and
testable with a `MockBackend` (assert on values; no real simulator). New Rust lives
in **one new file** + small edits to **three existing files**:

| Where | What changes |
|-------|--------------|
| `crates/piperine-interpreter/src/dataframe.rs` *(new)* | `Column`, `DataFrame`, `DataFrameObj` + `ExternClass` impl |
| `crates/piperine-interpreter/src/value.rs` | add `ExternClass::binary_op` default; re-export `DataFrame*` |
| `crates/piperine-interpreter/src/interpreter.rs` | `eval_binary_op` ExternObject arms + `binop_str`; `Expr::PartSelect` → `slice` |
| `crates/piperine-interpreter/src/extern_types.rs` | `AnalysisHandleObj` gains `"frame"`; `SignalObj` gains `binary_op` + `slice`/`values`-as-rhs |

> **Rationale for a new `dataframe.rs`** instead of piling onto `extern_types.rs`:
> the DataFrame type is ~250 lines (columns, builder, CSV, display). `extern_types.rs`
> is already the home of `SignalObj`/`ArrayObj`/`DeviceHandle`; keeping DataFrame
> separate keeps each file legible and matches the "one concept per file" tilt.

## 0. What already routes for free (verify, don't build)

These already work in `interpreter.rs` — confirm, write a test, move on:

- **`df["v(out)"]`** → `Expr::Index` arm (interpreter.rs ~`586`):
  ```rust
  Expr::Index(base, index) => {
      let array = self.eval_expr(base, scope)?;
      let idx = self.eval_expr(index, scope)?;
      match array {
          Value::ExternObject(obj) => obj.call_method("get", &[idx])
              .map_err(InterpreterError::Other),
          other => Err(/* type error */),
      }
  }
  ```
  So `df["v(out)"]` calls `DataFrameObj::get(["v(out)": String])`. **No change.**
- **`df["new"] = sig`** → `Stmt::Assign` Index branch (interpreter.rs ~`268`):
  ```rust
  if let Expr::Index(base, index) = &a.lval {
      // … evaluates base + index, then:
      Value::ExternObject(obj) => { obj.call_method("set", &[idx, value])?; }
  }
  ```
  So `DataFrameObj::set([key, value])` adds/replaces a column. **No change.**
- **Method chaining** `df.filter(...).to_csv(...)` already dispatches through the
  `ExternObject` method path. **No change.**

Only two real language changes remain: **operator overloading** (§3) and
**slicing** (§4).

## 1. The Rust type — `crates/piperine-interpreter/src/dataframe.rs` *(new file)*

Mirror `ArrayObj` exactly: a `Mutex<…>` field on the wrapper struct; the `Arc`
comes from `Value::ExternObject`. Re-export from `lib.rs` (`pub mod dataframe;`).

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::value::{AnalysisKind, AnalysisResult, ExternClass, Value, VectorData};
use crate::extern_types::SignalObj;

#[derive(Debug, Clone)]
pub enum Column {
    Real(Vec<f64>),
    Complex(Vec<(f64, f64)>),
    Int(Vec<i64>),
    Str(Vec<String>),
    Bool(Vec<bool>),     // comparison masks (kept as a real 0/1 column for now)
}

impl Column {
    fn len(&self) -> usize {
        match self {
            Column::Real(v) => v.len(),
            Column::Complex(v) => v.len(),
            Column::Int(v) => v.len(),
            Column::Str(v) => v.len(),
            Column::Bool(v) => v.len(),
        }
    }

    /// Back to a `VectorData` so we can hand a column to a `SignalObj`.
    /// Non-real columns are coerced to reals (Complex→re, Bool/Int→numeric).
    fn to_vector(&self) -> VectorData {
        match self {
            Column::Real(v)    => VectorData::Real(v.clone()),
            Column::Complex(v) => VectorData::Complex(v.clone()),
            Column::Int(v)     => VectorData::Real(v.iter().map(|&x| x as f64).collect()),
            Column::Bool(v)    => VectorData::Real(v.iter().map(|&b| b as i64 as f64).collect()),
            Column::Str(_)     => VectorData::Real(vec![]), // strings aren't numeric
        }
    }

    fn from_vector(v: &VectorData) -> Column {
        match v {
            VectorData::Real(r)    => Column::Real(r.clone()),
            VectorData::Complex(c) => Column::Complex(c.clone()),
        }
    }

    /// Value at row `i` rendered for CSV / display.
    fn cell(&self, i: usize) -> String {
        match self {
            Column::Real(v)    => v[i].to_string(),
            Column::Complex(v) => format!("{}+{}i", v[i].0, v[i].1),
            Column::Int(v)     => v[i].to_string(),
            Column::Str(v)     => v[i].clone(),
            Column::Bool(v)    => (v[i] as i64).to_string(),
        }
    }

    /// Keep only the rows whose index is in `keep` (for `filter`/`slice`).
    fn take_rows(&self, keep: &[usize]) -> Column {
        macro_rules! pick { ($v:expr) => { keep.iter().map(|&i| $v[i].clone()).collect() } }
        match self {
            Column::Real(v)    => Column::Real(pick!(v)),
            Column::Complex(v) => Column::Complex(pick!(v)),
            Column::Int(v)     => Column::Int(pick!(v)),
            Column::Str(v)     => Column::Str(pick!(v)),
            Column::Bool(v)    => Column::Bool(pick!(v)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DataFrame {
    pub names:   Vec<String>,
    pub columns: Vec<Column>,
    pub index:   Vec<usize>,
}

#[derive(Debug)]
pub struct DataFrameObj { df: Mutex<DataFrame> }

impl DataFrameObj {
    pub fn new(df: DataFrame) -> Value {
        Value::ExternObject(Arc::new(Self { df: Mutex::new(df) }))
    }
}
```

**Rationale — `to_vector` coerces.** `df["col"]` must return a `SignalObj`, whose
storage is a `VectorData` (only `Real`/`Complex` exist — see `value.rs`). So a
`Bool` mask column round-trips through a real 0/1 vector. That's also why §3 masks
are real 0/1: there is no `Value::Bool`, and `Signal`/`filter` already speak reals.

## 2. `DataFrame::from_result` + `.frame()`

`AnalysisResult` (value.rs) is `{ kind, dataset, vectors: HashMap<String,
VectorData>, run_errors }`. The builder puts the scale column first, sorts the
rest for determinism (HashMap order is random), and marks the scale as `index`.

```rust
impl DataFrame {
    pub fn from_result(res: &AnalysisResult) -> DataFrame {
        // Scale/axis column first, if present (same keys SignalObj::find_scale uses).
        let scale = ["time", "frequency", "sweep"].iter()
            .find(|k| res.vectors.contains_key(**k))
            .map(|s| s.to_string());

        let mut names = Vec::new();
        if let Some(s) = &scale { names.push(s.clone()); }
        let mut rest: Vec<String> = res.vectors.keys()
            .filter(|k| Some(k.as_str()) != scale.as_deref())
            .cloned().collect();
        rest.sort();                       // determinism: HashMap iteration is random
        names.extend(rest);

        let columns = names.iter()
            .map(|n| Column::from_vector(&res.vectors[n]))
            .collect();
        let index = if scale.is_some() { vec![0] } else { vec![] };
        DataFrame { names, columns, index }
    }
}
```

Expose it on the result handle. In `extern_types.rs`, `AnalysisHandleObj::call_method`,
add **one arm** next to `"signal"`/`"scale"`:

```rust
"frame" => Ok(crate::dataframe::DataFrameObj::new(
    crate::dataframe::DataFrame::from_result(&self.result),
)),
```

```verilog
DataFrame df = $tran(1e-9, 1e-6).frame();   // result API otherwise unchanged
```

## 3. Operator overloading — the one core language change

**Step 3a — extend the trait.** `crates/piperine-interpreter/src/value.rs`, the
`ExternClass` trait (currently *only* `type_name` + `call_method`). Add a default
so every existing impl (`AnalysisHandleObj`, `ArrayObj`, `DeviceHandle`) is
unaffected:

```rust
pub trait ExternClass: std::fmt::Debug + Send + Sync {
    fn type_name(&self) -> &str;
    fn call_method(&self, method: &str, args: &[Value]) -> Result<Value, String>;

    /// Binary operator against another value. `op` is one of
    /// "+","-","*","/","%","**","<","<=",">",">=","==","!=","&&","||".
    /// `self_on_left` is false when the object is the *right* operand
    /// (`2.0 * signal`). Default: unsupported.
    fn binary_op(&self, op: &str, _other: &Value, _self_on_left: bool)
        -> Result<Value, String>
    {
        Err(format!("operator `{op}` not supported on {}", self.type_name()))
    }
}
```

**Step 3b — dispatch from the evaluator.** `interpreter.rs`, `eval_binary_op`
(currently ends in a catch-all `(left, right) => Err(TypeError…)` ~line `678`).
Insert two arms **immediately before** that catch-all (keep all scalar arms first
so scalar math stays the fast path):

```rust
(Value::ExternObject(o), rhs) =>
    o.binary_op(binop_str(op), &rhs, true).map_err(InterpreterError::Other),
(lhs, Value::ExternObject(o)) =>
    o.binary_op(binop_str(op), &lhs, false).map_err(InterpreterError::Other),
```

and add the `BinOp → &str` map (free function in interpreter.rs). The `BinOp`
variants are exactly (`ast/expr.rs`): `OrOr AndAnd Eq Neq Le Ge Lt Gt Add Sub Mul
Div Pow Mod Shl Shr Xor XNor1 XNor2 BitOr BitAnd`.

```rust
fn binop_str(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",  BinOp::Sub => "-",  BinOp::Mul => "*", BinOp::Div => "/",
        BinOp::Mod => "%",  BinOp::Pow => "**",
        BinOp::Lt  => "<",  BinOp::Le  => "<=", BinOp::Gt  => ">", BinOp::Ge  => ">=",
        BinOp::Eq  => "==", BinOp::Neq => "!=",
        BinOp::AndAnd => "&&", BinOp::OrOr => "||",
        BinOp::Shl => "<<", BinOp::Shr => ">>", BinOp::Xor => "^",
        BinOp::BitAnd => "&", BinOp::BitOr => "|",
        BinOp::XNor1 => "~^", BinOp::XNor2 => "^~",
    }
}
```

> **Rationale — why `&str` and not pass `&BinOp` into the trait.** `value.rs`
> (where the trait lives) does not depend on the parser's `BinOp`. Passing a plain
> `&str` keeps the dependency arrow one-way (interpreter → value), so `ExternClass`
> impls stay parser-agnostic.

**Step 3c — implement it on `SignalObj`** (`extern_types.rs`). Element-wise; a
scalar broadcasts; comparisons emit a 0/1 mask Signal. Crucially, the result keeps
`self.result` so `.integral()`/`.at()` still find the scale.

```rust
impl SignalObj {
    /// Reals view (Complex→re), reused by call_method and binary_op.
    fn reals(&self) -> Vec<f64> {
        match &self.data {
            VectorData::Real(v) => v.clone(),
            VectorData::Complex(v) => v.iter().map(|(r, _)| *r).collect(),
        }
    }
    /// A detached Signal sharing this one's scale (so reductions still work).
    fn derived(&self, data: Vec<f64>) -> Value {
        Value::ExternObject(Arc::new(SignalObj {
            name: format!("<{}>", self.name),
            data: VectorData::Real(data),
            result: Arc::clone(&self.result),
        }))
    }
}

impl ExternClass for SignalObj {
    // … type_name / call_method unchanged …

    fn binary_op(&self, op: &str, other: &Value, self_on_left: bool)
        -> Result<Value, String>
    {
        let a = self.reals();
        // RHS reals: another Signal (element-wise) or a scalar (broadcast).
        let b: Vec<f64> = match other {
            Value::ExternObject(o) if o.type_name() == "Signal" => {
                // Reuse the public `values` method to read the other Signal's data
                // — ExternClass has no downcast, and this needs no new surface.
                match o.call_method("values", &[])? {
                    Value::RealVec(v) => v,
                    _ => return Err("Signal.values did not return a vector".into()),
                }
            }
            Value::Real(_) | Value::Integer(_) => vec![other.as_f64().unwrap(); a.len()],
            _ => return Err(format!(
                "cannot apply `{op}` between Signal and {}", other.type_name())),
        };
        if b.len() != a.len() {
            return Err(format!("Signal length mismatch: {} vs {}", a.len(), b.len()));
        }
        // Order matters for non-commutative ops when the Signal is on the right.
        let out: Vec<f64> = (0..a.len()).map(|i| {
            let (x, y) = if self_on_left { (a[i], b[i]) } else { (b[i], a[i]) };
            scalar_op(op, x, y)
        }).collect::<Result<_, _>>()?;
        Ok(self.derived(out))
    }
}

/// Scalar op → f64. Comparisons return 1.0/0.0 (no Value::Bool exists).
fn scalar_op(op: &str, x: f64, y: f64) -> Result<f64, String> {
    Ok(match op {
        "+" => x + y, "-" => x - y, "*" => x * y, "/" => x / y,
        "%" => x % y, "**" => x.powf(y),
        "<" => (x <  y) as i64 as f64, "<=" => (x <= y) as i64 as f64,
        ">" => (x >  y) as i64 as f64, ">=" => (x >= y) as i64 as f64,
        "==" => (x == y) as i64 as f64, "!=" => (x != y) as i64 as f64,
        _ => return Err(format!("operator `{op}` not supported on Signal")),
    })
}
```

> **Rationale — `o.call_method("values")` to read the RHS Signal.** `ExternClass`
> is a trait object with no `Any`/downcast, so we can't pattern-match the concrete
> `SignalObj`. Calling the existing public `values` method is the clean way to pull
> the other operand's data — no new method, works for anything that exposes a
> numeric `values`. The `type_name() == "Signal"` guard keeps it honest.

`Column`/`DataFrameObj` need **no** operator support in v1: `df["a"]` already
yields a `Signal`, so `df["a"] * df["b"]` is **Signal × Signal**, handled above.
That is the whole point — operate on columns, not the frame.

## 4. Slicing — `df[lo:hi]`, `s[lo:hi]`

`Expr::PartSelect(base, lo, hi)` parses (`ast/expr.rs:24` →
`PartSelect(Box<Expr>, Box<Expr>, Box<Expr>)`) but `interpreter.rs` ~`599`
currently errors:

```rust
Expr::PartSelect(_, _, _) => {
    Err(InterpreterError::Other("part-selects (`a[msb:lsb]`) are not supported".into()))
}
```

Replace with a `slice` dispatch:

```rust
Expr::PartSelect(base, lo, hi) => {
    let recv = self.eval_expr(base, scope)?;
    let lo = self.eval_expr(lo, scope)?;
    let hi = self.eval_expr(hi, scope)?;
    match recv {
        Value::ExternObject(obj) => obj.call_method("slice", &[lo, hi])
            .map_err(InterpreterError::Other),
        other => Err(InterpreterError::TypeError {
            expected: "sliceable object (Signal/DataFrame)".into(),
            got: other.type_name().into(),
        }),
    }
}
```

Semantics: **half-open `[lo, hi)`** (matches Rust ranges / `values()`); document it.
- `SignalObj::call_method("slice", [lo, hi])` → `self.derived(reals[lo..hi].to_vec())`.
- `DataFrameObj::slice` → `take_rows((lo..hi).collect())` on every column (§5).

## 5. `DataFrameObj::call_method` — the methods

Lock the mutex once per call (like `ArrayObj`). Full dispatch:

```rust
impl ExternClass for DataFrameObj {
    fn type_name(&self) -> &str { "DataFrame" }

    fn call_method(&self, method: &str, args: &[Value]) -> Result<Value, String> {
        let mut df = self.df.lock().unwrap();
        let nrows = df.columns.first().map_or(0, |c| c.len());

        match method {
            // df["v(out)"]  (string) → column Signal; integer → error in v1
            "get" => match args.first() {
                Some(Value::String(name)) => column_signal(&df, name),
                Some(other) => Err(format!("DataFrame index must be a column name (got {})",
                                           other.type_name())),
                None => Err("get(key) requires an argument".into()),
            },

            // df["p"] = sig  → mutate in place, add/replace a column (void).
            "set" => {
                let (name, col) = column_arg(&args)?;
                if nrows != 0 && col.len() != nrows {
                    return Err(format!("column '{name}' length {} != nrows {nrows}", col.len()));
                }
                upsert(&mut df, name, col);
                Ok(Value::Void)
            }

            // df = df.with_column("p", sig)  → clone → new frame (chainable).
            // Can't return `self`: call_method takes `&self`, not `Arc<Self>`. So a
            // *new* handle is both necessary and the right immutable-ish surface.
            "with_column" => {
                let (name, col) = column_arg(&args)?;
                if nrows != 0 && col.len() != nrows {
                    return Err(format!("column '{name}' length {} != nrows {nrows}", col.len()));
                }
                let mut next = df.clone();
                upsert(&mut next, name, col);
                Ok(DataFrameObj::new(next))
            }

            "cols" => Ok(string_array(&df.names)),
            "nrows" => Ok(Value::Integer(nrows as i64)),
            "ncols" => Ok(Value::Integer(df.columns.len() as i64)),
            "shape" => Ok(Value::RealVec(vec![nrows as f64, df.columns.len() as f64])),

            "index" | "scale" => {
                let &i = df.index.first().ok_or("frame has no index column")?;
                let name = df.names[i].clone();
                column_signal(&df, &name)
            }

            "select" => {                       // select('{"time","v(out)"}')
                let want = string_list(args.first())?;
                let mut names = Vec::new();
                let mut columns = Vec::new();
                for n in &want {
                    let i = df.names.iter().position(|c| c == n)
                        .ok_or_else(|| format!("no column '{n}'"))?;
                    names.push(n.clone());
                    columns.push(df.columns[i].clone());
                }
                Ok(DataFrameObj::new(DataFrame { names, columns, index: vec![] }))
            }

            "filter" => {                       // filter(mask Signal of 0/1)
                let mask = signal_reals(args.first())?;
                if mask.len() != nrows {
                    return Err(format!("filter mask len {} != nrows {nrows}", mask.len()));
                }
                let keep: Vec<usize> = (0..nrows).filter(|&i| mask[i] != 0.0).collect();
                Ok(DataFrameObj::new(row_subset(&df, &keep)))
            }

            "slice" => {                        // df[lo:hi]  (half-open)
                let lo = args.first().and_then(|v| v.as_integer()).unwrap_or(0).max(0) as usize;
                let hi = args.get(1).and_then(|v| v.as_integer())
                    .map(|h| h as usize).unwrap_or(nrows).min(nrows);
                let keep: Vec<usize> = (lo..hi).collect();
                Ok(DataFrameObj::new(row_subset(&df, &keep)))
            }

            "head" => {
                let n = args.first().and_then(|v| v.as_integer()).unwrap_or(5).max(0) as usize;
                let keep: Vec<usize> = (0..n.min(nrows)).collect();
                Ok(DataFrameObj::new(row_subset(&df, &keep)))
            }

            "to_csv" => {
                let path = args.first().and_then(|v| v.as_str())
                    .ok_or("to_csv(path) needs a string path")?;
                std::fs::write(path, render_csv(&df))
                    .map_err(|e| format!("to_csv: {e}"))?;
                Ok(Value::Void)
            }

            "show" => Ok(Value::String(render_table(&df, 10))),

            _ => Err(format!("unknown method '{method}' on DataFrame")),
        }
    }
}
```

Helpers in the same file (`column_signal` is the important one — it makes
`df["col"]` a fully-working `Signal`):

```rust
/// Build a Signal for column `name`, carrying the frame's index column(s) so
/// scale-dependent reductions (`integral`, `at`) keep working off a DataFrame.
fn column_signal(df: &DataFrame, name: &str) -> Result<Value, String> {
    let i = df.names.iter().position(|n| n == name)
        .ok_or_else(|| format!("no column '{name}'"))?;
    let data = df.columns[i].to_vector();

    let mut vectors = HashMap::new();
    for &ix in &df.index {                       // expose index under its real name
        vectors.insert(df.names[ix].clone(), df.columns[ix].to_vector());
    }
    vectors.insert(name.to_string(), data.clone());

    let result = Arc::new(AnalysisResult {
        kind: AnalysisKind::Tran, dataset: String::new(),
        vectors, run_errors: vec![],
    });
    Ok(Value::ExternObject(Arc::new(SignalObj { name: name.to_string(), data, result })))
}

fn signal_reals(v: Option<&Value>) -> Result<Vec<f64>, String> {
    match v {
        Some(Value::ExternObject(o)) => match o.call_method("values", &[])? {
            Value::RealVec(r) => Ok(r),
            _ => Err("expected a Signal".into()),
        },
        Some(Value::RealVec(r)) => Ok(r.clone()),
        _ => Err("expected a Signal argument".into()),
    }
}
/// Read a `(name, signal)` pair for set/with_column.
fn column_arg(args: &[Value]) -> Result<(String, Column), String> {
    let name = args.first().and_then(|v| v.as_str())
        .ok_or("expected (name: string, signal)")?.to_string();
    let col = Column::Real(signal_reals(args.get(1))?);
    Ok((name, col))
}
/// Add or replace a column by name.
fn upsert(df: &mut DataFrame, name: String, col: Column) {
    match df.names.iter().position(|n| *n == name) {
        Some(i) => df.columns[i] = col,
        None => { df.names.push(name); df.columns.push(col); }
    }
}
/// Column names as an Array handle of strings (reuses ArrayObj).
fn string_array(names: &[String]) -> Value {
    crate::extern_types::ArrayObj::new(
        names.iter().cloned().map(Value::String).collect())
}
/// Read a `select(...)` argument (an ArrayObj of strings) back into a Vec.
fn string_list(v: Option<&Value>) -> Result<Vec<String>, String> {
    let Some(Value::ExternObject(o)) = v else {
        return Err("select(names) expects a string array".into());
    };
    let n = match o.call_method("size", &[])? { Value::Integer(n) => n, _ => 0 };
    (0..n).map(|i| match o.call_method("get", &[Value::Integer(i)])? {
        Value::String(s) => Ok(s),
        other => Err(format!("select: non-string element {}", other.type_name())),
    }).collect()
}
fn row_subset(df: &DataFrame, keep: &[usize]) -> DataFrame {
    DataFrame {
        names: df.names.clone(),
        columns: df.columns.iter().map(|c| c.take_rows(keep)).collect(),
        index: df.index.clone(),
    }
}
fn render_csv(df: &DataFrame) -> String {
    let mut s = df.names.join(",");
    let nrows = df.columns.first().map_or(0, |c| c.len());
    for i in 0..nrows {
        s.push('\n');
        s.push_str(&df.columns.iter().map(|c| c.cell(i)).collect::<Vec<_>>().join(","));
    }
    s.push('\n');
    s
}
```

> **`column_signal` is the keystone.** A `Signal` needs an `Arc<AnalysisResult>`
> for its scale (see `SignalObj::find_scale`, which looks up `"time"`/`"frequency"`/
> `"sweep"`). A DataFrame is detached from any result, so we synthesize a minimal
> `AnalysisResult` carrying the requested column **plus the index column under its
> real name**. That makes `df["v(out)"].integral()` work *and* keeps a single Signal
> implementation — no DataFrame-specific Signal variant.

**Return-type note.** `cols()` returns an `ArrayObj` of strings (`string_array`),
and `select(...)` reads its argument back with `string_list` — both reuse the
existing `ArrayObj` (`size`/`get`), so no new array type is needed. `shape()`
returns `RealVec([nrows, ncols])` until tuples land (§6, deferred).

## 6. Display

No `Value::Bool`/tabular `Display` hook exists, and `$display(value)` prints an
`ExternObject` as `<extern_object>` (see `value.rs` `Display`). So v1 ships a
`show()` **method** returning a formatted `String`; the user writes `$display(df.show())`.

```rust
fn render_table(df: &DataFrame, max_rows: usize) -> String {
    let nrows = df.columns.first().map_or(0, |c| c.len());
    let mut out = df.names.join(" | ");
    out.push('\n');
    for i in 0..nrows.min(max_rows) {
        out.push_str(&df.columns.iter().map(|c| c.cell(i)).collect::<Vec<_>>().join(" | "));
        out.push('\n');
    }
    if nrows > max_rows { out.push_str(&format!("… ({nrows} rows)\n")); }
    out
}
```

(A real tabular `$display(df)` needs a `Display`/`to_string` hook on `ExternClass`
— deferred; `show()` is the unambiguous v1.)

## 7. Build order — status

1. ✅ **§1 type + §2 `.frame()` + §5 core methods + §0 string-index.** Done.
2. ✅ **§3 operator overloading + `SignalObj::binary_op` + `filter`.** Done.
3. ✅ **§4 slicing**, `select`/`head`/`with_column`, `show`, CSV, `concat`. Done.
   Also added: `Signal.sigma()`/`std()`, `Signal.yield_(thr, op)` for MC aggregation.
4. **Later:** `groupby(index)` (Phase 6); lambdas/`with` (Phase 9); PyO3 export.

## 8. Tests — `tests/e2e_dataframe_test.rs` (MockBackend)

Mirror `e2e_phase3_*` setup. Build a frame directly from a hand-made
`AnalysisResult` (no simulator needed for the type itself); use `MockBackend` only
for the `$tran(...).frame()` end-to-end case.

- `from_result` ordering: scale column first, rest sorted; `index == [0]`.
- `$tran(...).frame()` → `df.nrows()`, `df.ncols()`, `df.cols()` correct.
- `df["v(out)"]` is a Signal; `.max()/.rms()` match the raw vector.
- `df["v(out)"].integral()` works (scale carried via `column_signal`).
- vectorized: `df["a"] + df["b"]`, `df["v"] * 2.0`, `2.0 * df["v"]` → expected values.
- mask + filter: `df.filter(df["t"] > 1e-6).nrows()`.
- `with_column` adds a column (`ncols+1`); `select`/`slice`/`head` shapes.
- `to_csv` writes a parseable file (header + N rows); `show()` non-empty.

## 9. Export path (later, not blocking)

Because columns are contiguous single-dtype `Vec`s:
- **CSV** — `render_csv` above.
- **PyO3** — each `Column` → a numpy array (`f64`/`i64`/complex); frame → dict-of-
  arrays or a `polars`/`pandas` DataFrame; the `index` set → a pandas `MultiIndex`.
- **Plotting** — hand `index` + value columns to any downstream plotter.

Keeping the dtype-per-column invariant is what makes all three free.

## 10. Docs to update on landing

- `docs/lang/stdlib.md` — a `DataFrame` section (methods table, handle semantics,
  `result.frame()`), and note vectorized `Signal` operators (§3).
- `docs/ngspice/analyses.md` — cross-link results → frames.
- `docs/development/ROADMAP.md` — flip the DataFrame through-line items as they land.

This slots into the [ROADMAP](ROADMAP.md): the type + §1/§2/§3/§4 belong with the
data/results work (Phases 6–7); lambdas and assoc-arrays are Phase 9. DataFrame is
the through-line that makes all of them pay off together.
