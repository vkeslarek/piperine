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
    Bool(Vec<bool>),
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

    fn to_vector(&self) -> VectorData {
        match self {
            Column::Real(v)    => VectorData::Real(v.clone()),
            Column::Complex(v) => VectorData::Complex(v.clone()),
            Column::Int(v)     => VectorData::Real(v.iter().map(|&x| x as f64).collect()),
            Column::Bool(v)    => VectorData::Real(v.iter().map(|&b| b as i64 as f64).collect()),
            Column::Str(_)     => VectorData::Real(vec![]),
        }
    }

    fn from_vector(v: &VectorData) -> Column {
        match v {
            VectorData::Real(r)    => Column::Real(r.clone()),
            VectorData::Complex(c) => Column::Complex(c.clone()),
        }
    }

    fn cell(&self, i: usize) -> String {
        match self {
            Column::Real(v)    => v[i].to_string(),
            Column::Complex(v) => format!("{}+{}i", v[i].0, v[i].1),
            Column::Int(v)     => v[i].to_string(),
            Column::Str(v)     => v[i].clone(),
            Column::Bool(v)    => (v[i] as i64).to_string(),
        }
    }

    fn as_reals(&self) -> Vec<f64> {
        match self {
            Column::Real(v)    => v.clone(),
            Column::Complex(v) => v.iter().map(|(r, _)| *r).collect(),
            Column::Int(v)     => v.iter().map(|&x| x as f64).collect(),
            Column::Bool(v)    => v.iter().map(|&b| b as i64 as f64).collect(),
            Column::Str(_)     => vec![],
        }
    }

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

impl DataFrame {
    pub fn from_result(res: &AnalysisResult) -> DataFrame {
        let scale = ["time", "frequency", "sweep"].iter()
            .find(|k| res.vectors.contains_key(**k))
            .map(|s| s.to_string());

        let mut names = Vec::new();
        if let Some(s) = &scale { names.push(s.clone()); }
        let mut rest: Vec<String> = res.vectors.keys()
            .filter(|k| Some(k.as_str()) != scale.as_deref())
            .cloned().collect();
        rest.sort();
        names.extend(rest);

        let columns = names.iter()
            .map(|n| Column::from_vector(&res.vectors[n]))
            .collect();
        let index = if scale.is_some() { vec![0] } else { vec![] };
        DataFrame { names, columns, index }
    }
}

#[derive(Debug)]
pub struct DataFrameObj { df: Mutex<DataFrame> }

impl DataFrameObj {
    pub fn new(df: DataFrame) -> Value {
        Value::ExternObject(Arc::new(Self { df: Mutex::new(df) }))
    }
}

impl ExternClass for DataFrameObj {
    fn type_name(&self) -> &str { "DataFrame" }

    fn call_method(&self, method: &str, args: &[Value]) -> Result<Value, String> {
        let mut df = self.df.lock().unwrap();
        let nrows = df.columns.first().map_or(0, |c| c.len());

        match method {
            "get" => match args.first() {
                Some(Value::String(name)) => column_signal(&df, name),
                Some(other) => Err(format!("DataFrame index must be a column name (got {})",
                                           other.type_name())),
                None => Err("get(key) requires an argument".into()),
            },

            "set" => {
                let (name, col) = column_arg(&args)?;
                if nrows != 0 && col.len() != nrows {
                    return Err(format!("column '{name}' length {} != nrows {nrows}", col.len()));
                }
                upsert(&mut df, name, col);
                Ok(Value::Void)
            }

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

            "select" => {
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

            "filter" => {
                let mask = signal_reals(args.first())?;
                if mask.len() != nrows {
                    return Err(format!("filter mask len {} != nrows {nrows}", mask.len()));
                }
                let keep: Vec<usize> = (0..nrows).filter(|&i| mask[i] != 0.0).collect();
                Ok(DataFrameObj::new(row_subset(&df, &keep)))
            }

            "slice" => {
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

            // Append rows from `other` DataFrame. Columns matched by name; order
            // taken from `self`. Missing columns in `other` are filled with 0.0 Real.
            // Preserves `self.index`. Used by Monte-Carlo loops to accumulate runs.
            "concat" => {
                let other_val = args.first().ok_or("concat(other) requires a DataFrame")?;
                let other_obj = match other_val {
                    Value::ExternObject(o) if o.type_name() == "DataFrame" => o.clone(),
                    _ => return Err("concat(other): argument must be a DataFrame".into()),
                };
                // To read the other frame's columns we call its methods — no downcast.
                let other_ncols = match other_obj.call_method("ncols", &[])? {
                    Value::Integer(n) => n as usize,
                    _ => 0,
                };
                // Build name → column data map for `other` by pulling each column Signal.
                let other_cols_obj = other_obj.call_method("cols", &[])?;
                let other_names: Vec<String> = {
                    let arr = match &other_cols_obj { Value::ExternObject(o) => o, _ => return Err("unexpected".into()) };
                    let n = match arr.call_method("size", &[])? { Value::Integer(n) => n, _ => 0 };
                    (0..n).map(|i| match arr.call_method("get", &[Value::Integer(i)])? {
                        Value::String(s) => Ok::<String, String>(s),
                        _ => Err("unexpected non-string col name".into()),
                    }).collect::<Result<Vec<_>, _>>()?
                };
                let mut other_data: std::collections::HashMap<String, Vec<f64>> = Default::default();
                for name in &other_names {
                    let sig = other_obj.call_method("get", &[Value::String(name.clone())])?;
                    let sig_obj = match &sig { Value::ExternObject(o) => o, _ => continue };
                    match sig_obj.call_method("values", &[])? {
                        Value::RealVec(v) => { other_data.insert(name.clone(), v); }
                        _ => {}
                    }
                }
                let other_nrows = other_data.values().next().map_or(0, |v| v.len());

                let mut new_columns: Vec<Column> = Vec::new();
                for (col_name, col) in df.names.iter().zip(df.columns.iter()) {
                    let mut combined = col.as_reals();
                    let ext = other_data.get(col_name)
                        .cloned()
                        .unwrap_or_else(|| vec![0.0; other_nrows]);
                    combined.extend(ext);
                    new_columns.push(Column::Real(combined));
                }
                Ok(DataFrameObj::new(DataFrame {
                    names: df.names.clone(),
                    columns: new_columns,
                    index: df.index.clone(),
                }))
            }

            _ => Err(format!("unknown method '{method}' on DataFrame")),
        }
    }
}

fn column_signal(df: &DataFrame, name: &str) -> Result<Value, String> {
    let i = df.names.iter().position(|n| n == name)
        .ok_or_else(|| format!("no column '{name}'"))?;
    let data = df.columns[i].to_vector();

    let mut vectors = HashMap::new();
    for &ix in &df.index {
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

fn column_arg(args: &[Value]) -> Result<(String, Column), String> {
    let name = args.first().and_then(|v| v.as_str())
        .ok_or("expected (name: string, signal)")?.to_string();
    let col = Column::Real(signal_reals(args.get(1))?);
    Ok((name, col))
}

fn upsert(df: &mut DataFrame, name: String, col: Column) {
    match df.names.iter().position(|n| *n == name) {
        Some(i) => df.columns[i] = col,
        None => { df.names.push(name); df.columns.push(col); }
    }
}

fn string_array(names: &[String]) -> Value {
    crate::extern_types::ArrayObj::new(
        names.iter().cloned().map(Value::String).collect())
}

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
