//! The digital and mixed bodies of the buck slice (ir-rust phase B), built by hand through
//! `IrBuilder` from the MSL text each one transcribes, and the expression helper they share.
//! Every expression is given the type the MSL source gives it, written out at each node: the
//! fixture states the types, `Ir::check` judges them. Shared by the tests of `piperine-plan` and
//! `piperine-transpile` through `#[path]`.

use piperine_ir::{
    Addend, BinaryOp, BodyId, BodyNode, BodySpec, Branch, BranchKind, Carrier, CarrierRole, Constant, Drive, Event,
    EventSource, Expr, ExprId, FunctionId, IrBuilder, Method, NatureId, NodeRole, Operator, Origin, Param, Quantity,
    QuantityRole, Reaction, Real, Stmt, StmtId, Terminal, UnaryOp, ValueType,
};

/// The value types of the MSL source, by their aliases.
pub struct Ty;

impl Ty {
    pub fn u(n: u32) -> ValueType {
        ValueType::UInt(n)
    }

    pub fn i(n: u32) -> ValueType {
        ValueType::Int(n)
    }

    /// A value that may be unknown: the type of a carrier read and what is computed from one.
    pub fn flagged(t: ValueType) -> ValueType {
        ValueType::Flagged(Box::new(t))
    }

    /// `t` flagged when `flagged` holds.
    pub fn maybe(t: ValueType, flagged: bool) -> ValueType {
        if flagged { Ty::flagged(t) } else { t }
    }
}

/// Expression construction at one source position; every node takes its type from the caller.
pub struct Dx<'b> {
    pub b: &'b mut IrBuilder,
    pub file: &'static str,
    pub line: u32,
    pub col: u32,
}

impl<'b> Dx<'b> {
    pub fn new(b: &'b mut IrBuilder, file: &'static str, line: u32, col: u32) -> Self {
        Dx { b, file, line, col }
    }

    pub fn origin(&self) -> Origin {
        Origin { file: self.file.to_string(), line: self.line, col: self.col }
    }

    pub fn at(&mut self, line: u32, col: u32) -> &mut Self {
        self.line = line;
        self.col = col;
        self
    }

    fn emit(&mut self, kind: Expr, ty: ValueType) -> ExprId {
        let o = self.origin();
        self.b.expr(kind, ty, o)
    }

    pub fn int(&mut self, v: i128, ty: ValueType) -> ExprId {
        self.emit(Expr::Int(v), ty)
    }

    pub fn real(&mut self, v: f64) -> ExprId {
        self.emit(Expr::Real(Real::new(v)), ValueType::F64)
    }

    pub fn boolean(&mut self, v: bool) -> ExprId {
        self.emit(Expr::Bool(v), ValueType::Bool)
    }

    pub fn unit(&mut self) -> ExprId {
        self.emit(Expr::Unit, ValueType::Unit)
    }

    pub fn local(&mut self, i: u32, ty: ValueType) -> ExprId {
        self.emit(Expr::Local(i), ty)
    }

    pub fn param(&mut self, i: u32, ty: ValueType) -> ExprId {
        self.emit(Expr::Param(i), ty)
    }

    pub fn bin(&mut self, op: BinaryOp, lhs: ExprId, rhs: ExprId, ty: ValueType) -> ExprId {
        self.emit(Expr::Binary { op, lhs, rhs }, ty)
    }

    pub fn un(&mut self, op: UnaryOp, expr: ExprId, ty: ValueType) -> ExprId {
        self.emit(Expr::Unary { op, expr }, ty)
    }

    pub fn cast(&mut self, expr: ExprId, to: ValueType, ty: ValueType) -> ExprId {
        self.emit(Expr::Cast { expr, to }, ty)
    }

    pub fn method(&mut self, method: Method, receiver: ExprId, args: Vec<ExprId>, ty: ValueType) -> ExprId {
        self.emit(Expr::Method { method, receiver, args }, ty)
    }

