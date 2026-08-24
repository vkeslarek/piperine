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
    is_ground, Behavior, Branch, Design, ElabError, ElabErrorKind, Connection, Instance, Module,
    NetRef, NetType, Wire,
};
use crate::value::Value;

use super::passes::ElabPass;
use super::Elaborator;

/// The result of inlining a non-leaf child: its spliced leaf instances,
/// lifted wires, and rewritten connections.
type InlineResult = (Vec<Instance>, Vec<Wire>, Vec<Connection>);

/// The four generate-produced derived-content maps (T25's instances, T51's
/// wires/behaviors/branches) — bundled so `flatten_design`/`flatten_module`
/// take one argument instead of four, all keyed by owning-module name, all
/// MD-25-derived-only (never merged into `Design::modules`). Owned (not
/// borrowed) so `Default` (all-empty) is what every flatten test not
/// exercising generate passes — cloned once from `Elaborator::generated_*`
/// by `FlattenHierarchy::run`, cheap relative to elaboration as a whole.
#[derive(Default)]
pub(super) struct GeneratedContent {
    pub instances: HashMap<String, Vec<Instance>>,
    pub wires: HashMap<String, Vec<Wire>>,
    pub behaviors: HashMap<String, Vec<Behavior>>,
    pub branches: HashMap<String, Vec<Branch>>,
}

/// Flatten hierarchy into a leaf-only netlist, memoized in
/// `Design::flat_modules`. Runs as the last elaboration pass (after
/// `Typecheck`) so it sees the fully resolved, monomorphized, for-unrolled
/// module set.
pub(super) struct FlattenHierarchy;

impl ElabPass for FlattenHierarchy {
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        let generated = GeneratedContent {
            instances: elab.generated_instances.clone(),
            wires: elab.generated_wires.clone(),
            behaviors: elab.generated_behaviors.clone(),
            branches: elab.generated_branches.clone(),
        };
        Self::flatten_design(design, &generated)
    }
}

