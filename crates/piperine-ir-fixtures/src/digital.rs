//! The digital and mixed bodies of the buck slice (ir-rust phase B), built by hand through
//! `IrBuilder` from the MSL text each one transcribes, and the expression helper they share.
//! Every expression is given the type the MSL source gives it, written out at each node: the
//! fixture states the types, `Ir::check` judges them.

use piperine_ir::{
    Addend, BinaryOp, BodyId, BodyNode, BodySpec, Branch, BranchKind, Carrier, CarrierRole, Constant, Drive, Event,
    EventSource, Expr, ExprId, FunctionId, IrBuilder, LevelPart, MathFn, Method, NatureId, NodeRole, Operator, Origin, Param, Quantity,
    QuantityRole, Reaction, Real, Stmt, StmtId, Terminal, UnaryOp, ValueType,
};

/// The value types of the MSL source, by their aliases.
pub struct ValueAliases;

impl ValueAliases {
    /// The unsigned integer type of `n` bits.
    pub fn u(n: u32) -> ValueType {
        ValueType::UInt(n)
    }

    /// The signed integer type of `n` bits.
    pub fn i(n: u32) -> ValueType {
        ValueType::Int(n)
    }
}

/// Expression construction at one source position; every node takes its type from the caller.
pub struct Dx<'b> {
    /// The builder every node is made through.
    pub b: &'b mut IrBuilder,
    /// The source file every node is attributed to.
    pub file: &'static str,
    /// The source line the next node is attributed to.
    pub line: u32,
    /// The source column the next node is attributed to.
    pub col: u32,
}

impl<'b> Dx<'b> {
    /// A builder positioned at `file:line:col`.
    pub fn new(b: &'b mut IrBuilder, file: &'static str, line: u32, col: u32) -> Self {
        Dx { b, file, line, col }
    }

    /// The origin of the current position.
    pub fn origin(&self) -> Origin {
        Origin { file: self.file.to_string(), line: self.line, col: self.col }
    }

    /// Moves the position the next nodes are attributed to.
    pub fn at(&mut self, line: u32, col: u32) -> &mut Self {
        self.line = line;
        self.col = col;
        self
    }

    fn emit(&mut self, kind: Expr, ty: ValueType) -> ExprId {
        let o = self.origin();
        self.b.expr(kind, ty, o)
    }

    /// An integer literal of type `ty`.
    pub fn int(&mut self, v: i128, ty: ValueType) -> ExprId {
        self.emit(Expr::Int(v), ty)
    }

    /// A real literal.
    pub fn real(&mut self, v: f64) -> ExprId {
        self.emit(Expr::Real(Real::new(v)), ValueType::F64)
    }

    /// A boolean literal.
    pub fn boolean(&mut self, v: bool) -> ExprId {
        self.emit(Expr::Bool(v), ValueType::Bool)
    }

    /// The unit value.
    pub fn unit(&mut self) -> ExprId {
        self.emit(Expr::Unit, ValueType::Unit)
    }

    /// The read of local `i`, of type `ty`.
    pub fn local(&mut self, i: u32, ty: ValueType) -> ExprId {
        self.emit(Expr::Local(i), ty)
    }

    /// The read of parameter `i`, of type `ty`.
    pub fn param(&mut self, i: u32, ty: ValueType) -> ExprId {
        self.emit(Expr::Param(i), ty)
    }

    /// A binary operation of type `ty`.
    pub fn bin(&mut self, op: BinaryOp, lhs: ExprId, rhs: ExprId, ty: ValueType) -> ExprId {
        self.emit(Expr::Binary { op, lhs, rhs }, ty)
    }

    /// A unary operation of type `ty`.
    pub fn un(&mut self, op: UnaryOp, expr: ExprId, ty: ValueType) -> ExprId {
        self.emit(Expr::Unary { op, expr }, ty)
    }

    /// A cast of `expr` to `to`, of type `ty`.
    pub fn cast(&mut self, expr: ExprId, to: ValueType, ty: ValueType) -> ExprId {
        self.emit(Expr::Cast { expr, to }, ty)
    }

    /// A method call on `receiver`, of type `ty`.
    pub fn method(&mut self, method: Method, receiver: ExprId, args: Vec<ExprId>, ty: ValueType) -> ExprId {
        self.emit(Expr::Method { method, receiver, args }, ty)
    }

    /// A call of `function`, of type `ty`.
    pub fn call(&mut self, function: FunctionId, args: Vec<ExprId>, ty: ValueType) -> ExprId {
        self.emit(Expr::Call { function, args }, ty)
    }

    /// A conditional of type `ty`.
    pub fn if_(&mut self, cond: ExprId, then: ExprId, otherwise: ExprId, ty: ValueType) -> ExprId {
        self.emit(Expr::If { cond, then, otherwise }, ty)
    }

    /// A block of statements with an optional value of type `ty`.
    pub fn block(&mut self, stmts: Vec<StmtId>, value: Option<ExprId>, ty: ValueType) -> ExprId {
        self.emit(Expr::Block { stmts, value }, ty)
    }