    pub fn call(&mut self, function: FunctionId, args: Vec<ExprId>, ty: ValueType) -> ExprId {
        self.emit(Expr::Call { function, args }, ty)
    }

    pub fn if_(&mut self, cond: ExprId, then: ExprId, otherwise: ExprId, ty: ValueType) -> ExprId {
        self.emit(Expr::If { cond, then, otherwise }, ty)
    }

    pub fn block(&mut self, stmts: Vec<StmtId>, value: Option<ExprId>, ty: ValueType) -> ExprId {
        self.emit(Expr::Block { stmts, value }, ty)
    }

    pub fn let_(&mut self, local: u32, init: ExprId) -> StmtId {
        self.b.stmt(Stmt::Let { local, init })
    }

    pub fn stmt(&mut self, e: ExprId) -> StmtId {
        self.b.stmt(Stmt::Expr(e))
    }

    pub fn unknown(&mut self, ty: ValueType) -> ExprId {
        self.emit(Expr::Unknown, ty)
    }

    pub fn known_or(&mut self, value: ExprId, default: ExprId, ty: ValueType) -> ExprId {
        self.emit(Expr::KnownOr { value, default }, ty)
    }

    pub fn is_known(&mut self, value: ExprId) -> ExprId {
        self.emit(Expr::IsKnown(value), ValueType::Bool)
    }

    pub fn high(&mut self, value: ExprId) -> ExprId {
        self.emit(Expr::High(value), ValueType::Bool)
    }

    /// A read of carrier `i`, which holds `ty`: unknown until first written.
    pub fn carrier(&mut self, i: u32, ty: ValueType) -> ExprId {
        self.emit(Expr::Carrier(i), Ty::flagged(ty))
    }

    pub fn probe(&mut self, plus: Terminal, minus: Terminal) -> ExprId {
        self.emit(Expr::Probe { plus, minus }, ValueType::F64)
    }

    /// `carrier <- value;`, or `carrier <- value after delay;`.
    pub fn commit(&mut self, carrier: u32, value: ExprId, after: Option<ExprId>) -> StmtId {
        self.b.stmt(Stmt::Commit { carrier, value, after })
    }

    /// `if cond { stmts }` with no `else`: the `else` is `()`, as `piperine-lang` lowers it.
    pub fn when(&mut self, cond: ExprId, stmts: Vec<StmtId>, otherwise: Option<ExprId>) -> ExprId {
        let then = self.block(stmts, None, ValueType::Unit);
        let otherwise = match otherwise {
            Some(e) => e,
            None => self.unit(),
        };
        self.if_(cond, then, otherwise, ValueType::Unit)
    }
}

/// The buck-slice bodies and the functions they call, each beside the MSL text it transcribes.
pub struct Buck;

impl Buck {
    fn local(name: &str, ty: ValueType) -> piperine_ir::Local {
        piperine_ir::Local { name: name.to_string(), ty }
    }

    pub fn origin(file: &str, line: u32, col: u32) -> Origin {
        Origin { file: file.to_string(), line, col }
    }

    /// An empty body named `name`, declared at `file:line:1`.
    pub fn spec(name: &str, file: &str, line: u32) -> BodySpec {
        BodySpec {
            name: name.into(),
            origin: Buck::origin(file, line, 1),
            params: vec![],
            nodes: vec![],
            branches: vec![],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: None,
            children: vec![],
        }
    }

    /// An `f64` param at `file:line:5`.
    pub fn param(name: &str, file: &str, line: u32, default: Option<f64>, check: Option<ExprId>) -> Param {
        Param {
            name: name.into(),
            ty: ValueType::F64,
            default: default.map(|v| Constant::Real(Real::new(v))),
            check,
            structural: false,
            origin: Buck::origin(file, line, 5),
        }
    }

    /// `where <param index> <op> <bound>` at `file:line:col`.
    pub fn bound(b: &mut IrBuilder, file: &'static str, line: u32, col: u32, index: u32, op: BinaryOp, bound: ExprId) -> ExprId {
        let mut x = Dx::new(b, file, line, col);
        let p = x.param(index, ValueType::F64);
        x.bin(op, p, bound, ValueType::Bool)
    }

