//! The ratchets over test code (CC-12 AC 4): no support file is shared by `#[path]`, and no `#[allow]`
//! hides a design problem of a test. Each is a ratchet over `allowances/<crate>/<guard>.tsv`.

use piperine_repository_guards::{ItemScan, OwnershipRules, SourceTree};

#[test]
fn no_test_support_is_shared_by_path() {
    let findings = OwnershipRules::test_support_shared_by_path(&SourceTree::of_workspace());
    findings.judge("test_support_shared_by_path").assert_clean("test_support_shared_by_path");
}

#[test]
fn no_allow_in_test_code() {
    let findings = OwnershipRules::allow_in_test_code(&ItemScan::of(&SourceTree::of_workspace()));
    findings.judge("allow_in_test_code").assert_clean("allow_in_test_code");
}