    /// The statement binding local `local` to `init`.
    pub fn let_(&mut self, local: u32, init: ExprId) -> StmtId {
        self.b.stmt(Stmt::Let { local, init })
    }

    /// The statement evaluating `e` for its effect.
    pub fn stmt(&mut self, e: ExprId) -> StmtId {
        self.b.stmt(Stmt::Expr(e))
    }

    /// `same(a, b)`.
    pub fn same(&mut self, a: ExprId, b: ExprId) -> ExprId {
        self.emit(Expr::Same { a, b }, ValueType::Bool)
    }

    /// `cast::<_, to>(expr)`, with the tags `to` decodes when it is an enum; `ty` is the result, an `Option` of `to`.
    pub fn level_cast(&mut self, expr: ExprId, to: ValueType, tags: Option<Vec<i128>>, ty: ValueType) -> ExprId {
        self.emit(Expr::LevelCast { expr, to, tags }, ty)
    }

    /// The payload of the `Option` `option` (`ty`, its payload type).
    pub fn option_value(&mut self, option: ExprId, ty: ValueType) -> ExprId {
        self.emit(Expr::OptionPart { option, part: LevelPart::Value }, ty)
    }

    /// A math function of one argument.
    pub fn math(&mut self, f: MathFn, arg: ExprId, ty: ValueType) -> ExprId {
        self.emit(Expr::Math { f, args: vec![arg] }, ty)
    }

    /// A read of carrier `i`, which holds `ty`.
    pub fn carrier(&mut self, i: u32, ty: ValueType) -> ExprId {
        self.emit(Expr::Carrier(i), ty)
    }

    /// The probe of the branch from `plus` to `minus`.
    pub fn probe(&mut self, plus: Terminal, minus: Terminal) -> ExprId {
        self.emit(Expr::Probe { plus, minus }, ValueType::F64)
    }

    /// `carrier <- value;`, or `carrier <- value after delay;`.
    pub fn commit(&mut self, carrier: u32, value: ExprId, after: Option<ExprId>) -> StmtId {
        self.b.stmt(Stmt::Commit { carrier, value, after, target: None })
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
        piperine_ir::Local { name: name.to_string(), ty, mutable: false }
    }

    /// The origin `file:line:col`.
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