    /// `where <param index> > 0.0`.
    pub fn positive(b: &mut IrBuilder, file: &'static str, line: u32, col: u32, index: u32) -> ExprId {
        let zero = Dx::new(b, file, line, col).real(0.0);
        Buck::bound(b, file, line, col, index, BinaryOp::Gt, zero)
    }

    pub fn carrier(name: &str, ty: ValueType, role: CarrierRole, init: Option<Constant>, file: &str, line: u32, col: u32) -> Carrier {
        Carrier { name: name.into(), ty, role, init, origin: Buck::origin(file, line, col) }
    }

    /// The `Electrical` nature of `std/carriers.msl`, added once per builder.
    pub fn electrical(b: &mut IrBuilder) -> NatureId {
        if let Some(id) = b.find_nature("Electrical") {
            return id;
        }
        let voltage = Quantity { name: "Voltage".into(), access: "v".into(), units: "V".into(), abstol: Real::new(1e-6) };
        let current = Quantity { name: "Current".into(), access: "i".into(), units: "A".into(), abstol: Real::new(1e-12) };
        b.nature("Electrical", voltage, Some(current))
    }

    /// `c.rise()` of a `Bit` carrier: `Changed(c)` guarded by `c.high()` (spec, "`rise()`,
    /// `fall()` in the IR").
    pub fn rise(b: &mut IrBuilder, body: BodyId, carrier: u32, file: &'static str, line: u32, col: u32) -> u32 {
        let mut x = Dx::new(b, file, line, col);
        let c = x.carrier(carrier, ValueType::Bool);
        let guard = x.high(c);
        b.event(body, Event { source: EventSource::Changed(carrier), guard: Some(guard), origin: Buck::origin(file, line, col) })
    }

    pub fn reaction(b: &mut IrBuilder, body: BodyId, on: Vec<u32>, stmts: Vec<StmtId>, locals: Vec<piperine_ir::Local>, file: &str, line: u32) -> u32 {
        b.reaction(body, Reaction { on, stmts, locals, origin: Buck::origin(file, line, 5) })
    }

