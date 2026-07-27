//! PY-01/02/03/14 — the native `_piperine` design-reflection surface:
//! `load` returns a `_Design` whose `modules()`/`module(name)` enumerate the
//! elaborated POM, a module reflects its ports/nets/instances/params/behaviors,
//! and `select(path)` resolves a selector path (raising on a miss or a
//! malformed path).

mod common;

use pyo3::prelude::*;
use pyo3::types::PyModule;

use piperine_python::_piperine;

use common::{instance_pairs, loaded_design, sorted_names, PHDL};

/// PY-01/02: `load` returns a `_Design` whose `modules()` lists every
/// elaborated module; `module(name)` returns that module and raises when
/// the name is unknown.
#[test]
fn load_returns_reflected_design() -> PyResult<()> {
    let path = std::env::temp_dir().join("piperine_python_p3_load_test.phdl");
    std::fs::write(&path, PHDL)?;
    let path_str = path
        .to_str()
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

    let outcome = Python::with_gil(|py| -> PyResult<()> {
        let m = PyModule::new(py, "_piperine")?;
        _piperine(&m)?;

        let design = m.getattr("load")?.call1((path_str,))?;

        // Spec edge case: a nonexistent path raises (FileNotFoundError /
        // ValueError), never a silent success.
        assert!(
            m.getattr("load")?
                .call1(("/nonexistent/piperine_missing.phdl",))
                .is_err(),
            "loading a missing file must raise"
        );

        // modules() lists every elaborated module.
        let modules = design.getattr("modules")?.call0()?;
        let mut names: Vec<String> = modules
            .try_iter()?
            .map(|item| item?.getattr("name")?.extract::<String>())
            .collect::<PyResult<Vec<String>>>()?;
        names.sort();
        assert!(
            names.contains(&"Resistor".to_string()),
            "Resistor should be reflected, got {names:?}"
        );
        assert!(
            names.contains(&"DividerBoard".to_string()),
            "DividerBoard should be reflected, got {names:?}"
        );

        // module(name) returns the named module; missing → raises.
        let r = design
            .getattr("module")?
            .call1(("Resistor",))?
            .getattr("name")?
            .extract::<String>()?;
        assert_eq!(r, "Resistor");
        assert!(
            design.getattr("module")?.call1(("DoesNotExist",)).is_err(),
            "looking up a missing module must raise"
        );
        Ok(())
    });
    let _ = std::fs::remove_file(&path);
    outcome
}

/// PY-03 / spec AC14: a module reflects its ports, nets, instances,
/// params, and behaviors as typed lists with their attributes.
#[test]
fn module_reflects_structure() -> PyResult<()> {
    let path = std::env::temp_dir().join("piperine_python_p4_reflect_test.phdl");
    std::fs::write(&path, PHDL)?;
    let path_str = path
        .to_str()
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

    let outcome = Python::with_gil(|py| -> PyResult<()> {
        let design = loaded_design(py, path_str)?;

        // DividerBoard: 3 nets (gnd, vin, mid), 2 instances (r_bot, r_top),
        // and no ports/params/behaviors of its own.
        let board = design.getattr("module")?.call1(("DividerBoard",))?;
        assert_eq!(
            sorted_names(board.getattr("nets")?.call0()?)?,
            vec!["gnd", "mid", "vin"]
        );
        let pairs = instance_pairs(board.getattr("instances")?.call0()?)?;
        assert_eq!(pairs.len(), 2);
        assert!(pairs.contains(&("r_top".into(), "Resistor".into())));
        assert!(pairs.contains(&("r_bot".into(), "Resistor".into())));
        assert!(board.getattr("ports")?.call0()?.try_iter()?.next().is_none());
        assert!(board.getattr("params")?.call0()?.try_iter()?.next().is_none());
        assert!(board.getattr("behaviors")?.call0()?.try_iter()?.next().is_none());

        // Each net carries its discipline type.
        for item in board.getattr("nets")?.call0()?.try_iter()? {
            let net: Bound<'_, PyAny> = item?;
            assert_eq!(net.getattr("ty")?.extract::<String>()?, "Electrical");
        }

        // Resistor: ports (n, p) both `inout : Electrical`, one param `r`
        // (Real, default 1e3), one `analog` behavior.
        let resistor = design.getattr("module")?.call1(("Resistor",))?;
        assert_eq!(
            sorted_names(resistor.getattr("ports")?.call0()?)?,
            vec!["n", "p"]
        );
        for item in resistor.getattr("ports")?.call0()?.try_iter()? {
            let port: Bound<'_, PyAny> = item?;
            assert_eq!(port.getattr("direction")?.extract::<String>()?, "inout");
            assert_eq!(port.getattr("ty")?.extract::<String>()?, "Electrical");
        }
        let params: Vec<Bound<'_, PyAny>> = resistor
            .getattr("params")?
            .call0()?
            .try_iter()?
            .collect::<PyResult<Vec<_>>>()?;
        assert_eq!(params.len(), 1, "Resistor has one param");
        assert_eq!(params[0].getattr("name")?.extract::<String>()?, "r");
        assert_eq!(params[0].getattr("ty")?.extract::<String>()?, "Real");
        assert!((params[0].getattr("default")?.extract::<f64>()? - 1e3).abs() < 1e-6);

        let behaviors: Vec<Bound<'_, PyAny>> = resistor
            .getattr("behaviors")?
            .call0()?
            .try_iter()?
            .collect::<PyResult<Vec<_>>>()?;
        assert_eq!(behaviors.len(), 1, "Resistor has one behavior");
        assert_eq!(behaviors[0].getattr("kind")?.extract::<String>()?, "analog");
        Ok(())
    });
    let _ = std::fs::remove_file(&path);
    outcome
}

