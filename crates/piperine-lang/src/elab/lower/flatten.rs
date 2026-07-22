//! Hierarchy flattening pass — produces the leaf-only flat netlist consumed
//! by codegen. The authored hierarchy (`Design::modules`) is **never**
//! mutated; the pass writes only to `Design::flat_modules` (a memoized side
//! map). POM navigability mirrors the source.
//!
//! Algorithm: memoized bottom-up. For each module, clone the authored form,
//! recurse into every child (so each child is itself flat), keep leaf
//! instances as-is, and inline non-leaf children's sub-instances through a
//! net-rename map (the inline body lives in [`FlattenHierarchy::inline`]).
//!
//! See `.specs/features/hierarchy-flattening/` for the design.

use std::collections::HashSet;

use crate::pom::{Design, ElabError, ElabErrorKind, Module};

use super::passes::ElabPass;
use super::Elaborator;

/// Flatten hierarchy into a leaf-only netlist, memoized in
/// `Design::flat_modules`. Runs as the last elaboration pass (after
/// `Typecheck`) so it sees the fully resolved, monomorphized, for-unrolled
/// module set.
#[allow(dead_code)] // wired into PASSES in T4
pub(super) struct FlattenHierarchy;

impl ElabPass for FlattenHierarchy {
    fn run(&self, _elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        Self::flatten_design(design)
    }
}

impl FlattenHierarchy {
    /// Flatten every module in `design.modules` into `design.flat_modules`.
    /// The authored `modules` map is never mutated — only `flat_modules` is
    /// written. Entry point for the pass driver and for unit tests.
    #[allow(dead_code)] // wired into PASSES in T4
    pub(super) fn flatten_design(design: &mut Design) -> Result<(), ElabError> {
        let names: Vec<String> = design.modules.keys().cloned().collect();
        let mut in_progress = HashSet::new();
        for name in &names {
            Self::flatten_module(name, design, &mut in_progress)?;
        }
        Ok(())
    }

    /// True when `m` is a leaf — no sub-instances. The recursion base case:
    /// a leaf module's flat form equals its authored form.
    #[allow(dead_code)] // wired into PASSES in T4
    fn is_leaf(m: &Module) -> bool {
        m.instances.is_empty()
    }

    /// Flatten one module: clone the authored form (never mutate), recurse
    /// into each child so the child is itself flat, keep leaf instances
    /// as-is, inline non-leaf children via [`Self::inline`]. The result is
    /// memoized in `design.flat_modules` and returned.
    ///
    /// `in_progress` is the recursion stack — a module appearing in it is a
    /// recursive cycle and fails loud.
    #[allow(dead_code)] // wired into PASSES in T4
    fn flatten_module(
        name: &str,
        design: &mut Design,
        in_progress: &mut HashSet<String>,
    ) -> Result<Module, ElabError> {
        if let Some(flat) = design.flat_modules.get(name) {
            return Ok(flat.clone());
        }
        if in_progress.contains(name) {
            return Err(ElabError::from(ElabErrorKind::Other(format!(
                "recursive module instantiation: `{name}` appears in its own instantiation chain"
            ))));
        }
        let authored = design.modules.get(name).ok_or_else(|| {
            ElabError::from(ElabErrorKind::Other(format!(
                "flatten: unknown module `{name}`"
            )))
        })?;
        in_progress.insert(name.to_string());
        let mut m = authored.clone();
        // Take ownership of the instances up front: the loop recurses into
        // flatten_module (which mutates design.flat_modules) and calls inline
        // (which mutates m.wires/m.connections) — both would conflict with a
        // live borrow of m.instances.
        let authored_instances = std::mem::take(&mut m.instances);
        let mut kept = Vec::new();
        for inst in authored_instances {
            let child = Self::flatten_module(&inst.module, design, in_progress)?;
            if Self::is_leaf(&child) {
                kept.push(inst);
            } else {
                Self::inline(&inst, &child, &mut m)?;
            }
        }
        m.instances = kept;
        in_progress.remove(name);
        design.flat_modules.insert(name.to_string(), m.clone());
        Ok(m)
    }

