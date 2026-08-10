//! The IR well-formedness verifier (LOV-03/04/14) — rejects malformed IR
//! before any backend sees it. A green build is not a correct build: every
//! defect this IR exists to fix compiled cleanly while being wrong. The
//! verifier is the mechanical proof that the IR's invariants hold, so the
//! backends and the reference interpreter can assume them.
//!
//! Each check has a NAMED ERROR (typed enum, MD-09 style — never a free
//! string), and every error names the offending entity (a `CellId`, a
//! `NetId`, a `ValueId`, …) so the failure points at the construct to fix.
//! No catch-all arm: a construct the verifier does not recognise is itself
//! a defect (T17 gates this mechanically; the verifier's match arms are
//! already explicit-only).
//!
//! Top-level `verify` is the project-wide gate (per design.md §4). The
//! finite-difference oracle for `BehaviorKind` lives at
//! `behavior::fd_oracle::verify_derivatives` — its system-function home.

use crate::core::{
    Block, BlockId, CellId, Domain, Module, NetId, NetKind, Op, PureOp, SigRef, Trigger, Ty,
    ValueId,
};

/// Where in the IR a verification failure was found. The block the op
/// lives in, plus the op's index within the block (or within the nested
/// control-flow arm). For non-op failures (e.g. a cell with a bad writer),
/// `op_index` is unused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpSite {
    /// The block the failing op lives in.
    pub block: BlockId,
    /// The op's position within the block's op list (top-level ops only;
    /// nested If/Match arms are visited but their inner index is not
    /// tracked separately here).
    pub op_index: usize,
}

impl OpSite {
    /// Where the verifier was looking when it found the problem.
    fn at(block: BlockId, op_index: usize) -> Self {
        Self { block, op_index }
    }
}

