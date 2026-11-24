//! Resolved nets below the POM (resolved-nets RN-02, design C7, D4, D5, D8, D21): once the tree is placed, every
//! net a resolved carrier is bound to gets what combines its drives. The holders of a net's drives are ordered by
//! their instance paths (§4 rule 2), each drive is a net of its own, the net's `impl Resolve<X, V>` is the one its
//! drives picked (D21 for a net nobody drives), and one resolver instance `<net path>$resolve` over a resolver body
//! per (nature, `impl`, count of drives, readers of `others()`) calls `resolve::<k>` for the `k` drives present
//! (`Expr::Resolve`). The net records what it is made of in `NetData::resolution`.

use std::collections::BTreeMap;

use piperine_ir::{BodyId, BodySpec, Carrier, CarrierRole, Drive, Expr, FunctionId, InstanceId, InstanceSpec, NetId, NetSpec, Origin, Resolution, ValueType};

use super::elaborator::Elaboration;
use super::instances::ConcreteValue;
use super::variants::Half;
use crate::msl::Diagnostic;
use crate::pom::DeclId;
use crate::types::ResolveImpl;

/// What the body of a module says about one of its resolved carriers, which placement reads.
#[derive(Debug, Clone)]
pub(super) struct ResolvedPlan {
    /// The carrier's index in the body.
    pub(super) carrier: u32,
    /// The port or signal declaration.
    pub(super) decl: DeclId,
    /// The resolved nature of the carrier.
    pub(super) nature: DeclId,
    /// The `impl Resolve` the module's own drives picked (§5.2.15); `None` when the module drives nothing.
    pub(super) implementation: Option<DeclId>,
    /// Whether the module defines or commits the carrier: it holds a drive of the net.
    pub(super) drives: bool,
    /// Whether the module reads `others()` of the carrier.
    pub(super) reads_others: bool,
}

/// One resolved carrier of one placed instance, bound to a net.
#[derive(Debug, Clone)]
pub(super) struct ResolvedUse {
    pub(super) instance: InstanceId,
    /// The instance's path: its drive's place in the order of a net's drives.
    pub(super) path: String,
    pub(super) net: NetId,
    pub(super) plan: ResolvedPlan,
    /// The type `V` the carrier reads.
    pub(super) value: ValueType,
    /// The type `X` the carrier is driven with.
    pub(super) driven: ValueType,
    /// Whether the carrier is a port of the top: the host holds a drive of its net.
    pub(super) host_port: bool,
}

/// Where a resolved net was declared.
#[derive(Debug, Clone)]
pub(super) struct DeclaredNet {
    pub(super) declared_by: InstanceId,
    pub(super) path: String,
    pub(super) origin: Origin,
}

/// The resolved nets of a run so far: what placement saw, and the resolver bodies built from it.
#[derive(Default)]
pub(super) struct ResolverBodies {
    /// Every resolved carrier of every placed instance.
    uses: Vec<ResolvedUse>,
    /// The nets made for resolved carriers, by id.
    declared: BTreeMap<NetId, DeclaredNet>,
    /// The resolver bodies built, by `impl`, count of drives and readers of `others()`.
    bodies: BTreeMap<(DeclId, usize, Vec<u32>), BodyId>,
}

impl ResolverBodies {
    /// Remembers the resolved net `net`, made for a carrier of the instance `declared_by`.
    pub(super) fn declare(&mut self, net: NetId, declared: DeclaredNet) {
        self.declared.insert(net, declared);
    }

    /// Remembers a resolved carrier of a placed instance.
    pub(super) fn record(&mut self, used: ResolvedUse) {
        self.uses.push(used);
    }
}

/// The order of the drives of a net (§4 rule 2, Resolution): the holders' instance paths compared segment by
/// segment, names by code point, array indices numerically.
pub(super) struct DriveOrder;

