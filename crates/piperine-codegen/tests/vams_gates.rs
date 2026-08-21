//! LRM-COVERS: A.3.2, A.3.4
//! T48 (VAMS-18) — IEEE 1364 gate-primitive truth tables and drive-strength
//! resolution, exercised through `piperine_codegen`'s public surface
//! (`piperine_codegen::kernel::digital::{GatePrimitive, Strength}`).
//! Mirrors T48's "Done when": every primitive's truth table asserted;
//! drive strengths honored.

use piperine_codegen::kernel::digital::{GatePrimitive, Strength};
use piperine_solver::abi::LogicValue::{self, One, X, Z, Zero};

fn n(prim: GatePrimitive, a: LogicValue, b: LogicValue) -> LogicValue {
    prim.eval_n(&[a, b]).unwrap()
}

#[test]
fn and_nand_or_nor_xor_xnor_truth_tables() {
    // and (Table 7-4)
    assert_eq!(n(GatePrimitive::And, Zero, Zero), Zero);
    assert_eq!(n(GatePrimitive::And, Zero, One), Zero);
    assert_eq!(n(GatePrimitive::And, One, One), One);
    assert_eq!(n(GatePrimitive::And, One, X), X);
    // nand
    assert_eq!(n(GatePrimitive::Nand, One, One), Zero);
    assert_eq!(n(GatePrimitive::Nand, Zero, Zero), One);
    // or
    assert_eq!(n(GatePrimitive::Or, Zero, Zero), Zero);
    assert_eq!(n(GatePrimitive::Or, Zero, One), One);
    // nor
    assert_eq!(n(GatePrimitive::Nor, Zero, Zero), One);
    assert_eq!(n(GatePrimitive::Nor, One, Zero), Zero);
    // xor
    assert_eq!(n(GatePrimitive::Xor, Zero, One), One);
    assert_eq!(n(GatePrimitive::Xor, One, One), Zero);
    // xnor
    assert_eq!(n(GatePrimitive::Xnor, Zero, One), Zero);
    assert_eq!(n(GatePrimitive::Xnor, One, One), One);
}

#[test]
fn buf_not_truth_tables() {
    assert_eq!(GatePrimitive::Buf.eval1(Zero, None).unwrap(), Zero);
    assert_eq!(GatePrimitive::Buf.eval1(One, None).unwrap(), One);
    assert_eq!(GatePrimitive::Not.eval1(Zero, None).unwrap(), One);
    assert_eq!(GatePrimitive::Not.eval1(One, None).unwrap(), Zero);
    assert_eq!(GatePrimitive::Not.eval1(X, None).unwrap(), X);
}

#[test]
fn tristate_gate_truth_tables() {
    // bufif1: passes data when control=1, else Z
    assert_eq!(GatePrimitive::Bufif1.eval1(One, Some(One)).unwrap(), One);
    assert_eq!(GatePrimitive::Bufif1.eval1(One, Some(Zero)).unwrap(), Z);
    // bufif0: passes data when control=0, else Z
    assert_eq!(GatePrimitive::Bufif0.eval1(Zero, Some(Zero)).unwrap(), Zero);
    assert_eq!(GatePrimitive::Bufif0.eval1(Zero, Some(One)).unwrap(), Z);
    // notif1 / notif0
    assert_eq!(GatePrimitive::Notif1.eval1(One, Some(One)).unwrap(), Zero);
    assert_eq!(GatePrimitive::Notif0.eval1(One, Some(Zero)).unwrap(), Zero);
}

#[test]
fn drive_strengths_are_honored_in_conflict_resolution() {
    // A supply-strength driver overrides a strong-strength opposing driver.
    let (v, s) = Strength::resolve((One, Strength::Supply), (Zero, Strength::Strong));
    assert_eq!(v, One);
    assert_eq!(s, Strength::Supply);

    // Equal-strength opposing drivers contend to X (bus fight).
    let (v, _) = Strength::resolve((One, Strength::Strong), (Zero, Strength::Strong));
    assert_eq!(v, X);

    // A non-driving (HighZ) side always yields to the other driver.
    let (v, s) = Strength::resolve((Zero, Strength::HighZ), (One, Strength::Weak));
    assert_eq!(v, One);
    assert_eq!(s, Strength::Weak);
}