/// A verification failure naming the offending entity. The error type is a
/// closed enum so the caller can match on it; the [`std::fmt::Display`]
/// impl carries the human-readable diagnostic naming the entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// A `Cell.writer` does not reference a declared block. The IR's
    /// single-writer rule (T4) requires the writer to be a real block.
    CellWriterNotDeclared {
        cell: CellId,
        writer: BlockId,
        block_count: usize,
    },
    /// An analog net-access op (`ReadAcross`/`ReadThrough`/`ContributeThrough`/
    /// `ForceAcross`) targets a `Signal` net. The A0b mechanism — a Signal
    /// cannot be handed to analog stamping — is rejected here, before any
    /// matrix sees it.
    AnalogOpOnSignal { site: OpSite, net: NetId },
    /// A digital net-access op (`ReadSignal`/`Drive`) targets a `Node` net.
    SignalOpOnNode { site: OpSite, net: NetId },
    /// A `Chunk::Slice { lo, hi }` extends past the source value's width.
    ChunkOutOfBounds {
        chunk_index: usize,
        source: ValueId,
        lo: u32,
        hi: u32,
        source_width: u32,
    },
    /// An op's output type doesn't match what the op requires. Examples:
    /// `ReadAcross.out` must be `Real`; `CellRead.out` must match the
    /// cell's element type.
    OutputTypeMismatch {
        site: OpSite,
        out: ValueId,
        expected: Ty,
        found: Ty,
        context: &'static str,
    },
    /// A `Drive` op appears in an analog block, or `ContributeThrough`/
    /// `ForceAcross` in a digital block. The two domains produce different
    /// things and the IR keeps them apart (LOV-07).
    DomainOpMismatch {
        site: OpSite,
        domain: Domain,
        op_kind: &'static str,
    },
    /// A reference to an undeclared entity — an id out of bounds for the
    /// module's arena. The verifier catches these so the backends can
    /// assume every id resolves.
    UndeclaredValue { site: OpSite, value: ValueId, value_count: usize },
    UndeclaredNet { site: OpSite, net: NetId, net_count: usize },
    UndeclaredCell { site: OpSite, cell: CellId, cell_count: usize },
    /// A `CellWrite` appears in a block OTHER than the cell's declared
    /// `writer` (the A0d mechanism — a module-level var with two claimed
    /// owners). `Cell.writer` is a field, so a SECOND writer cannot be
    /// declared at construction time; this catches the remaining way two
    /// blocks could both write one cell — a rogue `CellWrite` op in a
    /// block that isn't the declared writer. Names BOTH blocks.
    CellWrittenByWrongBlock { site: OpSite, cell: CellId, declared_writer: BlockId, writing_block: BlockId },
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::CellWriterNotDeclared { cell, writer, block_count } => write!(
                f,
                "cell {cell:?} names writer {writer:?} but the module declares only \
                 {block_count} block(s); the single-writer rule requires the writer to exist"
            ),
            VerifyError::AnalogOpOnSignal { site, net } => write!(
                f,
                "block {:?} op {}: an analog net-access op references net {net:?}, but that \
                 net is a `Signal`, not a `Node` — a Signal cannot be handed to analog stamping \
                 (the A0b mechanism)",
                site.block, site.op_index
            ),
            VerifyError::SignalOpOnNode { site, net } => write!(
                f,
                "block {:?} op {}: a digital net-access op references net {net:?}, but that \
                 net is a `Node`, not a `Signal`",
                site.block, site.op_index
            ),
            VerifyError::ChunkOutOfBounds { chunk_index, source, lo, hi, source_width } => write!(
                f,
                "sigref chunk {chunk_index} slices value {source:?} at [{lo}:{hi}] but the \
                 value's width is only {source_width}"
            ),
            VerifyError::OutputTypeMismatch { site, out, expected, found, context } => write!(
                f,
                "block {:?} op {}: output value {out:?} should be {expected:?} ({context}) but \
                 is {found:?}",
                site.block, site.op_index,
            ),
            VerifyError::DomainOpMismatch { site, domain, op_kind } => write!(
                f,
                "block {:?} op {}: a `{op_kind}` op is not allowed in a `{:?}`-domain block",
                site.block, site.op_index, domain,
            ),
            VerifyError::UndeclaredValue { site, value, value_count } => write!(
                f,
                "block {:?} op {}: references value {value:?} but the module declares only \
                 {value_count} value(s)",
                site.block, site.op_index,
            ),
            VerifyError::UndeclaredNet { site, net, net_count } => write!(
                f,
                "block {:?} op {}: references net {net:?} but the module declares only \
                 {net_count} net(s)",
                site.block, site.op_index,
            ),
            VerifyError::UndeclaredCell { site, cell, cell_count } => write!(
                f,
                "block {:?} op {}: references cell {cell:?} but the module declares only \
                 {cell_count} cell(s)",
                site.block, site.op_index,
            ),
            VerifyError::CellWrittenByWrongBlock { site, cell, declared_writer, writing_block } => write!(
                f,
                "block {:?} op {}: writes cell {cell:?}, but that cell's declared writer is \
                 block {declared_writer:?} — block {writing_block:?} has no claim to write it \
                 (the A0d mechanism: a cell with two claimed owners)",
                site.block, site.op_index,
            ),
        }
    }
}

impl std::error::Error for VerifyError {}

/// The verifier — walks a [`Module`] and confirms every well-formedness
/// invariant. Owns the check methods so they are not loose functions
/// (MD-13 rule 2). Returns the FIRST error found: a single named entity is
/// enough to reproduce and fix the bug, matching `fd_oracle::verify_derivatives`.
pub struct Verifier<'m> {
    module: &'m Module,
}

impl<'m> Verifier<'m> {
    /// Build a verifier over `module`.
    pub fn new(module: &'m Module) -> Self {
        Self { module }
    }

