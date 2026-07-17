//! Live parameter mutation on a compiled circuit (solver-live-params
//! feature): loud addressing errors, bypass-cache invalidation, and idle
//! sets applying to the next analysis run — LIVE-03/04/05/08.

use piperine_solver::prelude::*;
use piperine_solver::abi::{DcAnalysisState, Stamp};

/// A linear resistor with one writable parameter `r` (bounds: r > 0),
/// declared `BYPASS_OK` so the DC device-bypass stamp cache applies to it.
struct Resistor {
    label: String,
    r: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}

impl Element for Resistor {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::BYPASS_OK
    }

    fn list_params(&self) -> Vec<ParamDescriptor> {
        vec![ParamDescriptor {
            name: "r".into(),
            kind: ValueKind::Real,
            default: Value::Real(self.r),
            unit: Some("ohm".into()),
            bounds: Bounds { min: Some(1e-9), max: None },
            scope: ParamScope::Instance,
            invalidation: Invalidation::Restamp,
        }]
    }

    fn get_param(&self, name: &str) -> Option<Value> {
        (name == "r").then(|| Value::Real(self.r))
    }

    fn set_param(&mut self, name: &str, value: Value) -> std::result::Result<Invalidation, ParamError> {
        if name != "r" {
            return Err(ParamError::Unknown(name.into()));
        }
        let Some(v) = value.as_real() else {
            return Err(ParamError::TypeMismatch { name: name.into(), expected: ValueKind::Real });
        };
        if v <= 0.0 {
            return Err(ParamError::OutOfRange { name: name.into(), value });
        }
        self.r = v;
        Ok(Invalidation::Restamp)
    }

    fn load_dc(
        &mut self,
        _state: &DcAnalysisState<'_>,
        _ctx: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let g = 1.0 / self.r;
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }
}

/// An ideal DC voltage source; writes to `dc` invalidate the operating point.
struct Vdc {
    label: String,
    v: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    branch: AnalogReference,
}

impl Element for Vdc {
    fn name(&self) -> &str {
        &self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }

    fn list_params(&self) -> Vec<ParamDescriptor> {
        vec![ParamDescriptor {
            name: "dc".into(),
            kind: ValueKind::Real,
            default: Value::Real(self.v),
            unit: Some("V".into()),
            bounds: Bounds::UNBOUNDED,
            scope: ParamScope::Instance,
            invalidation: Invalidation::OperatingPoint,
        }]
    }

    fn get_param(&self, name: &str) -> Option<Value> {
        (name == "dc").then(|| Value::Real(self.v))
    }

    fn set_param(&mut self, name: &str, value: Value) -> std::result::Result<Invalidation, ParamError> {
        if name != "dc" {
            return Err(ParamError::Unknown(name.into()));
        }
        let Some(v) = value.as_real() else {
            return Err(ParamError::TypeMismatch { name: name.into(), expected: ValueKind::Real });
        };
        self.v = v;
        Ok(Invalidation::OperatingPoint)
    }

    fn load_dc(
        &mut self,
        _state: &DcAnalysisState<'_>,
        _ctx: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let branch = self.branch.clone();
        vec![
            Stamp::Matrix(self.n1.clone(), branch.clone(), 1.0),
            Stamp::Matrix(branch.clone(), self.n1.clone(), 1.0),
            Stamp::Matrix(self.n2.clone(), branch.clone(), -1.0),
            Stamp::Matrix(branch.clone(), self.n2.clone(), -1.0),
            Stamp::Rhs(branch, self.v),
        ]
    }
}

/// 10 V source over r1 (top→mid) and r2 (mid→gnd): v(mid) = 10·r2/(r1+r2).
fn divider(r1: f64, r2: f64) -> CircuitInstance {
    let mut netlist = Netlist::new();
    let top = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let mid = netlist.connect_node(NodeIdentifier::Anonymous(2));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let v1 = Vdc { label: "v1".into(), v: 10.0, n1: top.clone(), n2: gnd.clone(), branch };
    let r1 = Resistor { label: "r1".into(), r: r1, n1: top, n2: mid.clone() };
    let r2 = Resistor { label: "r2".into(), r: r2, n1: mid, n2: gnd };

    CircuitInstance::from_devices_and_netlist(
        "divider",
        vec![Box::new(v1), Box::new(r1), Box::new(r2)],
        netlist,
    )
}

fn v_mid(result: &DcAnalysisResult) -> f64 {
    result.get_node(&NodeIdentifier::Anonymous(2)).expect("v(mid)")
}

// ── LIVE-04: unknown path fails loud with the path ──────────────────────────

#[test]
fn unknown_label_fails_loud_with_the_path() {
    let mut circuit = divider(1000.0, 1000.0);
    let err = circuit
        .set_element_param("nope", "r", Value::Real(1.0))
        .expect_err("unknown label must fail");
    assert!(err.to_string().contains("nope"), "error names the path: {err}");
}

// ── LIVE-03: unknown param fails loud, listing the element's params ─────────

