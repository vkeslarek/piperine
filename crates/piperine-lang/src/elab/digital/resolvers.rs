//! Resolved nets below the POM (resolved-nets RN-02, design C7, D4, D5, D8, D21): once the tree is placed, every
//! net a resolved carrier is bound to gets what combines its drives. The holders of a net's drives are ordered by
//! their instance paths (§4 rule 2), each drive is a net of its own, the net's `impl Resolve<X, V>` is the one its
//! drives picked (D21 for a net nobody drives), and one resolver instance `<net path>$resolve` over a resolver body
//! per (nature, `impl`, count of drives, readers of `others()`) calls `resolve::<k>` for the `k` drives present
//! (`Expr::Resolve`). The net records what it is made of in `NetData::resolution`.

use super::DigitalLowering;
use std::collections::BTreeMap;

use piperine_ir::{BodyId, BodySpec, Carrier, CarrierRole, Drive, Expr, FunctionId, Holder, InstanceId, InstanceSpec, NetId, NetSpec, Origin, Resolution, ValueType};

use crate::elab::functions::instances::ConcreteValue;
use crate::elab::functions::variants::Half;
use crate::msl::Diagnostic;
use crate::pom::DeclId;
use crate::types::ResolveImpl;

/// What the body of a module says about one of its resolved carriers, which placement reads.
#[derive(Debug, Clone)]
pub(in crate::elab) struct ResolvedPlan {
    /// The carrier's index in the body.
    pub(in crate::elab) carrier: u32,
    /// The port or signal declaration.
    pub(in crate::elab) decl: DeclId,
    /// The resolved nature of the carrier.
    pub(in crate::elab) nature: DeclId,
    /// The `impl Resolve` the module's own drives picked (§5.2.15); `None` when the module drives nothing.
    pub(in crate::elab) implementation: Option<DeclId>,
    /// Whether the module defines or commits the carrier: it holds a drive of the net.
    pub(in crate::elab) drives: bool,
    /// Whether the module reads `others()` of the carrier.
    pub(in crate::elab) reads_others: bool,
}

/// One resolved carrier of one placed instance, bound to a net.
#[derive(Debug, Clone)]
pub(in crate::elab) struct ResolvedUse {
    pub(in crate::elab) instance: InstanceId,
    /// The instance's path: its drive's place in the order of a net's drives.
    pub(in crate::elab) path: String,
    pub(in crate::elab) net: NetId,
    pub(in crate::elab) plan: ResolvedPlan,
    /// The type `V` the carrier reads.
    pub(in crate::elab) value: ValueType,
    /// The type `X` the carrier is driven with.
    pub(in crate::elab) driven: ValueType,
    /// Whether the carrier is a port of the top: the host holds a drive of its net.
    pub(in crate::elab) host_port: bool,
}

/// Where a resolved net was declared.
#[derive(Debug, Clone)]
pub(in crate::elab) struct DeclaredNet {
    pub(in crate::elab) declared_by: InstanceId,
    pub(in crate::elab) path: String,
    pub(in crate::elab) origin: Origin,
}

/// The resolved nets of a run so far: what placement saw, and the resolver bodies built from it.
#[derive(Default)]
pub(in crate::elab) struct ResolverBodies {
    /// Every resolved carrier of every placed instance.
    uses: Vec<ResolvedUse>,
    /// The nets made for resolved carriers, by id.
    declared: BTreeMap<NetId, DeclaredNet>,
    /// The resolver bodies built, by `impl`, count of drives and readers of `others()`.
    bodies: BTreeMap<(DeclId, usize, Vec<u32>), BodyId>,
}

impl ResolverBodies {
    /// Remembers the resolved net `net`, made for a carrier of the instance `declared_by`.
    pub(in crate::elab) fn declare(&mut self, net: NetId, declared: DeclaredNet) {
        self.declared.insert(net, declared);
    }

    /// Remembers a resolved carrier of a placed instance.
    pub(in crate::elab) fn record(&mut self, used: ResolvedUse) {
        self.uses.push(used);
    }
}

/// The order of the drives of a net (§4 rule 2, Resolution): the holders' instance paths compared segment by
/// segment, names by code point, array indices numerically.
pub(super) struct DriveOrder;

impl DriveOrder {
    /// The sort key of an instance path: each segment as its name and its array index, `a.cells[10]` after `a.cells[9]`.
    pub(in crate::elab) fn key(path: &str) -> Vec<(String, Option<u64>)> {
        path.split('.')
            .map(|segment| match segment.split_once('[') {
                Some((name, rest)) => (name.to_string(), rest.trim_end_matches(']').parse().ok()),
                None => (segment.to_string(), None),
            })
            .collect()
    }
}