/// PY-14 / spec AC15: `design.select(path)` resolves a hierarchical
/// selector path to a typed node selection; an unresolved path raises
/// (fail loud, never an empty-success per spec edge cases).
///
/// The POM selector grammar uses `/`-separated steps with optional
/// `axis::name` segments (e.g. `/r_top/port::p`): `/r_top` matches the
/// `r_top` instance under the (inferred) top module; `port::p` walks
/// that instance's module ports and filters by name `p`. The spec's
/// dot-notation examples (`"buck.r1.p"`) are an imprecision — the
/// actual selector grammar (parse.rs) does not accept `.`.
#[test]
fn select_resolves_path_and_errors_on_miss() -> PyResult<()> {
    let path = std::env::temp_dir().join("piperine_python_p5_select_test.phdl");
    std::fs::write(&path, PHDL)?;
    let path_str = path
        .to_str()
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

    let outcome = Python::with_gil(|py| -> PyResult<()> {
        let design = loaded_design(py, path_str)?;

        // One-step path: `/r_top` resolves to the labelled `r_top`
        // instance under the inferred top module (DividerBoard).
        let sel = design.getattr("select")?.call1(("/r_top",))?;
        assert_eq!(sel.getattr("len")?.call0()?.extract::<usize>()?, 1);
        let nodes: Vec<Bound<'_, PyAny>> = sel
            .getattr("nodes")?
            .call0()?
            .try_iter()?
            .collect::<PyResult<Vec<_>>>()?;
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].getattr("kind")?.extract::<String>()?, "instance");
        assert_eq!(nodes[0].getattr("name")?.extract::<String>()?, "r_top");

        // Two-step path: `/r_top/port::p` descends into the instance's
        // module (Resistor) and resolves port `p`.
        let port_sel = design.getattr("select")?.call1(("/r_top/port::p",))?;
        let port_nodes: Vec<Bound<'_, PyAny>> = port_sel
            .getattr("nodes")?
            .call0()?
            .try_iter()?
            .collect::<PyResult<Vec<_>>>()?;
        assert_eq!(port_nodes.len(), 1);
        assert_eq!(port_nodes[0].getattr("kind")?.extract::<String>()?, "port");
        assert_eq!(port_nodes[0].getattr("name")?.extract::<String>()?, "p");

        // Unresolved path → KeyError (fail loud, spec edge case).
        let miss = design
            .getattr("select")?
            .call1(("/does_not_exist",))
            .unwrap_err();
        assert!(
            miss.is_instance_of::<pyo3::exceptions::PyKeyError>(py),
            "unresolved select must raise KeyError, got {miss}"
        );

        // Malformed path → ValueError (parse failure surfaced loudly).
        let bad = design.getattr("select")?.call1(("not:::valid",)).unwrap_err();
        assert!(
            bad.is_instance_of::<pyo3::exceptions::PyValueError>(py),
            "malformed select must raise ValueError, got {bad}"
        );
        Ok(())
    });
    let _ = std::fs::remove_file(&path);
    outcome
}
