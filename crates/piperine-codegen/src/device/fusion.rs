//! Digital-cone fusion (SC-13): connected pure-combinational digital cones
//! are compiled into single `DigitalNetwork` elements (Verilator-style
//! whole-cone evaluation) instead of one solver `Element` per instance.

use std::collections::HashMap;

use piperine_solver::abi::DigitalNet;
use piperine_solver::abi::Element;

use crate::error::CodegenError;

use super::{AnalogInstance, DigitalInstance};
use super::builder::InstanceBuilder;

/// A fusion-eligible digital device: pure combinational (no clocked blocks,
/// no analog sampling) and no analog side. Register power-on inits are
/// carried into the fused network's bank state, so combinational modules
/// with initialised `var`s stay eligible.
pub(super) struct FusionCandidate {
    /// Index into `InstanceBuilder::devices`.
    device_index: usize,
    module_name: String,
    in_nets: Vec<DigitalNet>,
    out_nets: Vec<DigitalNet>,
    params: Vec<f64>,
    reg_inits: Vec<(crate::resolve::VarId, f64)>,
}

impl FusionCandidate {
    /// Whether the device qualifies: the fused cone only settles pure
    /// combinational logic — clocked or analog-sampling members keep the
    /// per-device path (bit-exact by construction there).
    pub(super) fn of(
        device_index: usize,
        module_name: &str,
        analog: &Option<AnalogInstance>,
        digital: &Option<DigitalInstance>,
        params: Vec<f64>,
    ) -> Option<Self> {
        if analog.is_some() {
            return None;
        }
        let d = digital.as_ref()?;
        let kernel = d.kernel();
        if !kernel.clocked_blocks().is_empty() || kernel.layout().num_analog() > 0 {
            return None;
        }
        Some(Self {
            device_index,
            module_name: module_name.to_string(),
            in_nets: d.input_nets().to_vec(),
            out_nets: d.output_nets().to_vec(),
            params,
            reg_inits: d.reg_init_values(),
        })
    }
}

impl<'c, 'p> InstanceBuilder<'c, 'p> {
    /// Fuse connected pure-combinational digital cones into single
    /// `DigitalNetwork` elements (SC-13): candidates union over shared nets
    /// into cones; a cone with ≥ 2 members and no internal feedback is
    /// rank-ordered (the circuit-wide topological order restricted to its
    /// members), compiled once, and its members drop out of the device list.
    /// A cone with internal feedback keeps the per-device path — the
    /// event/delta-cycle loop owns loop semantics (ring-oscillator style).
    pub(super) fn fuse_comb_cones(&mut self) -> Result<(), CodegenError> {
        use crate::kernel::digital::network::{DigitalNetwork, NetworkMember};
        use piperine_solver::abi::DigitalTopology;

        if !self.compiler.fuse_digital_cones || self.fusion_candidates.len() < 2 {
            return Ok(());
        }
        // Union-find over candidates, united when they share a net.
        let n = self.fusion_candidates.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn root(parent: &mut [usize], mut x: usize) -> usize {
            while parent[x] != x {
                parent[x] = parent[parent[x]];
                x = parent[x];
            }
            x
        }
        let mut net_owner: HashMap<usize, usize> = HashMap::new();
        for (ci, cand) in self.fusion_candidates.iter().enumerate() {
            for net in cand.in_nets.iter().chain(&cand.out_nets) {
                if let Some(&other) = net_owner.get(&net.0) {
                    let (ra, rb) = (root(&mut parent, ci), root(&mut parent, other));
                    if ra != rb {
                        parent[ra.max(rb)] = ra.min(rb);
                    }
                } else {
                    net_owner.insert(net.0, ci);
                }
            }
        }
        let mut cones: HashMap<usize, Vec<usize>> = HashMap::new();
        for ci in 0..n {
            cones.entry(root(&mut parent, ci)).or_default().push(ci);
        }

        // Rank + feedback from the circuit-wide topology.
        let topo = DigitalTopology::build(&self.devices);
        let rank_of = |dev: usize| {
            topo.topo_order.iter().position(|&d| d == dev).unwrap_or(usize::MAX)
        };
        let loopy: std::collections::HashSet<usize> = {
            // A back edge whose endpoints are both fusion candidates marks
            // their cone as internal-feedback (kept per-device).
            let mut set = std::collections::HashSet::new();
            for &(src_pos, dst_pos) in &topo.back_edges {
                let (src, dst) = (topo.topo_order[src_pos], topo.topo_order[dst_pos]);
                let src_cand = self.fusion_candidates.iter().position(|c| c.device_index == src);
                let dst_cand = self.fusion_candidates.iter().position(|c| c.device_index == dst);
                if let (Some(a), Some(b)) = (src_cand, dst_cand)
                    && root(&mut parent, a) == root(&mut parent, b)
                {
                    set.insert(root(&mut parent, a));
                }
            }
            set
        };

        let net_count = self.digital_nets.len();
        let mut fused_device_idxs: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut networks: Vec<Box<dyn Element>> = Vec::new();
        for (cone_root, mut cone) in cones {
            if cone.len() < 2 || loopy.contains(&cone_root) {
                continue;
            }
            cone.sort_by_key(|&ci| rank_of(self.fusion_candidates[ci].device_index));
            let mut members = Vec::with_capacity(cone.len());
            let (mut int_base, mut real_base, mut param_base) = (0usize, 0usize, 0usize);
            for &ci in &cone {
                let (module_name, in_nets, out_nets, params, reg_inits) = {
                    let cand = &self.fusion_candidates[ci];
                    (
                        cand.module_name.clone(),
                        cand.in_nets.clone(),
                        cand.out_nets.clone(),
                        cand.params.clone(),
                        cand.reg_inits.clone(),
                    )
                };
                fused_device_idxs.insert(self.fusion_candidates[ci].device_index);
                let module = self.module_arc(&module_name)?;
                let layout = crate::kernel::digital::DigitalLayout::build(
                    &module,
                    module.digital.as_ref().ok_or_else(|| {
                        CodegenError::Invalid(format!("`{module_name}` has no digital body"))
                    })?,
                );
                let member_param_base = param_base;
                param_base += params.len();
                members.push(NetworkMember {
                    module,
                    in_nets,
                    out_nets,
                    params,
                    int_base,
                    real_base,
                    param_base: member_param_base,
                    reg_inits,
                });
                int_base += layout.num_int_slots();
                real_base += layout.num_real_slots();
            }
            let source = self.fusion_candidates[cone[0]].device_index;
            networks.push(Box::new(DigitalNetwork::build(members, net_count, source)?));
        }
        if networks.is_empty() {
            return Ok(());
        }
        self.build_info.fused_networks = networks.len();
        let kept: Vec<Box<dyn Element>> = std::mem::take(&mut self.devices)
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !fused_device_idxs.contains(i))
            .map(|(_, d)| d)
            .collect();
        self.devices = kept;
        self.devices.extend(networks);
        Ok(())
    }
}
