//! `Transpiler::new` on an `Ir` that fails `check()` (feature `ir-rust`, RS-01 AC 5, check C5):
//! no transpiler and so no source, and the error holds the check's entries, each naming an item
//! and its origin. And a checked form with no lowering yet, refused by name with its path and
//! origin instead of failing later in the compiler (RS-01 AC 4; check C110; verification F-B6); the
//! `limit` or `ac_stim` function reading a carrier that was refused here lowers since analog-structure.

use std::sync::Arc;

use piperine_ir::{
    Addend, BodyId, BodySpec, Branch, BranchKind, CarrierRole, Expr, IrBuilder, Local,
    NodeRole, Operator, Origin, QuantityRole, Site, Stmt, Terminal, ValueType,
};
use piperine_transpile::{Error, Transpiler};

use piperine_ir_fixtures::devices;
use piperine_ir_fixtures::digital;
use piperine_ir_fixtures::alone;

use alone::Alone;
use devices::Devices;
use digital::{Buck, Dx};

#[test]
fn an_ir_that_fails_check_gives_no_source() {
    // A resistor instance with no param bound breaks `check()`.
    let mut b = IrBuilder::new();
    let body = Devices::resistor(&mut b);
    let top = Devices::alone(&mut b, body, "R", 1, vec![]);
    let ir = b.finish(top);
    let expected = ir.check();
    assert!(!expected.is_empty(), "the fixture must fail check()");

    match Transpiler::new(Arc::new(ir)) {
        Ok(_) => panic!("a broken IR gives a transpiler, and with it a source"),
        Err(Error::Rejected(entries)) => {
            assert_eq!(entries, expected);
            assert!(entries.iter().all(|e| !e.item.is_empty() && !e.what.is_empty() && !e.origin.file.is_empty()));
        }
        Err(other) => panic!("expected the check's entries, got {other}"),
    }
}

const F: &str = "sampled.msl";

/// Associated test helpers of `reject`, grouped on one fixture type.
struct RejectFixture;

impl RejectFixture {
    fn at(line: u32, col: u32) -> Origin {
        Buck::origin(F, line, col)
    }

    /// An `analog` body over one port `a` with `en: Bit` as an `in` carrier:
    ///
    /// ```text
    /// module Sampled(a: Electrical, in en: Bit) {}            // sampled.msl:1
    /// analog for Sampled {
    ///     let x = <the site, over en>;                // sampled.msl:3
    ///     i(a) +> x;                                          // sampled.msl:4
    /// }
    /// ```
    ///
    /// `site` builds the `limit` or `ac_stim` expression from `en` read as a real (the
    /// analog read of a carrier §3.3.5 allows), with `limits`/`stims` naming its site.
    fn sampled(b: &mut IrBuilder, limits: Vec<Site>, stims: Vec<Site>, site: fn(&mut IrBuilder, piperine_ir::ExprId) -> piperine_ir::ExprId) -> BodyId {
        let el = Buck::electrical(b);
        let mut x = Dx::new(b, F, 3, 20);
        let en = x.carrier(0, ValueType::Bool);
        let one = x.real(1.0);
        let zero = x.real(0.0);
        let level = x.if_(en, one, zero, ValueType::F64);
        let init = site(b, level);
        let let_x = b.stmt(Stmt::Let { local: 0, init });
        let value = b.expr(Expr::Local(0), ValueType::F64, RejectFixture::at(4, 13));
        let contribute = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value }],
        });
        let body = b.body(BodySpec {
            nodes: vec![piperine_ir::BodyNode { name: "a".into(), nature: el, role: NodeRole::Port, init: None, origin: RejectFixture::at(1, 16) }],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind: BranchKind::Flow }],
            limits,
            stims,
            locals: vec![
                Local { name: "x".into(), ty: ValueType::F64, mutable: false },
                Local { name: "new".into(), ty: ValueType::F64, mutable: false },
                Local { name: "old".into(), ty: ValueType::F64, mutable: false },
            ],
            analog: Some(vec![let_x, contribute]),
            ..Buck::spec("Sampled", F, 1)
        });
        b.carrier(body, Buck::carrier("en", ValueType::Bool, CarrierRole::In, None, F, 1, 33));
        body
    }
}

#[test]
fn a_limit_or_stim_function_reading_a_carrier_lowers() {
    // analog-structure T21 (AS-06 AC 2, AC 5) lifted the refusal this test asserted (verification
    // F-B6): a `limit` or `ac_stim` function that reads a carrier (`en` read as a real, the
    // analog read §3.3.5 allows) is an ordinary read of the site's function, decoded as a `terms`
    // function decodes it (abi.md F2: an analog function may read discrete pins). The source is
    // generated and the function names the carrier.
    let source = |make: fn(&mut IrBuilder) -> BodyId| {
        let (ir, _) = Alone::design(make);
        Transpiler::new(Arc::new(ir)).unwrap_or_else(|e| panic!("the design passes check(): {e}")).source().unwrap_or_else(|e| panic!("a function reading a carrier lowers: {e}"))
    };
    let limit = source(|b| {
        let limit = |b: &mut IrBuilder, level| {
            let new = b.expr(Expr::Local(1), ValueType::F64, RejectFixture::at(3, 40));
            let f = b.expr(Expr::Closure { params: vec![1, 2], body: new }, ValueType::Fn(vec![ValueType::F64; 2], Box::new(ValueType::F64)), RejectFixture::at(3, 30));
            b.expr(Expr::Limit { site: 0, x: level, f, seed: None }, ValueType::F64, RejectFixture::at(3, 13))
        };
        RejectFixture::sampled(b, vec![Site { name: "lim".into(), origin: RejectFixture::at(3, 13), enabled: None }], vec![], limit)
    });
    assert!(limit.contains("lim_limit_block") && limit.contains("en"), "{limit}");
    let stim = source(|b| {
        let stim = |b: &mut IrBuilder, level| b.expr(Expr::Stim { site: 0, mag: level, phase: None }, ValueType::F64, RejectFixture::at(3, 13));
        RejectFixture::sampled(b, vec![], vec![Site { name: "ac".into(), origin: RejectFixture::at(3, 13), enabled: None }], stim)
    });
    assert!(stim.contains("ac_stim_block") && stim.contains("en"), "{stim}");
}