impl DriveOrder {
    /// The sort key of an instance path: each segment as its name and its array index, `a.cells[10]` after `a.cells[9]`.
    pub(super) fn key(path: &str) -> Vec<(String, Option<u64>)> {
        path.split('.')
            .map(|segment| match segment.split_once('[') {
                Some((name, rest)) => (name.to_string(), rest.trim_end_matches(']').parse().ok()),
                None => (segment.to_string(), None),
            })
            .collect()
    }
}

impl Elaboration<'_, '_> {
    /// Builds the resolver of every resolved net the placed tree has. Nothing is built after a diagnostic.
    pub(super) fn build_resolvers(&mut self) {
        if !self.diagnostics.is_empty() {
            return;
        }
        let mut by_net: BTreeMap<NetId, Vec<ResolvedUse>> = BTreeMap::new();
        for used in std::mem::take(&mut self.resolvers.uses) {
            by_net.entry(used.net).or_default().push(used);
        }
        for (net, group) in by_net {
            self.resolve_net(net, group);
        }
    }

    /// The resolver of one net: its drives, its `impl`, its others nets, its resolver body and instance.
    fn resolve_net(&mut self, net: NetId, group: Vec<ResolvedUse>) {
        let Some(declared) = self.resolvers.declared.get(&net).cloned() else { return };
        let mut holders: Vec<&ResolvedUse> = group.iter().filter(|used| used.plan.drives).collect();
        holders.sort_by_key(|used| DriveOrder::key(&used.path));
        let host = group.iter().any(|used| used.host_port);
        let sample = &group[0];
        let Some(implementation) = self.net_implementation(&holders, sample) else { return };
        let Some(driven) = self.stored_type(&implementation.driven) else { return };
        if let Some(clash) = holders.iter().find(|used| used.driven != driven) {
            let (file, span) = (self.design.decl_data(clash.plan.decl).file, self.design.decl_data(clash.plan.decl).span);
            self.unsupported(file, span, &format!("two driven types on the net `{}`", declared.path));
            return;
        }
        let value = sample.value.clone();
        let mut drive_nets = Vec::new();
        let mut owners: Vec<Option<InstanceId>> = Vec::new();
        let mut readers: Vec<(u32, NetId)> = Vec::new();
        let slots = host.then_some(None).into_iter().chain(holders.iter().map(|used| Some(*used)));
        for (index, slot) in slots.enumerate() {
            let drive = self.ir.net(NetSpec { path: format!("{}$drive{index}", declared.path), ty: driven.clone(), declared_by: declared.declared_by, init: None, origin: declared.origin.clone() });
            drive_nets.push(drive);
            owners.push(slot.map(|used| used.instance));
            if slot.is_some_and(|used| used.plan.reads_others) {
                let others = self.ir.net(NetSpec { path: format!("{}$others{index}", declared.path), ty: value.clone(), declared_by: declared.declared_by, init: None, origin: declared.origin.clone() });
                readers.push((index as u32, others));
            }
        }
        let reader_indices: Vec<u32> = readers.iter().map(|(index, _)| *index).collect();
        let Some(body) = self.resolver_body(&implementation, drive_nets.len(), &reader_indices, &driven, &value, &declared) else { return };
        let name = declared.path.rsplit('.').next().unwrap_or(&declared.path).to_string();
        let resolver = self.ir.instance(InstanceSpec { path: format!("{}$resolve", declared.path), name: format!("{name}$resolve"), body, parent: Some(declared.declared_by), nodes: vec![], params: vec![], origin: declared.origin.clone() });
        let bound: Vec<NetId> = drive_nets.iter().copied().chain([net]).chain(readers.iter().map(|(_, others)| *others)).collect();
        self.ir.bind_nets(resolver, bound);
        self.ir.resolve_net(net, Resolution { resolver, driven, drives: drive_nets, holders: owners, others: readers });
    }

