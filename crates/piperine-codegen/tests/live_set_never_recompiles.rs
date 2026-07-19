//! MD-18 proof for live sets (LIVE-02): once a circuit is compiled, `set` +
//! re-solve cycles never JIT again — `AnalogKernel::compile_count` stays
//! constant across ≥10 cycles mixing `Restamp` and `Temperature`
//! invalidations. Lives in its own test binary (single `#[test]`) so the
//! process-global compile counter is not polluted by concurrent tests.

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_codegen::ir::LoweredBody;
use piperine_codegen::{AnalogKernel, CircuitCompiler};
use piperine_solver::abi::{
    AnalogDevice, AnalogReference, DcAnalysisState, DigitalDevice, Element, ElementCapabilities,
    Introspect, Invalidation, ParamDescriptor, ParamError, ParamScope, Stamp, Value, ValueKind,
};
use piperine_solver::prelude::{Bounds, Context, NodeIdentifier};

const DIVIDER: &str = r#"
    discipline Electrical { potential v : Real; flow i : Real; }

    mod R (inout p : Electrical, inout n : Electrical) {
        param r : Real = 1.0e3;
    }
    analog R { I(p, n) <+ V(p, n) / r; }

    mod Vsrc (inout p : Electrical, inout n : Electrical) {
        param dc : Real = 10.0;
    }
    analog Vsrc { V(p, n) <- dc; }

    mod Top () {
        wire gnd : Electrical;
        wire top : Electrical;
        wire mid : Electrical;
        v1 : Vsrc(.p = top, .n = gnd) {};
        r1 : R(.p = top, .n = mid) {};
        r2 : R(.p = mid, .n = gnd) {};
    }
"#;

/// A temperature-dependent resistor whose `temp` write reports
/// [`Invalidation::Temperature`] — the non-JIT leg of the proof: the
/// recompute is a runtime constant refresh, never a compilation.
struct TempResistor {
    r0: f64,
    tc: f64,
    temp: f64,
    r_eff: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}

impl TempResistor {
    const TNOM: f64 = 300.15;

    fn refresh(&mut self) {
        self.r_eff = self.r0 * (1.0 + self.tc * (self.temp - Self::TNOM));
    }
}

impl AnalogDevice for TempResistor {
    fn set_temperature(&mut self, t: f64) {
        self.temp = t;
        self.refresh();
    }

    fn load_dc(
        &mut self,
        _state: &DcAnalysisState<'_>,
        _ctx: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let g = 1.0 / self.r_eff;
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }
}

impl DigitalDevice for TempResistor {}

impl Introspect for TempResistor {
    fn list_params(&self) -> Vec<ParamDescriptor> {
        vec![ParamDescriptor {
            name: "temp".into(),
            kind: ValueKind::Real,
            default: Value::Real(Self::TNOM),
            unit: Some("K".into()),
            bounds: Bounds::UNBOUNDED,
            scope: ParamScope::Instance,
            invalidation: Invalidation::Temperature,
        }]
    }

    fn get_param(&self, name: &str) -> Option<Value> {
        (name == "temp").then(|| Value::Real(self.temp))
    }

    fn set_param(&mut self, name: &str, value: Value) -> Result<Invalidation, ParamError> {
        if name != "temp" {
            return Err(ParamError::Unknown(name.into()));
        }
        let Some(v) = value.as_real() else {
            return Err(ParamError::TypeMismatch { name: name.into(), expected: ValueKind::Real });
        };
        self.temp = v;
        self.refresh();
        Ok(Invalidation::Temperature)
    }
}

impl Element for TempResistor {
    fn name(&self) -> &str {
        "rt"
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }
}

#[test]
fn live_sets_never_recompile_across_restamp_and_temperature_cycles() {
    let design = parse_and_elaborate(DIVIDER, &piperine_lang::SourceMap::dummy())
        .expect("divider elaborates");
    let bodies: HashMap<String, LoweredBody> =
        piperine_codegen::ir::lower_bodies(&design).expect("divider lowers");

    let before_build = AnalogKernel::compile_count();
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("Top").expect("circuit builds");
    let per_build = AnalogKernel::compile_count() - before_build;
    assert!(per_build > 0, "a build must JIT at least one kernel");

    // Wire a temperature-dependent element (rt: mid→gnd, 2 kΩ at TNOM)
    // into the compiled circuit before the first solve freezes the matrix.
    let mid_id = info.nets.get("mid").expect("net `mid`").clone();
    let n_mid = circuit.netlist.connect_node(mid_id.clone());
    let n_gnd = circuit.netlist.connect_node(NodeIdentifier::Gnd);
    let mut rt = TempResistor { r0: 2000.0, tc: 1e-3, temp: 0.0, r_eff: 0.0, n1: n_mid, n2: n_gnd };
    rt.temp = TempResistor::TNOM;
    rt.refresh();
    circuit.devices.push(Box::new(rt));

    let read_mid = |r: &piperine_solver::prelude::DcAnalysisResult| -> f64 {
        r.get_node(&mid_id).expect("v(mid)")
    };
    // v(mid) = 10 · (r2 ∥ rt) / (1k + r2 ∥ rt)
    let expected = |r2: f64, rt_eff: f64| -> f64 {
        let par = r2 * rt_eff / (r2 + rt_eff);
        10.0 * par / (1000.0 + par)
    };

    let after_build = AnalogKernel::compile_count();
    for cycle in 0..10 {
        // Restamp leg: the JIT resistor's value moves each cycle.
        let r2 = 1000.0 + 500.0 * cycle as f64;
        let inv = circuit
            .set_element_param("r2", "r", Value::Real(r2))
            .expect("restamp set on jit device");
        assert_eq!(inv, Invalidation::Restamp);

        // Temperature leg: the tempco resistor's instance temperature moves.
        let temp = TempResistor::TNOM + 25.0 * cycle as f64;
        let inv = circuit
            .set_element_param("rt", "temp", Value::Real(temp))
            .expect("temperature set");
        assert_eq!(inv, Invalidation::Temperature);

        let result = circuit.dc(Context::default()).unwrap().solve().unwrap();
        let rt_eff = 2000.0 * (1.0 + 1e-3 * (temp - TempResistor::TNOM));
        let want = expected(r2, rt_eff);
        let got = read_mid(&result);
        assert!(
            (got - want).abs() < 1e-9,
            "cycle {cycle}: v(mid) = {got}, want {want} (r2 = {r2}, rt = {rt_eff})"
        );
    }

    assert_eq!(
        AnalogKernel::compile_count(),
        after_build,
        "MD-18: 10 set+solve cycles must not JIT — compile count moved"
    );
}