impl DigitalLowering<'_, '_, '_> {
    /// Builds the resolver of every resolved net the placed tree has. Nothing is built after a diagnostic.
    pub(in crate::elab) fn build_resolvers(&mut self) {
        if !self.state.diagnostics.is_empty() {
            return;
        }
        let mut by_net: BTreeMap<NetId, Vec<ResolvedUse>> = BTreeMap::new();
        for used in std::mem::take(&mut self.state.digital.resolvers.uses) {
            by_net.entry(used.net).or_default().push(used);
        }
        for (net, group) in by_net {
            self.resolve_net(net, group);
        }
    }

    /// The resolver of one net: its drives, its `impl`, its others nets, its resolver body and instance.
    fn resolve_net(&mut self, net: NetId, group: Vec<ResolvedUse>) {
        let Some(declared) = self.state.digital.resolvers.declared.get(&net).cloned() else { return };
        let mut holders: Vec<&ResolvedUse> = group.iter().filter(|used| used.plan.drives).collect();
        holders.sort_by_key(|used| (DriveOrder::key(&used.path), used.plan.carrier));
        let host = group.iter().any(|used| used.host_port);
        let sample = &group[0];
        if self.one_driven_type(&holders, &declared) {
            return;
        }
        let Some(implementation) = self.net_implementation(&holders, sample) else { return };
        let Some(driven) = self.values().stored_type(&implementation.driven) else { return };
        let value = sample.value.clone();
        let mut drive_nets = Vec::new();
        let mut owners: Vec<Option<Holder>> = Vec::new();
        let mut readers: Vec<(u32, NetId)> = Vec::new();
        let slots = host.then_some(None).into_iter().chain(holders.iter().map(|used| Some(*used)));
        for (index, slot) in slots.enumerate() {
            let drive = self.state.builder.net(NetSpec { path: format!("{}$drive{index}", declared.path), ty: driven.clone(), declared_by: declared.declared_by, init: None, origin: declared.origin.clone() });
            drive_nets.push(drive);
            owners.push(slot.map(|used| Holder { instance: used.instance, carrier: used.plan.carrier }));
            if slot.is_some_and(|used| used.plan.reads_others) {
                let others = self.state.builder.net(NetSpec { path: format!("{}$others{index}", declared.path), ty: value.clone(), declared_by: declared.declared_by, init: None, origin: declared.origin.clone() });
                readers.push((index as u32, others));
            }
        }
        let reader_indices: Vec<u32> = readers.iter().map(|(index, _)| *index).collect();
        let Some(body) = self.resolver_body(&implementation, drive_nets.len(), &reader_indices, &driven, &value, &declared) else { return };
        let name = declared.path.rsplit('.').next().unwrap_or(&declared.path).to_string();
        let resolver = self.state.builder.instance(InstanceSpec { path: format!("{}$resolve", declared.path), name: format!("{name}$resolve"), body, parent: Some(declared.declared_by), nodes: vec![], params: vec![], origin: declared.origin.clone() });
        let bound: Vec<NetId> = drive_nets.iter().copied().chain([net]).chain(readers.iter().map(|(_, others)| *others)).collect();
        if let Err(error) = self.state.builder.bind_nets(resolver, bound) {
            self.internal_error(&error.to_string());
        }
        if let Err(error) = self.state.builder.resolve_net(net, Resolution { resolver, driven, drives: drive_nets, holders: owners, others: readers }) {
            self.internal_error(&error.to_string());
        }
    }

    /// `E0225` when two drives of one net hold different driven types (§5.2.15), naming both holders' instance paths
    /// and both types; whether it was reported.
    fn one_driven_type(&mut self, holders: &[&ResolvedUse], declared: &DeclaredNet) -> bool {
        let Some((first, rest)) = holders.split_first() else { return false };
        let Some(clash) = rest.iter().find(|used| used.driven != first.driven) else { return false };
        let data = self.state.design.decl_data(clash.plan.decl);
        let (file, span) = (data.file, data.span);
        let message = format!("the net `{}` is driven with `{}` by `{}` and with `{}` by `{}`: every drive of a net holds one type (§5.2.15)", declared.path, first.driven, first.path, clash.driven, clash.path);
        self.report(file, Diagnostic::new("E0225", message, span, "a drive of another type"));
        true
    }

    /// The `impl Resolve` of a net: the one its holders' drives picked, else the net's own choice when nobody drives
    /// it (D21): the nature's only `impl`, else its `impl Resolve<V>`, else `E0181` naming the net's carrier.
    fn net_implementation(&mut self, holders: &[&ResolvedUse], sample: &ResolvedUse) -> Option<ResolveImpl> {
        let impls = self.state.types.resolve_impls(sample.plan.nature).to_vec();
        if let Some(picked) = holders.iter().find_map(|used| used.plan.implementation) {
            return impls.into_iter().find(|candidate| candidate.implementation == picked);
        }
        match impls.as_slice() {
            [only] => Some(only.clone()),
            several => {
                let found = several.iter().find(|candidate| candidate.driven == candidate.value).cloned();
                if found.is_none() {
                    let data = self.state.design.decl_data(sample.plan.decl);
                    let (file, span) = (data.file, data.span);
                    let nature = self.state.design.decl_data(sample.plan.nature).name.as_ref().map_or_else(String::new, |n| n.text.clone());
                    let carrier = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
                    let message = format!("no drive of the net `{carrier}` picks an `impl Resolve` of `{nature}`, which has several and none is `impl Resolve<V>` (§5.2.15)");
                    self.report(file, Diagnostic::new("E0181", message, span, "which `impl Resolve`?"));
                }
                found
            }
        }
    }

    /// The resolver body for `count` drives of `implementation` with the others of `readers`: `d0`..`d{count-1}` in,
    /// `v` out as the `Resolve` of every drive present, and one `o{j}` out per reader as the `Resolve` of all but its own.
    fn resolver_body(&mut self, implementation: &ResolveImpl, count: usize, readers: &[u32], driven: &ValueType, value: &ValueType, declared: &DeclaredNet) -> Option<BodyId> {
        let key = (implementation.implementation, count, readers.to_vec());
        if let Some(body) = self.state.digital.resolvers.bodies.get(&key) {
            return Some(*body);
        }
        let function = self.state.design.decl_data(implementation.implementation).children.iter().copied().find(|child| self.state.design.decl_data(*child).name.as_ref().is_some_and(|n| n.text == "resolve"))?;
        let mut by_count: Vec<FunctionId> = Vec::new();
        for drives in 0..=count {
            by_count.push(self.functions().request_instance(function, vec![ConcreteValue::Const(drives as i128)], Half::Whole)?.0);
        }
        let origin = declared.origin.clone();
        let spec = BodySpec { name: format!("{}$resolve~{count}{}", self.resolver_name(implementation), Self::reader_suffix(readers)), origin: origin.clone(), params: vec![], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] };
        let body = self.state.builder.body(spec);
        for index in 0..count {
            let built = self.state.builder.carrier(body, Carrier { name: format!("d{index}"), ty: driven.clone(), role: CarrierRole::In, init: None, origin: origin.clone(), count: None });
            self.built(built);
        }
        let out = |name: String, origin: &Origin| Carrier { name, ty: value.clone(), role: CarrierRole::Out, init: None, origin: origin.clone(), count: None };
        let built = self.state.builder.carrier(body, out("v".to_string(), &origin));
        let combined = self.built(built)?;
        let drives: Vec<u32> = (0..count as u32).collect();
        let all = self.emit(Expr::Resolve { by_count: by_count.clone(), drives: drives.clone(), except: None }, value.clone(), origin.clone());
        let built = self.state.builder.drive(body, Drive { carrier: combined, value: all, after: None, origin: origin.clone(), element: None, path: vec![] });
        self.built(built);
        for reader in readers {
            let built = self.state.builder.carrier(body, out(format!("o{reader}"), &origin));
            let carrier = self.built(built)?;
            let others = self.emit(Expr::Resolve { by_count: by_count.clone(), drives: drives.clone(), except: Some(*reader) }, value.clone(), origin.clone());
            let built = self.state.builder.drive(body, Drive { carrier, value: others, after: None, origin: origin.clone(), element: None, path: vec![] });
            self.built(built);
        }
        self.state.digital.resolvers.bodies.insert(key, body);
        Some(body)
    }

    /// The name of the `impl`'s resolver bodies: `Total` for a nature, `Total<bool>` for one of its `impl`s of the driven type `bool`.
    fn resolver_name(&self, implementation: &ResolveImpl) -> String {
        let target = self.state.impls.impls().iter().find(|entry| entry.decl == implementation.implementation).and_then(|entry| match &entry.target {
            crate::types::ImplTarget::Declared(decl) => self.state.design.decl_data(*decl).name.as_ref().map(|n| n.text.clone()),
            _ => None,
        });
        format!("{}<{}>", target.unwrap_or_default(), implementation.driven.source_name(self.state.design))
    }

    /// `+others0+others2`: which drives' holders read `others()` through the resolver.
    fn reader_suffix(readers: &[u32]) -> String {
        readers.iter().map(|reader| format!("+others{reader}")).collect()
    }
}