    /// Inline a non-leaf child's contents into the parent: build the rename
    /// map (child port → parent net, child wire → fresh parent wire), splice
    /// the child's leaf sub-instances and connections through the remap, and
    /// drop the now-inlined `inst`. FLAT-02 / FLAT-03.
    ///
    /// Stubbed in T2 — a non-leaf child is dropped from the flat form without
    /// splicing. T3 fills in the real net remapping.
    fn inline(_inst: &crate::pom::Instance, _child: &Module, _parent: &mut Module) -> Result<(), ElabError> {
        // T2: drop the non-leaf instance (the driver's structure is the test
        // target here). T3 replaces this body with the rename-map splice.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Driver tests — the inline body is stubbed (T2 scope); T3 tests the
    //! real net remapping. Every test carries the non-destructive assertion:
    //! `Design::modules` deep-equal before and after the pass runs.

    use super::*;
    use crate::parse::ast::Direction;
    use crate::pom::{Instance, Module, NetType, Port, Wire};

    fn elec() -> NetType {
        NetType::Discipline("Electrical".to_string())
    }

    fn port(name: &str) -> Port {
        Port {
            span: None,
            attributes: Vec::new(),
            direction: Direction::Inout,
            name: name.to_string(),
            ty: elec(),
        }
    }

    fn wire(name: &str) -> Wire {
        Wire {
            span: None,
            attributes: Vec::new(),
            name: name.to_string(),
            ty: elec(),
        }
    }

    fn leaf_module(name: &str) -> Module {
        Module::new(
            name.to_string(),
            vec![port("p"), port("n")],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    fn instance(label: &str, module: &str) -> Instance {
        Instance {
            span: None,
            attributes: Vec::new(),
            label: Some(label.to_string()),
            module: module.to_string(),
            ports: Vec::new(),
            params: Vec::new(),
        }
    }

    /// A non-leaf module with the given instances (no own behavior — the
    /// driver does not inspect behaviors).
    fn parent_module(name: &str, instances: Vec<Instance>) -> Module {
        Module::new(
            name.to_string(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            instances,
            Vec::new(),
            Vec::new(),
        )
    }

    /// Deep-equality snapshot of `design.modules` for the non-destructive
    /// assertion. Compares the authored map before and after the pass.
    fn snapshot_modules(design: &Design) -> String {
        format!("{:?}", design.modules)
    }

    #[test]
    fn flatten_leaf_module_equals_authored() {
        let mut design = Design::new();
        design.insert_module("Resistor".to_string(), leaf_module("Resistor"));
        let before = snapshot_modules(&design);
        FlattenHierarchy::flatten_design(&mut design).expect("flatten");
        assert_eq!(before, snapshot_modules(&design), "modules untouched (non-destructive)");
        let flat = design.flat_module("Resistor").expect("Resistor flat form");
        let auth = design.module("Resistor").expect("Resistor authored");
        assert_eq!(
            format!("{flat:?}"),
            format!("{auth:?}"),
            "a leaf module's flat form equals its authored form"
        );
    }

    #[test]
    fn flatten_parent_of_leaves_keeps_all_instances() {
        let mut design = Design::new();
        design.insert_module("Resistor".to_string(), leaf_module("Resistor"));
        design.insert_module(
            "Divider".to_string(),
            parent_module("Divider", vec![instance("r1", "Resistor"), instance("r2", "Resistor")]),
        );
        let before = snapshot_modules(&design);
        FlattenHierarchy::flatten_design(&mut design).expect("flatten");
        assert_eq!(before, snapshot_modules(&design), "modules untouched (non-destructive)");
        let flat = design.flat_module("Divider").expect("Divider flat form");
        assert_eq!(flat.instances.len(), 2, "both leaf instances kept");
        assert!(flat.instances.iter().all(|i| i.module == "Resistor"), "all flat instances are leaves");
    }

    #[test]
    fn flatten_drops_non_leaf_instances_in_stub() {
        // T2 scope: the inline body is stubbed — a non-leaf child is dropped
        // from the flat form rather than spliced. T3 replaces the stub with
        // the real rename-map splice. This test pins the stub behavior so
        // T3's change is visible.
        let mut design = Design::new();
        design.insert_module("Resistor".to_string(), leaf_module("Resistor"));
        design.insert_module(
            "Ladder".to_string(),
            parent_module("Ladder", vec![instance("r1", "Resistor")]),
        );
        design.insert_module(
            "Top".to_string(),
            parent_module(
                "Top",
                vec![
                    instance("kept", "Resistor"),
                    instance("dropped", "Ladder"),
                ],
            ),
        );
        let before = snapshot_modules(&design);
        FlattenHierarchy::flatten_design(&mut design).expect("flatten");
        assert_eq!(before, snapshot_modules(&design), "modules untouched (non-destructive)");
        let flat = design.flat_module("Top").expect("Top flat form");
        assert_eq!(flat.instances.len(), 1, "non-leaf child dropped by stub inline");
        assert_eq!(flat.instances[0].label.as_deref(), Some("kept"));
    }

    #[test]
    fn flatten_detects_self_cycle() {
        let mut design = Design::new();
        design.insert_module(
            "Loop".to_string(),
            parent_module("Loop", vec![instance("self", "Loop")]),
        );
        let before = snapshot_modules(&design);
        let err = FlattenHierarchy::flatten_design(&mut design)
            .err()
            .expect("self-cycle must fail loud");
        let msg = err.to_string();
        assert!(
            msg.contains("recursive module instantiation") && msg.contains("Loop"),
            "cycle error names the module: {msg}"
        );
        assert_eq!(before, snapshot_modules(&design), "modules untouched even on failure");
    }

    #[test]
    fn flatten_detects_transitive_cycle() {
        let mut design = Design::new();
        design.insert_module(
            "Alpha".to_string(),
            parent_module("Alpha", vec![instance("b", "Beta")]),
        );
        design.insert_module(
            "Beta".to_string(),
            parent_module("Beta", vec![instance("a", "Alpha")]),
        );
        let before = snapshot_modules(&design);
        let err = FlattenHierarchy::flatten_design(&mut design)
            .err()
            .expect("transitive cycle must fail loud");
        let msg = err.to_string();
        assert!(
            msg.contains("recursive module instantiation"),
            "cycle error is a recursion diagnostic: {msg}"
        );
        assert!(
            msg.contains("Alpha") || msg.contains("Beta"),
            "cycle error names a participant: {msg}"
        );
        assert_eq!(before, snapshot_modules(&design), "modules untouched even on failure");
    }

    #[test]
    fn flatten_is_idempotent() {
        // Running the pass twice yields the same flat_modules map. Memoization
        // short-circuits the second run.
        let mut design = Design::new();
        design.insert_module("Resistor".to_string(), leaf_module("Resistor"));
        design.insert_module(
            "Divider".to_string(),
            parent_module("Divider", vec![instance("r1", "Resistor")]),
        );
        FlattenHierarchy::flatten_design(&mut design).expect("first flatten");
        let after_first = format!("{:?}", design.flat_modules);
        FlattenHierarchy::flatten_design(&mut design).expect("second flatten");
        let after_second = format!("{:?}", design.flat_modules);
        assert_eq!(after_first, after_second, "flatten is memoized / idempotent");
    }

    #[test]
    fn flatten_writes_only_flat_modules() {
        // Sanity: every module appears in flat_modules after the pass.
        let mut design = Design::new();
        design.insert_module("Resistor".to_string(), leaf_module("Resistor"));
        design.insert_module(
            "Divider".to_string(),
            parent_module("Divider", vec![instance("r1", "Resistor")]),
        );
        FlattenHierarchy::flatten_design(&mut design).expect("flatten");
        assert!(design.flat_modules.contains_key("Resistor"), "Resistor flattened");
        assert!(design.flat_modules.contains_key("Divider"), "Divider flattened");
    }
}
