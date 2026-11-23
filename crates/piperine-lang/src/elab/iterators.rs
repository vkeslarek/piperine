//! Arrays, ranges and iterators of Appendix H (H.5 to H.7; feature `composite-values`, CV-15).
//!
//! An iterator is a value that does nothing by itself (H.7 rule 4), so it has no IR value: the
//! chain a `for` or a consumer (`fold`, `sum`, `count`, `any`, `all`, `contains`) reads is written
//! out as one counting loop over the positions of its source. The elements of the source (an
//! array's, or `a + j` of a range) are read by position `j`; `map`, `enumerate`, `zip` and `rev`
//! are functions of the position, and `filter` is a guard the rest of the loop body sits under, so
//! the closures run once per element, in the order of the sequence. A `for` over an array reads
//! the elements by value (§1.8.9). `first`, `last` and `get` give an `Option` (H.5 rule 4 and 5).
//!
//! A chain is read where it is used: a `let` of an iterator or a range binds no value, the name
//! stands for the chain written there, whose closures capture by value (§1.4.6).

use piperine_ir::{BinaryOp, Expr, ExprId as LoweredExpr, Local, Origin, Place, Stmt, StmtId as LoweredStmt, UnaryOp, ValueType};

use super::compound::Pair;
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::msl::Span;
use crate::pom::{BlockNode, ExprId, ExprKind, PatId, PatKind, Resolution};
use crate::types::{Method, Ty};

/// Where the elements of a sequence come from.
enum Source {
    /// The elements of the array the expression gives, by value, from index 0.
    Array(ExprId),
    /// The integers of `lo..hi` (or `lo..=hi`).
    Range {
        /// The first value.
        lo: ExprId,
        /// The bound.
        hi: ExprId,
        /// Whether `hi` is included.
        inclusive: bool,
    },
}

/// One adaptor of H.7, in the order the source writes them.
enum Adaptor {
    /// `map(f)`: the closure literal.
    Map(ExprId),
    /// `filter(f)`: the closure literal.
    Filter(ExprId),
    /// `enumerate()`.
    Enumerate,
    /// `zip(other)`.
    Zip(Box<Sequence>),
    /// `rev()`.
    Rev,
}

/// A sequence as the source writes it: a source and its adaptors.
pub(super) struct Sequence {
    /// Where the elements start.
    source: Source,
    /// What each element goes through, first to last.
    adaptors: Vec<Adaptor>,
}

/// The source of a sequence, evaluated once before its loop.
enum Bound {
    /// The array held in this local.
    Array {
        /// The local holding the array.
        local: u32,
        /// The type of an element.
        element: ValueType,
    },
    /// The range whose first value is held in this local.
    Range {
        /// The local holding the first value.
        low: u32,
        /// The integer type of the range.
        element: ValueType,
    },
}

/// One adaptor with what it needs from before the loop.
enum Stage {
    /// `map(f)`.
    Map(ExprId),
    /// `filter(f)`.
    Filter(ExprId),
    /// `enumerate()`: the counter local when a `filter` came before it (the index counts the
    /// elements that pass), else the position itself.
    Enumerate(Option<u32>),
    /// `zip(other)`.
    Zip(Box<Prepared>),
    /// `rev()` over this many positions.
    Rev(LoweredExpr),
}

/// A sequence with its source evaluated: ready to be read by position.
pub(super) struct Prepared {
    /// The source.
    bound: Bound,
    /// The adaptors, first to last.
    stages: Vec<Stage>,
    /// The number of positions, a `usize` that is a constant or a local read.
    length: LoweredExpr,
    /// Whether a `filter` is among the stages: the positions are then not the elements.
    filtered: bool,
}

/// The statements of a loop body being built and the guards the later ones sit under.
struct LoopBody {
    /// The statements, in order.
    stmts: Vec<LoweredStmt>,
    /// Each `filter` condition and the number of statements before it.
    guards: Vec<(usize, LoweredExpr)>,
}

/// What a consumer does with each element.
enum Consumer {
    /// `sum()`.
    Sum,
    /// `count()`.
    Count,
    /// `fold(init, f)`: the closure literal.
    Fold(ExprId),
    /// `any(f)`: the closure literal.
    Any(ExprId),
    /// `all(f)`: the closure literal.
    All(ExprId),
    /// `contains(x)` of an array: the value sought, held in this local.
    Contains(u32),
}