    /// Run every check, in declared order. Stops at the first failure.
    pub fn run(self) -> Result<(), VerifyError> {
        self.check_cell_writers_declared()?;
        self.check_chunk_bounds()?;
        for (block_id, block) in self.module.blocks() {
            self.check_block(block_id, block)?;
        }
        Ok(())
    }

    /// Check 1 — every cell's `writer` references a declared block.
    fn check_cell_writers_declared(&self) -> Result<(), VerifyError> {
        let block_count = self.module.block_count();
        for (cell_id, cell) in self.module.cells() {
            if cell.writer.0 as usize >= block_count {
                return Err(VerifyError::CellWriterNotDeclared {
                    cell: cell_id,
                    writer: cell.writer,
                    block_count,
                });
            }
        }
        Ok(())
    }

    /// Check 3 — every `Chunk::Slice { lo, hi }` in any block's `Edge`
    /// trigger satisfies `hi <= source_width`. (SigRefs appear elsewhere
    /// only in triggers this phase; the check is general enough to extend.)
    fn check_chunk_bounds(&self) -> Result<(), VerifyError> {
        for (_block_id, block) in self.module.blocks() {
            if let Trigger::Edge(sig_ref, _kind) = &block.trigger {
                self.check_sig_ref_chunks(sig_ref)?;
            }
        }
        Ok(())
    }

    /// Verify every chunk in a SigRef.
    fn check_sig_ref_chunks(&self, sig_ref: &SigRef) -> Result<(), VerifyError> {
        for (i, chunk) in sig_ref.0.iter().enumerate() {
            use crate::core::Chunk;
            match chunk {
                Chunk::Slice { value, lo, hi } => {
                    if (*value).0 as usize >= self.module.value_count() {
                        continue; // undeclared-value is not THIS check's job
                    }
                    let source_width = self.module.value_ty(*value).width();
                    if *hi > source_width || *lo > *hi {
                        return Err(VerifyError::ChunkOutOfBounds {
                            chunk_index: i,
                            source: *value,
                            lo: *lo,
                            hi: *hi,
                            source_width,
                        });
                    }
                }
                Chunk::Const(_) => {}
            }
        }
        Ok(())
    }

    /// Checks 2/4/5 — every op in the block references declared entities,
    /// has consistent widths, matches its net kind, and is allowed in the
    /// block's domain.
    fn check_block(&self, block_id: BlockId, block: &Block) -> Result<(), VerifyError> {
        for (op_index, op) in block.ops().iter().enumerate() {
            let site = OpSite::at(block_id, op_index);
            self.check_op(site, block.domain, op)?;
        }
        Ok(())
    }

