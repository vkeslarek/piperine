//! The plan of targets: each crate holds no more integration-test targets than `data/target_plan.tsv`
//! lets it, and the tasks that merge targets lower the plan.

use piperine_repository_guards::{SourceTree, SuiteCensus};

/// Reads the plan.
struct TargetPlan;

impl TargetPlan {
    /// The `(crate, targets)` rows of `data/target_plan.tsv`.
    fn rows() -> Vec<(String, usize)> {
        let text = std::fs::read_to_string(SourceTree::guards_directory().join("data/target_plan.tsv")).expect("data/target_plan.tsv");
        text.lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
            .filter_map(|line| line.split_once('\t').and_then(|(name, count)| Some((name.to_string(), count.trim().parse().ok()?))))
            .collect()
    }
}

#[test]
fn no_crate_has_more_targets_than_its_plan() {
    let census = SuiteCensus::take();
    let plan = TargetPlan::rows();
    let mut over = Vec::new();
    for crate_census in census.crates() {
        match plan.iter().find(|(name, _)| *name == crate_census.crate_name) {
            Some((_, allowed)) if crate_census.targets > *allowed => over.push(format!("{}: {} targets, the plan allows {allowed}", crate_census.crate_name, crate_census.targets)),
            None if crate_census.targets > 0 => over.push(format!("{} has no row in data/target_plan.tsv", crate_census.crate_name)),
            _ => {}
        }
    }
    assert!(over.is_empty(), "targets over the plan (merge targets, never raise the plan):\n{}", over.join("\n"));
}