impl Elaboration<'_, '_> {
    /// The sequence `expr` is: an array (read by value), a range, or a chain of adaptors over
    /// either, a `let`-bound name standing for what it was bound to. `None` after a diagnostic.
    pub(super) fn sequence(&mut self, scope: &Scope, expr: ExprId) -> Option<Sequence> {
        let expr = self.peel_parens(expr);
        let node = self.design.expr_node(expr).clone();
        if matches!(self.typed(expr), Some(Ty::Array(..))) {
            return Some(Sequence { source: Source::Array(expr), adaptors: Vec::new() });
        }
        match &node.kind {
            ExprKind::Range { lo: Some(lo), hi: Some(hi), inclusive } => Some(Sequence { source: Source::Range { lo: *lo, hi: *hi, inclusive: *inclusive }, adaptors: Vec::new() }),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) if let Some(init) = scope.let_sources.get(&decl).copied() => self.sequence(scope, init),
                _ => {
                    self.unsupported(scope.file, node.span, "an iterator that is not a chain written where it is used or bound by a `let`");
                    None
                }
            },
            ExprKind::MethodCall { recv, args, .. } => {
                let method = self.types.method(expr);
                if method == Some(Method::ArrayIter) {
                    return Some(Sequence { source: Source::Array(*recv), adaptors: Vec::new() });
                }
                let adaptor = match (method, args.as_slice()) {
                    (Some(Method::IteratorMap), [f]) => Adaptor::Map(f.value),
                    (Some(Method::IteratorFilter), [f]) => Adaptor::Filter(f.value),
                    (Some(Method::IteratorEnumerate), []) => Adaptor::Enumerate,
                    (Some(Method::IteratorRev), []) => Adaptor::Rev,
                    (Some(Method::IteratorZip), [other]) => Adaptor::Zip(Box::new(self.sequence(scope, other.value)?)),
                    _ => {
                        self.unsupported(scope.file, node.span, "an iterator method outside Appendix H.7");
                        return None;
                    }
                };
                let mut inner = self.sequence(scope, *recv)?;
                inner.adaptors.push(adaptor);
                Some(inner)
            }
            _ => {
                self.unsupported(scope.file, node.span, "a `for` over anything but an array, a range or an iterator chain");
                None
            }
        }
    }

    /// `let name = <iterator or range>;` binds no value: the name stands for the chain.
    pub(super) fn is_sequence_binding(&self, binding: crate::pom::DeclId) -> bool {
        matches!(self.typed_local(binding), Some(Ty::Iterator { .. } | Ty::Range { .. }))
    }

    /// Evaluates the source of `sequence` and its adaptors' state into `prelude`: the array and the
    /// range bounds once, the counters, the number of positions. `None` after a diagnostic.
    pub(super) fn prepare(&mut self, scope: &mut Scope, sequence: &Sequence, prelude: &mut Vec<LoweredStmt>, origin: &Origin, at: Span) -> Option<Prepared> {
        let usize_ty = ValueType::UInt(64);
        let (bound, mut length) = match &sequence.source {
            Source::Array(array) => {
                let value = self.lower_expr(scope, *array)?;
                let ValueType::Array(element, count) = self.type_of(value) else {
                    self.unsupported(scope.file, self.design.expr_node(*array).span, "an array that is not an array value");
                    return None;
                };
                let local = self.hold(scope, prelude, "array", value);
                (Bound::Array { local, element: *element }, self.emit(Expr::Int(i128::from(count)), usize_ty.clone(), origin.clone()))
            }
            Source::Range { lo, hi, inclusive } => {
                let (low, high) = (self.lower_expr(scope, *lo)?, self.lower_expr(scope, *hi)?);
                let element = self.type_of(low);
                if !element.is_integer() {
                    self.unsupported(scope.file, self.design.expr_node(*lo).span, "a range of anything but integers");
                    return None;
                }
                let (low_local, high_local) = (self.hold(scope, prelude, "low", low), self.hold(scope, prelude, "high", high));
                let length = self.range_length(scope, prelude, (low_local, high_local), &element, *inclusive, origin);
                (Bound::Range { low: low_local, element }, length)
            }
        };
        let mut stages = Vec::new();
        let mut filtered = false;
        for adaptor in &sequence.adaptors {
            match adaptor {
                Adaptor::Map(closure) => stages.push(Stage::Map(*closure)),
                Adaptor::Filter(closure) => {
                    filtered = true;
                    stages.push(Stage::Filter(*closure));
                }
                Adaptor::Enumerate => {
                    let counter = if filtered {
                        let zero = self.emit(Expr::Int(0), usize_ty.clone(), origin.clone());
                        Some(self.hold_mutable(scope, prelude, "count", zero))
                    } else {
                        None
                    };
                    stages.push(Stage::Enumerate(counter));
                }
                Adaptor::Zip(other) => {
                    let other = self.prepare(scope, other, prelude, origin, at)?;
                    if filtered || other.filtered {
                        self.unsupported(scope.file, at, "a `zip` of an iterator that was filtered");
                        return None;
                    }
                    length = self.shorter(scope, prelude, length, other.length, origin);
                    stages.push(Stage::Zip(Box::new(other)));
                }
                Adaptor::Rev => {
                    if filtered {
                        self.unsupported(scope.file, at, "a `rev` of an iterator that was filtered");
                        return None;
                    }
                    stages.push(Stage::Rev(length));
                }
            }
        }
        Some(Prepared { bound, stages, length, filtered })
    }

    /// `value` bound to a fresh immutable local named after `what`, added to `stmts`.
    fn hold(&mut self, scope: &mut Scope, stmts: &mut Vec<LoweredStmt>, what: &str, value: LoweredExpr) -> u32 {
        let local = scope.locals.len() as u32;
        scope.locals.push(Local { name: format!("_it${what}{local}"), ty: self.type_of(value), mutable: false });
        stmts.push(self.stmt(Stmt::Let { local, init: value }));
        local
    }

    /// `value` bound to a fresh assignable local named after `what`, added to `stmts`.
    fn hold_mutable(&mut self, scope: &mut Scope, stmts: &mut Vec<LoweredStmt>, what: &str, value: LoweredExpr) -> u32 {
        let local = self.hold(scope, stmts, what, value);
        scope.locals[local as usize].mutable = true;
        local
    }

    /// A read of local `local` of `scope`.
    fn read(&mut self, scope: &Scope, local: u32, origin: &Origin) -> LoweredExpr {
        self.emit(Expr::Local(local), scope.locals[local as usize].ty.clone(), origin.clone())
    }

    /// The number of integers of the range of `element` whose bounds are held in `bounds`, a
    /// `usize` held in a local (H.6 rule 3): nothing when the bound is not above the first value.
    fn range_length(&mut self, scope: &mut Scope, prelude: &mut Vec<LoweredStmt>, bounds: (u32, u32), element: &ValueType, inclusive: bool, origin: &Origin) -> LoweredExpr {
        let usize_ty = ValueType::UInt(64);
        let wide = if matches!(element, ValueType::Int(_)) { ValueType::Int(64) } else { usize_ty.clone() };
        let (low, high) = (self.read(scope, bounds.0, origin), self.read(scope, bounds.1, origin));
        let test = if inclusive { BinaryOp::Ge } else { BinaryOp::Gt };
        let nonempty = self.emit(Expr::Binary { op: test, lhs: high, rhs: low }, ValueType::Bool, origin.clone());
        let (high, low) = (self.read(scope, bounds.1, origin), self.read(scope, bounds.0, origin));
        let (high, low) = (self.emit(Expr::Cast { expr: high, to: wide.clone() }, wide.clone(), origin.clone()), self.emit(Expr::Cast { expr: low, to: wide.clone() }, wide.clone(), origin.clone()));
        let span = self.emit(Expr::Binary { op: BinaryOp::Sub, lhs: high, rhs: low }, wide.clone(), origin.clone());
        let mut count = self.emit(Expr::Cast { expr: span, to: usize_ty.clone() }, usize_ty.clone(), origin.clone());
        if inclusive {
            let one = self.emit(Expr::Int(1), usize_ty.clone(), origin.clone());
            count = self.emit(Expr::Binary { op: BinaryOp::Add, lhs: count, rhs: one }, usize_ty.clone(), origin.clone());
        }
        let zero = self.emit(Expr::Int(0), usize_ty.clone(), origin.clone());
        let length = self.emit(Expr::If { cond: nonempty, then: count, otherwise: zero }, usize_ty, origin.clone());
        let local = self.hold(scope, prelude, "len", length);
        self.read(scope, local, origin)
    }

    /// The smaller of two position counts, held in a local.
    fn shorter(&mut self, scope: &mut Scope, prelude: &mut Vec<LoweredStmt>, a: LoweredExpr, b: LoweredExpr, origin: &Origin) -> LoweredExpr {
        let cond = self.emit(Expr::Binary { op: BinaryOp::Lt, lhs: a, rhs: b }, ValueType::Bool, origin.clone());
        let smaller = self.emit(Expr::If { cond, then: a, otherwise: b }, ValueType::UInt(64), origin.clone());
        let local = self.hold(scope, prelude, "len", smaller);
        self.read(scope, local, origin)
    }

    /// The element of `prepared` after its first `upto` stages at position `j`, its statements added
    /// to `body`. The result is a read of a local, so it can be used more than once.
    fn element(&mut self, scope: &mut Scope, prepared: &Prepared, upto: usize, j: LoweredExpr, body: &mut LoopBody, origin: &Origin) -> Option<LoweredExpr> {
        let Some(stage) = upto.checked_sub(1).map(|k| &prepared.stages[k]) else {
            return Some(self.source_element(scope, prepared, j, body, origin));
        };
        match stage {
            Stage::Map(closure) => {
                let inner = self.element(scope, prepared, upto - 1, j, body, origin)?;
                let result = self.apply(scope, *closure, &[inner], body)?;
                self.bind_element(scope, body, "map", result, origin)
            }
            Stage::Filter(closure) => {
                let inner = self.element(scope, prepared, upto - 1, j, body, origin)?;
                let keep = self.apply(scope, *closure, &[inner], body)?;
                body.guards.push((body.stmts.len(), keep));
                Some(inner)
            }
            Stage::Enumerate(counter) => {
                let inner = self.element(scope, prepared, upto - 1, j, body, origin)?;
                let index = match counter {
                    Some(counter) => {
                        let current = self.read(scope, *counter, origin);
                        let taken = self.hold(scope, &mut body.stmts, "index", current);
                        let one = self.emit(Expr::Int(1), ValueType::UInt(64), origin.clone());
                        body.stmts.push(self.stmt(Stmt::Assign { place: Place { local: *counter, path: Vec::new() }, op: Some(BinaryOp::Add), value: one }));
                        self.read(scope, taken, origin)
                    }
                    None => j,
                };
                let pair_ty = ValueType::Tuple(vec![ValueType::UInt(64), self.type_of(inner)]);
                let pair = self.emit(Expr::Tuple(vec![index, inner]), pair_ty, origin.clone());
                self.bind_element(scope, body, "pair", pair, origin)
            }
            Stage::Zip(other) => {
                let first = self.element(scope, prepared, upto - 1, j, body, origin)?;
                let second = self.element(scope, other, other.stages.len(), j, body, origin)?;
                let pair_ty = ValueType::Tuple(vec![self.type_of(first), self.type_of(second)]);
                let pair = self.emit(Expr::Tuple(vec![first, second]), pair_ty, origin.clone());
                self.bind_element(scope, body, "pair", pair, origin)
            }
            Stage::Rev(length) => {
                let one = self.emit(Expr::Int(1), ValueType::UInt(64), origin.clone());
                let last = self.emit(Expr::Binary { op: BinaryOp::Sub, lhs: *length, rhs: one }, ValueType::UInt(64), origin.clone());
                let from_end = self.emit(Expr::Binary { op: BinaryOp::Sub, lhs: last, rhs: j }, ValueType::UInt(64), origin.clone());
                self.element(scope, prepared, upto - 1, from_end, body, origin)
            }
        }
    }

    /// The element of the source of `prepared` at position `j`: `a[j]`, or `low + j`.
    fn source_element(&mut self, scope: &mut Scope, prepared: &Prepared, j: LoweredExpr, body: &mut LoopBody, origin: &Origin) -> LoweredExpr {
        let (read, ty) = match &prepared.bound {
            Bound::Array { local, element } => {
                let array = self.read(scope, *local, origin);
                (self.emit(Expr::Index { expr: array, index: j }, element.clone(), origin.clone()), element.clone())
            }
            Bound::Range { low, element } => {
                let low = self.read(scope, *low, origin);
                let offset = self.emit(Expr::Cast { expr: j, to: element.clone() }, element.clone(), origin.clone());
                (self.emit(Expr::Binary { op: BinaryOp::Add, lhs: low, rhs: offset }, element.clone(), origin.clone()), element.clone())
            }
        };
        let local = self.hold(scope, &mut body.stmts, "item", read);
        scope.locals[local as usize].ty = ty;
        self.read(scope, local, origin)
    }

    /// `value` held in a local of the loop body.
    fn bind_element(&mut self, scope: &mut Scope, body: &mut LoopBody, what: &str, value: LoweredExpr, origin: &Origin) -> Option<LoweredExpr> {
        let local = self.hold(scope, &mut body.stmts, what, value);
        Some(self.read(scope, local, origin))
    }

    /// The closure literal `closure` called with `values`: its parameters bound by statements added
    /// to `body`, then its body lowered. `None` after a diagnostic.
    fn apply(&mut self, scope: &mut Scope, closure: ExprId, values: &[LoweredExpr], body: &mut LoopBody) -> Option<LoweredExpr> {
        let node = self.design.expr_node(closure).clone();
        let ExprKind::Closure { params, body: closure_body, .. } = node.kind else {
            self.unsupported(scope.file, node.span, "a closure argument that is not a closure literal");
            return None;
        };
        if params.len() != values.len() {
            self.unsupported(scope.file, node.span, "a closure with another number of parameters than the row gives it");
            return None;
        }
        for (param, value) in params.iter().zip(values) {
            self.bind_pattern(scope, param.pat, *value, &mut body.stmts)?;
        }
        scope.nesting += 1;
        let lowered = self.lower_expr(scope, closure_body);
        scope.nesting -= 1;
        lowered
    }

    /// The pattern `pat` bound to `value`: a name, `_` or a tuple of those, the bindings added to
    /// `stmts`. `None` after a diagnostic.
    fn bind_pattern(&mut self, scope: &mut Scope, pat: PatId, value: LoweredExpr, stmts: &mut Vec<LoweredStmt>) -> Option<()> {
        let node = self.design.pat_node(pat).clone();
        match node.kind {
            PatKind::Ident { binding, sub: None, .. } => {
                let local = self.bind_local(scope, binding)?;
                stmts.push(self.stmt(Stmt::Let { local, init: value }));
                Some(())
            }
            PatKind::Wild => Some(()),
            PatKind::Tuple(pats) => {
                let ValueType::Tuple(parts) = self.type_of(value) else {
                    self.unsupported(scope.file, node.span, "a tuple pattern over a value that is not a tuple");
                    return None;
                };
                let origin = self.expr_origins[value.index()].clone();
                for (k, (sub, part)) in pats.iter().zip(parts).enumerate() {
                    let field = self.emit(Expr::Field { expr: value, index: k as u32 }, part, origin.clone());
                    self.bind_pattern(scope, *sub, field, stmts)?;
                }
                Some(())
            }
            _ => {
                self.unsupported(scope.file, node.span, "a pattern other than a name, `_` or a tuple of them here");
                None
            }
        }
    }

    /// The statements of `body` with each guard wrapping what follows it in an `if` of `()`.
    fn guarded(&mut self, body: LoopBody, origin: &Origin) -> Vec<LoweredStmt> {
        let mut stmts = body.stmts;
        for (at, cond) in body.guards.into_iter().rev() {
            let inner = stmts.split_off(at);
            let then = self.emit(Expr::Block { stmts: inner, value: None }, ValueType::Unit, origin.clone());
            let otherwise = self.emit(Expr::Unit, ValueType::Unit, origin.clone());
            let whole = self.emit(Expr::If { cond, then, otherwise }, ValueType::Unit, origin.clone());
            stmts.push(self.stmt(Stmt::Expr(whole)));
        }
        stmts
    }

    /// The counting loop over the positions of `prepared` with `body` (already guarded), as one
    /// statement.
    fn position_loop(&mut self, scope: &mut Scope, prepared: &Prepared, counter: u32, body: Vec<LoweredStmt>, origin: &Origin) -> LoweredStmt {
        let low = self.emit(Expr::Int(0), ValueType::UInt(64), origin.clone());
        let _ = scope;
        let looped = self.emit(Expr::ForRange { local: counter, low, high: prepared.length, inclusive: false, reverse: false, body }, ValueType::Unit, origin.clone());
        self.stmt(Stmt::Expr(looped))
    }

    /// A fresh loop counter local.
    fn position_local(&mut self, scope: &mut Scope) -> u32 {
        let local = scope.locals.len() as u32;
        scope.locals.push(Local { name: format!("_it$position{local}"), ty: ValueType::UInt(64), mutable: false });
        local
    }

    /// `for pat in iter { body }` over an array, a range with adaptors or an iterator chain: the
    /// counting loop of the module doc. `None` after a diagnostic.
    pub(super) fn lower_sequence_for(&mut self, scope: &mut Scope, pat: PatId, iter: ExprId, body: &BlockNode, origin: Origin) -> Option<LoweredExpr> {
        let sequence = self.sequence(scope, iter)?;
        let mut stmts = Vec::new();
        let span = self.design.expr_node(iter).span;
        let prepared = self.prepare(scope, &sequence, &mut stmts, &origin, span)?;
        let counter = self.position_local(scope);
        let j = self.read(scope, counter, &origin);
        let mut loop_body = LoopBody { stmts: Vec::new(), guards: Vec::new() };
        let element = self.element(scope, &prepared, prepared.stages.len(), j, &mut loop_body, &origin)?;
        self.bind_pattern(scope, pat, element, &mut loop_body.stmts)?;
        let user = self.lower_loop_body(scope, body)?;
        loop_body.stmts.extend(user);
        let guarded = self.guarded(loop_body, &origin);
        stmts.push(self.position_loop(scope, &prepared, counter, guarded, &origin));
        Some(self.emit(Expr::Block { stmts, value: None }, ValueType::Unit, origin))
    }

    /// A row of H.5 to H.7 read as a value: `len`, `is_empty`, `contains` of an array or a range,
    /// and the consumers `fold`, `sum`, `count`, `any`, `all`. `None` after a diagnostic.
    pub(super) fn lower_sequence_method(&mut self, scope: &mut Scope, expr: ExprId, method: Method, recv: ExprId, args: &[ExprId], ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        match method {
            Method::ArrayLen => {
                let array = self.lower_expr(scope, recv)?;
                Some(self.emit(Expr::Length(array), ValueType::UInt(64), origin))
            }
            Method::ArrayIsEmpty => {
                let array = self.lower_expr(scope, recv)?;
                let length = self.emit(Expr::Length(array), ValueType::UInt(64), origin.clone());
                let zero = self.emit(Expr::Int(0), ValueType::UInt(64), origin.clone());
                Some(self.emit(Expr::Binary { op: BinaryOp::Eq, lhs: length, rhs: zero }, ValueType::Bool, origin))
            }
            Method::RangeContains => self.lower_range_contains(scope, recv, args[0], origin),
            Method::RangeIsEmpty => self.lower_range_is_empty(scope, recv, origin),
            Method::ArrayContains => {
                let sought = self.lower_expr(scope, args[0])?;
                let mut stmts = Vec::new();
                let sought_local = self.hold(scope, &mut stmts, "sought", sought);
                self.lower_consumer(scope, stmts, Sequence { source: Source::Array(recv), adaptors: Vec::new() }, Consumer::Contains(sought_local), ty, origin, span)
            }
            Method::IteratorSum | Method::IteratorCount | Method::IteratorFold | Method::IteratorAny | Method::IteratorAll => {
                let consumer = match (method, args) {
                    (Method::IteratorSum, []) => Consumer::Sum,
                    (Method::IteratorCount, []) => Consumer::Count,
                    (Method::IteratorFold, [_, f]) => Consumer::Fold(*f),
                    (Method::IteratorAny, [f]) => Consumer::Any(*f),
                    (Method::IteratorAll, [f]) => Consumer::All(*f),
                    _ => {
                        self.unsupported(scope.file, span, "an iterator consumer with other arguments than the row takes");
                        return None;
                    }
                };
                let sequence = self.sequence(scope, recv)?;
                let mut stmts = Vec::new();
                if let Consumer::Fold(_) = consumer {
                    let init = self.lower_expr(scope, args[0])?;
                    let local = self.hold_mutable(scope, &mut stmts, "acc", init);
                    return self.lower_consumer_with(scope, stmts, sequence, consumer, Some(local), ty, origin, span);
                }
                self.lower_consumer(scope, stmts, sequence, consumer, ty, origin, span)
            }
            other => {
                self.unsupported(scope.file, span, &format!("a `{other:?}` row read as a value"));
                None
            }
        }
    }

    /// A consumer with no accumulator of its own to start from.
    fn lower_consumer(&mut self, scope: &mut Scope, prelude: Vec<LoweredStmt>, sequence: Sequence, consumer: Consumer, ty: ValueType, origin: Origin, span: Span) -> Option<LoweredExpr> {
        self.lower_consumer_with(scope, prelude, sequence, consumer, None, ty, origin, span)
    }

    /// The loop of a consumer: the accumulator (`started` when the row gave its first value) is the
    /// block's value. `prelude` holds what was evaluated before the sequence.
    fn lower_consumer_with(&mut self, scope: &mut Scope, mut stmts: Vec<LoweredStmt>, sequence: Sequence, consumer: Consumer, started: Option<u32>, ty: ValueType, origin: Origin, span: Span) -> Option<LoweredExpr> {
        let prepared = self.prepare(scope, &sequence, &mut stmts, &origin, span)?;
        let accumulator = match started {
            Some(local) => local,
            None => {
                let first = match (&consumer, &ty) {
                    (Consumer::Any(_) | Consumer::Contains(_), _) => self.emit(Expr::Bool(false), ValueType::Bool, origin.clone()),
                    (Consumer::All(_), _) => self.emit(Expr::Bool(true), ValueType::Bool, origin.clone()),
                    (_, ty) => self.zero(ty, origin.clone())?,
                };
                self.hold_mutable(scope, &mut stmts, "acc", first)
            }
        };
        let counter = self.position_local(scope);
        let j = self.read(scope, counter, &origin);
        let mut body = LoopBody { stmts: Vec::new(), guards: Vec::new() };
        let element = self.element(scope, &prepared, prepared.stages.len(), j, &mut body, &origin)?;
        let own = Place { local: accumulator, path: Vec::new() };
        match consumer {
            Consumer::Sum => {
                body.stmts.push(self.stmt(Stmt::Assign { place: own, op: Some(BinaryOp::Add), value: element }));
            }
            Consumer::Count => {
                let one = self.emit(Expr::Int(1), ValueType::UInt(64), origin.clone());
                body.stmts.push(self.stmt(Stmt::Assign { place: own, op: Some(BinaryOp::Add), value: one }));
            }
            Consumer::Fold(closure) => {
                let held = self.read(scope, accumulator, &origin);
                let next = self.apply(scope, closure, &[held, element], &mut body)?;
                body.stmts.push(self.stmt(Stmt::Assign { place: own, op: None, value: next }));
            }
            Consumer::Any(closure) => {
                let found = self.apply(scope, closure, &[element], &mut body)?;
                self.settle_and_leave(&mut body, found, accumulator, true, &origin);
            }
            Consumer::All(closure) => {
                let holds = self.apply(scope, closure, &[element], &mut body)?;
                let fails = self.emit(Expr::Unary { op: UnaryOp::Not, expr: holds }, ValueType::Bool, origin.clone());
                self.settle_and_leave(&mut body, fails, accumulator, false, &origin);
            }
            Consumer::Contains(sought) => {
                let wanted = self.read(scope, sought, &origin);
                let equal = self.emit(Expr::Binary { op: BinaryOp::Eq, lhs: element, rhs: wanted }, ValueType::Bool, origin.clone());
                self.settle_and_leave(&mut body, equal, accumulator, true, &origin);
            }
        }
        let guarded = self.guarded(body, &origin);
        stmts.push(self.position_loop(scope, &prepared, counter, guarded, &origin));
        let result = self.read(scope, accumulator, &origin);
        let result_ty = self.type_of(result);
        Some(self.emit(Expr::Block { stmts, value: Some(result) }, result_ty, origin))
    }

    /// `if decided { acc = value; break }`: the rest of the elements are not visited (H.7 rule 10).
    fn settle_and_leave(&mut self, body: &mut LoopBody, decided: LoweredExpr, accumulator: u32, value: bool, origin: &Origin) {
        let set = self.emit(Expr::Bool(value), ValueType::Bool, origin.clone());
        let assign = self.stmt(Stmt::Assign { place: Place { local: accumulator, path: Vec::new() }, op: None, value: set });
        let leave = self.emit_exit(Expr::Break(None), origin.clone());
        let leave = self.stmt(Stmt::Expr(leave));
        let then = self.emit(Expr::Block { stmts: vec![assign, leave], value: None }, ValueType::Unit, origin.clone());
        let otherwise = self.emit(Expr::Unit, ValueType::Unit, origin.clone());
        let whole = self.emit(Expr::If { cond: decided, then, otherwise }, ValueType::Unit, origin.clone());
        body.stmts.push(self.stmt(Stmt::Expr(whole)));
    }

    /// `r.contains(x)` of a range (H.6 rule 1): each bound the range has is tested.
    fn lower_range_contains(&mut self, scope: &mut Scope, recv: ExprId, sought: ExprId, origin: Origin) -> Option<LoweredExpr> {
        let (lo, hi, inclusive) = self.range_parts(scope, recv)?;
        let mut tests = Vec::new();
        if let Some(lo) = lo {
            let (low, value) = (self.lower_expr(scope, lo)?, self.lower_expr(scope, sought)?);
            tests.push(self.emit(Expr::Binary { op: BinaryOp::Le, lhs: low, rhs: value }, ValueType::Bool, origin.clone()));
        }
        if let Some(hi) = hi {
            let (value, high) = (self.lower_expr(scope, sought)?, self.lower_expr(scope, hi)?);
            let op = if inclusive { BinaryOp::Le } else { BinaryOp::Lt };
            tests.push(self.emit(Expr::Binary { op, lhs: value, rhs: high }, ValueType::Bool, origin.clone()));
        }
        let mut result = match tests.pop() {
            Some(last) => last,
            None => self.emit(Expr::Bool(true), ValueType::Bool, origin.clone()),
        };
        while let Some(earlier) = tests.pop() {
            result = self.emit(Expr::Binary { op: BinaryOp::And, lhs: earlier, rhs: result }, ValueType::Bool, origin.clone());
        }
        Some(result)
    }

    /// `r.is_empty()` of a range (H.6 rule 2): `!(lo < hi)`, `!(lo <= hi)` for `..=`.
    fn lower_range_is_empty(&mut self, scope: &mut Scope, recv: ExprId, origin: Origin) -> Option<LoweredExpr> {
        let (Some(lo), Some(hi), inclusive) = self.range_parts(scope, recv)? else {
            self.unsupported(scope.file, self.design.expr_node(recv).span, "`is_empty` of a range with a missing bound");
            return None;
        };
        let (low, high) = (self.lower_expr(scope, lo)?, self.lower_expr(scope, hi)?);
        let op = if inclusive { BinaryOp::Le } else { BinaryOp::Lt };
        let ordered = self.emit(Expr::Binary { op, lhs: low, rhs: high }, ValueType::Bool, origin.clone());
        Some(self.emit(Expr::Unary { op: UnaryOp::Not, expr: ordered }, ValueType::Bool, origin))
    }

    /// The bounds and inclusiveness of the range `expr` stands for: a range expression, or a
    /// name `let`-bound to one.
    fn range_parts(&mut self, scope: &Scope, expr: ExprId) -> Option<(Option<ExprId>, Option<ExprId>, bool)> {
        let expr = self.peel_parens(expr);
        let node = self.design.expr_node(expr).clone();
        match &node.kind {
            ExprKind::Range { lo, hi, inclusive } => Some((*lo, *hi, *inclusive)),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) if let Some(init) = scope.let_sources.get(&decl).copied() => self.range_parts(scope, init),
                _ => {
                    self.unsupported(scope.file, node.span, "a range that is not written where it is used or bound by a `let`");
                    None
                }
            },
            _ => {
                self.unsupported(scope.file, node.span, "a range that is not written where it is used or bound by a `let`");
                None
            }
        }
    }

    /// `a.first()`, `a.last()`, `a.get(i)` of an array as the pair of an `Option` (H.5 rules 4 and 5):
    /// `None` when the array has no such element, the payload then the zero. `None` after a
    /// diagnostic.
    pub(super) fn lower_array_option(&mut self, scope: &mut Scope, method: Method, recv: ExprId, args: &[ExprId], payload: ValueType, origin: Origin) -> Option<Pair> {
        let usize_ty = ValueType::UInt(64);
        let array = self.lower_expr(scope, recv)?;
        let ValueType::Array(_, count) = self.type_of(array) else {
            self.unsupported(scope.file, self.design.expr_node(recv).span, "a value that is not an array");
            return None;
        };
        let literal = |this: &mut Self, value: bool| this.emit(Expr::Bool(value), ValueType::Bool, origin.clone());
        let mut at = |this: &mut Self, index: u64| {
            let array = this.lower_expr(scope, recv)?;
            let index = this.emit(Expr::Int(i128::from(index)), usize_ty.clone(), origin.clone());
            Some(this.emit(Expr::Index { expr: array, index }, payload.clone(), origin.clone()))
        };
        match (method, count) {
            (Method::ArrayFirst | Method::ArrayLast, 0) => {
                let is = literal(self, false);
                let val = self.zero(&payload, origin)?;
                Some(Pair { is, val, literal: Some(false) })
            }
            (Method::ArrayFirst, _) => {
                let (is, val) = (literal(self, true), at(self, 0)?);
                Some(Pair { is, val, literal: Some(true) })
            }
            (Method::ArrayLast, _) => {
                let (is, val) = (literal(self, true), at(self, count - 1)?);
                Some(Pair { is, val, literal: Some(true) })
            }
            (Method::ArrayGet, _) => {
                let [index] = args else { return None };
                let mut bound = |this: &mut Self| {
                    let wanted = this.lower_expr(scope, *index)?;
                    let limit = this.emit(Expr::Int(i128::from(count)), usize_ty.clone(), origin.clone());
                    Some((wanted, this.emit(Expr::Binary { op: BinaryOp::Lt, lhs: wanted, rhs: limit }, ValueType::Bool, origin.clone())))
                };
                let (_, is) = bound(self)?;
                let (index_read, inside) = bound(self)?;
                let array = self.lower_expr(scope, recv)?;
                let element = self.emit(Expr::Index { expr: array, index: index_read }, payload.clone(), origin.clone());
                let zero = self.zero(&payload, origin.clone())?;
                let val = self.emit(Expr::If { cond: inside, then: element, otherwise: zero }, payload, origin);
                Some(Pair { is, val, literal: None })
            }
            _ => None,
        }
    }
}
