//! `Transpiler::new` on an `Ir` that fails `check()` (feature `ir-rust`, RS-01 AC 5, check C5):
//! no transpiler and so no source, and the error holds the check's entries, each naming an item
//! and its origin. And a checked form with no lowering yet, refused by name with its path and
//! origin instead of failing later in the compiler (RS-01 AC 4; check C110; verification F-B6):
//! a `limit` or `ac_stim` function reading a carrier.

use std::sync::Arc;

use piperine_ir::{
    Addend, BodyId, BodySpec, Branch, BranchKind, CarrierRole, Expr, IrBuilder, Local,
    NodeRole, Operator, Origin, QuantityRole, Site, Stmt, Terminal, ValueType,
};
use piperine_transpile::{Error, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "support/alone.rs"]
mod alone;

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

/// The source of the checked design holding the body `make` builds: the transpiler's refusal,
/// as `(form, path, origin)`.
fn refusal(make: impl FnOnce(&mut IrBuilder) -> BodyId) -> (String, String, Origin) {
    let (ir, _) = Alone::design(make);
    let transpiler = Transpiler::new(Arc::new(ir)).unwrap_or_else(|e| panic!("the design passes check(): {e}"));
    match transpiler.source() {
        Ok(_) => panic!("a form with no lowering gave a source"),
        Err(Error::Unsupported { form, path, origin }) => (form, path, origin),
        Err(other) => panic!("expected the form refused by name, got {other}"),
    }
}

const F: &str = "sampled.msl";

fn at(line: u32, col: u32) -> Origin {
    Buck::origin(F, line, col)
}

/// An `analog` body over one port `a` with `en: Bit` as an `in` carrier:
///
/// ```text
/// module Sampled(a: Electrical, in en: Bit) {}            // sampled.msl:1
/// analog for Sampled {
///     let x = <the site, over en.high()>;                // sampled.msl:3
///     i(a) +> x;                                          // sampled.msl:4
/// }
/// ```
///
/// `site` builds the `limit` or `ac_stim` expression from `en.high()` read as a real (the
/// analog read of a carrier §3.3.5 allows), with `limits`/`stims` naming its site.
fn sampled(b: &mut IrBuilder, limits: Vec<Site>, stims: Vec<Site>, site: fn(&mut IrBuilder, piperine_ir::ExprId) -> piperine_ir::ExprId) -> BodyId {
    let el = Buck::electrical(b);
    let mut x = Dx::new(b, F, 3, 20);
    let en = x.carrier(0, ValueType::Bool);
    let high = x.high(en);
    let one = x.real(1.0);
    let zero = x.real(0.0);
    let level = x.if_(high, one, zero, ValueType::F64);
    let init = site(b, level);
    let let_x = b.stmt(Stmt::Let { local: 0, init });
    let value = b.expr(Expr::Local(0), ValueType::F64, at(4, 13));
    let contribute = b.stmt(Stmt::Contribute {
        branch: 0,
        quantity: QuantityRole::Flow,
        addends: vec![Addend { operator: Operator::Direct, negated: false, value }],
    });
    let body = b.body(BodySpec {
        nodes: vec![piperine_ir::BodyNode { name: "a".into(), nature: el, role: NodeRole::Port, init: None, origin: at(1, 16) }],
        branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind: BranchKind::Flow }],
        limits,
        stims,
        locals: vec![
            Local { name: "x".into(), ty: ValueType::F64 },
            Local { name: "new".into(), ty: ValueType::F64 },
            Local { name: "old".into(), ty: ValueType::F64 },
        ],
        analog: Some(vec![let_x, contribute]),
        ..Buck::spec("Sampled", F, 1)
    });
    b.carrier(body, Buck::carrier("en", ValueType::Bool, CarrierRole::In, None, F, 1, 33));
    body
}

#[test]
fn a_limit_or_stim_function_reading_a_carrier_is_refused_by_name() {
    // `let x = limit(<en.high() as a real>, |new, old| new);` at sampled.msl:3:13: the site's
    // function reads the carrier `en`, which no lowering decodes there (verification F-B6)
    let (form, path, origin) = refusal(|b| {
        let limit = |b: &mut IrBuilder, level| {
            let new = b.expr(Expr::Local(1), ValueType::F64, at(3, 40));
            let f = b.expr(
                Expr::Closure { params: vec![1, 2], body: new },
                ValueType::Fn(vec![ValueType::F64; 2], Box::new(ValueType::F64)),
                at(3, 30),
            );
            b.expr(Expr::Limit { site: 0, x: level, f, seed: None }, ValueType::F64, at(3, 13))
        };
        sampled(b, vec![Site { name: "lim".into(), origin: at(3, 13) }], vec![], limit)
    });
    assert_eq!(form, "a `limit` function reading the carrier `en`");
    assert_eq!(path, "body Sampled limit lim");
    assert_eq!((origin.file.as_str(), origin.line, origin.col), (F, 3, 13));

    // `let x = ac_stim(<en.high() as a real>);` at sampled.msl:3:13: the same for a stimulus
    let (form, path, origin) = refusal(|b| {
        let stim = |b: &mut IrBuilder, level| b.expr(Expr::Stim { site: 0, mag: level, phase: None }, ValueType::F64, at(3, 13));
        sampled(b, vec![], vec![Site { name: "ac".into(), origin: at(3, 13) }], stim)
    });
    assert_eq!(form, "a `ac_stim` function reading the carrier `en`");
    assert_eq!(path, "body Sampled ac_stim ac");
    assert_eq!((origin.file.as_str(), origin.line, origin.col), (F, 3, 13));
}
