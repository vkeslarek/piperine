//! End-to-end tests for DataFrame, following DATAFRAME.md

use std::collections::HashMap;

use piperine_interpreter::value::{AnalysisKind, AnalysisResult, ExternClass, Value, VectorData};
use piperine_interpreter::AnalysisHandleObj;
use piperine_interpreter::dataframe::{DataFrame, DataFrameObj};

#[test]
fn test_dataframe_from_result() {
    let mut vectors = HashMap::new();
    vectors.insert("time".into(), VectorData::Real(vec![0.0, 1e-9, 2e-9]));
    vectors.insert("v(out)".into(), VectorData::Real(vec![0.0, 1.0, 2.0]));
    vectors.insert("i(v1)".into(), VectorData::Real(vec![-0.1, -0.2, -0.3]));

    let result = AnalysisResult {
        kind: AnalysisKind::Tran,
        dataset: "tran1".into(),
        vectors,
        run_errors: vec![],
    };

    let df = DataFrame::from_result(&result);
    // scale column "time" should be first
    assert_eq!(df.names[0], "time");
    // sorted rest: "i(v1)", "v(out)"
    assert_eq!(df.names[1], "i(v1)");
    assert_eq!(df.names[2], "v(out)");
    assert_eq!(df.index, vec![0]);
}

#[test]
fn test_dataframe_obj_methods() {
    let mut vectors = HashMap::new();
    vectors.insert("time".into(), VectorData::Real(vec![0.0, 1.0, 2.0]));
    vectors.insert("v(out)".into(), VectorData::Real(vec![0.0, 1.0, 4.0]));

    let result = AnalysisResult {
        kind: AnalysisKind::Tran,
        dataset: "tran1".into(),
        vectors,
        run_errors: vec![],
    };

    let handle = AnalysisHandleObj::new(result, "TranResult");
    let obj = match handle {
        Value::ExternObject(o) => o,
        _ => panic!("Expected ExternObject"),
    };

    let df_val = obj.call_method("frame", &[]).unwrap();
    let df_obj = match &df_val {
        Value::ExternObject(o) => o.clone(),
        _ => panic!("Expected ExternObject"),
    };

    // cols
    let cols = df_obj.call_method("cols", &[]).unwrap();
    match cols {
        Value::ExternObject(o) => assert_eq!(o.type_name(), "Array"),
        _ => panic!("Expected Array ExternObject"),
    }

    // nrows
    let nrows = df_obj.call_method("nrows", &[]).unwrap();
    assert_eq!(nrows, Value::Integer(3));

    // ncols
    let ncols = df_obj.call_method("ncols", &[]).unwrap();
    assert_eq!(ncols, Value::Integer(2));

    // get
    let vout = df_obj.call_method("get", &[Value::String("v(out)".into())]).unwrap();
    match vout {
        Value::ExternObject(o) => assert_eq!(o.type_name(), "Signal"),
        _ => panic!("Expected Signal ExternObject"),
    }
    
    // index
    let idx = df_obj.call_method("index", &[]).unwrap();
    match idx {
        Value::ExternObject(o) => assert_eq!(o.type_name(), "Signal"),
        _ => panic!("Expected Signal ExternObject"),
    }
}

#[test]
fn test_dataframe_operator_overloading() {
    let mut vectors = HashMap::new();
    vectors.insert("time".into(), VectorData::Real(vec![0.0, 1.0]));
    vectors.insert("a".into(), VectorData::Real(vec![1.0, 2.0]));
    vectors.insert("b".into(), VectorData::Real(vec![3.0, 4.0]));

    let result = AnalysisResult {
        kind: AnalysisKind::Tran,
        dataset: "tran1".into(),
        vectors,
        run_errors: vec![],
    };

    let df = DataFrame::from_result(&result);
    let df_val = DataFrameObj::new(df);
    let obj = match &df_val {
        Value::ExternObject(o) => o,
        _ => panic!("Expected ExternObject"),
    };

    let sig_a = obj.call_method("get", &[Value::String("a".into())]).unwrap();
    let sig_b = obj.call_method("get", &[Value::String("b".into())]).unwrap();

    let sig_a_obj = match &sig_a { Value::ExternObject(o) => o, _ => panic!() };

    // a + b
    let sum = sig_a_obj.binary_op("+", &sig_b, true).unwrap();
    match &sum {
        Value::ExternObject(o) => {
            let values = o.call_method("values", &[]).unwrap();
            assert_eq!(values, Value::RealVec(vec![4.0, 6.0]));
        }
        _ => panic!(),
    }

    // a * 2.0
    let mul = sig_a_obj.binary_op("*", &Value::Real(2.0), true).unwrap();
    match &mul {
        Value::ExternObject(o) => {
            let values = o.call_method("values", &[]).unwrap();
            assert_eq!(values, Value::RealVec(vec![2.0, 4.0]));
        }
        _ => panic!(),
    }
}

