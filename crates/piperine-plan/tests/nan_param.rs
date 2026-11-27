//! Scope: the bits of a param that folded to NaN (the `msl-fixes` fix round, `GAPS.md` closed by params-and-io): the plan
//! gives the pin the library's own NaN, `f64::NAN`, whatever bits the elaborator's `0.0 / 0.0` has.

use piperine_ir::{BodySpec, Constant, InstanceSpec, Ir, IrBuilder, Origin, Param, Real, ValueType};
use piperine_plan::BodyPlan;

/// A body with one `f64` param whose default is `value`.
struct NanBody;

impl NanBody {
    /// The IR of `Top` with the param `lo` defaulting to `value`.
    fn ir(value: f64) -> Ir {
        let origin = Origin { file: "nan.msl".into(), line: 1, col: 1 };
        let mut builder = IrBuilder::new();
        let param = Param { name: "lo".into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(value))), check: None, structural: false, origin: origin.clone() };
        let body = builder.body(BodySpec { name: "Top".into(), origin: origin.clone(), params: vec![param], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] });
        let top = builder.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body, parent: None, nodes: vec![], params: vec![Constant::Real(Real::new(value))], origin });
        builder.finish(top)
    }
}

#[test]
fn a_nan_param_default_is_the_librarys_nan() {
    // the elaborator's `0.0 / 0.0` is `0xfff8..`; the pin holds `f64::NAN`'s bits (`0x7ff8..`)
    let negative_nan = f64::from_bits(0xfff8_0000_0000_0000);
    let ir = NanBody::ir(negative_nan);
    let plan = BodyPlan::build(ir.bodies().next().expect("a body")).expect("the body plans");
    let shape = plan.shape().shape();
    let pin = shape.pins.iter().find(|pin| pin.name.as_str() == "lo").expect("the param pin");
    assert_eq!(pin.init.as_slice(), f64::NAN.to_le_bytes().as_slice());
}
