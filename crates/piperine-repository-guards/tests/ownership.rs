//! The ownership guards (CC-04, CC-07 in part): a function belongs to a type, an `impl` lives with its
//! type, no `#[allow]` hides a design problem, and every owner the design names exists with its
//! methods. Each guard is a ratchet over `allowances/<crate>/<guard>.tsv`: it fails on a violation
//! the allowance does not list and on a listed row that no longer exists.

use std::sync::OnceLock;

use piperine_repository_guards::{Findings, ItemScan, OwnershipRules, SourceTree};

/// The scan of the workspace, taken once for the guards of this target.
static SCAN: OnceLock<ItemScan> = OnceLock::new();

/// Runs the rules of this target over the shared scan.
struct OwnershipGuards;

impl OwnershipGuards {
    /// The shared scan.
    fn scan() -> &'static ItemScan {
        SCAN.get_or_init(|| ItemScan::of(&SourceTree::of_workspace()))
    }

    /// Fails when the findings differ from the guard's allowances.
    fn assert_clean(guard: &str, findings: Findings) {
        findings.judge(guard).assert_clean(guard);
    }
}

#[test]
fn no_free_function_outside_the_language_exceptions() {
    OwnershipGuards::assert_clean("free_functions", OwnershipRules::free_functions(OwnershipGuards::scan()));
}

#[test]
fn no_free_helper_function_in_test_code() {
    OwnershipGuards::assert_clean("test_helper_functions", OwnershipRules::test_helper_functions(OwnershipGuards::scan()));
}

#[test]
fn an_inherent_impl_lives_with_its_type() {
    OwnershipGuards::assert_clean("impls_away_from_their_type", OwnershipRules::impls_away_from_their_type(OwnershipGuards::scan()));
}

#[test]
fn every_owner_of_the_design_exists_and_has_its_methods() {
    let owners = std::fs::read_to_string(SourceTree::guards_directory().join("data/owners.tsv")).expect("data/owners.tsv");
    OwnershipGuards::assert_clean("owners", OwnershipRules::missing_owners(OwnershipGuards::scan(), &owners));
}

#[test]
fn no_allow_hides_a_design_problem() {
    OwnershipGuards::assert_clean("allow_attributes", OwnershipRules::allow_attributes(OwnershipGuards::scan()));
}
