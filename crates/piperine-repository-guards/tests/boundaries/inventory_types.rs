//! The types the inventory of code-clarity found written in two crates (findings.md A4 and A6),
//! each with the one crate that owns it: a type of that name anywhere else is a finding.

use piperine_repository_guards::{CodeOrigin, Findings, ItemScan, ScannedKind, SourceTree};

/// Each type name of the inventory and the crate that owns it.
const OWNERS: [(&str, &str); 3] = [
    ("Backend", "piperine-api"),
    ("LogicPattern", "piperine-api"),
    ("DynamicInteger", "piperine-numbers"),
];

/// Reads where the types of the inventory are declared.
struct InventoryTypes;

impl InventoryTypes {
    /// One finding per declaration of an inventory type outside its owner crate.
    fn outside_their_owner(scan: &ItemScan) -> Findings {
        let mut findings = Findings::new();
        for item in scan.of_kind(ScannedKind::Type).filter(|item| item.origin == CodeOrigin::Product) {
            let owner = OWNERS.iter().find(|(name, _)| *name == item.name).map(|(_, owner)| *owner);
            if owner.is_some_and(|owner| owner != item.crate_name) {
                findings.add(&item.crate_name, format!("{}::{}", item.file.display(), item.name));
            }
        }
        findings
    }
}

#[test]
fn no_type_of_the_inventory_is_defined_in_two_crates() {
    let scan = ItemScan::of(&SourceTree::of_workspace());
    InventoryTypes::outside_their_owner(&scan).judge("inventory_types_in_two_crates").assert_clean("inventory_types_in_two_crates");
}