    /// All per-op checks, dispatching by op variant. Explicit arms only —
    /// no `_ =>` (the A0c defence).
    fn check_op(&self, site: OpSite, domain: Domain, op: &Op) -> Result<(), VerifyError> {
        match op {
            Op::ReadAcross { a, b, out } => {
                self.check_analog_node_pair(site, domain, "ReadAcross", *a, *b)?;
                self.check_value_ty(site, *out, Ty::Real, "ReadAcross output must be Real")?;
            }
            Op::ReadThrough { a, b, out } => {
                self.check_analog_node_pair(site, domain, "ReadThrough", *a, *b)?;
                self.check_value_ty(site, *out, Ty::Real, "ReadThrough output must be Real")?;
            }
            Op::ContributeThrough { a, b, value } => {
                self.check_analog_node_pair(site, domain, "ContributeThrough", *a, *b)?;
                self.check_value_ty(site, *value, Ty::Real, "ContributeThrough value must be Real")?;
            }
            Op::ForceAcross { a, b, value } => {
                self.check_analog_node_pair(site, domain, "ForceAcross", *a, *b)?;
                self.check_value_ty(site, *value, Ty::Real, "ForceAcross value must be Real")?;
            }
            Op::ReadSignal { net, out } => {
                self.check_signal_net(site, domain, "ReadSignal", *net)?;
                // The output type must match the signal's `ty`.
                let expected = match self.module.net_at(*net).kind() {
                    NetKind::Signal { ty } => ty.clone(),
                    NetKind::Node { .. } => unreachable!("check_signal_net rejected Node"),
                };
                self.check_value_ty(site, *out, expected, "ReadSignal output must match the signal's type")?;
            }
            Op::Drive { net, value } => {
                self.check_signal_net(site, domain, "Drive", *net)?;
                let expected = match self.module.net_at(*net).kind() {
                    NetKind::Signal { ty } => ty.clone(),
                    NetKind::Node { .. } => unreachable!("check_signal_net rejected Node"),
                };
                self.check_value_ty(site, *value, expected, "Drive value must match the signal's type")?;
            }
            Op::CellRead { cell, addr, out } => {
                self.check_declared_cell(site, *cell)?;
                if let Some(addr_v) = addr {
                    self.check_declared_value(site, *addr_v)?;
                    // Address must be Bits(some width) — an integer index.
                    self.check_value_is_bits(site, *addr_v, "CellRead address must be bits-typed")?;
                }
                // Output type matches the cell's element type.
                let cell_ty = &self.module.cell_at(*cell).ty;
                let expected = cell_ty.element().cloned().unwrap_or_else(|| cell_ty.clone());
                self.check_value_ty(site, *out, expected, "CellRead output must match the cell's element type")?;
            }
            Op::CellWrite { cell, addr, value } => {
                self.check_declared_cell(site, *cell)?;
                let declared_writer = self.module.cell_at(*cell).writer;
                if declared_writer != site.block {
                    return Err(VerifyError::CellWrittenByWrongBlock {
                        site,
                        cell: *cell,
                        declared_writer,
                        writing_block: site.block,
                    });
                }
                if let Some(addr_v) = addr {
                    self.check_declared_value(site, *addr_v)?;
                    self.check_value_is_bits(site, *addr_v, "CellWrite address must be bits-typed")?;
                }
                self.check_declared_value(site, *value)?;
                let cell_ty = &self.module.cell_at(*cell).ty;
                let expected = cell_ty.element().cloned().unwrap_or_else(|| cell_ty.clone());
                self.check_value_ty(site, *value, expected, "CellWrite value must match the cell's element type")?;
            }
            Op::Pure { op: pure, args, out } => {
                self.check_pure_op(site, *pure, args, *out)?;
            }
            Op::Call { intrinsic: _, args, out } => {
                // The intrinsic catalog is added in a later phase — for now,
                // check only that the value references are declared.
                for a in args {
                    self.check_declared_value(site, *a)?;
                }
                self.check_declared_value(site, *out)?;
            }
            Op::If { cond, then_ops, else_ops } => {
                self.check_declared_value(site, *cond)?;
                self.check_value_is_bits(site, *cond, "If condition must be bits-typed (width 1)")?;
                // Width 1 specifically — the condition is a single bit.
                if self.module.value_ty(*cond).width() != 1 {
                    return Err(VerifyError::OutputTypeMismatch {
                        site,
                        out: *cond,
                        expected: Ty::Bits(1),
                        found: self.module.value_ty(*cond).clone(),
                        context: "If condition must be a single bit",
                    });
                }
                for inner in then_ops.iter().chain(else_ops.iter()) {
                    self.check_op(site, domain, inner)?;
                }
            }
            Op::Match { scrutinee, arms } => {
                self.check_declared_value(site, *scrutinee)?;
                self.check_value_is_bits(site, *scrutinee, "Match scrutinee must be bits-typed")?;
                for (_, body) in arms {
                    for inner in body {
                        self.check_op(site, domain, inner)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Helper: an analog node-pair op (`ReadAcross`/etc.) must target two
    /// `Node` nets, and must run in an `Analog`-domain block.
    fn check_analog_node_pair(
        &self,
        site: OpSite,
        domain: Domain,
        op_kind: &'static str,
        a: NetId,
        b: NetId,
    ) -> Result<(), VerifyError> {
        if domain != Domain::Analog {
            return Err(VerifyError::DomainOpMismatch { site, domain, op_kind });
        }
        for net in [a, b] {
            if net.0 as usize >= self.module.net_count() {
                return Err(VerifyError::UndeclaredNet {
                    site,
                    net,
                    net_count: self.module.net_count(),
                });
            }
            if !self.module.net_at(net).kind().is_node() {
                return Err(VerifyError::AnalogOpOnSignal { site, net });
            }
        }
        Ok(())
    }

    /// Helper: a signal net op (`ReadSignal`/`Drive`) must target a
    /// `Signal` net, and must run in a `Digital`-domain block.
    fn check_signal_net(
        &self,
        site: OpSite,
        domain: Domain,
        op_kind: &'static str,
        net: NetId,
    ) -> Result<(), VerifyError> {
        if domain != Domain::Digital {
            return Err(VerifyError::DomainOpMismatch { site, domain, op_kind });
        }
        if net.0 as usize >= self.module.net_count() {
            return Err(VerifyError::UndeclaredNet {
                site,
                net,
                net_count: self.module.net_count(),
            });
        }
        if !self.module.net_at(net).kind().is_signal() {
            return Err(VerifyError::SignalOpOnNode { site, net });
        }
        Ok(())
    }

    /// Helper: confirm a `ValueId` is declared and has the expected type.
    fn check_value_ty(
        &self,
        site: OpSite,
        value: ValueId,
        expected: Ty,
        context: &'static str,
    ) -> Result<(), VerifyError> {
        self.check_declared_value(site, value)?;
        let found = self.module.value_ty(value).clone();
        if found != expected {
            return Err(VerifyError::OutputTypeMismatch {
                site,
                out: value,
                expected,
                found,
                context,
            });
        }
        Ok(())
    }

    /// Helper: confirm a value is declared and is `Bits(_)` (some width).
    fn check_value_is_bits(
        &self,
        site: OpSite,
        value: ValueId,
        context: &'static str,
    ) -> Result<(), VerifyError> {
        self.check_declared_value(site, value)?;
        match self.module.value_ty(value) {
            Ty::Bits(_) => Ok(()),
            other => Err(VerifyError::OutputTypeMismatch {
                site,
                out: value,
                expected: Ty::Bits(1),
                found: other.clone(),
                context,
            }),
        }
    }

    /// Helper: confirm a `ValueId` is in range for the module's arena.
    fn check_declared_value(&self, site: OpSite, value: ValueId) -> Result<(), VerifyError> {
        if value.0 as usize >= self.module.value_count() {
            return Err(VerifyError::UndeclaredValue {
                site,
                value,
                value_count: self.module.value_count(),
            });
        }
        Ok(())
    }

    /// Helper: confirm a `CellId` is in range.
    fn check_declared_cell(&self, site: OpSite, cell: CellId) -> Result<(), VerifyError> {
        if cell.0 as usize >= self.module.cell_count() {
            return Err(VerifyError::UndeclaredCell {
                site,
                cell,
                cell_count: self.module.cell_count(),
            });
        }
        Ok(())
    }

    /// Check 2 (Pure op) — width consistency per op kind.
    fn check_pure_op(
        &self,
        site: OpSite,
        op: PureOp,
        args: &[ValueId],
        out: ValueId,
    ) -> Result<(), VerifyError> {
        for a in args {
            self.check_declared_value(site, *a)?;
        }
        self.check_declared_value(site, out)?;
        match op {
            PureOp::Add | PureOp::Sub | PureOp::Mul | PureOp::Div => {
                // Binary arithmetic: args[0], args[1], out all same width.
                if args.len() != 2 {
                    return Err(VerifyError::OutputTypeMismatch {
                        site,
                        out,
                        expected: Ty::Real,
                        found: Ty::Real,
                        context: "binary arithmetic op needs exactly 2 args",
                    });
                }
                let w0 = self.module.value_ty(args[0]).width();
                let w1 = self.module.value_ty(args[1]).width();
                let wo = self.module.value_ty(out).width();
                if w0 != w1 || w0 != wo {
                    return Err(VerifyError::OutputTypeMismatch {
                        site,
                        out,
                        expected: self.module.value_ty(args[0]).clone(),
                        found: self.module.value_ty(out).clone(),
                        context: "arithmetic op arg/out widths must agree",
                    });
                }
            }
            PureOp::Neg => {
                // Unary: arg and out same width.
                if args.len() != 1 {
                    return Err(VerifyError::OutputTypeMismatch {
                        site,
                        out,
                        expected: Ty::Real,
                        found: Ty::Real,
                        context: "Neg needs exactly 1 arg",
                    });
                }
                let wa = self.module.value_ty(args[0]).width();
                let wo = self.module.value_ty(out).width();
                if wa != wo {
                    return Err(VerifyError::OutputTypeMismatch {
                        site,
                        out,
                        expected: self.module.value_ty(args[0]).clone(),
                        found: self.module.value_ty(out).clone(),
                        context: "Neg arg/out widths must agree",
                    });
                }
            }
            PureOp::Eq | PureOp::Ne | PureOp::Lt | PureOp::Le | PureOp::Gt | PureOp::Ge => {
                // Comparison: two args same width, out is Bits(1).
                if args.len() != 2 {
                    return Err(VerifyError::OutputTypeMismatch {
                        site,
                        out,
                        expected: Ty::Bits(1),
                        found: Ty::Bits(1),
                        context: "comparison op needs exactly 2 args",
                    });
                }
                let w0 = self.module.value_ty(args[0]).width();
                let w1 = self.module.value_ty(args[1]).width();
                if w0 != w1 {
                    return Err(VerifyError::OutputTypeMismatch {
                        site,
                        out,
                        expected: self.module.value_ty(args[0]).clone(),
                        found: self.module.value_ty(args[1]).clone(),
                        context: "comparison op arg widths must agree",
                    });
                }
                let expected = Ty::Bits(1);
                let found = self.module.value_ty(out).clone();
                if found != expected {
                    return Err(VerifyError::OutputTypeMismatch {
                        site,
                        out,
                        expected,
                        found,
                        context: "comparison op output must be Bits(1)",
                    });
                }
            }
            PureOp::Select => {
                // select(cond, then, else): 3 args, out matches then/else.
                if args.len() != 3 {
                    return Err(VerifyError::OutputTypeMismatch {
                        site,
                        out,
                        expected: Ty::Real,
                        found: Ty::Real,
                        context: "Select needs exactly 3 args (cond, then, else)",
                    });
                }
                let w_then = self.module.value_ty(args[1]).width();
                let w_else = self.module.value_ty(args[2]).width();
                if w_then != w_else {
                    return Err(VerifyError::OutputTypeMismatch {
                        site,
                        out,
                        expected: self.module.value_ty(args[1]).clone(),
                        found: self.module.value_ty(args[2]).clone(),
                        context: "Select then/else widths must agree",
                    });
                }
                let expected = self.module.value_ty(args[1]).clone();
                let found = self.module.value_ty(out).clone();
                if expected != found {
                    return Err(VerifyError::OutputTypeMismatch {
                        site,
                        out,
                        expected,
                        found,
                        context: "Select output must match then/else",
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Inline unit tests for the verifier's check helpers. The per-check
    //! rejection fixtures with both malformed and well-formed variants live
    //! in `crates/piperine-ir/tests/verify.rs` (integration tests, MD-28).
    use super::*;

    #[test]
    fn an_empty_module_verifies() {
        let m = Module::new();
        assert!(Verifier::new(&m).run().is_ok());
    }
}