impl FlattenHierarchy {
    /// Flatten every module in `design.modules` into `design.flat_modules`.
    /// The authored `modules` map is never mutated — only `flat_modules` is
    /// written. `generated` is T25/T51's per-module generate-produced
    /// content (MD-25: merged into the derived form only, never into
    /// `design.modules`). Entry point for the pass driver and for unit
    /// tests (which pass `&Default::default()` when generate isn't under
    /// test).
    pub(super) fn flatten_design(
        design: &mut Design,
        generated: &GeneratedContent,
    ) -> Result<(), ElabError> {
        let names: Vec<String> = design.modules.keys().cloned().collect();
        let mut in_progress = HashSet::new();
        for name in &names {
            Self::flatten_module(name, design, generated, &mut in_progress)?;
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
        generated: &GeneratedContent,
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
        let mut authored_instances = std::mem::take(&mut m.instances);

        // T25 (VAMS-04, MD-25): splice this module's generate-produced
        // instances in on the derived side only — `design.modules` (the
        // clone `authored` came from) never saw them. A generated label
        // colliding with an authored one fails loud (spec Edge Cases: no
        // silent rename); `generate.rs`'s own `check_no_collisions` already
        // ruled out generated-vs-generated collisions.
        // T51 (LRMW2-52): splice generate-produced wires/behaviors/branches
        // in the same derived-only way — before `expand_array` reads
        // `m.wires` below, so a broadcast connection to a generated wire
        // sees its width. Collisions with an authored wire/behavior name
        // are not checked here (unlike instances): a generated wire's name
        // is always dotted-scoped (`genblk<n>.name`/`explicit_name.name`),
        // which cannot collide with a bare authored net name.
        if let Some(gen_wires) = generated.wires.get(name) {
            m.wires.extend(gen_wires.iter().cloned());
        }
        if let Some(gen_behaviors) = generated.behaviors.get(name) {
            m.behaviors.extend(gen_behaviors.iter().cloned());
        }
        if let Some(gen_branches) = generated.branches.get(name) {
            m.branches.extend(gen_branches.iter().cloned());
        }

        if let Some(gen_insts) = generated.instances.get(name) {
            for g in gen_insts {
                if authored_instances.iter().any(|a| a.name() == g.name()) {
                    return Err(ElabError::from(ElabErrorKind::Other(format!(
                        "module `{name}`: generate produced an instance named `{}`, \
                         colliding with an authored instance of the same name",
                        g.name()
                    ))));
                }
            }
            authored_instances.extend(gen_insts.iter().cloned());
        }

        // LRMC-C5-08: expand each `name [msb:lsb] (...)` instance into
        // `msb.abs_diff(lsb) + 1` leaf instances on the derived side only
        // (MD-25 — `authored_instances` came from `m` which is itself a
        // clone of the untouched `design.modules` entry; `Self::expand_array`
        // never mutates its input).
        let authored_instances = authored_instances
            .into_iter()
            .map(|inst| Self::expand_array(inst, &m.wires))
            .collect::<Result<Vec<Vec<Instance>>, ElabError>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<Instance>>();

        let mut kept = Vec::new();
        for inst in authored_instances {
            // LRMC-C7-13 (T39): a UDP instance (`primitive`-declared,
            // syntactically identical to a module instantiation, T39) has
            // no sub-hierarchy to flatten into — `design.udps`, not
            // `design.modules`, is where its declaration lives. Treated
            // as a leaf directly, the same outcome an ordinary
            // hierarchy-free module would get, without ever calling
            // `flatten_module` (which only knows `design.modules`).
            if design.udps.contains_key(&inst.module) {
                kept.push(inst);
                continue;
            }
            let child = Self::flatten_module(&inst.module, design, generated, in_progress)?;
            if Self::is_leaf(&child) {
                kept.push(inst);
            } else {
                let (insts, wires, conns) = Self::inline(&inst, &child, design)?;
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

    /// Expands one instance into `msb.abs_diff(lsb) + 1` leaf instances if
    /// it carries an array range (`Instance::range`, LRMC-C5-08), else
    /// returns it unchanged as the sole element. Each expanded instance's
    /// port connections are sliced per element: a connection naming a wire
    /// whose declared width equals the instance count becomes a single-bit
    /// `NetRef::indexed` (bit `v`, the element's own position in the
    /// `[msb:lsb]` range — the connected bus is assumed declared with the
    /// same `[msb:lsb]`, the common real-world convention this task's own
    /// `buf8 [7:0] (out[7:0], in[7:0])` example itself follows); a
    /// scalar-width wire (or an already-indexed connection) is shared,
    /// unchanged, across every element (a fan-out idiom — e.g. a shared
    /// power/ground net); any other width is a genuine mismatch and fails
    /// loud.
    fn expand_array(inst: Instance, wires: &[Wire]) -> Result<Vec<Instance>, ElabError> {
        let Some((msb, lsb)) = inst.range else { return Ok(vec![inst]) };
        let count = msb.abs_diff(lsb) + 1;
        let bits: Vec<i64> = if msb >= lsb { (lsb..=msb).rev().collect() } else { (msb..=lsb).collect() };
        let base_label = inst.label.clone().unwrap_or_default();

        bits.into_iter()
            .map(|v| {
                let ports = inst
                    .ports
                    .iter()
                    .map(|nr| {
                        if nr.index().is_some() {
                            return Ok(nr.clone());
                        }
                        let Some(wire) = wires.iter().find(|w| w.name() == nr.net()) else {
                            // Not a declared wire of this module (a port
                            // passed straight through, or a constant) —
                            // no width to check against; share unchanged.
                            return Ok(nr.clone());
                        };
                        let width = wire.ty.width();
                        if width == u64::from(count) {
                            Ok(NetRef::indexed(nr.net().to_string(), v as u64))
                        } else if width == 1 {
                            Ok(nr.clone())
                        } else {
                            Err(ElabError::from(ElabErrorKind::Other(format!(
                                "array of instances `{base_label} [{msb}:{lsb}]`: connected net `{}` has width {width}, \
                                 which matches neither the instance count ({count}) nor a scalar (1)",
                                nr.net()
                            ))))
                        }
                    })
                    .collect::<Result<Vec<NetRef>, ElabError>>()?;
                Ok(Instance {
                    span: inst.span,
                    label_span: None,
                    type_span: None,
                    attributes: inst.attributes.clone(),
                    doc: inst.doc.clone(),
                    label: Some(format!("{base_label}[{v}]")),
                    module: inst.module.clone(),
                    ports,
                    params: inst.params.clone(),
                    range: None,
                })
            })
            .collect()
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
        design: &Design,
    ) -> Result<InlineResult, ElabError> {
        let mut rho: HashMap<String, NetRef> = HashMap::new();
        let mut lifted_wires: Vec<Wire> = Vec::new();
        // §6.3.6/§9.18 (LRMW2-61/62, Table 9-29): `inst`'s own override of
        // each of the six hierarchical system parameters (the "specified"
        // term at this level; the identity value when `inst` carries none)
        // is the "hier" term every sub-instance spliced out of `inst`'s
        // body inherits. Combined per each parameter's own Table 9-29 rule
        // with each spliced instance's own override (its own "specified"
        // term), recursively — `inline` runs once per hierarchy level on
        // the way up, so a leaf several levels deep accumulates the full
        // composition along its path by the time it reaches the top.
        let inst_hier: Vec<(f64, bool)> =
            HIER_PARAMS.iter().map(|p| (p.value_of(inst.params()), p.present_in(inst.params()))).collect();

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
                nodeset: w.nodeset,
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
            // Table 9-29: each hierarchical parameter's own composition
            // rule (`compose(specified, hier)`). Only written back onto `s`
            // when `s`'s own module actually declares that parameter
            // (Piperine's disclosed vehicle for the six system parameters,
            // `crates/piperine-codegen/src/device/analog/instance.rs`) — a
            // leaf module that never declared one has no slot to carry the
            // composed value into, and the ordinary unknown-parameter-
            // override check downstream would otherwise fail loud on every
            // unrelated instance under a scaled/rotated/... ancestor. Only
            // written when *some* override was actually present at this
            // level or an ancestor's — never gated on "composed !=
            // identity": `$hflip`/`$vflip`'s whole domain is `{+1, -1}`
            // (LRM: "`$hflip` = +1 or -1"), so an even number of overrides
            // routinely composes right back to the identity value, and a
            // presence check (not a value check) is the only way to still
            // write that genuinely-composed identity back rather than
            // silently keeping `s`'s own stale pre-composition value.
            let declared: std::collections::HashSet<&str> = design
                .modules
                .get(&s.module)
                .map(|m| m.params.iter().map(|p| p.name.as_str()).collect())
                .unwrap_or_default();
            let mut params = s.params.clone();
            for (p, &(hier, hier_present)) in HIER_PARAMS.iter().zip(&inst_hier) {
                let s_present = p.present_in(s.params());
                if !(hier_present || s_present) || !declared.contains(p.name) {
                    continue;
                }
                let composed = (p.compose)(hier, p.value_of(s.params()));
                params = p.with_value(params, composed);
            }
            spliced_instances.push(Instance {
                span: s.span,
                label_span: None,
                type_span: None,
                attributes: s.attributes.clone(),
                doc: s.doc.clone(),
                label: Some(new_label),
                module: s.module.clone(),
                ports: new_ports,
                params,
                range: s.range,
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
    /// verbatim. An indexed reference (`net[i]`) expands to the scalar
    /// element of the parent net it maps onto (gap-3, NGP-20) via
    /// [`Self::expand_array_element`]. An unknown net fails loud naming the
    /// net and the owning child module (dangling/typo guard).
    fn remap(nr: &NetRef, rho: &HashMap<String, NetRef>, child: &Module) -> Result<NetRef, ElabError> {
        if is_ground(&nr.net) || child.is_ground(&nr.net) {
            return Ok(nr.clone());
        }
        let target = rho.get(&nr.net).ok_or_else(|| {
            ElabError::from(ElabErrorKind::Other(format!(
                "net `{}` in module `{}` is neither a port nor a wire (dangling)",
                nr.net, child.name
            )))
        })?;
        match nr.index {
            None => Ok(target.clone()),
            Some(i) => Self::expand_array_element(nr, i, target, child),
        }
    }

    /// Expand `net[i]` into the scalar net of the parent binding `target`
    /// (NGP-20). The POM carries only a constant index — `eval_net_ref`
    /// const-folds it during elaboration and fails loud on anything it
    /// cannot fold (NGP-21) — so the remaining loud cases are structural:
    ///
    /// - the child net is not an array net, so `net[i]` is meaningless;
    /// - `i` is outside the net's declared width;
    /// - the parent binding is itself indexed, i.e. `net[i]` would name a
    ///   two-dimensional element the POM cannot express.
    fn expand_array_element(
        nr: &NetRef,
        index: u64,
        target: &NetRef,
        child: &Module,
    ) -> Result<NetRef, ElabError> {
        let Some(width) = Self::array_width(child, &nr.net) else {
            return Err(ElabError::from(ElabErrorKind::Other(format!(
                "net `{}` in module `{}` is not an array net but is indexed as `{nr}`",
                nr.net, child.name
            ))));
        };
        if index >= width {
            return Err(ElabError::from(ElabErrorKind::Other(format!(
                "array-net index out of range: `{nr}` in module `{}` — width is {width}",
                child.name
            ))));
        }
        if target.index.is_some() {
            return Err(ElabError::from(ElabErrorKind::Other(format!(
                "array-net element `{nr}` in module `{}` binds to `{target}`, which is \
                 already an element — nested array indexing is not representable",
                child.name
            ))));
        }
        Ok(NetRef::indexed(target.net.clone(), index))
    }

    /// The declared width of `net` in `child`, when `net` is an array port or
    /// wire. `None` when the net is scalar (or absent — `remap` has already
    /// proven presence before calling this).
    fn array_width(child: &Module, net: &str) -> Option<u64> {
        let ty = child
            .ports
            .iter()
            .find(|p| p.name == net)
            .map(|p| &p.ty)
            .or_else(|| child.wires.iter().find(|w| w.name == net).map(|w| &w.ty))?;
        match ty {
            NetType::Array(_, n) => Some(*n),
            NetType::Discipline(_) | NetType::Digital1364(_) => None,
        }
    }

}

/// One of §9.18/Table 9-29's six hierarchical system parameters, as
/// Piperine's disclosed vehicle sees it: an ordinary module-declared
/// `parameter real` (or `parameter integer`, for `hflip`/`vflip`), combined
/// across instantiation levels by [`FlattenHierarchy::inline`] via `compose`
/// — `compose(hier, specified)`, matching Table 9-29's own
/// `f(specified, hier)` shape.
struct HierParam {
    name: &'static str,
    /// Table 9-29's top-level value — also the value below which a
    /// composed result is treated as "no override" (skipped rather than
    /// written back, so an untouched module never gains a synthetic
    /// parameter override).
    identity: f64,
    compose: fn(f64, f64) -> f64,
}

impl HierParam {
    /// This parameter's value among an instance's param overrides (Table
    /// 9-29's "specified" term); `identity` when absent or non-numeric.
    fn value_of(&self, params: &[(String, Value)]) -> f64 {
        params
            .iter()
            .find(|(name, _)| name == self.name)
            .and_then(|(_, v)| match v {
                Value::Real(r) => Some(*r),
                Value::Int(i) => Some(*i as f64),
                Value::Nat(n) => Some(*n as f64),
                _ => None,
            })
            .unwrap_or(self.identity)
    }

    /// Whether an override for this parameter is explicitly present among
    /// an instance's param overrides — distinct from `value_of`'s
    /// resolved-with-default reading, since a composed value can
    /// legitimately coincide with `identity` (`$hflip`/`$vflip`'s whole
    /// domain is `{+1, -1}`).
    fn present_in(&self, params: &[(String, Value)]) -> bool {
        params.iter().any(|(name, _)| name == self.name)
    }

    /// Upsert this parameter's override to `composed` in a param-override
    /// list.
    fn with_value(&self, mut params: Vec<(String, Value)>, composed: f64) -> Vec<(String, Value)> {
        match params.iter_mut().find(|(name, _)| name == self.name) {
            Some((_, v)) => *v = Value::Real(composed),
            None => params.push((self.name.to_string(), Value::Real(composed))),
        }
        params
    }
}

/// Table 9-29, in full: `$mfactor`/`$hflip`/`$vflip` multiply
/// (`specified * hier`), `$xposition`/`$yposition` add (`specified +
/// hier`), `$angle` adds modulo 360 degrees.
const HIER_PARAMS: &[HierParam] = &[
    HierParam { name: "mfactor", identity: 1.0, compose: |hier, specified| hier * specified },
    HierParam { name: "xposition", identity: 0.0, compose: |hier, specified| hier + specified },
    HierParam { name: "yposition", identity: 0.0, compose: |hier, specified| hier + specified },
    HierParam { name: "angle", identity: 0.0, compose: |hier, specified| (hier + specified).rem_euclid(360.0) },
    HierParam { name: "hflip", identity: 1.0, compose: |hier, specified| hier * specified },
    HierParam { name: "vflip", identity: 1.0, compose: |hier, specified| hier * specified },
];

#[cfg(test)]
mod tests {
    //! Driver + inline tests. Every test carries the non-destructive
    //! assertion: `Design::modules` deep-equal before and after the pass.

    use super::*;
    use crate::pom::{Connection, Direction, Instance, Module, NetType, Port, Wire};

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
            nodeset: None,
        }
    }

    fn array_wire(name: &str, width: u64) -> Wire {
        Wire {
            span: None,
            attributes: Vec::new(),
            doc: None,
            name: name.to_string(),
            ty: NetType::Array(Box::new(elec()), width),
            nodeset: None,
        }
    }

    fn array_port(name: &str, width: u64) -> Port {
        Port {
            span: None,
            attributes: Vec::new(),
            doc: None,
            direction: Direction::Inout,
            name: name.to_string(),
            ty: NetType::Array(Box::new(elec()), width),
        }
    }

    /// A child module whose sub-instance `r1` binds `{net}[{index}]` on its
    /// first terminal and the scalar port `p` on its second — the minimal
    /// shape for exercising array-net expansion in `remap`.
    fn seg_indexing(net: &str, index: u64, ports: Vec<Port>, wires: Vec<Wire>) -> Module {
        module_with(
            "Seg",
            ports,
            wires,
            vec![Instance {
                span: None,
                label_span: None,
                type_span: None,
                attributes: Vec::new(),
                doc: None,
                label: Some("r1".to_string()),
                module: "Resistor".to_string(),
                ports: vec![NetRef::indexed(net, index), NetRef::simple("p")],
                params: Vec::new(),
                range: None,
            }],
            Vec::new(),
        )
    }

    /// `Top` instantiating `Seg` with the given parent bindings.
    fn design_with_seg(seg: Module, top_bindings: Vec<NetRef>) -> Design {
        let mut design = Design::new();
        design.insert_module("Resistor".to_string(), leaf_module("Resistor"));
        design.insert_module("Seg".to_string(), seg);
        design.insert_module(
            "Top".to_string(),
            parent_module(
                "Top",
                vec![Instance {
                    span: None,
                    label_span: None,
                    type_span: None,
                    attributes: Vec::new(),
                    doc: None,
                    label: Some("x".to_string()),
                    module: "Seg".to_string(),
                    ports: top_bindings,
                    params: Vec::new(),
                    range: None,
                }],
            ),
        );
        design
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
            label_span: None,
            type_span: None,
            attributes: Vec::new(),
            doc: None,
            label: Some(label.to_string()),
            module: module.to_string(),
            ports: Vec::new(),
            params: Vec::new(),
            range: None,
        }
    }

    fn instance_ports(label: &str, module: &str, ports: &[&str]) -> Instance {
        Instance {
            span: None,
            label_span: None,
            type_span: None,
            attributes: Vec::new(),
            doc: None,
            label: Some(label.to_string()),
            module: module.to_string(),
            ports: ports.iter().map(|p| NetRef::simple(*p)).collect(),
            params: Vec::new(),
            range: None,
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
        FlattenHierarchy::flatten_design(&mut design, &Default::default()).expect("flatten");
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
        FlattenHierarchy::flatten_design(&mut design, &Default::default()).expect("flatten");
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
        let err = FlattenHierarchy::flatten_design(&mut design, &Default::default())
            .expect_err("self-cycle must fail loud");
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
        let err = FlattenHierarchy::flatten_design(&mut design, &Default::default())
            .expect_err("transitive cycle must fail loud");
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
        FlattenHierarchy::flatten_design(&mut design, &Default::default()).expect("first flatten");
        let after_first = format!("{:?}", design.flat_modules);
        FlattenHierarchy::flatten_design(&mut design, &Default::default()).expect("second flatten");
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
        FlattenHierarchy::flatten_design(&mut design, &Default::default()).expect("flatten");
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
        FlattenHierarchy::flatten_design(&mut design, &Default::default()).expect("flatten");
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
        FlattenHierarchy::flatten_design(&mut design, &Default::default()).expect("flatten");
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
        FlattenHierarchy::flatten_design(&mut design, &Default::default()).expect("flatten");
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
        FlattenHierarchy::flatten_design(&mut design, &Default::default()).expect("flatten");
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
        FlattenHierarchy::flatten_design(&mut design, &Default::default()).expect("flatten");
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
        let err = FlattenHierarchy::flatten_design(&mut design, &Default::default())
            .expect_err("dangling net must fail loud");
        let msg = err.to_string();
        assert!(msg.contains("missing"), "error names the dangling net: {msg}");
        assert!(msg.contains("Seg"), "error names the child module: {msg}");
        assert_eq!(before, snapshot_modules(&design), "modules untouched even on failure");
    }

    // ── T21 / gap-3: array-net expansion (NGP-20, NGP-21) ─────────────────

    #[test]
    fn inline_indexed_array_wire_expands_to_scalar_net() {
        // NGP-20: `bus[1]` on a child array wire expands to the element of
        // the lifted parent wire — `x.bus[1]` — not a dropped index and not
        // a fail-loud deferral.
        let seg = seg_indexing("bus", 1, vec![port("p")], vec![array_wire("bus", 3)]);
        let mut design = design_with_seg(seg, vec![NetRef::simple("a")]);
        let before = snapshot_modules(&design);
        FlattenHierarchy::flatten_design(&mut design, &Default::default()).expect("flatten expands the array element");
        assert_eq!(before, snapshot_modules(&design), "modules untouched (non-destructive)");

        let flat = design.flat_module("Top").expect("Top flat form");
        let r1 = flat
            .instances
            .iter()
            .find(|i| i.label.as_deref() == Some("x.r1"))
            .expect("x.r1 spliced");
        assert_eq!(r1.ports[0].net, "x.bus", "element resolves against the lifted wire");
        assert_eq!(r1.ports[0].index, Some(1), "the constant index is preserved");
        assert_eq!(r1.ports[0].to_string(), "x.bus[1]", "scalar net name of the element");
        assert_eq!(r1.ports[1].net, "a", "the scalar terminal still remaps normally");
    }

    #[test]
    fn inline_indexed_array_port_expands_against_the_parent_binding() {
        // NGP-20, the port branch: an array *port* indexed inside the child
        // expands against whatever parent net the port is bound to.
        let seg = seg_indexing("bus", 2, vec![array_port("bus", 3), port("p")], Vec::new());
        let mut design =
            design_with_seg(seg, vec![NetRef::simple("outer"), NetRef::simple("a")]);
        FlattenHierarchy::flatten_design(&mut design, &Default::default()).expect("flatten expands the array element");
        let flat = design.flat_module("Top").expect("Top flat form");
        let r1 = flat
            .instances
            .iter()
            .find(|i| i.label.as_deref() == Some("x.r1"))
            .expect("x.r1 spliced");
        assert_eq!(r1.ports[0].to_string(), "outer[2]", "element of the bound parent net");
    }

    #[test]
    fn inline_indexed_netref_out_of_range_fails_loud() {
        // NGP-20's bound: an index past the declared width is a loud error
        // naming the reference and the width, never a silently invented net.
        let seg = seg_indexing("bus", 3, vec![port("p")], vec![array_wire("bus", 3)]);
        let mut design = design_with_seg(seg, vec![NetRef::simple("a")]);
        let before = snapshot_modules(&design);
        let err = FlattenHierarchy::flatten_design(&mut design, &Default::default())
            .expect_err("out-of-range index must fail loud");
        let msg = err.to_string();
        assert!(msg.contains("out of range"), "error names the range violation: {msg}");
        assert!(msg.contains("bus[3]"), "error names the offending reference: {msg}");
        assert!(msg.contains("width is 3"), "error names the declared width: {msg}");
        assert_eq!(before, snapshot_modules(&design), "modules untouched even on failure");
    }

    #[test]
    fn inline_indexed_scalar_net_fails_loud() {
        // Indexing a net that was never declared as an array is a loud error
        // — expansion must not fabricate an element of a scalar net.
        let seg = seg_indexing("bus", 0, vec![port("p")], vec![wire("bus")]);
        let mut design = design_with_seg(seg, vec![NetRef::simple("a")]);
        let err = FlattenHierarchy::flatten_design(&mut design, &Default::default())
            .expect_err("indexing a scalar net must fail loud");
        let msg = err.to_string();
        assert!(msg.contains("is not an array net"), "error names the misuse: {msg}");
        assert!(msg.contains("bus[0]"), "error names the offending reference: {msg}");
    }

    #[test]
    fn inline_indexed_netref_onto_an_indexed_binding_fails_loud() {
        // A child array port bound to an element of a parent array net would
        // make `bus[0]` a two-dimensional reference the POM cannot express.
        // Loud, rather than a silently flattened `outer[1][0]`.
        let seg = seg_indexing("bus", 0, vec![array_port("bus", 3), port("p")], Vec::new());
        let mut design =
            design_with_seg(seg, vec![NetRef::indexed("outer", 1), NetRef::simple("a")]);
        let err = FlattenHierarchy::flatten_design(&mut design, &Default::default())
            .expect_err("nested array indexing must fail loud");
        let msg = err.to_string();
        assert!(msg.contains("nested array indexing"), "error names the limitation: {msg}");
        assert!(msg.contains("outer[1]"), "error names the parent binding: {msg}");
    }

    #[test]
    fn array_net_element_survives_elaboration_end_to_end() {
        // NGP-20 through the real front end: an array net indexed with
        // an elaboration constant flattens instead of failing loud.
        let src = "`include \"disciplines.vams\"\n\
            `include \"spice/passives.va\"\n\
            module Seg(p, n);\n\
              inout electrical p, n;\n\
              electrical bus[0:1];\n\
              res #(.r(1.0)) r0 (.p(p), .n(bus[0]));\n\
              res #(.r(1.0)) r1 (.p(bus[0]), .n(n));\n\
            endmodule\n\
            module Top();\n\
              electrical gnd, a;\n\
              Seg s (.p(a), .n(gnd));\n\
            endmodule\n";
        let design = crate::parse_and_elaborate(src, &crate::SourceMap::dummy())
            .expect("array-net indexing elaborates and flattens");
        let flat = design.flat_module("Top").expect("Top flat form");
        let nets: Vec<String> = flat
            .instances
            .iter()
            .flat_map(|i| i.ports.iter().map(|p| p.to_string()))
            .collect();
        assert!(
            nets.contains(&"s.bus[0]".to_string()),
            "the array element became a scalar net of the flat form: {nets:?}"
        );
    }

    #[test]
    fn non_constant_array_index_fails_loud() {
        // NGP-21: an index that is not an elaboration constant still fails
        // loud, naming the indexed net and the name that could not be folded.
        let src = "`include \"disciplines.vams\"\n\
            `include \"spice/passives.va\"\n\
            module Top();\n\
              electrical gnd;\n\
              electrical bus[0:2];\n\
              res #(.r(1.0)) r0 (.p(bus[k]), .n(gnd));\n\
            endmodule\n";
        let err = crate::parse_and_elaborate(src, &crate::SourceMap::dummy())
            .expect_err("a non-constant index must fail loud");
        let msg = err.to_string();
        // The contract is fail-loud naming what could not be folded. VAMS
        // catches this one stage earlier than PHDL did (the port
        // connection's own const-eval, rather than the net-ref index), so
        // the message identifies the index context and the offending name
        // rather than the indexed net.
        assert!(msg.contains("index"), "error identifies the index context: {msg}");
        assert!(msg.contains("k"), "error names the unfoldable index expression: {msg}");
        assert!(
            msg.contains("undefined name") || msg.contains("const eval"),
            "error explains the value could not be folded at elaboration: {msg}"
        );
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
        FlattenHierarchy::flatten_design(&mut design, &Default::default()).expect("flatten");
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