    /// The `impl Resolve` of a net: the one its holders' drives picked, else the net's own choice when nobody drives
    /// it (D21): the nature's only `impl`, else its `impl Resolve<V>`, else `E0181` naming the net's carrier.
    fn net_implementation(&mut self, holders: &[&ResolvedUse], sample: &ResolvedUse) -> Option<ResolveImpl> {
        let impls = self.types.resolve_impls(sample.plan.nature).to_vec();
        if let Some(picked) = holders.iter().find_map(|used| used.plan.implementation) {
            return impls.into_iter().find(|candidate| candidate.implementation == picked);
        }
        match impls.as_slice() {
            [only] => Some(only.clone()),
            several => {
                let found = several.iter().find(|candidate| candidate.driven == candidate.value).cloned();
                if found.is_none() {
                    let data = self.design.decl_data(sample.plan.decl);
                    let (file, span) = (data.file, data.span);
                    let nature = self.design.decl_data(sample.plan.nature).name.as_ref().map_or_else(String::new, |n| n.text.clone());
                    let message = format!("no drive of this net picks an `impl Resolve` of `{nature}`, which has several and none is `impl Resolve<V>` (§5.2.15)");
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
        if let Some(body) = self.resolvers.bodies.get(&key) {
            return Some(*body);
        }
        let function = self.design.decl_data(implementation.implementation).children.iter().copied().find(|child| self.design.decl_data(*child).name.as_ref().is_some_and(|n| n.text == "resolve"))?;
        let mut by_count: Vec<FunctionId> = Vec::new();
        for drives in 0..=count {
            by_count.push(self.request_instance(function, vec![ConcreteValue::Const(drives as i128)], Half::Whole)?.0);
        }
        let origin = declared.origin.clone();
        let spec = BodySpec { name: format!("{}$resolve~{count}{}", self.resolver_name(implementation), Self::reader_suffix(readers)), origin: origin.clone(), params: vec![], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] };
        let body = self.ir.body(spec);
        for index in 0..count {
            self.ir.carrier(body, Carrier { name: format!("d{index}"), ty: driven.clone(), role: CarrierRole::In, init: None, origin: origin.clone(), count: None });
        }
        let out = |name: String, origin: &Origin| Carrier { name, ty: value.clone(), role: CarrierRole::Out, init: None, origin: origin.clone(), count: None };
        let combined = self.ir.carrier(body, out("v".to_string(), &origin));
        let drives: Vec<u32> = (0..count as u32).collect();
        let all = self.emit(Expr::Resolve { by_count: by_count.clone(), drives: drives.clone(), except: None }, value.clone(), origin.clone());
        self.ir.drive(body, Drive { carrier: combined, value: all, after: None, origin: origin.clone(), element: None, path: vec![] });
        for reader in readers {
            let carrier = self.ir.carrier(body, out(format!("o{reader}"), &origin));
            let others = self.emit(Expr::Resolve { by_count: by_count.clone(), drives: drives.clone(), except: Some(*reader) }, value.clone(), origin.clone());
            self.ir.drive(body, Drive { carrier, value: others, after: None, origin: origin.clone(), element: None, path: vec![] });
        }
        self.resolvers.bodies.insert(key, body);
        Some(body)
    }

    /// The name of the `impl`'s resolver bodies: `Total` for a nature, `Total<bool>` for one of its `impl`s of the driven type `bool`.
    fn resolver_name(&self, implementation: &ResolveImpl) -> String {
        let target = self.impls.impls().iter().find(|entry| entry.decl == implementation.implementation).and_then(|entry| match &entry.target {
            crate::types::ImplTarget::Declared(decl) => self.design.decl_data(*decl).name.as_ref().map(|n| n.text.clone()),
            _ => None,
        });
        format!("{}<{}>", target.unwrap_or_default(), implementation.driven.source_name(self.design))
    }

    /// `+others0+others2`: which drives' holders read `others()` through the resolver.
    fn reader_suffix(readers: &[u32]) -> String {
        readers.iter().map(|reader| format!("+others{reader}")).collect()
    }
}
