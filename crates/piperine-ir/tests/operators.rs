//! Scope: the IR forms of `analog-operators` (delay sites, noise sources, session parameters,
//! trace sites and symbolic differentiation), built by hand through `IrBuilder`, checked,
//! printed and round-tripped through serde (AO-01, AO-02, AO-03, AO-07, AO-09).

#[path = "support/devices.rs"]
#[allow(dead_code)]
mod devices;

use devices::{Devices, Ex};
use piperine_ir::{
    Addend, BodySpec, Branch, BranchKind, Constant, DelaySite, NoiseSource, Expr, ExprId, FunctionSpec, Ir, IrBuilder, InstanceSpec, NodeRef, NodeRole, Operator, Origin, Param, QuantityRole, Real, Stmt, StmtId, Target, Terminal, ValueType,
    BodyNode,
};

/// What a test writes into the body `Operators` builds.
#[derive(Default)]
struct Written {
    analog: Vec<StmtId>,
    delays: Vec<DelaySite>,
    noise_sources: Vec<NoiseSource>,
}

/// The one body `Op(a)` with the flow branch `(a, gnd)` and the param `td: f64` (index 0), whose
/// `analog` block the test writes; placed alone as the design.
struct Operators;

impl Operators {
    const FILE: &'static str = "operators.msl";

    /// The origin every form of the fixture shares.
    fn at() -> Origin {
        Origin { file: Self::FILE.into(), line: 3, col: 5 }
    }

    /// The expression helper at the fixed position.
    fn ex(b: &mut IrBuilder) -> Ex<'_> {
        Ex::new(b, Self::FILE, 3, 5)
    }

    /// `i(a, gnd) +> value;`
    fn contribute(b: &mut IrBuilder, value: ExprId) -> StmtId {
        b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Flow, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] })
    }

    /// The design `Op` with everything `write` returns.
    fn ir(write: impl FnOnce(&mut IrBuilder) -> Written) -> Ir {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let origin = Origin { file: Self::FILE.into(), line: 1, col: 1 };
        let Written { analog, delays, noise_sources } = write(&mut b);
        let delay_time = Param { name: "td".into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(1e-9))), check: None, structural: false, origin: origin.clone() };
        let body = b.body(BodySpec {
            name: "Op".into(),
            origin: origin.clone(),
            params: vec![delay_time],
            nodes: vec![BodyNode { name: "a".into(), nature: el, role: NodeRole::Port, init: None, origin: origin.clone() }],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: Some(analog),
            children: vec![],
        });
        for site in delays {
            b.delay(body, site);
        }
        for source in noise_sources {
            b.noise_source(body, source);
        }
        let params = vec![Constant::Real(Real::new(1e-9))];
        let top = b.instance(InstanceSpec { path: "Op".into(), name: "Op".into(), body, parent: None, nodes: vec![NodeRef::Ground], params, origin });
        b.finish(top)
    }

    /// Whether any entry of `broken` says `what`.
    fn names(broken: &[piperine_ir::Broken], what: &str) -> bool {
        broken.iter().any(|entry| entry.what.contains(what))
    }

    /// The text of `ir` after a serde round trip, which must equal `ir`.
    fn round_trip(ir: &Ir) -> Ir {
        serde_json::from_str(&serde_json::to_string(ir).expect("serialize")).expect("deserialize")
    }

    /// `i(a, gnd) +> 0.5 * delay<k>.value;`
    fn contribute_half_the_delay(b: &mut IrBuilder, k: u32) -> StmtId {
        let read = b.expr(Expr::Delay(k), ValueType::F64, Self::at());
        let mut e = Self::ex(b);
        let half = e.real(0.5);
        let value = e.binary(piperine_ir::BinaryOp::Mul, half, read);
        Self::contribute(b, value)
    }

    /// The site `delay(v(a), td, <maximum>)` named `d`.
    fn delay_site(b: &mut IrBuilder, maximum: Option<f64>) -> DelaySite {
        let input = Self::ex(b).probe(Terminal::Node(0), Terminal::Ground);
        let time = b.expr(Expr::Param(0), ValueType::F64, Self::at());
        let maximum = maximum.map(|m| Self::ex(b).real(m));
        DelaySite { name: "d".into(), input, time, maximum, origin: Self::at() }
    }
}

#[test]
fn a_delay_site_is_built_checked_printed_and_round_tripped() {
    // `i(a, gnd) +> 0.5 * delay(v(a), td, 2e-9)`: one site D0 read once.
    let ir = Operators::ir(|b| {
        let site = Operators::delay_site(b, Some(2e-9));
        Written { analog: vec![Operators::contribute_half_the_delay(b, 0)], delays: vec![site], ..Written::default() }
    });
    assert_eq!(ir.check(), vec![], "{}", ir);
    let text = ir.to_string();
    assert!(text.contains("delay0(probe(a, gnd), td, 2e-9) \"d\""), "{text}");
    assert!(text.contains("(0.5 * delay0.value)"), "{text}");
    let body = ir.bodies().next().expect("a body");
    assert_eq!(body.delays().len(), 1);
    assert_eq!(Operators::round_trip(&ir), ir);
    // The site's three parts and the read are dependencies, as a derivative's are.
    assert_eq!(body.dependencies(Target::DelayInput(0)).nodes, vec![0]);
    assert_eq!(body.dependencies(Target::DelayTime(0)).params, vec![0]);
    assert!(body.dependencies(Target::DelayMaximum(0)).nodes.is_empty());
    assert_eq!(body.delay_reads(Target::Flow(0)), vec![0]);
    assert_eq!(body.dependencies(Target::Flow(0)).delays, vec![0]);
    // The output is an unknown of the system: it reaches a sum affinely.
    assert!(body.is_affine(0));
}