    /// A carrier declaration of the given type, role and initial value, at `file:line:col`.
    pub fn carrier(name: &str, ty: ValueType, role: CarrierRole, init: Option<Constant>, file: &str, line: u32, col: u32) -> Carrier {
        Carrier { name: name.into(), ty, role, init, origin: Buck::origin(file, line, col), count: None }
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

    /// `c.rise()` of a `Bit` carrier: `Changed(c)` guarded by `c` (spec, "`rise()`, `fall()` in the IR").
    pub fn rise(b: &mut IrBuilder, body: BodyId, carrier: u32, file: &'static str, line: u32, col: u32) -> u32 {
        let mut x = Dx::new(b, file, line, col);
        let c = x.carrier(carrier, ValueType::Bool);
        let guard = c;
        b.event(body, Event { source: EventSource::Changed(carrier), guard: Some(guard), origin: Buck::origin(file, line, col) })
    }

    /// Adds a reaction on the carriers `on` to `body`, returning its index.
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
        let toggled = x.un(UnaryOp::Not, clk, ValueType::Bool);
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
    ///         if !run { reference <- 0; }
    ///         else if reference < target { reference <- reference + 1; }
    ///         else if reference > target { reference <- reference - 1; }
    ///     }
    /// }
    /// ```
    pub fn ref_ramp(b: &mut IrBuilder) -> BodyId {
        const F: &str = "control.msl";
        let body = b.body(Buck::spec("RefRamp", F, 22));
        let word = ValueAliases::u(10);
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
        let same = x.bin(BinaryOp::Eq, r, t, ValueType::Bool);
        b.drive(body, Drive { carrier: at_target, value: same, after: None, origin: Buck::origin(F, 24, 5), element: None, path: vec![] });
        let rise = Buck::rise(b, body, tick, F, 27, 8);
        let mut x = Dx::new(b, F, 30, 14);
        let r = x.carrier(reference, word.clone());
        let t = x.carrier(target, word.clone());
        let above = x.bin(BinaryOp::Gt, r, t, ValueType::Bool);
        let r = x.carrier(reference, word.clone());
        let one = x.int(1, word.clone());
        let down = x.bin(BinaryOp::Sub, r, one, word.clone());
        let commit_down = x.commit(reference, down, None);
        let third = x.when(above, vec![commit_down], None);
        x.at(29, 14);
        let r = x.carrier(reference, word.clone());
        let t = x.carrier(target, word.clone());
        let below = x.bin(BinaryOp::Lt, r, t, ValueType::Bool);
        let r = x.carrier(reference, word.clone());
        let one = x.int(1, word.clone());
        let up = x.bin(BinaryOp::Add, r, one, word.clone());
        let commit_up = x.commit(reference, up, None);
        let second = x.when(below, vec![commit_up], Some(third));
        x.at(28, 12);
        let running = x.carrier(run, ValueType::Bool);
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
        let word = ValueAliases::u(10);
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
        let up = b.event(body, Event { source: EventSource::Cross { e: rising, ttol: None, etol: None }, guard: None, origin: Buck::origin(F, 11, 8) });
        let down = b.event(body, Event { source: EventSource::Cross { e: falling, ttol: None, etol: None }, guard: None, origin: Buck::origin(F, 12, 8) });
        Buck::reaction(b, body, vec![up], vec![set], vec![], F, 11);
        Buck::reaction(b, body, vec![down], vec![clear], vec![], F, 12);
        body
    }

    /// The D flip-flop of abi.md §11.4, `clk.rise()` as `changed` guarded by `high()`:
    ///
    /// ```text
    /// pub module Dff(in clk: Bit, in d: Bit, out q: Bit = false) {}
    /// digital for Dff { on clk.rise() { q <- d; } }
    /// ```
    pub fn dff(b: &mut IrBuilder) -> BodyId {
        const F: &str = "dff.msl";
        let body = b.body(Buck::spec("Dff", F, 1));
        b.carrier(body, Buck::carrier("clk", ValueType::Bool, CarrierRole::In, None, F, 1, 19));
        b.carrier(body, Buck::carrier("d", ValueType::Bool, CarrierRole::In, None, F, 1, 32));
        b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 44));
        let rise = Buck::rise(b, body, 0, F, 2, 22);
        let mut x = Dx::new(b, F, 2, 37);
        let d = x.carrier(1, ValueType::Bool);
        let commit = x.commit(2, d, None);
        Buck::reaction(b, body, vec![rise], vec![commit], vec![], F, 2);
        body
    }

    /// `control.msl:9-14`, `SETPOINT_RESET` 100 and `SETPOINT_MAX` 168 folded (`spec.msl:22-24`):
    ///
    /// ```text
    /// pub module SetpointReg(in wr: Bit, in data: Bit<u8>, out code: Bit<u8> = SETPOINT_RESET) {}
    /// digital for SetpointReg {
    ///     on wr.rise() {
    ///         code <- data.min(SETPOINT_MAX);
    ///     }
    /// }
    /// ```
    pub fn setpoint_reg(b: &mut IrBuilder) -> BodyId {
        const F: &str = "control.msl";
        let u8_ = ValueAliases::u(8);
        let body = b.body(Buck::spec("SetpointReg", F, 9));
        b.carrier(body, Buck::carrier("wr", ValueType::Bool, CarrierRole::In, None, F, 9, 27));
        b.carrier(body, Buck::carrier("data", u8_.clone(), CarrierRole::In, None, F, 9, 39));
        b.carrier(body, Buck::carrier("code", u8_.clone(), CarrierRole::Out, Some(Constant::Int(100)), F, 9, 60));
        let rise = Buck::rise(b, body, 0, F, 11, 8);
        let mut x = Dx::new(b, F, 12, 12);
        let data = x.carrier(1, u8_.clone());
        let max = x.int(168, u8_.clone());
        let capped = x.method(Method::Min, data, vec![max], u8_.clone());
        let commit = x.commit(2, capped, None);
        Buck::reaction(b, body, vec![rise], vec![commit], vec![], F, 11);
        body
    }

    /// `dpwm.msl:40-80`, with the functions it calls (`rev3`, `dithered_width`). `&&` and `||` over
    /// carriers are `&` and `|` (the lowering of a read of a carrier); `N_PERIOD - 1`, `SAMPLE_COUNT`,
    /// `DEAD_COUNTS` and `N_PERIOD - DEAD_COUNTS` are folded to 399, 0, 4 and 396 (`spec.msl`).
    /// The `Never` monitor of line 53 is a stdlib module with an `emit!`, out of this slice.
    ///
    /// ```text
    /// pub module Dpwm(in clk: Bit, in run: Bit, in duty: Bit<u12>, in oc: Bit, in sink_oc: Bit,
    ///                 out hs: Bit = false, out ls: Bit = false, out sample: Bit, out cut: Bit = false) {
    ///     signal count: Bit<u9> = 0;  signal width: Bit<u9> = 0;  signal frame: Bit<u3> = 0;
    ///     signal hs_off: Bit<u9> = 0;  signal hs_cut: Bit = false;  signal ls_cut: Bit = false;
    ///     sample = count == SAMPLE_COUNT;
    /// }
    /// digital for Dpwm {
    ///     on clk.rise() {
    ///         let wrap = count == N_PERIOD - 1;
    ///         let next = if wrap { 0 } else { count + 1 };
    ///         let enabled = run;
    ///         let w = if wrap { dithered_width(duty, frame + 1) } else { width };
    ///         let hs_trip = !wrap && (hs_cut || (hs && oc));
    ///         let ls_trip = !wrap && (ls_cut || (ls && sink_oc));
    ///         let hs_next = enabled && !hs_trip && next < w;
    ///         let off_at = if wrap { w } else if hs && !hs_next { next } else { hs_off };
    ///         let ls_next = enabled && !hs_next && !ls_trip
    ///             && next >= off_at + DEAD_COUNTS && next < N_PERIOD - DEAD_COUNTS;
    ///         count <- next; width <- w; hs_off <- off_at; hs_cut <- hs_trip; ls_cut <- ls_trip;
    ///         hs <- hs_next; ls <- ls_next;
    ///         if wrap { frame <- frame + 1; cut <- hs_cut; }
    ///     }
    /// }
    /// ```
    pub fn dpwm(b: &mut IrBuilder) -> BodyId {
        const F: &str = "dpwm.msl";
        let rev3 = Buck::rev3(b);
        let dithered = Buck::dithered_width(b, rev3);
        let body = b.body(Buck::spec("Dpwm", F, 40));
        let (bit, u9, u3, u12) = (ValueType::Bool, ValueAliases::u(9), ValueAliases::u(3), ValueAliases::u(12));
        let no = || Some(Constant::Bool(false));
        let zero = || Some(Constant::Int(0));
        for (name, ty, role, init, line, col) in [
            ("clk", bit.clone(), CarrierRole::In, None, 40, 17),
            ("run", bit.clone(), CarrierRole::In, None, 40, 30),
            ("duty", u12.clone(), CarrierRole::In, None, 40, 43),
            ("oc", bit.clone(), CarrierRole::In, None, 40, 61),
            ("sink_oc", bit.clone(), CarrierRole::In, None, 40, 73),
            ("hs", bit.clone(), CarrierRole::Out, no(), 41, 17),
            ("ls", bit.clone(), CarrierRole::Out, no(), 41, 39),
            ("sample", bit.clone(), CarrierRole::Out, None, 41, 61),
            ("cut", bit.clone(), CarrierRole::Out, no(), 41, 78),
            ("count", u9.clone(), CarrierRole::Signal, zero(), 42, 12),
            ("width", u9.clone(), CarrierRole::Signal, zero(), 43, 12),
            ("frame", u3.clone(), CarrierRole::Signal, zero(), 44, 12),
            ("hs_off", u9.clone(), CarrierRole::Signal, zero(), 46, 12),
            ("hs_cut", bit.clone(), CarrierRole::Signal, no(), 48, 12),
            ("ls_cut", bit.clone(), CarrierRole::Signal, no(), 49, 12),
        ] {
            b.carrier(body, Buck::carrier(name, ty, role, init, F, line, col));
        }
        let (clk, run, duty, oc, sink_oc, hs, ls, sample, cut) = (0, 1, 2, 3, 4, 5, 6, 7, 8);
        let (count, width, frame, hs_off, hs_cut, ls_cut) = (9, 10, 11, 12, 13, 14);
        let fb = bit.clone();
        let f9 = u9.clone();

        // sample = count == SAMPLE_COUNT;
        let mut x = Dx::new(b, F, 51, 14);
        let c = x.carrier(count, u9.clone());
        let at_zero = x.int(0, u9.clone());
        let same = x.bin(BinaryOp::Eq, c, at_zero, fb.clone());
        b.drive(body, Drive { carrier: sample, value: same, after: None, origin: Buck::origin(F, 51, 5), element: None, path: vec![] });
        let rise = Buck::rise(b, body, clk, F, 56, 8);

        let locals = [
            ("wrap", fb.clone()),
            ("next", f9.clone()),
            ("enabled", bit.clone()),
            ("w", f9.clone()),
            ("hs_trip", fb.clone()),
            ("ls_trip", fb.clone()),
            ("hs_next", fb.clone()),
            ("off_at", f9.clone()),
            ("ls_next", fb.clone()),
        ];
        let (wrap, next, enabled, w, hs_trip, ls_trip, hs_next, off_at, ls_next) = (0, 1, 2, 3, 4, 5, 6, 7, 8);
        let mut x = Dx::new(b, F, 57, 20);
        let mut stmts = Vec::new();
        // let wrap = count == N_PERIOD - 1;
        let c = x.carrier(count, u9.clone());
        let last = x.int(399, u9.clone());
        let v = x.bin(BinaryOp::Eq, c, last, fb.clone());
        stmts.push(x.let_(wrap, v));
        // let next = if wrap { 0 } else { count + 1 };
        x.at(58, 20);
        let cond = x.local(wrap, fb.clone());
        let restart = x.int(0, u9.clone());
        let c = x.carrier(count, u9.clone());
        let one = x.int(1, u9.clone());
        let step = x.bin(BinaryOp::Add, c, one, f9.clone());
        let v = x.if_(cond, restart, step, f9.clone());
        stmts.push(x.let_(next, v));
        // let enabled = run;
        x.at(59, 23);
        let v = x.carrier(run, bit.clone());
        stmts.push(x.let_(enabled, v));
        // let w = if wrap { dithered_width(duty, frame + 1) } else { width };
        x.at(61, 17);
        let cond = x.local(wrap, fb.clone());
        let word = x.carrier(duty, u12.clone());
        let fr = x.carrier(frame, u3.clone());
        let one = x.int(1, u3.clone());
        let following = x.bin(BinaryOp::Add, fr, one, u3.clone());
        let latched = x.call(dithered, vec![word, following], f9.clone());
        let held = x.carrier(width, u9.clone());
        let v = x.if_(cond, latched, held, f9.clone());
        stmts.push(x.let_(w, v));
        // let hs_trip = !wrap && (hs_cut || (hs && oc));
        // let ls_trip = !wrap && (ls_cut || (ls && sink_oc));
        for (local, cut_flag, side, trip, line) in [(hs_trip, hs_cut, hs, oc, 62), (ls_trip, ls_cut, ls, sink_oc, 63)] {
            x.at(line, 23);
            let wr = x.local(wrap, fb.clone());
            let not_wrap = x.un(UnaryOp::Not, wr, fb.clone());
            let cf = x.carrier(cut_flag, bit.clone());
            let on = x.carrier(side, bit.clone());
            let tripped = x.carrier(trip, bit.clone());
            let on_and_tripped = x.bin(BinaryOp::And, on, tripped, fb.clone());
            let either = x.bin(BinaryOp::Or, cf, on_and_tripped, fb.clone());
            let v = x.bin(BinaryOp::And, not_wrap, either, fb.clone());
            stmts.push(x.let_(local, v));
        }
        // let hs_next = enabled && !hs_trip && next < w;
        x.at(64, 23);
        let en = x.local(enabled, bit.clone());
        let t = x.local(hs_trip, fb.clone());
        let not_t = x.un(UnaryOp::Not, t, fb.clone());
        let a = x.bin(BinaryOp::And, en, not_t, fb.clone());
        let n = x.local(next, f9.clone());
        let wl = x.local(w, f9.clone());
        let inside = x.bin(BinaryOp::Lt, n, wl, fb.clone());
        let v = x.bin(BinaryOp::And, a, inside, fb.clone());
        stmts.push(x.let_(hs_next, v));
        // let off_at = if wrap { w } else if hs && !hs_next { next } else { hs_off };
        x.at(65, 22);
        let cond = x.local(wrap, fb.clone());
        let wl = x.local(w, f9.clone());
        let on = x.carrier(hs, bit.clone());
        let hn = x.local(hs_next, fb.clone());
        let not_hn = x.un(UnaryOp::Not, hn, fb.clone());
        let falls = x.bin(BinaryOp::And, on, not_hn, fb.clone());
        let n = x.local(next, f9.clone());
        let kept = x.carrier(hs_off, u9.clone());
        let inner = x.if_(falls, n, kept, f9.clone());
        let v = x.if_(cond, wl, inner, f9.clone());
        stmts.push(x.let_(off_at, v));
        // let ls_next = enabled && !hs_next && !ls_trip && next >= off_at + DEAD_COUNTS && next < N_PERIOD - DEAD_COUNTS;
        x.at(66, 23);
        let en = x.local(enabled, bit.clone());
        let hn = x.local(hs_next, fb.clone());
        let not_hn = x.un(UnaryOp::Not, hn, fb.clone());
        let a = x.bin(BinaryOp::And, en, not_hn, fb.clone());
        let lt = x.local(ls_trip, fb.clone());
        let not_lt = x.un(UnaryOp::Not, lt, fb.clone());
        let a = x.bin(BinaryOp::And, a, not_lt, fb.clone());
        x.at(67, 16);
        let n = x.local(next, f9.clone());
        let oa = x.local(off_at, f9.clone());
        let dead = x.int(4, u9.clone());
        let start = x.bin(BinaryOp::Add, oa, dead, f9.clone());
        let after_dead = x.bin(BinaryOp::Ge, n, start, fb.clone());
        let a = x.bin(BinaryOp::And, a, after_dead, fb.clone());
        let n = x.local(next, f9.clone());
        let end = x.int(396, u9.clone());
        let before_end = x.bin(BinaryOp::Lt, n, end, fb.clone());
        let v = x.bin(BinaryOp::And, a, before_end, fb.clone());
        stmts.push(x.let_(ls_next, v));
        // count <- next; width <- w; hs_off <- off_at; hs_cut <- hs_trip; ls_cut <- ls_trip; hs <- hs_next; ls <- ls_next;
        for (line, carrier, local, ty) in [
            (68, count, next, f9.clone()),
            (69, width, w, f9.clone()),
            (70, hs_off, off_at, f9.clone()),
            (71, hs_cut, hs_trip, fb.clone()),
            (72, ls_cut, ls_trip, fb.clone()),
            (73, hs, hs_next, fb.clone()),
            (74, ls, ls_next, fb.clone()),
        ] {
            x.at(line, 9);
            let v = x.local(local, ty);
            stmts.push(x.commit(carrier, v, None));
        }
        // if wrap { frame <- frame + 1; cut <- hs_cut; }
        x.at(76, 13);
        let fr = x.carrier(frame, u3.clone());
        let one = x.int(1, u3.clone());
        let advanced = x.bin(BinaryOp::Add, fr, one, u3.clone());
        let advance = x.commit(frame, advanced, None);
        x.at(77, 13);
        let was_cut = x.carrier(hs_cut, bit.clone());
        let report = x.commit(cut, was_cut, None);
        x.at(75, 9);
        let cond = x.local(wrap, fb.clone());
        let at_wrap = x.when(cond, vec![advance, report], None);
        stmts.push(x.stmt(at_wrap));
        let locals = locals.into_iter().map(|(n, t)| Buck::local(n, t)).collect();
        Buck::reaction(b, body, vec![rise], stmts, locals, F, 56);
        body
    }

    /// `control.msl:46-75`, the constants folded (`spec.msl`: `KP_Q4` 64, `KI_Q4` 8, `KD_Q4`
    /// 1024, `D_POLE_SHIFT` 2, `Q_SHIFT` 4, `E_MIN` -256, `E_MAX` 255, `DUTY_MAX` 2880) and `&&`,
    /// `||` over carriers written `&`, `|`:
    ///
    /// ```text
    /// pub module Pid(in tick: Bit, in run: Bit, in reference: Bit<u10>, in meas: Bit<u10>,
    ///                out duty: Bit<u12> = 0) {
    ///     signal integ: Bit<i32> = 0;  signal deriv: Bit<i32> = 0;  signal e_prev: Bit<i16> = 0;
    /// }
    /// digital for Pid {
    ///     on tick.rise() {
    ///         if !run {
    ///             integ <- 0; deriv <- 0; e_prev <- 0; duty <- 0;
    ///         } else {
    ///             let e = ((reference as i16) - (meas as i16)).clamp(E_MIN, E_MAX);
    ///             let p = KP_Q4 * (e as i32);
    ///             let d = (deriv >> D_POLE_SHIFT) + KD_Q4 * ((e - e_prev) as i32);
    ///             let u = (p + integ + d) >> Q_SHIFT;
    ///             let u_max = DUTY_MAX as i32;
    ///             let windup = (u > u_max && e > 0) || (u < 0 && e < 0);
    ///             if !windup {
    ///                 integ <- (integ + KI_Q4 * (e as i32)).clamp(0, u_max << Q_SHIFT);
    ///             }
    ///             deriv <- d;
    ///             e_prev <- e;
    ///             duty <- u.clamp(0, u_max) as u12;
    ///         }
    ///     }
    /// }
    /// ```
    pub fn pid(b: &mut IrBuilder) -> BodyId {
        const F: &str = "control.msl";
        let body = b.body(Buck::spec("Pid", F, 46));
        let (bit, u10, u12, i16_, i32_) = (ValueType::Bool, ValueAliases::u(10), ValueAliases::u(12), ValueAliases::i(16), ValueAliases::i(32));
        let zero = || Some(Constant::Int(0));
        for (name, ty, role, init, line, col) in [
            ("tick", bit.clone(), CarrierRole::In, None, 46, 19),
            ("run", bit.clone(), CarrierRole::In, None, 46, 33),
            ("reference", u10.clone(), CarrierRole::In, None, 46, 46),
            ("meas", u10.clone(), CarrierRole::In, None, 46, 72),
            ("duty", u12.clone(), CarrierRole::Out, zero(), 47, 16),
            ("integ", i32_.clone(), CarrierRole::Signal, zero(), 49, 12),
            ("deriv", i32_.clone(), CarrierRole::Signal, zero(), 51, 12),
            ("e_prev", i16_.clone(), CarrierRole::Signal, zero(), 53, 12),
        ] {
            b.carrier(body, Buck::carrier(name, ty, role, init, F, line, col));
        }
        let (tick, run, reference, meas, duty, integ, deriv, e_prev) = (0, 1, 2, 3, 4, 5, 6, 7);
        let rise = Buck::rise(b, body, tick, F, 56, 8);
        let (fb, f32_) = (bit.clone(), i32_.clone());
        let locals = [("e", i16_.clone()), ("p", i32_.clone()), ("d", f32_.clone()), ("u", f32_.clone()), ("u_max", i32_.clone()), ("windup", fb.clone())];
        let (e, p, d, u, u_max, windup) = (0, 1, 2, 3, 4, 5);

        // if !run { integ <- 0; deriv <- 0; e_prev <- 0; duty <- 0; }
        let mut x = Dx::new(b, F, 57, 12);
        let running = x.carrier(run, bit.clone());
        let stopped = x.un(UnaryOp::Not, running, bit.clone());
        x.at(58, 13);
        let mut reset = Vec::new();
        for (carrier, ty) in [(integ, i32_.clone()), (deriv, i32_.clone()), (e_prev, i16_.clone()), (duty, u12.clone())] {
            let z = x.int(0, ty);
            reset.push(x.commit(carrier, z, None));
        }
        let reset = x.block(reset, None, ValueType::Unit);

        let mut run_stmts = Vec::new();
        // let e = ((reference as i16) - (meas as i16)).clamp(E_MIN, E_MAX);
        x.at(61, 21);
        let rf = x.carrier(reference, u10.clone());
        let rs = x.cast(rf, i16_.clone(), i16_.clone());
        let m = x.carrier(meas, u10.clone());
        let ms = x.cast(m, i16_.clone(), i16_.clone());
        let diff = x.bin(BinaryOp::Sub, rs, ms, i16_.clone());
        let lo = x.int(-256, i16_.clone());
        let hi = x.int(255, i16_.clone());
        let v = x.method(Method::Clamp, diff, vec![lo, hi], i16_.clone());
        run_stmts.push(x.let_(e, v));
        // let p = KP_Q4 * (e as i32);
        x.at(62, 21);
        let kp = x.int(64, i32_.clone());
        let el = x.local(e, i16_.clone());
        let ew = x.cast(el, i32_.clone(), i32_.clone());
        let v = x.bin(BinaryOp::Mul, kp, ew, i32_.clone());
        run_stmts.push(x.let_(p, v));
        // let d = (deriv >> D_POLE_SHIFT) + KD_Q4 * ((e - e_prev) as i32);
        x.at(63, 21);
        let dv = x.carrier(deriv, i32_.clone());
        let pole = x.int(2, i32_.clone());
        let filtered = x.bin(BinaryOp::Shr, dv, pole, f32_.clone());
        let kd = x.int(1024, i32_.clone());
        let el = x.local(e, i16_.clone());
        let ep = x.carrier(e_prev, i16_.clone());
        let de = x.bin(BinaryOp::Sub, el, ep, i16_.clone());
        let dw = x.cast(de, i32_.clone(), f32_.clone());
        let kick = x.bin(BinaryOp::Mul, kd, dw, f32_.clone());
        let v = x.bin(BinaryOp::Add, filtered, kick, f32_.clone());
        run_stmts.push(x.let_(d, v));
        // let u = (p + integ + d) >> Q_SHIFT;
        x.at(64, 21);
        let pl = x.local(p, i32_.clone());
        let ig = x.carrier(integ, i32_.clone());
        let sum = x.bin(BinaryOp::Add, pl, ig, f32_.clone());
        let dl = x.local(d, f32_.clone());
        let sum = x.bin(BinaryOp::Add, sum, dl, f32_.clone());
        let q = x.int(4, i32_.clone());
        let v = x.bin(BinaryOp::Shr, sum, q, f32_.clone());
        run_stmts.push(x.let_(u, v));
        // let u_max = DUTY_MAX as i32;
        x.at(65, 25);
        let dm = x.int(2880, u12.clone());
        let v = x.cast(dm, i32_.clone(), i32_.clone());
        run_stmts.push(x.let_(u_max, v));
        // let windup = (u > u_max && e > 0) || (u < 0 && e < 0);
        x.at(66, 26);
        let ul = x.local(u, f32_.clone());
        let um = x.local(u_max, i32_.clone());
        let over = x.bin(BinaryOp::Gt, ul, um, fb.clone());
        let el = x.local(e, i16_.clone());
        let z = x.int(0, i16_.clone());
        let pos = x.bin(BinaryOp::Gt, el, z, bit.clone());
        let high = x.bin(BinaryOp::And, over, pos, fb.clone());
        let ul = x.local(u, f32_.clone());
        let z = x.int(0, i32_.clone());
        let under = x.bin(BinaryOp::Lt, ul, z, fb.clone());
        let el = x.local(e, i16_.clone());
        let z = x.int(0, i16_.clone());
        let neg = x.bin(BinaryOp::Lt, el, z, bit.clone());
        let low = x.bin(BinaryOp::And, under, neg, fb.clone());
        let v = x.bin(BinaryOp::Or, high, low, fb.clone());
        run_stmts.push(x.let_(windup, v));
        // if !windup { integ <- (integ + KI_Q4 * (e as i32)).clamp(0, u_max << Q_SHIFT); }
        x.at(68, 26);
        let ig = x.carrier(integ, i32_.clone());
        let ki = x.int(8, i32_.clone());
        let el = x.local(e, i16_.clone());
        let ew = x.cast(el, i32_.clone(), i32_.clone());
        let gain = x.bin(BinaryOp::Mul, ki, ew, i32_.clone());
        let acc = x.bin(BinaryOp::Add, ig, gain, f32_.clone());
        let z = x.int(0, i32_.clone());
        let um = x.local(u_max, i32_.clone());
        let q = x.int(4, i32_.clone());
        let top = x.bin(BinaryOp::Shl, um, q, i32_.clone());
        let clamped = x.method(Method::Clamp, acc, vec![z, top], f32_.clone());
        let integrate = x.commit(integ, clamped, None);
        x.at(67, 13);
        let wl = x.local(windup, fb.clone());
        let free = x.un(UnaryOp::Not, wl, fb.clone());
        let cond = x.when(free, vec![integrate], None);
        run_stmts.push(x.stmt(cond));
        // deriv <- d; e_prev <- e; duty <- u.clamp(0, u_max) as u12;
        x.at(70, 13);
        let dl = x.local(d, f32_.clone());
        run_stmts.push(x.commit(deriv, dl, None));
        x.at(71, 13);
        let el = x.local(e, i16_.clone());
        run_stmts.push(x.commit(e_prev, el, None));
        x.at(72, 13);
        let ul = x.local(u, f32_.clone());
        let z = x.int(0, i32_.clone());
        let um = x.local(u_max, i32_.clone());
        let bounded = x.method(Method::Clamp, ul, vec![z, um], f32_.clone());
        let word = x.cast(bounded, u12.clone(), u12.clone());
        run_stmts.push(x.commit(duty, word, None));
        x.at(59, 16);
        let running_block = x.block(run_stmts, None, ValueType::Unit);
        x.at(57, 9);
        let body_if = x.if_(stopped, reset, running_block, ValueType::Unit);
        let stmt = x.stmt(body_if);
        let locals = locals.into_iter().map(|(n, t)| Buck::local(n, t)).collect();
        Buck::reaction(b, body, vec![rise], vec![stmt], locals, F, 56);
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
            let not = x.un(UnaryOp::Not, blocker, ValueType::Bool);
            let value = x.bin(BinaryOp::BitAnd, cmd, not, ValueType::Bool);
            x.at(line, 38);
            let after = x.param(0, ValueType::F64);
            b.drive(body, Drive { carrier: target, value, after: Some(after), origin: Buck::origin(F, line, 5), element: None, path: vec![] });
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
    /// analog for Switch { i(p, n) +> v(p, n) * if g { 1.0 / ron } else { 1.0 / roff }; }
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
        let on = g;
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
        let k_ty = ValueAliases::u(3);
        let mut x = Dx::new(b, "dpwm.msl", 14, 24);
        let k = x.local(0, k_ty.clone());
        let one = x.int(1, ValueAliases::u(3));
        let k_and_1 = x.bin(BinaryOp::BitAnd, k, one, k_ty.clone());
        let two = x.int(2, ValueAliases::i(32));
        let shifted = x.bin(BinaryOp::Shl, k_and_1, two, k_ty.clone());
        let k = x.local(0, k_ty.clone());
        let two_mask = x.int(2, ValueAliases::u(3));
        let middle = x.bin(BinaryOp::BitAnd, k, two_mask, k_ty.clone());
        let high_low = x.bin(BinaryOp::BitOr, shifted, middle, k_ty.clone());
        let k = x.local(0, k_ty.clone());
        let two = x.int(2, ValueAliases::i(32));
        let down = x.bin(BinaryOp::Shr, k, two, k_ty.clone());
        let one = x.int(1, ValueAliases::u(3));
        let low = x.bin(BinaryOp::BitAnd, down, one, k_ty.clone());
        let body = x.bin(BinaryOp::BitOr, high_low, low, k_ty.clone());
        let origin = Origin { file: "dpwm.msl".into(), line: 14, col: 1 };
        b.function(piperine_ir::FunctionSpec {
            name: "rev3".into(),
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
        let frame_ty = ValueAliases::u(3);
        let width_ty = ValueAliases::u(9);
        let mut x = Dx::new(b, "dpwm.msl", 19, 18);
        let duty = x.local(0, ValueAliases::u(12));
        let three = x.int(3, ValueAliases::i(32));
        let coarse = x.bin(BinaryOp::Shr, duty, three, ValueAliases::u(12));
        let coarse = x.cast(coarse, ValueAliases::u(9), ValueAliases::u(9));
        let let_coarse = x.let_(2, coarse);
        x.at(20, 16);
        let duty = x.local(0, ValueAliases::u(12));
        let seven = x.int(7, ValueAliases::u(12));
        let fine = x.bin(BinaryOp::BitAnd, duty, seven, ValueAliases::u(12));
        let fine = x.cast(fine, ValueAliases::u(3), ValueAliases::u(3));
        let let_fine = x.let_(3, fine);
        x.at(21, 5);
        let frame = x.local(1, frame_ty.clone());
        let rev = x.call(rev3, vec![frame], frame_ty.clone());
        let fine = x.local(3, ValueAliases::u(3));
        let cond = x.bin(BinaryOp::Lt, rev, fine, ValueType::Bool);
        let coarse = x.local(2, ValueAliases::u(9));
        let one = x.int(1, ValueAliases::u(9));
        let wider = x.bin(BinaryOp::Add, coarse, one, ValueAliases::u(9));
        let coarse = x.local(2, ValueAliases::u(9));
        let width = x.if_(cond, wider, coarse, width_ty.clone());
        x.at(18, 52);
        let body = x.block(vec![let_coarse, let_fine], Some(width), width_ty.clone());
        let origin = Origin { file: "dpwm.msl".into(), line: 18, col: 1 };
        b.function(piperine_ir::FunctionSpec {
            name: "dithered_width".into(),
            origin,
            params: vec![Buck::local("duty", ValueAliases::u(12)), Buck::local("frame", frame_ty.clone())],
            ret: width_ty,
            locals: vec![
                Buck::local("duty", ValueAliases::u(12)),
                Buck::local("frame", frame_ty),
                Buck::local("coarse", ValueAliases::u(9)),
                Buck::local("fine", ValueAliases::u(3)),
            ],
            body,
        })
    }
}