    /// `dpwm.msl:7-10`:
    ///
    /// ```text
    /// pub module Osc(out clk: Bit = false) {
    ///     f: f64 = 200M where f > 0.0;
    /// }
    /// digital for Osc { on clk.changed() { clk <- !clk after 0.5 / f; } }
    /// ```
    pub fn osc(b: &mut IrBuilder) -> BodyId {
        const F: &str = "dpwm.msl";
        let check = Buck::positive(b, F, 8, 22, 0);
        let body = b.body(BodySpec { params: vec![Buck::param("f", F, 8, Some(200e6), Some(check))], ..Buck::spec("Osc", F, 7) });
        b.carrier(body, Buck::carrier("clk", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 7, 20));
        let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Buck::origin(F, 10, 22) });
        let mut x = Dx::new(b, F, 10, 45);
        let clk = x.carrier(0, ValueType::Bool);
        let toggled = x.un(UnaryOp::Not, clk, Ty::flagged(ValueType::Bool));
        x.at(10, 57);
        let half = x.real(0.5);
        let f = x.param(0, ValueType::F64);
        let delay = x.bin(BinaryOp::Div, half, f, ValueType::F64);
        let commit = x.commit(0, toggled, Some(delay));
        Buck::reaction(b, body, vec![changed], vec![commit], vec![], F, 10);
        body
    }

    /// `control.msl:22-32`:
    ///
    /// ```text
    /// pub module RefRamp(in tick: Bit, in run: Bit, in target: Bit<u10>,
    ///                    out reference: Bit<u10> = 0, out at_target: Bit) {
    ///     at_target = reference == target;
    /// }
    /// digital for RefRamp {
    ///     on tick.rise() {
    ///         if !run.known_or(false) { reference <- 0; }
    ///         else if reference < target { reference <- reference + 1; }
    ///         else if reference > target { reference <- reference - 1; }
    ///     }
    /// }
    /// ```
    pub fn ref_ramp(b: &mut IrBuilder) -> BodyId {
        const F: &str = "control.msl";
        let body = b.body(Buck::spec("RefRamp", F, 22));
        let word = Ty::u(10);
        for (name, ty, role, init, line, col) in [
            ("tick", ValueType::Bool, CarrierRole::In, None, 22, 20),
            ("run", ValueType::Bool, CarrierRole::In, None, 22, 34),
            ("target", word.clone(), CarrierRole::In, None, 22, 47),
            ("reference", word.clone(), CarrierRole::Out, Some(Constant::Int(0)), 23, 20),
            ("at_target", ValueType::Bool, CarrierRole::Out, None, 23, 52),
        ] {
            b.carrier(body, Buck::carrier(name, ty, role, init, F, line, col));
        }
        let (tick, run, target, reference, at_target) = (0, 1, 2, 3, 4);
        let mut x = Dx::new(b, F, 24, 17);
        let r = x.carrier(reference, word.clone());
        let t = x.carrier(target, word.clone());
        let same = x.bin(BinaryOp::Eq, r, t, Ty::flagged(ValueType::Bool));
        b.drive(body, Drive { carrier: at_target, value: same, after: None, origin: Buck::origin(F, 24, 5) });
        let rise = Buck::rise(b, body, tick, F, 27, 8);
        let mut x = Dx::new(b, F, 30, 14);
        let r = x.carrier(reference, word.clone());
        let t = x.carrier(target, word.clone());
        let above = x.bin(BinaryOp::Gt, r, t, Ty::flagged(ValueType::Bool));
        let r = x.carrier(reference, word.clone());
        let one = x.int(1, word.clone());
        let down = x.bin(BinaryOp::Sub, r, one, Ty::flagged(word.clone()));
        let commit_down = x.commit(reference, down, None);
        let third = x.when(above, vec![commit_down], None);
        x.at(29, 14);
        let r = x.carrier(reference, word.clone());
        let t = x.carrier(target, word.clone());
        let below = x.bin(BinaryOp::Lt, r, t, Ty::flagged(ValueType::Bool));
        let r = x.carrier(reference, word.clone());
        let one = x.int(1, word.clone());
        let up = x.bin(BinaryOp::Add, r, one, Ty::flagged(word.clone()));
        let commit_up = x.commit(reference, up, None);
        let second = x.when(below, vec![commit_up], Some(third));
        x.at(28, 12);
        let run_c = x.carrier(run, ValueType::Bool);
        let no = x.boolean(false);
        let running = x.known_or(run_c, no, ValueType::Bool);
        let stopped = x.un(UnaryOp::Not, running, ValueType::Bool);
        let zero = x.int(0, word.clone());
        let rest = x.commit(reference, zero, None);
        let first = x.when(stopped, vec![rest], Some(second));
        let stmt = x.stmt(first);
        Buck::reaction(b, body, vec![rise], vec![stmt], vec![], F, 27);
        body
    }

    /// `sense.msl:19-34`:
    ///
    /// ```text
    /// pub module Adc(ain: Electrical, in sample: Bit, out code: Bit<u10> = 0, out done: Bit = false) {
    ///     vref: f64 = 1.6 where vref > 0.0;
    ///     tconv: f64 = 500n where tconv > 0.0;
    ///     tstrobe: f64 = 5n where tstrobe > 0.0;
    /// }
    /// digital for Adc {
    ///     on sample.rise() {
    ///         let q = (v(ain) / vref * 1024.0 + 0.5).floor().clamp(0.0, 1023.0) as u10;
    ///         code <- q after tconv;
    ///         done <- true after tconv;
    ///         done <- false after tconv + tstrobe;
    ///     }
    /// }
    /// ```
    pub fn adc(b: &mut IrBuilder) -> BodyId {
        const F: &str = "sense.msl";
        let el = Buck::electrical(b);
        let vref = Buck::positive(b, F, 21, 25, 0);
        let tconv = Buck::positive(b, F, 23, 27, 1);
        let tstrobe = Buck::positive(b, F, 25, 26, 2);
        let body = b.body(BodySpec {
            params: vec![
                Buck::param("vref", F, 21, Some(1.6), Some(vref)),
                Buck::param("tconv", F, 23, Some(500e-9), Some(tconv)),
                Buck::param("tstrobe", F, 25, Some(5e-9), Some(tstrobe)),
            ],
            nodes: vec![BodyNode { name: "ain".into(), nature: el, role: NodeRole::Port, init: None, origin: Buck::origin(F, 19, 18) }],
            ..Buck::spec("Adc", F, 19)
        });
        let word = Ty::u(10);
        b.carrier(body, Buck::carrier("sample", ValueType::Bool, CarrierRole::In, None, F, 19, 33));
        b.carrier(body, Buck::carrier("code", word.clone(), CarrierRole::Out, Some(Constant::Int(0)), F, 19, 52));
        b.carrier(body, Buck::carrier("done", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 19, 76));
        let rise = Buck::rise(b, body, 0, F, 28, 8);
        let mut x = Dx::new(b, F, 29, 17);
        let v = x.probe(Terminal::Node(0), Terminal::Ground);
        let vref = x.param(0, ValueType::F64);
        let ratio = x.bin(BinaryOp::Div, v, vref, ValueType::F64);
        let full = x.real(1024.0);
        let scaled = x.bin(BinaryOp::Mul, ratio, full, ValueType::F64);
        let half = x.real(0.5);
        let rounded = x.bin(BinaryOp::Add, scaled, half, ValueType::F64);
        let floor = x.method(Method::Floor, rounded, vec![], ValueType::F64);
        let lo = x.real(0.0);
        let hi = x.real(1023.0);
        let clamped = x.method(Method::Clamp, floor, vec![lo, hi], ValueType::F64);
        let q = x.cast(clamped, word.clone(), word.clone());
        let let_q = x.let_(0, q);
        x.at(30, 9);
        let q = x.local(0, word.clone());
        let tconv = x.param(1, ValueType::F64);
        let code = x.commit(1, q, Some(tconv));
        x.at(31, 9);
        let on = x.boolean(true);
        let tconv = x.param(1, ValueType::F64);
        let done_on = x.commit(2, on, Some(tconv));
        x.at(32, 9);
        let off = x.boolean(false);
        let tconv = x.param(1, ValueType::F64);
        let tstrobe = x.param(2, ValueType::F64);
        let end = x.bin(BinaryOp::Add, tconv, tstrobe, ValueType::F64);
        let done_off = x.commit(2, off, Some(end));
        Buck::reaction(b, body, vec![rise], vec![let_q, code, done_on, done_off], vec![Buck::local("q", word)], F, 28);
        body
    }

    /// The comparator: `A2D` of `std/convert.msl:9-13`, `above(e)` written as `Cross(e)`.
    ///
    /// ```text
    /// pub module A2D(a: Electrical, out d: Bit = false) { vth: f64 = 0.6; hyst: f64 = 50m; }
    /// digital for A2D {
    ///     on above(v(a) - vth - hyst / 2.0) { d <- true; }
    ///     on above(vth - hyst / 2.0 - v(a)) { d <- false; }
    /// }
    /// ```
    pub fn comparator(b: &mut IrBuilder) -> BodyId {
        const F: &str = "convert.msl";
        let el = Buck::electrical(b);
        let body = b.body(BodySpec {
            params: vec![Buck::param("vth", F, 9, Some(0.6), None), Buck::param("hyst", F, 9, Some(50e-3), None)],
            nodes: vec![BodyNode { name: "a".into(), nature: el, role: NodeRole::Port, init: None, origin: Buck::origin(F, 9, 19) }],
            ..Buck::spec("A2D", F, 9)
        });
        b.carrier(body, Buck::carrier("d", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 9, 34));
        // on above(v(a) - vth - hyst / 2.0) { d <- true; }
        let mut x = Dx::new(b, F, 11, 14);
        let v = x.probe(Terminal::Node(0), Terminal::Ground);
        let vth = x.param(0, ValueType::F64);
        let over = x.bin(BinaryOp::Sub, v, vth, ValueType::F64);
        let hyst = x.param(1, ValueType::F64);
        let two = x.real(2.0);
        let half = x.bin(BinaryOp::Div, hyst, two, ValueType::F64);
        let rising = x.bin(BinaryOp::Sub, over, half, ValueType::F64);
        x.at(11, 41);
        let on = x.boolean(true);
        let set = x.commit(0, on, None);
        // on above(vth - hyst / 2.0 - v(a)) { d <- false; }
        x.at(12, 14);
        let vth = x.param(0, ValueType::F64);
        let hyst = x.param(1, ValueType::F64);
        let two = x.real(2.0);
        let half = x.bin(BinaryOp::Div, hyst, two, ValueType::F64);
        let low = x.bin(BinaryOp::Sub, vth, half, ValueType::F64);
        let v = x.probe(Terminal::Node(0), Terminal::Ground);
        let falling = x.bin(BinaryOp::Sub, low, v, ValueType::F64);
        x.at(12, 41);
        let off = x.boolean(false);
        let clear = x.commit(0, off, None);
        let up = b.event(body, Event { source: EventSource::Cross(rising), guard: None, origin: Buck::origin(F, 11, 8) });
        let down = b.event(body, Event { source: EventSource::Cross(falling), guard: None, origin: Buck::origin(F, 12, 8) });
        Buck::reaction(b, body, vec![up], vec![set], vec![], F, 11);
        Buck::reaction(b, body, vec![down], vec![clear], vec![], F, 12);
        body
    }

    /// `power.msl:74-78`:
    ///
    /// ```text
    /// pub module GateDriver(in hs_cmd: Bit, in ls_cmd: Bit, out hs_gate: Bit, out ls_gate: Bit) {
    ///     tpd: f64 = 10n where tpd > 0.0;
    ///     hs_gate = (hs_cmd & !ls_cmd) after tpd;
    ///     ls_gate = (ls_cmd & !hs_cmd) after tpd;
    /// }
    /// ```
    pub fn gate_driver(b: &mut IrBuilder) -> BodyId {
        const F: &str = "power.msl";
        let check = Buck::positive(b, F, 75, 26, 0);
        let body = b.body(BodySpec { params: vec![Buck::param("tpd", F, 75, Some(10e-9), Some(check))], ..Buck::spec("GateDriver", F, 74) });
        let level = ValueType::Bool;
        for (name, role, col) in [("hs_cmd", CarrierRole::In, 30), ("ls_cmd", CarrierRole::In, 46), ("hs_gate", CarrierRole::Out, 63), ("ls_gate", CarrierRole::Out, 81)] {
            b.carrier(body, Buck::carrier(name, level.clone(), role, None, F, 74, col));
        }
        for (target, own, other, line) in [(2, 0, 1, 76), (3, 1, 0, 77)] {
            let mut x = Dx::new(b, F, line, 15);
            let cmd = x.carrier(own, ValueType::Bool);
            let blocker = x.carrier(other, ValueType::Bool);
            let not = x.un(UnaryOp::Not, blocker, Ty::flagged(ValueType::Bool));
            let value = x.bin(BinaryOp::BitAnd, cmd, not, Ty::flagged(ValueType::Bool));
            x.at(line, 38);
            let after = x.param(0, ValueType::F64);
            b.drive(body, Drive { carrier: target, value, after: Some(after), origin: Buck::origin(F, line, 5) });
        }
        body
    }

    /// A switch between `p` and `n` whose conductance an `analog` block reads from a gate carrier
    /// through `high()` (§3.3.5): `PowerSwitch` of `power.msl:52-70` with the channel's
    /// `transition` (stdlib, out of this slice) replaced by the gate level itself.
    ///
    /// ```text
    /// pub module Switch(p: Electrical, n: Electrical, in g: Bit) {
    ///     ron: f64 = 20m where ron > 0.0;
    ///     roff: f64 = 1M where roff > ron;
    /// }
    /// analog for Switch { i(p, n) +> v(p, n) * if g.high() { 1.0 / ron } else { 1.0 / roff }; }
    /// ```
    pub fn switch(b: &mut IrBuilder) -> BodyId {
        const F: &str = "switch.msl";
        let el = Buck::electrical(b);
        let ron = Buck::positive(b, F, 2, 30, 0);
        let lower = Dx::new(b, F, 3, 32).param(0, ValueType::F64);
        let roff = Buck::bound(b, F, 3, 25, 1, BinaryOp::Gt, lower);
        let mut x = Dx::new(b, F, 5, 21);
        let v = x.probe(Terminal::Node(0), Terminal::Node(1));
        let g = x.carrier(0, ValueType::Bool);
        let on = x.high(g);
        let one = x.real(1.0);
        let r = x.param(0, ValueType::F64);
        let g_on = x.bin(BinaryOp::Div, one, r, ValueType::F64);
        let one = x.real(1.0);
        let r = x.param(1, ValueType::F64);
        let g_off = x.bin(BinaryOp::Div, one, r, ValueType::F64);
        let conductance = x.if_(on, g_on, g_off, ValueType::F64);
        let value = x.bin(BinaryOp::Mul, v, conductance, ValueType::F64);
        let stmt = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value }],
        });
        let port = |name: &str, col: u32| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: Buck::origin(F, 1, col) };
        let body = b.body(BodySpec {
            params: vec![Buck::param("ron", F, 2, Some(20e-3), Some(ron)), Buck::param("roff", F, 3, Some(1e6), Some(roff))],
            nodes: vec![port("p", 19), port("n", 34)],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            analog: Some(vec![stmt]),
            ..Buck::spec("Switch", F, 1)
        });
        b.carrier(body, Buck::carrier("g", ValueType::Bool, CarrierRole::In, None, F, 1, 52));
        body
    }

    /// `dpwm.msl:14`: `fn rev3(k: u3) -> u3 { ((k & 1) << 2) | (k & 2) | ((k >> 2) & 1) }`. A shift
    /// amount literal is `i32`, Rust's fallback for an unconstrained shift operand.
    pub fn rev3(b: &mut IrBuilder) -> FunctionId {
        Buck::rev3_with(b, "rev3", false)
    }

    /// `rev3` lowered for an argument that may be unknown (msl-digital's variant `rev3~1`): every
    /// operator over `k` may be unknown too.
    pub fn rev3_flagged(b: &mut IrBuilder) -> FunctionId {
        Buck::rev3_with(b, "rev3~1", true)
    }

    fn rev3_with(b: &mut IrBuilder, name: &str, flagged: bool) -> FunctionId {
        let k_ty = Ty::maybe(Ty::u(3), flagged);
        let mut x = Dx::new(b, "dpwm.msl", 14, 24);
        let k = x.local(0, k_ty.clone());
        let one = x.int(1, Ty::u(3));
        let k_and_1 = x.bin(BinaryOp::BitAnd, k, one, k_ty.clone());
        let two = x.int(2, Ty::i(32));
        let shifted = x.bin(BinaryOp::Shl, k_and_1, two, k_ty.clone());
        let k = x.local(0, k_ty.clone());
        let two_mask = x.int(2, Ty::u(3));
        let middle = x.bin(BinaryOp::BitAnd, k, two_mask, k_ty.clone());
        let high_low = x.bin(BinaryOp::BitOr, shifted, middle, k_ty.clone());
        let k = x.local(0, k_ty.clone());
        let two = x.int(2, Ty::i(32));
        let down = x.bin(BinaryOp::Shr, k, two, k_ty.clone());
        let one = x.int(1, Ty::u(3));
        let low = x.bin(BinaryOp::BitAnd, down, one, k_ty.clone());
        let body = x.bin(BinaryOp::BitOr, high_low, low, k_ty.clone());
        let origin = Origin { file: "dpwm.msl".into(), line: 14, col: 1 };
        b.function(piperine_ir::FunctionSpec {
            name: name.into(),
            origin,
            params: vec![Buck::local("k", k_ty.clone())],
            ret: k_ty.clone(),
            locals: vec![Buck::local("k", k_ty)],
            body,
        })
    }

    /// `dpwm.msl:18-22`:
    ///
    /// ```text
    /// fn dithered_width(duty: u12, frame: u3) -> u9 {
    ///     let coarse = (duty >> 3) as u9;
    ///     let fine = (duty & 7) as u3;
    ///     if rev3(frame) < fine { coarse + 1 } else { coarse }
    /// }
    /// ```
    pub fn dithered_width(b: &mut IrBuilder, rev3: FunctionId) -> FunctionId {
        Buck::dithered_width_with(b, "dithered_width", rev3, false)
    }

    /// `dithered_width` lowered for a frame index that may be unknown (msl-digital's variant
    /// `dithered_width~01`, reached by `dithered_width(duty.known_or(0), frame + 1)`), calling
    /// `rev3~1`: the comparison, and so the width, may be unknown.
    pub fn dithered_width_flagged(b: &mut IrBuilder, rev3_flagged: FunctionId) -> FunctionId {
        Buck::dithered_width_with(b, "dithered_width~01", rev3_flagged, true)
    }

    fn dithered_width_with(b: &mut IrBuilder, name: &str, rev3: FunctionId, flagged: bool) -> FunctionId {
        let frame_ty = Ty::maybe(Ty::u(3), flagged);
        let width_ty = Ty::maybe(Ty::u(9), flagged);
        let mut x = Dx::new(b, "dpwm.msl", 19, 18);
        let duty = x.local(0, Ty::u(12));
        let three = x.int(3, Ty::i(32));
        let coarse = x.bin(BinaryOp::Shr, duty, three, Ty::u(12));
        let coarse = x.cast(coarse, Ty::u(9), Ty::u(9));
        let let_coarse = x.let_(2, coarse);
        x.at(20, 16);
        let duty = x.local(0, Ty::u(12));
        let seven = x.int(7, Ty::u(12));
        let fine = x.bin(BinaryOp::BitAnd, duty, seven, Ty::u(12));
        let fine = x.cast(fine, Ty::u(3), Ty::u(3));
        let let_fine = x.let_(3, fine);
        x.at(21, 5);
        let frame = x.local(1, frame_ty.clone());
        let rev = x.call(rev3, vec![frame], frame_ty.clone());
        let fine = x.local(3, Ty::u(3));
        let cond = x.bin(BinaryOp::Lt, rev, fine, Ty::maybe(ValueType::Bool, flagged));
        let coarse = x.local(2, Ty::u(9));
        let one = x.int(1, Ty::u(9));
        let wider = x.bin(BinaryOp::Add, coarse, one, Ty::u(9));
        let coarse = x.local(2, Ty::u(9));
        let width = x.if_(cond, wider, coarse, width_ty.clone());
        x.at(18, 52);
        let body = x.block(vec![let_coarse, let_fine], Some(width), width_ty.clone());
        let origin = Origin { file: "dpwm.msl".into(), line: 18, col: 1 };
        b.function(piperine_ir::FunctionSpec {
            name: name.into(),
            origin,
            params: vec![Buck::local("duty", Ty::u(12)), Buck::local("frame", frame_ty.clone())],
            ret: width_ty,
            locals: vec![
                Buck::local("duty", Ty::u(12)),
                Buck::local("frame", frame_ty),
                Buck::local("coarse", Ty::u(9)),
                Buck::local("fine", Ty::u(3)),
            ],
            body,
        })
    }
}