#[test]
fn unknown_param_fails_loud_and_lists_available_params() {
    let mut circuit = divider(1000.0, 1000.0);
    let err = circuit
        .set_element_param("r1", "bogus", Value::Real(1.0))
        .expect_err("unknown param must fail");
    let msg = err.to_string();
    assert!(msg.contains("r1"), "error names the element: {msg}");
    assert!(msg.contains("bogus"), "error names the bad param: {msg}");
    assert!(msg.contains("available parameters"), "error lists candidates: {msg}");
    assert!(msg.contains("`r`") || msg.contains(": r"), "candidate list holds `r`: {msg}");
}

#[test]
fn unknown_param_on_paramless_element_says_so() {
    struct Mute;
    impl Element for Mute {
        fn name(&self) -> &str { "mute" }
        fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::empty() }
    }
    let mut circuit = CircuitInstance::from_devices_and_netlist(
        "mute",
        vec![Box::new(Mute) as Box<dyn Element>],
        Netlist::new(),
    );
    let err = circuit
        .set_element_param("mute", "x", Value::Real(1.0))
        .expect_err("element with no params must reject");
    assert!(
        err.to_string().contains("declares no writable parameters"),
        "empty-list case is explicit: {err}"
    );
}

// ── Edge case: out-of-bounds set fails loud via ParamDescriptor bounds ──────

#[test]
fn out_of_bounds_set_fails_loud_and_leaves_value_unchanged() {
    let mut circuit = divider(1000.0, 1000.0);
    // 1e-12 is positive (the element itself would accept it) but below the
    // declared bounds minimum 1e-9 — only the central ParamDescriptor
    // bounds gate can reject it, proving no partial apply happened.
    let err = circuit
        .set_element_param("r1", "r", Value::Real(1e-12))
        .expect_err("below-bounds value must fail");
    let msg = err.to_string();
    assert!(msg.contains("out of bounds"), "bounds rejection is explicit: {msg}");
    assert!(msg.contains("1e-9") || msg.contains("0.000000001"), "bounds in message: {msg}");
    let r1 = circuit.all_devices().iter().find(|d| d.name() == "r1").unwrap();
    assert_eq!(r1.get_param("r"), Some(Value::Real(1000.0)), "value unchanged");
}

// ── LIVE-08: an idle set applies to the next analysis run ───────────────────

#[test]
fn idle_set_applies_to_the_next_analysis_run() {
    let mut circuit = divider(1000.0, 1000.0);
    let first = circuit.dc(Context::default()).unwrap().solve().unwrap();
    assert!((v_mid(&first) - 5.0).abs() < 1e-9, "10·1k/2k = 5 V");

    // No analysis running — the set simply lands on the element and the
    // next run picks it up. Numeric-only change reports `Restamp`.
    let inv = circuit
        .set_element_param("r2", "r", Value::Real(3000.0))
        .expect("live set on idle circuit");
    assert_eq!(inv, Invalidation::Restamp);

    let second = circuit.dc(Context::default()).unwrap().solve().unwrap();
    assert!(
        (v_mid(&second) - 7.5).abs() < 1e-9,
        "10·3k/4k = 7.5 V after the set, got {}",
        v_mid(&second)
    );

    // A source-value set reports `OperatingPoint` and the next run reflects it.
    let inv = circuit
        .set_element_param("v1", "dc", Value::Real(20.0))
        .expect("source set");
    assert_eq!(inv, Invalidation::OperatingPoint);
    let third = circuit.dc(Context::default()).unwrap().solve().unwrap();
    assert!((v_mid(&third) - 15.0).abs() < 1e-9, "20·3k/4k = 15 V, got {}", v_mid(&third));
}

// ── LIVE-05: a set through a held analysis drops the bypass stamp cache ─────

#[test]
fn set_through_held_dc_analysis_invalidates_bypass_cache() {
    let mut circuit = divider(1000.0, 1000.0);
    let mut dc = circuit.dc(Context::default()).unwrap();

    let first = dc.solve().unwrap();
    assert!((v_mid(&first) - 5.0).abs() < 1e-9);

    // The set lands while the analysis holds a warm stamp cache. Without
    // invalidation the re-solve's unmoved warm start reuses the stale
    // stamps and silently locks in the old operating point (CP-11 trap).
    dc.set_element_param("r2", "r", Value::Real(3000.0)).expect("live set");

    let second = dc.solve().unwrap();
    assert!(
        (v_mid(&second) - 7.5).abs() < 1e-9,
        "stale bypass froze the linearization: got {} V, want 7.5 V",
        v_mid(&second)
    );
}

// ── Addressing errors through a held analysis stay loud ─────────────────────

#[test]
fn set_through_held_dc_analysis_keeps_loud_errors() {
    let mut circuit = divider(1000.0, 1000.0);
    let mut dc = circuit.dc(Context::default()).unwrap();
    let err = dc
        .set_element_param("r1", "bogus", Value::Real(1.0))
        .expect_err("unknown param must fail");
    assert!(err.to_string().contains("available parameters"), "{err}");
}