#[test]
fn test_dataframe_mask_filter() {
    let mut vectors = HashMap::new();
    vectors.insert("time".into(), VectorData::Real(vec![0.0, 1.0, 2.0, 3.0]));
    vectors.insert("v".into(), VectorData::Real(vec![0.5, 1.5, 0.5, 1.5]));

    let result = AnalysisResult {
        kind: AnalysisKind::Tran,
        dataset: "tran1".into(),
        vectors,
        run_errors: vec![],
    };

    let df = DataFrame::from_result(&result);
    let df_val = DataFrameObj::new(df);
    let obj = match &df_val {
        Value::ExternObject(o) => o,
        _ => panic!("Expected ExternObject"),
    };

    let sig_v = obj.call_method("get", &[Value::String("v".into())]).unwrap();
    let sig_v_obj = match &sig_v { Value::ExternObject(o) => o, _ => panic!() };

    // mask = v > 1.0
    let mask = sig_v_obj.binary_op(">", &Value::Real(1.0), true).unwrap();

    // df.filter(mask)
    let filtered = obj.call_method("filter", &[mask]).unwrap();
    let filt_obj = match &filtered { Value::ExternObject(o) => o, _ => panic!() };

    let nrows = filt_obj.call_method("nrows", &[]).unwrap();
    assert_eq!(nrows, Value::Integer(2));
}

#[test]
fn test_dataframe_with_column_and_slice() {
    let mut vectors = HashMap::new();
    vectors.insert("time".into(), VectorData::Real(vec![0.0, 1.0, 2.0, 3.0]));

    let result = AnalysisResult {
        kind: AnalysisKind::Tran,
        dataset: "tran1".into(),
        vectors,
        run_errors: vec![],
    };

    let df = DataFrame::from_result(&result);
    let df_val = DataFrameObj::new(df);
    let obj = match &df_val { Value::ExternObject(o) => o, _ => panic!() };

    let sig_time = obj.call_method("get", &[Value::String("time".into())]).unwrap();

    let df2 = obj.call_method("with_column", &[Value::String("t2".into()), sig_time]).unwrap();
    let df2_obj = match &df2 { Value::ExternObject(o) => o, _ => panic!() };
    let ncols = df2_obj.call_method("ncols", &[]).unwrap();
    assert_eq!(ncols, Value::Integer(2));

    // slice 1:3
    let slice = df2_obj.call_method("slice", &[Value::Integer(1), Value::Integer(3)]).unwrap();
    let slice_obj = match &slice { Value::ExternObject(o) => o, _ => panic!() };
    let nrows = slice_obj.call_method("nrows", &[]).unwrap();
    assert_eq!(nrows, Value::Integer(2));
}

#[test]
fn test_signal_rhs_scalar_operator() {
    // `2.0 * signal` — Signal is the right operand (self_on_left=false).
    let mut vectors = HashMap::new();
    vectors.insert("time".into(), VectorData::Real(vec![0.0, 1.0]));
    vectors.insert("v".into(), VectorData::Real(vec![3.0, 6.0]));
    let result = AnalysisResult {
        kind: AnalysisKind::Tran, dataset: "t".into(),
        vectors, run_errors: vec![],
    };
    let df = DataFrame::from_result(&result);
    let df_val = DataFrameObj::new(df);
    let obj = match &df_val { Value::ExternObject(o) => o, _ => panic!() };
    let sig = obj.call_method("get", &[Value::String("v".into())]).unwrap();
    let sig_obj = match &sig { Value::ExternObject(o) => o, _ => panic!() };

    // 2.0 * signal (self_on_left=false)
    let result = sig_obj.binary_op("*", &Value::Real(2.0), false).unwrap();
    match &result {
        Value::ExternObject(o) => {
            let vals = o.call_method("values", &[]).unwrap();
            assert_eq!(vals, Value::RealVec(vec![6.0, 12.0]));
        }
        _ => panic!(),
    }

    // 10.0 - signal (self_on_left=false): 10-3=7, 10-6=4
    let sub = sig_obj.binary_op("-", &Value::Real(10.0), false).unwrap();
    match &sub {
        Value::ExternObject(o) => {
            let vals = o.call_method("values", &[]).unwrap();
            assert_eq!(vals, Value::RealVec(vec![7.0, 4.0]));
        }
        _ => panic!(),
    }
}

