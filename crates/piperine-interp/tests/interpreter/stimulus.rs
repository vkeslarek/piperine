//! The function of an `ac_stim` site (feature `ir-interp`, INT-11 AC 1): `RE = mag·cos(phase)`
//! and `IM = mag·sin(phase)`, and exactly `RE = mag`, `IM = 0` when the site has no phase.

use piperine_ir::{Addend, BodyNode, BodySpec, Branch, BranchKind, IrBuilder, NodeRole, Operator, Origin, QuantityRole, Site, Stmt, Terminal};

use piperine_ir_fixtures::devices;
use crate::models;

use devices::{Devices, Ex};
use models::{In, Models};
use piperine_device_fixtures::shapes::View;

/// `Phased`: `i(p, n) +> ac_stim(mag, phase)` over real constants.
struct Lab;

impl Lab {
    fn phased(mag: f64, phase: f64) -> Models {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let mut e = Ex::new(&mut b, "phased.msl", 2, 9);
        let (m, p) = (e.real(mag), e.real(phase));
        let stim = e.stim(0, m, Some(p));
        let contribute = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value: stim }],
        });
        let at = |line: u32| Origin { file: "phased.msl".into(), line, col: 1 };
        let port = |name: &str| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: at(1) };
        let body = b.body(BodySpec {
            name: "Phased".into(),
            origin: at(1),
            params: vec![],
            nodes: vec![port("p"), port("n")],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![Site { name: "ac".into(), origin: at(2), enabled: None }],
            locals: vec![],
            analog: Some(vec![contribute]),
            children: vec![],
        });
        let top = Devices::alone(&mut b, body, "P", 1, vec![]);
        Models::of(b.finish(top))
    }
}

#[test]
fn a_stim_writes_its_phasor() {
    // The Vsource's site has no phase: RE is `ac`, IM exactly 0.
    let m = Models::all();
    let v = m.instance("Vsource");
    let view = View { shape: m.module("Vsource").prototype() };
    let (reads, _, _) = view.function("stim:0");
    assert_eq!(reads.len(), 1, "the site reads `ac` alone: {reads:?}");
    for ac in [1.0, 0.25, -3.0] {
        let out = Models::eval(&v, 0, &[In::F64(ac)], None).expect("stim");
        assert_eq!(out.values, [ac, 0.0]);
    }
    // A site with a phase of pi/2: RE within 1e-16 of 0, IM = mag.
    for (mag, phase) in [(2.0, std::f64::consts::FRAC_PI_2), (0.5, 0.3), (1.0, -1.1)] {
        let p = Lab::phased(mag, phase);
        let h = p.instance("Phased");
        let out = Models::eval(&h, 0, &[], None).expect("stim");
        assert_eq!(out.values, [mag * phase.cos(), mag * phase.sin()]);
    }
    let p = Lab::phased(2.0, std::f64::consts::FRAC_PI_2);
    let out = Models::eval(&p.instance("Phased"), 0, &[], None).expect("stim");
    assert!(out.values[0].abs() <= 1e-16 * 2.0 + 1e-16, "RE {}", out.values[0]);
    assert_eq!(out.values[1], 2.0);
}
