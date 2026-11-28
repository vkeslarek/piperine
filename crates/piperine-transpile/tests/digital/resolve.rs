//! Resolved nets printed and run (resolved-nets RN-03, part II §4 rule 8, abi.md §9 D1): a resolver body reads each
//! drive net through a pin that holds the driven type and a plain `driving` byte, and calls the instance of the
//! nature's `resolve` for the count of present drives; a body that drives a resolved carrier writes the value and sets
//! `driving` to 1. The bodies are hand-built and compiled with `rustc`; each expected value is written from the rule
//! cited at its assertion.
#![cfg(unix)]

use piperine_ir::{BinaryOp, BodyId, CarrierRole, Drive, Expr, FunctionSpec, IrBuilder, Local, ValueType};

use piperine_ir_fixtures::digital;
use piperine_ir_fixtures::alone;

use alone::Alone;
use piperine_parity::compiled::{FedBytes, CompiledDevice};
use digital::{Buck, Dx, ValueAliases};

const F: &str = "sum.msl";

/// The bodies of the `resolve` function and its instances.
struct ResolveBodies;

impl ResolveBodies {
    /// `resolve::<k>` of the nature, `k` in `0..=2`.
    fn instances(b: &mut IrBuilder) -> Vec<piperine_ir::FunctionId> {
        (0..=2u64)
            .map(|count| {
                let mut x = Dx::new(b, F, 3, 5);
                let mut total = x.real(1000.0 * count as f64);
                for at in 0..count {
                    let drives = x.local(0, ValueType::Array(Box::new(ValueAliases::u(8)), count));
                    let index = x.int(at as i128, ValueAliases::u(64));
                    let element = x.b.expr(Expr::Index { expr: drives, index }, ValueAliases::u(8), x.origin());
                    let widened = x.cast(element, ValueType::F64, ValueType::F64);
                    total = x.bin(BinaryOp::Add, total, widened, ValueType::F64);
                }
                b.function(FunctionSpec {
                    name: format!("resolve~{count}"),
                    origin: Buck::origin(F, 3, 5),
                    params: vec![Local { name: "drives".into(), ty: ValueType::Array(Box::new(ValueAliases::u(8)), count), mutable: false }],
                    ret: ValueType::F64,
                    // the locals of a function begin with its parameters
                    locals: vec![Local { name: "drives".into(), ty: ValueType::Array(Box::new(ValueAliases::u(8)), count), mutable: false }],
                    body: total,
                })
            })
            .collect()
    }

    /// The resolver of two drives: `v = resolve(d0, d1)`.
    fn resolver(b: &mut IrBuilder) -> BodyId {
        let by_count = ResolveBodies::instances(b);
        let body = b.body(Buck::spec("Sum$resolve~2", F, 4));
        for name in ["d0", "d1"] {
            b.carrier(body, Buck::carrier(name, ValueAliases::u(8), CarrierRole::In, None, F, 4, 5)).expect("a body this builder handed out");
        }
        b.carrier(body, Buck::carrier("v", ValueType::F64, CarrierRole::Out, None, F, 4, 5)).expect("a body this builder handed out");
        let resolve = b.expr(Expr::Resolve { by_count, drives: vec![0, 1], except: None }, ValueType::F64, Buck::origin(F, 4, 9));
        b.drive(body, Drive { carrier: 2, value: resolve, after: None, origin: Buck::origin(F, 4, 9), element: None, path: vec![] }).expect("a body this builder handed out");
        body
    }

    /// The holder: `w` is resolved (value `f64`, driven `u8`), driven with 5; `seen` follows `w.others()`.
    fn source(b: &mut IrBuilder) -> BodyId {
        let body = b.body(Buck::spec("Source", F, 6));
        b.carrier(body, Buck::carrier("w", ValueType::F64, CarrierRole::Resolved { driven: ValueAliases::u(8), port: true }, None, F, 7, 5)).expect("a body this builder handed out");
        b.carrier(body, Buck::carrier("seen", ValueType::F64, CarrierRole::Signal, None, F, 8, 5)).expect("a body this builder handed out");
        let mut x = Dx::new(b, F, 7, 9);
        let five = x.int(5, ValueAliases::u(8));
        b.drive(body, Drive { carrier: 0, value: five, after: None, origin: Buck::origin(F, 7, 9), element: None, path: vec![] }).expect("a body this builder handed out");
        let others = b.expr(Expr::Others(0), ValueType::F64, Buck::origin(F, 8, 9));
        b.drive(body, Drive { carrier: 1, value: others, after: None, origin: Buck::origin(F, 8, 9), element: None, path: vec![] }).expect("a body this builder handed out");
        body
    }
}

/// The bytes of a drive net.
struct DriveBytes;

impl DriveBytes {
    /// A drive net's bytes: the value, then the `driving` byte.
    fn drive(value: u8, driving: u8) -> Vec<u8> {
        vec![value, driving]
    }
}

#[test]
fn resolve_calls_the_instance_of_the_present_count() {
    // abi.md §9 D1: an absent drive (`driving` 0) is left out of what `resolve` receives, whatever its value bytes hold;
    // a present drive of any value, zero included, is passed. `resolve~k` adds `1000 * k` to the sum, so the result names
    // the count the function was called with.
    let device = CompiledDevice::of_body(Alone::design(ResolveBodies::resolver).0, "Sum$resolve~2");
    let run = |d0: Vec<u8>, d1: Vec<u8>| device.run("drive:v", &[&d0, &d1]).expect("the resolver runs")[0];
    assert_eq!(run(DriveBytes::drive(3, 1), DriveBytes::drive(4, 1)), 2007.0, "two present drives: resolve::<2>([3, 4])");
    assert_eq!(run(DriveBytes::drive(3, 1), DriveBytes::drive(9, 0)), 1003.0, "an absent drive of value 9 is left out: resolve::<1>([3])");
    assert_eq!(run(DriveBytes::drive(9, 0), DriveBytes::drive(4, 1)), 1004.0, "the first absent: resolve::<1>([4]), the order of the present ones kept");
    assert_eq!(run(DriveBytes::drive(0, 1), DriveBytes::drive(0, 1)), 2000.0, "a present drive of value 0 is passed: resolve::<2>([0, 0])");
    assert_eq!(run(DriveBytes::drive(7, 0), DriveBytes::drive(7, 0)), 0.0, "no drive present: resolve::<0>([])");
}

#[test]
fn a_drive_write_marks_the_drive_present() {
    // abi.md §9 D1: a driver writes `driving` 1 with every value it writes; the drive pin holds the driven type's bytes,
    // then the byte. `w.others()` reads the others pin, which holds `V`.
    let device = CompiledDevice::of_body(Alone::design(ResolveBodies::source).0, "Source");
    let (cells, _) = device.run_cells("drive:w", &[]).expect("the drive runs");
    assert_eq!(cells, vec![vec![5, 1]], "the value 5 and `driving` 1");
    let seen = device.run("drive:seen", &[&FedBytes::real(2.5)]).expect("the follower runs");
    assert_eq!(seen, vec![2.5], "`seen` is what the others pin holds");
}