#[test]
fn test_signal_integral_from_dataframe_column() {
    // df["v(out)"].integral() must find the time scale via column_signal's
    // synthesized AnalysisResult, not the original analysis object.
    let mut vectors = HashMap::new();
    vectors.insert("time".into(), VectorData::Real(vec![0.0, 1.0, 2.0]));
    vectors.insert("v(out)".into(), VectorData::Real(vec![0.0, 2.0, 2.0]));
    let result = AnalysisResult {
        kind: AnalysisKind::Tran, dataset: "t".into(),
        vectors, run_errors: vec![],
    };
    let df = DataFrame::from_result(&result);
    let df_val = DataFrameObj::new(df);
    let obj = match &df_val { Value::ExternObject(o) => o, _ => panic!() };
    let sig = obj.call_method("get", &[Value::String("v(out)".into())]).unwrap();
    let sig_obj = match &sig { Value::ExternObject(o) => o, _ => panic!() };

    // Trapezoidal: (0+2)/2*1 + (2+2)/2*1 = 1 + 2 = 3
    let integral = sig_obj.call_method("integral", &[]).unwrap();
    assert_eq!(integral, Value::Real(3.0));
}

#[test]
fn test_dataframe_to_csv() {
    let mut vectors = HashMap::new();
    vectors.insert("time".into(), VectorData::Real(vec![0.0, 1.0]));
    vectors.insert("v".into(), VectorData::Real(vec![0.5, 1.5]));
    let result = AnalysisResult {
        kind: AnalysisKind::Tran, dataset: "t".into(),
        vectors, run_errors: vec![],
    };
    let df = DataFrame::from_result(&result);
    let df_val = DataFrameObj::new(df);
    let obj = match &df_val { Value::ExternObject(o) => o, _ => panic!() };

    let path = "/tmp/test_df.csv";
    obj.call_method("to_csv", &[Value::String(path.into())]).unwrap();
    let content = std::fs::read_to_string(path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines[0], "time,v");
    assert_eq!(lines.len(), 3); // header + 2 rows
}

#[test]
fn test_dataframe_concat() {
    // Simulates MC: two runs with same columns, concat into one frame.
    let make_result = |t: &[f64], v: &[f64]| {
        let mut vectors = HashMap::new();
        vectors.insert("time".into(), VectorData::Real(t.to_vec()));
        vectors.insert("v".into(), VectorData::Real(v.to_vec()));
        AnalysisResult {
            kind: AnalysisKind::Tran, dataset: "t".into(),
            vectors, run_errors: vec![],
        }
    };
    let df1 = DataFrameObj::new(DataFrame::from_result(&make_result(&[0.0, 1.0], &[0.5, 1.5])));
    let df2 = DataFrameObj::new(DataFrame::from_result(&make_result(&[0.0, 1.0], &[2.0, 3.0])));

    let obj1 = match &df1 { Value::ExternObject(o) => o, _ => panic!() };
    let combined = obj1.call_method("concat", &[df2]).unwrap();
    let cobj = match &combined { Value::ExternObject(o) => o, _ => panic!() };
    let nrows = cobj.call_method("nrows", &[]).unwrap();
    assert_eq!(nrows, Value::Integer(4));
    let ncols = cobj.call_method("ncols", &[]).unwrap();
    assert_eq!(ncols, Value::Integer(2));
}

#[test]
fn test_signal_sigma_and_yield() {
    let mut vectors = HashMap::new();
    // values: 1,2,3,4,5
    vectors.insert("time".into(), VectorData::Real(vec![0.0,1.0,2.0,3.0,4.0]));
    vectors.insert("v".into(), VectorData::Real(vec![1.0,2.0,3.0,4.0,5.0]));
    let result = AnalysisResult {
        kind: AnalysisKind::Tran, dataset: "t".into(),
        vectors, run_errors: vec![],
    };
    let df = DataFrame::from_result(&result);
    let df_val = DataFrameObj::new(df);
    let obj = match &df_val { Value::ExternObject(o) => o, _ => panic!() };
    let sig = obj.call_method("get", &[Value::String("v".into())]).unwrap();
    let sig_obj = match &sig { Value::ExternObject(o) => o, _ => panic!() };

    // sample std dev of [1,2,3,4,5] = sqrt(10/4) = sqrt(2.5)
    let sigma = sig_obj.call_method("sigma", &[]).unwrap();
    match sigma {
        Value::Real(v) => assert!((v - 2.5f64.sqrt()).abs() < 1e-10),
        _ => panic!("expected Real"),
    }

    // yield >= 3.0: values [3,4,5] → 3/5 = 0.6
    let y = sig_obj.call_method("yield_", &[Value::Real(3.0), Value::String(">=".into())]).unwrap();
    assert_eq!(y, Value::Real(0.6));
}
