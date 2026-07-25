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

use std::collections::{HashMap, HashSet};

use crate::pom::{
    is_ground, Design, ElabError, ElabErrorKind, Connection, Instance, Module, NetRef, Wire,
};

use super::passes::ElabPass;
use super::Elaborator;

/// Flatten hierarchy into a leaf-only netlist, memoized in
/// `Design::flat_modules`. Runs as the last elaboration pass (after
/// `Typecheck`) so it sees the fully resolved, monomorphized, for-unrolled
/// module set.
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
                let (insts, wires, conns) = Self::inline(&inst, &child)?;
                kept.extend(insts);
                m.wires.extend(wires);
                m.connections.extend(conns);
            }
        }
        m.instances = kept;
        in_progress.remove(name);
        design.flat_modules.insert(name.to_string(), m.clone());
        Ok(m)
    }

    /// Inline a non-leaf `child` (already flat) into the parent, producing
    /// the spliced leaf instances, lifted wires, and spliced connections that
    /// replace `inst` in the parent's flat form. FLAT-02 / FLAT-03.
    ///
    /// Rename map `ρ` (child-net-name → parent [`NetRef`]):
    /// - child port `i` → `inst.ports[i]` (positional binding; unconnected
    ///   trailing ports are absent from `ρ`, so any internal reference to
    ///   one fails loud in [`Self::remap`]);
    /// - child wire `w` → fresh `"{inst.name()}.{w.name}"` (same discipline),
    ///   with a parent wire emitted under that name.
    ///
    /// Every child sub-instance is relabeled `"{inst.name()}.{s.name()}"`
    /// (collision-free: `inst.name()` is unique among the parent's
    /// instances, and the dotted prefix composes through nesting:
    /// `x.seg0.rc`). Connections and sub-instance port bindings are
    /// rewritten through [`Self::remap`].
    ///
    /// Behaviors and module-level vars are NOT inlined — codegen still
    /// compiles each leaf sub-instance's body by module name. A mid-level
    /// module's own analog/digital body is out of scope for the MVP
    /// (`urc` is pure-structural: a `for` over leaf segments).
    fn inline(
        inst: &Instance,
        child: &Module,
    ) -> Result<(Vec<Instance>, Vec<Wire>, Vec<Connection>), ElabError> {
        let mut rho: HashMap<String, NetRef> = HashMap::new();
        let mut lifted_wires: Vec<Wire> = Vec::new();

        // Child port i → parent NetRef (positional binding).
        for (i, port) in child.ports.iter().enumerate() {
            if let Some(parent_ref) = inst.ports.get(i) {
                rho.insert(port.name.clone(), parent_ref.clone());
            }
        }
        // Child wire → fresh parent wire "{inst.name()}.{wire.name}".
        for w in &child.wires {
            let fresh = format!("{}.{}", inst.name(), w.name);
            rho.insert(w.name.clone(), NetRef::simple(fresh.clone()));
            lifted_wires.push(Wire {
                span: None,
                attributes: w.attributes.clone(),
                doc: w.doc.clone(),
                name: fresh,
                ty: w.ty.clone(),
            });
        }

        // Splice sub-instances through remap, with path-prefixed labels.
        let mut spliced_instances = Vec::with_capacity(child.instances.len());
        for s in &child.instances {
            let new_label = format!("{}.{}", inst.name(), s.name());
            let new_ports = s
                .ports
                .iter()
                .map(|nr| Self::remap(nr, &rho, child))
                .collect::<Result<Vec<_>, _>>()?;
            spliced_instances.push(Instance {
                span: s.span,
                attributes: s.attributes.clone(),
                doc: s.doc.clone(),
                label: Some(new_label),
                module: s.module.clone(),
                ports: new_ports,
                params: s.params.clone(),
            });
        }

        // Splice connections through remap.
        let mut spliced_connections = Vec::with_capacity(child.connections.len());
        for c in &child.connections {
            spliced_connections.push(Connection {
                span: c.span,
                lhs: Self::remap(&c.lhs, &rho, child)?,
                rhs: Self::remap(&c.rhs, &rho, child)?,
            });
        }

        Ok((spliced_instances, lifted_wires, spliced_connections))
    }

    /// Rewrite a [`NetRef`] through the rename map `ρ`. Ground is preserved
    /// verbatim. An indexed reference (`net[i]`) fails loud as gap-3 deferred
    /// (array-net expansion is not built). An unknown net fails loud naming
    /// the net and the owning child module (dangling/typo guard).
    fn remap(nr: &NetRef, rho: &HashMap<String, NetRef>, child: &Module) -> Result<NetRef, ElabError> {
        if is_ground(&nr.net) {
            return Ok(nr.clone());
        }
        match rho.get(&nr.net) {
            Some(target) => {
                if nr.index.is_some() {
                    return Err(ElabError::from(ElabErrorKind::Other(format!(
                        "array-net expansion not yet implemented (gap-3): `{nr}` in module `{}`",
                        child.name
                    ))));
                }
                Ok(target.clone())
            }
            None => Err(ElabError::from(ElabErrorKind::Other(format!(
                "net `{}` in module `{}` is neither a port nor a wire (dangling)",
                nr.net, child.name
            )))),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Driver + inline tests. Every test carries the non-destructive
    //! assertion: `Design::modules` deep-equal before and after the pass.

    use super::*;
    use crate::parse::ast::Direction;
    use crate::pom::{Connection, Instance, Module, NetType, Port, Wire};

    fn elec() -> NetType {
        NetType::Discipline("Electrical".to_string())
    }

    fn port(name: &str) -> Port {
        Port {
            span: None,
            attributes: Vec::new(),
            doc: None,
            direction: Direction::Inout,
            name: name.to_string(),
            ty: elec(),
        }
    }

    fn wire(name: &str) -> Wire {
        Wire {
            span: None,
            attributes: Vec::new(),
            doc: None,
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
            doc: None,
            label: Some(label.to_string()),
            module: module.to_string(),
            ports: Vec::new(),
            params: Vec::new(),
        }
    }

    fn instance_ports(label: &str, module: &str, ports: &[&str]) -> Instance {
        Instance {
            span: None,
            attributes: Vec::new(),
            doc: None,
            label: Some(label.to_string()),
            module: module.to_string(),
            ports: ports.iter().map(|p| NetRef::simple(*p)).collect(),
            params: Vec::new(),
        }
    }

    fn connection(lhs: &str, rhs: &str) -> Connection {
        Connection {
            span: None,
            lhs: NetRef::simple(lhs),
            rhs: NetRef::simple(rhs),
        }
    }

    /// A non-leaf module with the given ports/wires/instances/connections.
    fn module_with(
        name: &str,
        ports: Vec<Port>,
        wires: Vec<Wire>,
        instances: Vec<Instance>,
        connections: Vec<Connection>,
    ) -> Module {
        Module::new(name.to_string(), ports, Vec::new(), wires, instances, connections, Vec::new())
    }

    /// A non-leaf module with the given instances (no ports/wires/conns).
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
    /// assertion.
    fn snapshot_modules(design: &Design) -> String {
        format!("{:?}", design.modules)
    }

    // ── T2 driver tests (leaf, parent-of-leaves, cycle, idempotency) ───────

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

    // ── T3 inline tests (rename map, path-prefixed labels, fail-loud) ──────

    /// A two-segment `Seg` module: ports (p, n), wire (mid), two leaf
    /// instances r1:Resistor(p, mid) and r2:Resistor(mid, n). When inlined
    /// into a parent that connects Seg(.p=a, .n=b), the parent's flat form
    /// gets x.r1(a, x.mid), x.r2(x.mid, b), and a lifted wire x.mid.
    fn seg_module() -> Module {
        module_with(
            "Seg",
            vec![port("p"), port("n")],
            vec![wire("mid")],
            vec![
                instance_ports("r1", "Resistor", &["p", "mid"]),
                instance_ports("r2", "Resistor", &["mid", "n"]),
            ],
            Vec::new(),
        )
    }

    #[test]
    fn inline_binds_ports_and_lifts_wires() {
        let mut design = Design::new();
        design.insert_module("Resistor".to_string(), leaf_module("Resistor"));
        design.insert_module("Seg".to_string(), seg_module());
        design.insert_module(
            "Top".to_string(),
            module_with(
                "Top",
                Vec::new(),
                Vec::new(),
                vec![instance_ports("x", "Seg", &["a", "b"])],
                Vec::new(),
            ),
        );
        let before = snapshot_modules(&design);
        FlattenHierarchy::flatten_design(&mut design).expect("flatten");
        assert_eq!(before, snapshot_modules(&design), "modules untouched (non-destructive)");

        let flat = design.flat_module("Top").expect("Top flat form");
        // The parent instance `x` is gone; its two leaf sub-instances remain.
        let labels: Vec<&str> = flat.instances.iter().map(|i| i.label.as_deref().unwrap()).collect();
        assert_eq!(labels, vec!["x.r1", "x.r2"], "sub-instances relabeled with parent prefix");

        let r1 = flat.instances.iter().find(|i| i.label.as_deref() == Some("x.r1")).expect("x.r1");
        let r2 = flat.instances.iter().find(|i| i.label.as_deref() == Some("x.r2")).expect("x.r2");
        // Port binding: Seg.p → parent net "a", Seg.n → parent net "b".
        assert_eq!(r1.ports[0].net, "a", "x.r1.p binds to parent net a");
        // Child wire lifted to fresh parent wire "x.mid".
        assert_eq!(r1.ports[1].net, "x.mid", "x.r1.n binds to lifted wire x.mid");
        assert_eq!(r2.ports[0].net, "x.mid", "x.r2.p binds to lifted wire x.mid");
        assert_eq!(r2.ports[1].net, "b", "x.r2.n binds to parent net b");

        // The lifted wire exists with the original discipline.
        let lifted = flat.wires.iter().find(|w| w.name == "x.mid").expect("x.mid wire lifted");
        assert_eq!(lifted.ty, elec(), "lifted wire keeps the child's discipline");
    }

    #[test]
    fn inline_two_siblings_produce_distinct_labels_and_wires() {
        let mut design = Design::new();
        design.insert_module("Resistor".to_string(), leaf_module("Resistor"));
        design.insert_module("Seg".to_string(), seg_module());
        design.insert_module(
            "Top".to_string(),
            module_with(
                "Top",
                Vec::new(),
                Vec::new(),
                vec![
                    instance_ports("x", "Seg", &["a", "b"]),
                    instance_ports("y", "Seg", &["b", "c"]),
                ],
                Vec::new(),
            ),
        );
        let before = snapshot_modules(&design);
        FlattenHierarchy::flatten_design(&mut design).expect("flatten");
        assert_eq!(before, snapshot_modules(&design), "modules untouched (non-destructive)");

        let flat = design.flat_module("Top").expect("Top flat form");
        let labels: Vec<&str> = flat.instances.iter().map(|i| i.label.as_deref().unwrap()).collect();
        assert_eq!(
            labels,
            vec!["x.r1", "x.r2", "y.r1", "y.r2"],
            "two siblings produce collision-free x.*/y.* labels"
        );
        let wire_names: Vec<&str> = flat.wires.iter().map(|w| w.name.as_str()).collect();
        assert!(wire_names.contains(&"x.mid"), "x.mid lifted");
        assert!(wire_names.contains(&"y.mid"), "y.mid lifted — distinct from x.mid");
        assert_eq!(flat.wires.len(), 2, "exactly two lifted wires, no collisions");
    }

    #[test]
    fn inline_nesting_composes_through_three_levels() {
        // Top → Mid → Seg → Resistor. The leaf instance label composes:
        // "seg0.rc.r1".
        let mut design = Design::new();
        design.insert_module("Resistor".to_string(), leaf_module("Resistor"));
        design.insert_module("Seg".to_string(), seg_module());
        // Mid instantiates Seg as `rc`, exposing Mid's ports straight through.
        design.insert_module(
            "Mid".to_string(),
            module_with(
                "Mid",
                vec![port("p"), port("n")],
                Vec::new(),
                vec![instance_ports("rc", "Seg", &["p", "n"])],
                Vec::new(),
            ),
        );
        design.insert_module(
            "Top".to_string(),
            module_with(
                "Top",
                Vec::new(),
                Vec::new(),
                vec![instance_ports("seg0", "Mid", &["a", "b"])],
                Vec::new(),
            ),
        );
        let before = snapshot_modules(&design);
        FlattenHierarchy::flatten_design(&mut design).expect("flatten");
        assert_eq!(before, snapshot_modules(&design), "modules untouched (non-destructive)");

        let flat = design.flat_module("Top").expect("Top flat form");
        let labels: Vec<&str> = flat.instances.iter().map(|i| i.label.as_deref().unwrap()).collect();
        assert_eq!(
            labels,
            vec!["seg0.rc.r1", "seg0.rc.r2"],
            "label nesting composes through three levels"
        );
        // The doubly-lifted wire name composes too.
        assert!(
            flat.wires.iter().any(|w| w.name == "seg0.rc.mid"),
            "lifted wire name nests: seg0.rc.mid"
        );
    }

    #[test]
    fn inline_splices_child_connections_through_remap() {
        // Seg with an explicit connection mid ↔ p (legal-ish for the test).
        let seg = module_with(
            "Seg",
            vec![port("p"), port("n")],
            vec![wire("mid")],
            vec![instance_ports("r1", "Resistor", &["p", "mid"])],
            vec![connection("mid", "n")],
        );
        let mut design = Design::new();
        design.insert_module("Resistor".to_string(), leaf_module("Resistor"));
        design.insert_module("Seg".to_string(), seg);
        design.insert_module(
            "Top".to_string(),
            module_with(
                "Top",
                Vec::new(),
                Vec::new(),
                vec![instance_ports("x", "Seg", &["a", "b"])],
                Vec::new(),
            ),
        );
        let before = snapshot_modules(&design);
        FlattenHierarchy::flatten_design(&mut design).expect("flatten");
        assert_eq!(before, snapshot_modules(&design), "modules untouched (non-destructive)");

        let flat = design.flat_module("Top").expect("Top flat form");
        // Connection `mid ↔ n` splices through ρ: mid → x.mid, n → b.
        assert_eq!(flat.connections.len(), 1, "one spliced connection");
        let c = &flat.connections[0];
        assert_eq!(c.lhs.net, "x.mid", "connection lhs remapped to lifted wire");
        assert_eq!(c.rhs.net, "b", "connection rhs remapped to parent net");
    }

    #[test]
    fn inline_ground_nets_pass_through_remap_unchanged() {
        // A port bound to ground: remap must preserve the ground alias
        // verbatim, never invent a lifted wire for it.
        let seg = module_with(
            "Seg",
            vec![port("p"), port("n")],
            Vec::new(),
            vec![instance_ports("r1", "Resistor", &["p", "n"])],
            Vec::new(),
        );
        let mut design = Design::new();
        design.insert_module("Resistor".to_string(), leaf_module("Resistor"));
        design.insert_module("Seg".to_string(), seg);
        design.insert_module(
            "Top".to_string(),
            module_with(
                "Top",
                Vec::new(),
                Vec::new(),
                vec![instance_ports("x", "Seg", &["a", "gnd"])],
                Vec::new(),
            ),
        );
        let before = snapshot_modules(&design);
        FlattenHierarchy::flatten_design(&mut design).expect("flatten");
        assert_eq!(before, snapshot_modules(&design), "modules untouched (non-destructive)");
        let flat = design.flat_module("Top").expect("Top flat form");
        let r1 = flat.instances.iter().find(|i| i.label.as_deref() == Some("x.r1")).expect("x.r1");
        assert_eq!(r1.ports[1].net, "gnd", "ground preserved verbatim by remap");
        assert!(r1.ports[1].index.is_none(), "ground not indexed");
    }

    #[test]
    fn inline_dangling_net_fails_loud() {
        // A child sub-instance references a net that is neither a port nor a
        // wire of the child — a typo or a missing declaration. Flatten must
        // fail loud naming the net and the child.
        let seg = module_with(
            "Seg",
            vec![port("p"), port("n")],
            Vec::new(),
            vec![instance_ports("r1", "Resistor", &["p", "missing"])],
            Vec::new(),
        );
        let mut design = Design::new();
        design.insert_module("Resistor".to_string(), leaf_module("Resistor"));
        design.insert_module("Seg".to_string(), seg);
        design.insert_module(
            "Top".to_string(),
            module_with(
                "Top",
                Vec::new(),
                Vec::new(),
                vec![instance_ports("x", "Seg", &["a", "b"])],
                Vec::new(),
            ),
        );
        let before = snapshot_modules(&design);
        let err = FlattenHierarchy::flatten_design(&mut design)
            .err()
            .expect("dangling net must fail loud");
        let msg = err.to_string();
        assert!(msg.contains("missing"), "error names the dangling net: {msg}");
        assert!(msg.contains("Seg"), "error names the child module: {msg}");
        assert_eq!(before, snapshot_modules(&design), "modules untouched even on failure");
    }

    #[test]
    fn inline_indexed_netref_fails_as_gap3_deferred() {
        // An indexed NetRef (bus[0]) — array-net expansion is gap-3 deferred.
        // remap must fail loud rather than silently dropping the index.
        let seg = module_with(
            "Seg",
            vec![port("p")],
            vec![wire("bus")],
            vec![Instance {
                span: None,
                attributes: Vec::new(),
                doc: None,
                label: Some("r1".to_string()),
                module: "Resistor".to_string(),
                ports: vec![NetRef::indexed("bus", 0), NetRef::simple("p")],
                params: Vec::new(),
            }],
            Vec::new(),
        );
        let mut design = Design::new();
        design.insert_module("Resistor".to_string(), leaf_module("Resistor"));
        design.insert_module("Seg".to_string(), seg);
        design.insert_module(
            "Top".to_string(),
            module_with(
                "Top",
                Vec::new(),
                Vec::new(),
                vec![instance_ports("x", "Seg", &["a"])],
                Vec::new(),
            ),
        );
        let before = snapshot_modules(&design);
        let err = FlattenHierarchy::flatten_design(&mut design)
            .err()
            .expect("indexed netref must fail loud (gap-3)");
        let msg = err.to_string();
        assert!(
            msg.contains("array-net expansion") && msg.contains("gap-3"),
            "error names gap-3 deferral: {msg}"
        );
        assert!(msg.contains("bus"), "error names the indexed net: {msg}");
        assert_eq!(before, snapshot_modules(&design), "modules untouched even on failure");
    }

    #[test]
    fn inline_yields_only_leaf_instances_in_flat_form() {
        // Replaces the T2 stub test: a non-leaf child is now spliced (not
        // dropped), and the resulting flat form contains only leaf
        // instances — every sub-instance resolves to a leaf module.
        let mut design = Design::new();
        design.insert_module("Resistor".to_string(), leaf_module("Resistor"));
        design.insert_module("Seg".to_string(), seg_module());
        design.insert_module(
            "Top".to_string(),
            module_with(
                "Top",
                Vec::new(),
                Vec::new(),
                vec![
                    instance_ports("direct", "Resistor", &["a", "b"]),
                    instance_ports("nested", "Seg", &["a", "b"]),
                ],
                Vec::new(),
            ),
        );
        let before = snapshot_modules(&design);
        FlattenHierarchy::flatten_design(&mut design).expect("flatten");
        assert_eq!(before, snapshot_modules(&design), "modules untouched (non-destructive)");
        let flat = design.flat_module("Top").expect("Top flat form");
        // 1 direct leaf + 2 spliced leaves from Seg = 3 leaf instances.
        assert_eq!(flat.instances.len(), 3, "1 direct + 2 spliced leaves");
        assert!(
            flat.instances.iter().all(|i| i.module == "Resistor"),
            "flat form contains only leaf instances"
        );
    }
}
