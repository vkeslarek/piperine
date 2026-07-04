//! [`BenchRunner`] — discovers and runs `bench` entry points (piperine-bench/docs/SPEC.md
//! §2/§9): each gets a freshly forked [`Design`] and [`SimHost`], so staged
//! overrides never leak between entry points.

use piperine_lang::eval::{EvalError, Interpreter};
use piperine_lang::Design;

use crate::host::SimHost;
use crate::session::SimSession;

/// The outcome of one entry-point run.
#[derive(Debug)]
pub enum BenchOutcome {
    /// Ran to completion; every `$assert` (if any) held.
    Passed,
    /// An `$assert`/`$error`/`$fatal` raised (piperine-bench/docs/SPEC.md §8 — a test's
    /// contract is its asserts).
    Failed(String),
    /// Anything else went wrong: an undefined name, a type mismatch, a
    /// solver/elaboration error.
    Error(String),
}

/// One `(bench module, fn name, outcome)` record.
pub struct BenchResult {
    pub module: String,
    pub entry: String,
    pub outcome: BenchOutcome,
}

pub struct BenchReport {
    pub results: Vec<BenchResult>,
}

impl BenchReport {
    pub fn all_passed(&self) -> bool {
        self.results.iter().all(|r| matches!(r.outcome, BenchOutcome::Passed))
    }
}

pub struct BenchRunner<'d> {
    design: &'d Design,
}

impl<'d> BenchRunner<'d> {
    pub fn new(design: &'d Design) -> Self {
        Self { design }
    }

    /// Run every entry point in every `bench` block.
    pub fn run_all(&self) -> BenchReport {
        let mut results = Vec::new();
        for bench in self.design.benches() {
            for entry in bench.entry_points() {
                let outcome = self.run_entry(bench.module(), &entry.sig.name);
                results.push(BenchResult { module: bench.module().to_string(), entry: entry.sig.name.clone(), outcome });
            }
        }
        BenchReport { results }
    }

    /// Run a single named entry point (`piperine test <module>::<fn>`-style
    /// selection), against a fresh fork (piperine-bench/docs/SPEC.md §9 isolation).
    pub fn run_entry(&self, module: &str, entry: &str) -> BenchOutcome {
        let Some(bench) = self.design.bench(module) else {
            return BenchOutcome::Error(format!("no bench attached to module `{module}`"));
        };
        let Some(f) = bench.fn_by_name(entry) else {
            return BenchOutcome::Error(format!("bench `{module}` has no fn `{entry}`"));
        };

        let session = SimSession::new(self.design.fork(), module.to_string());
        let mut host = SimHost::new(session);
        let mut interp = Interpreter::new(&mut host);
        match interp.call_fn_decl(f, vec![]) {
            Ok(_) => BenchOutcome::Passed,
            Err(EvalError::AssertFailed(msg)) => BenchOutcome::Failed(msg),
            Err(EvalError::Fatal(msg)) => BenchOutcome::Failed(msg),
            Err(other) => BenchOutcome::Error(other.to_string()),
        }
    }
}