#[test]
fn a_delay_read_out_of_range_or_in_a_function_is_broken() {
    // `delay1` with one site, and `delay0` in a body with none.
    let one_site = Operators::ir(|b| {
        let site = Operators::delay_site(b, None);
        Written { analog: vec![Operators::contribute_half_the_delay(b, 1)], delays: vec![site], ..Written::default() }
    });
    assert!(Operators::names(&one_site.check(), "delay read of site 1, out of range (1 sites)"), "{:?}", one_site.check());
    let none = Operators::ir(|b| Written { analog: vec![Operators::contribute_half_the_delay(b, 0)], delays: vec![], ..Written::default() });
    assert!(Operators::names(&none.check(), "delay read of site 0, out of range (0 sites)"), "{:?}", none.check());
    // A function that reads a delay: a function has no body to own the site.
    let in_function = Operators::ir(|b| {
        let read = b.expr(Expr::Delay(0), ValueType::F64, Operators::at());
        b.function(FunctionSpec { name: "late".into(), origin: Operators::at(), params: vec![], ret: ValueType::F64, locals: vec![], body: read });
        let site = Operators::delay_site(b, None);
        Written { analog: vec![Operators::contribute_half_the_delay(b, 0)], delays: vec![site], ..Written::default() }
    });
    assert!(Operators::names(&in_function.check(), "a delay read outside a body"), "{:?}", in_function.check());
}

/// Noise sources.
impl Operators {
    /// `i(a, gnd) +> noise<k>.value;`
    fn contribute_the_noise(b: &mut IrBuilder, k: u32) -> StmtId {
        let read = b.expr(Expr::Noise(k), ValueType::F64, Self::at());
        Self::contribute(b, read)
    }

    /// The source `noise(4e-21, "thermal")` named `n`; `group` is its literal name.
    fn noise_site(b: &mut IrBuilder, group: Option<&str>) -> NoiseSource {
        let density = Self::ex(b).real(4e-21);
        NoiseSource { name: "n".into(), group: group.map(str::to_string), density, origin: Self::at() }
    }
}

#[test]
fn a_noise_source_is_built_checked_printed_and_round_tripped() {
    // `i(a, gnd) +> noise(4e-21, "thermal")`: one source N0 read once.
    let ir = Operators::ir(|b| {
        let source = Operators::noise_site(b, Some("thermal"));
        Written { analog: vec![Operators::contribute_the_noise(b, 0)], noise_sources: vec![source], ..Written::default() }
    });
    assert_eq!(ir.check(), vec![], "{}", ir);
    let text = ir.to_string();
    assert!(text.contains("noise0(4e-21, \"thermal\") \"n\""), "{text}");
    assert!(text.contains("noise0.value"), "{text}");
    let body = ir.bodies().next().expect("a body");
    assert_eq!(body.noise_sources().len(), 1);
    assert_eq!(Operators::round_trip(&ir), ir);
    assert!(body.dependencies(Target::NoiseDensity(0)).nodes.is_empty());
    assert_eq!(body.noise_reads(Target::Flow(0)), vec![0]);
    assert_eq!(body.dependencies(Target::Flow(0)).noise_sources, vec![0]);
    // An unnamed source prints its group as `none` and round-trips too.
    let unnamed = Operators::ir(|b| {
        let source = Operators::noise_site(b, None);
        Written { analog: vec![Operators::contribute_the_noise(b, 0)], noise_sources: vec![source], ..Written::default() }
    });
    assert!(unnamed.to_string().contains("noise0(4e-21, none)"), "{unnamed}");
    assert_eq!(Operators::round_trip(&unnamed), unnamed);
}

#[test]
fn a_noise_read_out_of_range_or_in_a_function_is_broken() {
    let none = Operators::ir(|b| Written { analog: vec![Operators::contribute_the_noise(b, 0)], ..Written::default() });
    assert!(Operators::names(&none.check(), "noise read of source 0, out of range (0 sources)"), "{:?}", none.check());
    let in_function = Operators::ir(|b| {
        let read = b.expr(Expr::Noise(0), ValueType::F64, Operators::at());
        b.function(FunctionSpec { name: "hiss".into(), origin: Operators::at(), params: vec![], ret: ValueType::F64, locals: vec![], body: read });
        let source = Operators::noise_site(b, None);
        Written { analog: vec![Operators::contribute_the_noise(b, 0)], noise_sources: vec![source], ..Written::default() }
    });
    assert!(Operators::names(&in_function.check(), "a noise read outside a body"), "{:?}", in_function.check());
}
