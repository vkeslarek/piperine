//! The five M1 device bodies of `devices.rs` in one checked `Ir`, behind an `Interpreter`, and
//! small readers of a shape: the module of a body by name, its pins, terms, inputs and functions
//! as plain values a test compares.

use std::sync::Arc;

use piperine_abi::caller::{CallError, Handle};
use piperine_abi::raw::Module;
use piperine_interp::Interpreter;
use piperine_ir::{
    Addend, BinaryOp, BodyId, BodyNode, BodySpec, Branch, BranchKind, Ir, IrBuilder, Local, NodeRole, Operator, Origin,
    QuantityRole, Site, Stmt, Terminal, ValueType,
};

use piperine_ir_fixtures::devices::{Devices, Ex};

/// An `Interpreter` over a design that holds every M1 device body.
pub struct Models {
    pub interp: Interpreter,
}

impl Models {
    /// `Resistor`, `Capacitor`, `Vsource`, `Isource`, `Diode`; the top is a resistor alone.
    pub fn all() -> Models {
        Models::of(Devices::design_of_every_device())
    }

    /// An `Interpreter` over `ir`, which must pass `check()`.
    pub fn of(ir: Ir) -> Models {
        let interp = Interpreter::new(Arc::new(ir)).unwrap_or_else(|r| panic!("the fixture fails check(): {:?}", r.entries));
        Models { interp }
    }

    pub fn body(&self, name: &str) -> BodyId {
        let ir = self.interp.ir();
        ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body `{name}`")).id()
    }

    pub fn module(&self, name: &str) -> &'static Module {
        self.interp.module(self.body(name)).unwrap_or_else(|| panic!("body `{name}` has no module"))
    }

    /// `Hold(p, n)`: `i(p, n) +> limit(x, |new, old| old, seed)`, the closure returning `old`
    /// so `LIMITED` shows which `old` was taken. `x` is `v(p, n)`; with `guarded` it is
    /// `if c == c { v(p, n) } else { v(p, n) }` over a closure `c`, which passes `check()` and
    /// faults when evaluated (two closures do not compare). Locals: new 0, old 1.
    pub fn hold(seed: Option<f64>, guarded: bool) -> Models {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let mut e = Ex::new(&mut b, "hold.msl", 3, 9);
        let v = e.probe(Terminal::Node(0), Terminal::Node(1));
        let x = if guarded {
            e.at(3, 14);
            let old = e.local(1);
            let c = e.closure(vec![0, 1], old);
            let same = e.binary(BinaryOp::Eq, c, c);
            let w = e.probe(Terminal::Node(0), Terminal::Node(1));
            e.if_f64(same, v, w)
        } else {
            v
        };
        e.at(3, 30);
        let old = e.local(1);
        let f = e.closure(vec![0, 1], old);
        let seed = seed.map(|s| e.real(s));
        let limited = e.limit(0, x, f, seed);
        let contribute = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value: limited }],
        });
        let at = |line: u32| Origin { file: "hold.msl".into(), line, col: 1 };
        let port = |name: &str| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: at(1) };
        let local = |name: &str| Local { name: name.into(), ty: ValueType::F64, mutable: false };
        let body = b.body(BodySpec {
            name: "Hold".into(),
            origin: at(1),
            params: vec![],
            nodes: vec![port("p"), port("n")],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![Site { name: "y".into(), origin: at(3), enabled: None }],
            stims: vec![],
            locals: vec![local("new"), local("old")],
            analog: Some(vec![contribute]),
            children: vec![],
        });
        let top = Devices::alone(&mut b, body, "H", 1, vec![]);
        Models::of(b.finish(top))
    }

    /// An instance of `name` at its defaults.
    pub fn instance(&self, name: &str) -> Handle {
        Handle::create(self.module(name), &[]).unwrap_or_else(|e| panic!("create `{name}`: {e}"))
    }
}

/// What one call wrote: each write as an `F64`, and the partials.
#[derive(Debug, Clone, PartialEq)]
pub struct Out {
    pub values: Vec<f64>,
    pub jac: Vec<f64>,
}

/// One read cell's bytes.
pub enum In {
    F64(f64),
    Byte(u8),
}

impl Models {
    /// Runs function `k` of `h` on `input`, every partial seeded unless `seed` is given. Every
    /// write is an `F64` (terms, `PROPOSED`/`LIMITED`, `RE`/`IM`).
    pub fn eval(h: &Handle, k: u32, input: &[In], seed: Option<&[u8]>) -> Result<Out, CallError> {
        let f = h.shape().functions.get(k as usize).unwrap_or_else(|| panic!("no function {k}"));
        let (writes, partials) = (f.writes.len(), f.partials.len());
        let bytes: Vec<Vec<u8>> = input
            .iter()
            .map(|x| match x {
                In::F64(v) => v.to_le_bytes().to_vec(),
                In::Byte(b) => vec![*b],
            })
            .collect();
        let input: Vec<&[u8]> = bytes.iter().map(|b| b.as_slice()).collect();
        let mut out = vec![[0u8; 8]; writes];
        let mut output: Vec<&mut [u8]> = out.iter_mut().map(|b| &mut b[..]).collect();
        let all = vec![1u8; partials];
        let seed = seed.unwrap_or(&all);
        let mut jac = vec![0.0; partials];
        h.eval(k, &input, &mut output, seed, &mut jac)?;
        Ok(Out { values: out.iter().map(|b| f64::from_le_bytes(*b)).collect(), jac })
    }

    /// The bytes of `eval`'s writes and partials, for bit-for-bit comparison.
    pub fn bits(out: &Out) -> (Vec<u64>, Vec<u64>) {
        (out.values.iter().map(|v| v.to_bits()).collect(), out.jac.iter().map(|v| v.to_bits()).collect())
    }
}
