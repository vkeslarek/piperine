//! The proof ledger guards (CC-12 AC 5): every proof of every feature folder and every row of the
//! Book's inventory names a test that exists, every retired check is recorded, and the parity
//! oracles are not reduced. Rows that were dangling when the ledger arrived are listed in
//! `allowances/proof_ledger.tsv`; the guard fails on a dangling row that is not listed and on a
//! listed row that is not dangling any more.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use piperine_repository_guards::{DanglingReference, ProofLedger};

/// The ledger of this repository, read once for every test of the target.
static LEDGER: OnceLock<ProofLedger> = OnceLock::new();

/// Reads the files of the guard crate and judges dangling references against them.
struct LedgerGuard;

impl LedgerGuard {
    /// The shared ledger.
    fn ledger() -> &'static ProofLedger {
        LEDGER.get_or_init(ProofLedger::of_workspace)
    }

    /// The text of a file of the guards crate.
    fn crate_file(relative: &str) -> String {
        let path = Self::ledger().root().join("crates/piperine-repository-guards").join(relative);
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    /// The rows of `allowances/proof_ledger.tsv`.
    fn allowed_rows() -> BTreeSet<String> {
        Self::crate_file("allowances/proof_ledger.tsv").lines().filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_string).collect()
    }

    /// Fails on a dangling reference that is not listed, and on a listed row that is not dangling
    /// among the `owners` this test judges.
    fn judge(found: &[DanglingReference], owner_is_judged: impl Fn(&str) -> bool) {
        let found_rows: BTreeSet<String> = found.iter().map(DanglingReference::row).collect();
        let allowed = Self::allowed_rows();
        let unlisted: Vec<&String> = found_rows.difference(&allowed).collect();
        let stale: Vec<&String> = allowed.difference(&found_rows).filter(|row| owner_is_judged(row.split('\t').next().unwrap_or(""))).collect();
        assert!(unlisted.is_empty(), "dangling and not listed in allowances/proof_ledger.tsv:\n{}", unlisted.iter().map(|r| r.as_str()).collect::<Vec<_>>().join("\n"));
        assert!(stale.is_empty(), "listed in allowances/proof_ledger.tsv but no longer dangling (delete the rows):\n{}", stale.iter().map(|r| r.as_str()).collect::<Vec<_>>().join("\n"));
    }
}

#[test]
fn every_proof_names_a_test_that_exists() {
    LedgerGuard::judge(&LedgerGuard::ledger().dangling_proofs(), |owner| owner != "tests/book/inventory.tsv");
}

#[test]
fn every_inventory_row_names_a_test_that_exists() {
    LedgerGuard::judge(&LedgerGuard::ledger().dangling_inventory_rows(), |owner| owner == "tests/book/inventory.tsv");
}

#[test]
fn every_retired_check_is_recorded() {
    let marked: BTreeSet<(String, String)> = LedgerGuard::ledger()
        .features()
        .iter()
        .flat_map(|file| file.checks().into_iter().filter(|check| check.claim.contains("(retired by code-clarity ")).map(|check| (file.feature().to_string(), check.id)).collect::<Vec<_>>())
        .collect();
    let recorded: BTreeSet<(String, String)> = LedgerGuard::crate_file("../../.specs/features/code-clarity/retired.tsv")
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let cells: Vec<&str> = l.split('\t').collect();
            Some((cells.first()?.to_string(), cells.get(1)?.to_string()))
        })
        .collect();
    assert_eq!(marked.difference(&recorded).collect::<Vec<_>>(), Vec::<&(String, String)>::new(), "a check says it was retired and has no row in retired.tsv");
    assert_eq!(recorded.difference(&marked).collect::<Vec<_>>(), Vec::<&(String, String)>::new(), "a row of retired.tsv names a check that does not say it was retired");
}

#[test]
fn the_parity_oracles_are_not_reduced() {
    let ledger = LedgerGuard::ledger();
    let missing: Vec<String> = LedgerGuard::crate_file("data/parity_oracles.tsv").lines().filter(|l| !l.is_empty() && !l.starts_with('#')).filter(|row| !ledger.oracle_exists(row)).map(str::to_string).collect();
    assert!(missing.is_empty(), "parity oracles that no longer exist as tests:\n{}", missing.join("\n"));
}
