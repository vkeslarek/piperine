//! Loops of undelayed definitions across instances (part III §8.2.3, resolved-nets RN-05). The checker finds the loops one
//! module writes (`E0339`); a loop only the placed tree closes is found here, on the finished IR: the nets are the nodes of a
//! graph and each drive without an `after` is an edge from the nets it reads to the net it writes (a drive of a resolved
//! carrier writes its holder's drive net, and the resolver of a net reads its drive nets). A loop that passes through a
//! one-driver net is `E0339` naming its drives; a loop among resolved nets is left to relaxation (§8.2.3).

use std::collections::{BTreeMap, BTreeSet};

use piperine_ir::{CarrierRole, Holder, Instance, Ir, NetId, Target};

/// One loop of undelayed drives through a one-driver net: the paths of the instances that hold its drives and the carriers
/// they write, in net order.
pub(super) struct LoopFault {
    /// `Top.a.y`: the instance and the carrier of each drive of the loop.
    pub(super) drives: Vec<String>,
}

/// The drive graph of one elaborated design.
pub(super) struct DriveLoops<'ir> {
    ir: &'ir Ir,
    /// For each net, the nets whose value its undelayed drives read, with a label for the drive that reads.
    reads: BTreeMap<NetId, Vec<(NetId, String)>>,
}

impl<'ir> DriveLoops<'ir> {
    /// Every loop through a one-driver net of `ir`, in net order.
    pub(super) fn run(ir: &'ir Ir) -> Vec<LoopFault> {
        let mut graph = DriveLoops { ir, reads: BTreeMap::new() };
        graph.collect();
        graph.faults()
    }

    /// The nets the machinery of a resolved net owns: its drive nets and its others nets, which are never one-driver.
    fn internal_nets(&self) -> BTreeSet<NetId> {
        self.ir.nets().filter_map(|net| net.resolution()).flat_map(|resolution| resolution.drives.iter().copied().chain(resolution.others.iter().map(|(_, net)| *net))).collect()
    }

    /// The edges of every undelayed drive of every instance.
    fn collect(&mut self) {
        for instance in self.ir.instances() {
            let body = instance.body();
            for (index, drive) in body.drives().iter().enumerate() {
                if drive.after.is_some() {
                    continue;
                }
                let Some(target) = self.written_net(instance, drive.carrier) else { continue };
                let label = format!("{}.{}", instance.path(), body.carriers()[drive.carrier as usize].name);
                let mut sources: Vec<NetId> = body.carrier_reads(Target::Drive(index as u32)).into_iter().filter_map(|c| instance.net(c as usize).map(|net| net.id())).collect();
                // a resolver reads the drive nets its `resolve` lists, and a holder's `others()` the other drives of its net
                sources.extend(body.resolve_drive_reads(Target::Drive(index as u32)).into_iter().filter_map(|c| instance.net(c as usize).map(|net| net.id())));
                for carrier in body.others_reads(Target::Drive(index as u32)) {
                    sources.extend(self.other_drives(instance, carrier));
                }
                self.reads.entry(target).or_default().extend(sources.into_iter().map(|source| (source, label.clone())));
            }
        }
    }

    /// The net a drive of `carrier` of `instance` writes: the carrier's net, or for a resolved carrier the drive net of the
    /// instance's holder.
    fn written_net(&self, instance: Instance<'ir>, carrier: u32) -> Option<NetId> {
        let net = instance.net(carrier as usize)?;
        if !matches!(instance.body().carriers()[carrier as usize].role, CarrierRole::Resolved { .. }) {
            return Some(net.id());
        }
        let resolution = net.resolution()?;
        let position = resolution.holders.iter().position(|holder| *holder == Some(Holder { instance: instance.id(), carrier }))?;
        resolution.drives.get(position).copied()
    }

    /// The drive nets of the net of the resolved `carrier` of `instance` but its own.
    fn other_drives(&self, instance: Instance<'ir>, carrier: u32) -> Vec<NetId> {
        let Some(net) = instance.net(carrier as usize) else { return Vec::new() };
        let Some(resolution) = net.resolution() else { return Vec::new() };
        let own = resolution.holders.iter().position(|holder| *holder == Some(Holder { instance: instance.id(), carrier }));
        resolution.drives.iter().enumerate().filter(|(position, _)| Some(*position) != own).map(|(_, drive)| *drive).collect()
    }

    /// The strongly connected components with a cycle, those through a one-driver net as faults.
    fn faults(&self) -> Vec<LoopFault> {
        let internal = self.internal_nets();
        let one_driver = |net: NetId| !internal.contains(&net) && self.ir.net(net).resolution().is_none();
        let mut faults = Vec::new();
        let mut seen = BTreeSet::new();
        for start in self.reads.keys().copied() {
            if seen.contains(&start) {
                continue;
            }
            let reachable = self.closure(start);
            if !reachable.contains(&start) {
                continue;
            }
            let component: Vec<NetId> = reachable.iter().copied().filter(|net| self.closure(*net).contains(&start)).collect();
            seen.extend(component.iter().copied());
            if !component.iter().any(|net| one_driver(*net)) {
                continue;
            }
            let mut drives: BTreeSet<String> = BTreeSet::new();
            for net in &component {
                drives.extend(self.reads.get(net).into_iter().flatten().filter(|(source, _)| component.contains(source)).map(|(_, label)| label.clone()));
            }
            faults.push(LoopFault { drives: drives.into_iter().collect() });
        }
        faults
    }

    /// The nets `start` is reached from through the drives that read them, `start` included only when a cycle returns to it.
    fn closure(&self, start: NetId) -> BTreeSet<NetId> {
        let mut found = BTreeSet::new();
        let mut stack: Vec<NetId> = self.reads.get(&start).into_iter().flatten().map(|(source, _)| *source).collect();
        while let Some(net) = stack.pop() {
            if found.insert(net) {
                stack.extend(self.reads.get(&net).into_iter().flatten().map(|(source, _)| *source));
            }
        }
        found
    }
}
