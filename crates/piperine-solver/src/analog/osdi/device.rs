use crate::analog::device::{
    AnalogDevice, EvalFlags, InitError, InitInfo, SimFlags, SimParams, ErrorCode, ErrorPayload, SimInfo
};
use crate::analog::osdi::ffi::{
    OsdiDescriptor, OsdiInitInfo, OsdiSimInfo, OsdiSimParasOwned, SCRATCH,
    PARA_KIND_MASK, PARA_KIND_OPVAR, PARA_KIND_MODEL, PARA_TY_MASK, PARA_TY_REAL, PARA_TY_STR, PARA_TY_INT,
    ACCESS_FLAG_SET, ACCESS_FLAG_INSTANCE, JACOBIAN_ENTRY_RESIST, JACOBIAN_ENTRY_RESIST_CONST, JACOBIAN_ENTRY_REACT, JACOBIAN_ENTRY_REACT_CONST,
};
use crate::analog::osdi::loader::OsdiLib;
use crate::analog::netlist::{AnalogReference, NodeIdentifier};
use std::os::raw::{c_void, c_char};
use std::sync::Arc;

pub struct OsdiDevice {
    pub name: String,
    pub lib: Arc<OsdiLib>,
    pub descriptor_idx: usize,
    pub terminals: Vec<NodeIdentifier>,
    pub params: Vec<(String, f64)>,
    pub str_params: Vec<(String, String)>,
}

impl OsdiDevice {
    pub fn new(
        name: String,
        lib: Arc<OsdiLib>,
        descriptor_idx: usize,
        terminals: Vec<NodeIdentifier>,
    ) -> Self {
        Self { name, lib, descriptor_idx, terminals, params: Vec::new(), str_params: Vec::new() }
    }

    pub fn new_with_params(
        name: String,
        lib: Arc<OsdiLib>,
        descriptor_idx: usize,
        terminals: Vec<NodeIdentifier>,
        params: Vec<(String, f64)>,
    ) -> Self {
        Self { name, lib, descriptor_idx, terminals, params, str_params: Vec::new() }
    }

    pub fn new_with_all_params(
        name: String,
        lib: Arc<OsdiLib>,
        descriptor_idx: usize,
        terminals: Vec<NodeIdentifier>,
        params: Vec<(String, f64)>,
        str_params: Vec<(String, String)>,
    ) -> Self {
        Self { name, lib, descriptor_idx, terminals, params, str_params }
    }

    fn desc(&self) -> &OsdiDescriptor {
        self.lib.descriptor(self.descriptor_idx)
    }

    fn make_sim_paras_owned(paras: &SimParams) -> OsdiSimParasOwned {
        OsdiSimParasOwned::new(
            &crate::solver::Context {
                gmin: paras.gmin,
                reltol: paras.reltol,
                vntol: paras.vntol,
                abstol: paras.abstol,
                tnom: paras.tnom,
                ..Default::default()
            },
            paras.ini_lim
        )
    }
}

fn set_params_via_access(
    desc: &OsdiDescriptor,
    model_data: &mut Vec<u8>,
    inst_data: &mut Vec<u8>,
    params: &[(String, f64)],
    model_only: bool,
) -> Vec<bool> {
    let total = (desc.num_params + desc.num_opvars) as usize;
    if total == 0 || desc.param_opvar.is_null() { return vec![false; params.len()]; }
    let po_slice = unsafe { std::slice::from_raw_parts(desc.param_opvar, total) };
    let mut found_vec = Vec::with_capacity(params.len());
    for (param_name, value) in params {
        let mut found = false;
        'outer: for (id, po) in po_slice.iter().enumerate() {
            let kind = po.flags & PARA_KIND_MASK;
            let ty = po.flags & PARA_TY_MASK;
            if kind == PARA_KIND_OPVAR || (ty != PARA_TY_REAL && ty != PARA_TY_INT) { continue; }
            let is_model_kind = kind == PARA_KIND_MODEL;
            if model_only != is_model_kind { continue; }
            let alias_matches = (0..=po.num_alias as usize).any(|k| {
                if po.name.is_null() { return false; }
                let name_ptr = unsafe { po.name.add(k).read() };
                if name_ptr.is_null() { return false; }
                let s = unsafe { std::ffi::CStr::from_ptr(name_ptr) }.to_string_lossy();
                s.eq_ignore_ascii_case(param_name)
            });
            if !alias_matches { continue; }
            if let Some(access_fn) = desc.access {
                let flags = if is_model_kind { ACCESS_FLAG_SET } else { ACCESS_FLAG_SET | ACCESS_FLAG_INSTANCE };
                let ptr = unsafe { access_fn(inst_data.as_mut_ptr() as *mut c_void, model_data.as_mut_ptr() as *mut c_void, id as u32, flags) };
                if !ptr.is_null() {
                    match ty {
                        PARA_TY_INT => unsafe { (ptr as *mut i32).write_unaligned(*value as i32); },
                        _ => unsafe { (ptr as *mut f64).write_unaligned(*value); },
                    }
                    if is_model_kind {
                        if let Some(f) = desc.given_flag_model { unsafe { f(model_data.as_mut_ptr() as *mut c_void, id as u32); } }
                    } else if let Some(f) = desc.given_flag_instance {
                        unsafe { f(inst_data.as_mut_ptr() as *mut c_void, id as u32); }
                    }
                    found = true;
                }
            }
            break 'outer;
        }
        found_vec.push(found);
    }
    found_vec
}

fn set_str_params_via_access(
    desc: &OsdiDescriptor,
    model_data: &mut Vec<u8>,
    inst_data: &mut Vec<u8>,
    str_params: &[(String, String)],
) {
    if str_params.is_empty() || desc.param_opvar.is_null() { return; }
    let total = (desc.num_params + desc.num_opvars) as usize;
    if total == 0 { return; }
    let po_slice = unsafe { std::slice::from_raw_parts(desc.param_opvar, total) };

    let live_cstrings: Vec<std::ffi::CString> = str_params.iter().map(|(_, v)| std::ffi::CString::new(v.as_str()).unwrap_or_default()).collect();

    for ((param_name, _), cstr) in str_params.iter().zip(live_cstrings.iter()) {
        'outer: for (id, po) in po_slice.iter().enumerate() {
            let kind = po.flags & PARA_KIND_MASK;
            if kind == PARA_KIND_OPVAR || po.flags & PARA_TY_MASK != PARA_TY_STR { continue; }
            let is_model_kind = kind == PARA_KIND_MODEL;
            let alias_matches = (0..=po.num_alias as usize).any(|k| {
                let alias_ptr = unsafe { po.name.add(k).read_unaligned() };
                if alias_ptr.is_null() { return false; }
                let alias = unsafe { std::ffi::CStr::from_ptr(alias_ptr) };
                alias.to_str().map(|s| s.eq_ignore_ascii_case(param_name)).unwrap_or(false)
            });
            if !alias_matches { continue; }
            if let Some(access_fn) = desc.access {
                let flags = if is_model_kind { ACCESS_FLAG_SET } else { ACCESS_FLAG_SET | ACCESS_FLAG_INSTANCE };
                let ptr = unsafe { access_fn(inst_data.as_mut_ptr() as *mut c_void, model_data.as_mut_ptr() as *mut c_void, id as u32, flags) };
                if !ptr.is_null() {
                    let str_ptr: *const c_char = cstr.as_ptr();
                    unsafe { (ptr as *mut *const c_char).write_unaligned(str_ptr) };
                    if is_model_kind {
                        if let Some(f) = desc.given_flag_model { unsafe { f(model_data.as_mut_ptr() as *mut c_void, id as u32); } }
                    } else if let Some(f) = desc.given_flag_instance {
                        unsafe { f(inst_data.as_mut_ptr() as *mut c_void, id as u32); }
                    }
                }
            }
            break 'outer;
        }
    }
}

impl AnalogDevice for OsdiDevice {
    type ModelData = Vec<u8>;
    type InstanceData = Vec<u8>;

    fn name(&self) -> &str { &self.name }
    fn num_nodes(&self) -> usize { self.desc().num_nodes as usize }
    fn num_terminals(&self) -> usize { self.desc().num_terminals as usize }
    fn num_states(&self) -> usize { (self.desc().num_states as usize).max(1) }
    fn instance_size(&self) -> usize { self.desc().instance_size as usize }
    fn num_resistive_jacobian_entries(&self) -> usize { self.desc().num_resistive_jacobian_entries as usize }
    fn num_reactive_jacobian_entries(&self) -> usize { self.desc().num_reactive_jacobian_entries as usize }

    fn setup_model(&self, model: &mut Self::ModelData, paras: &SimParams, info: &mut InitInfo) {
        if model.is_empty() { model.resize(self.desc().model_size as usize, 0); }
        let sp = Self::make_sim_paras_owned(paras);
        let ffi_paras = sp.as_raw();
        let mut ffi_errors = Vec::with_capacity(32);
        let mut ffi_info = OsdiInitInfo { flags: info.flags.bits(), num_errors: 0, errors: ffi_errors.as_mut_ptr() };
        if let Some(setup) = self.desc().setup_model {
            unsafe { setup(std::ptr::null_mut(), model.as_mut_ptr() as *mut c_void, &ffi_paras, &mut ffi_info); }
        }
        unsafe {
            ffi_errors.set_len(ffi_info.num_errors as usize);
            for err in ffi_errors { info.push_error(InitError::Generic(ErrorCode(err.code), ErrorPayload(err.payload))); }
        }
    }

    fn setup_instance(&self, model: &Self::ModelData, instance: &mut Self::InstanceData, temp: f64, flags: SimFlags, paras: &SimParams, info: &mut InitInfo) {
        if instance.is_empty() { instance.resize(self.desc().instance_size as usize, 0); }
        let sp = Self::make_sim_paras_owned(paras);
        let ffi_paras = sp.as_raw();
        let mut ffi_errors = Vec::with_capacity(32);
        let mut ffi_info = OsdiInitInfo { flags: flags.bits(), num_errors: 0, errors: ffi_errors.as_mut_ptr() };
        if let Some(setup) = self.desc().setup_instance {
            unsafe { setup(std::ptr::null_mut(), instance.as_mut_ptr() as *mut c_void, model.as_ptr() as *mut c_void, temp, self.desc().num_terminals as u32, &ffi_paras, &mut ffi_info); }
        }
        unsafe {
            ffi_errors.set_len(ffi_info.num_errors as usize);
            for err in ffi_errors { info.push_error(InitError::Generic(ErrorCode(err.code), ErrorPayload(err.payload))); }
        }
    }

    fn allocate_nodes(&self, instance_name: &str, terminals: &[NodeIdentifier], netlist: &mut crate::analog::netlist::Netlist) -> Vec<Option<AnalogReference>> {
        let desc = self.desc();
        let num_nodes = desc.num_nodes as usize;
        let num_terminals = desc.num_terminals as usize;
        let mut node_refs = Vec::with_capacity(num_nodes);

        static INTERNAL_NODE_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
        fn alloc_internal_node() -> NodeIdentifier {
            NodeIdentifier::Anonymous(INTERNAL_NODE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
        }

        for i in 0..num_nodes {
            let osdi_node = unsafe { &*desc.nodes.add(i) };
            let is_flow = osdi_node.is_flow;

            let cref = if i < num_terminals {
                netlist.connect_node(terminals[i].clone())
            } else if is_flow {
                let node_name = if osdi_node.name.is_null() {
                    format!("br_{}", i)
                } else {
                    unsafe { std::ffi::CStr::from_ptr(osdi_node.name) }.to_string_lossy().into_owned()
                };
                let branch_id = crate::analog::netlist::BranchIdentifier::new(instance_name.to_string(), node_name);
                netlist.connect_branch(branch_id)
            } else {
                netlist.connect_node(alloc_internal_node())
            };
            node_refs.push(if cref.idx().is_some() { Some(cref) } else { None });
        }
        node_refs
    }

    fn set_params(&self, model: &mut Self::ModelData, instance: &mut Self::InstanceData, params: &[(String, f64)], str_params: &[(String, String)]) {
        set_params_via_access(self.desc(), model, instance, params, true);
        set_params_via_access(self.desc(), model, instance, params, false);
        set_str_params_via_access(self.desc(), model, instance, str_params);
    }

    fn bind_nodes(&self, instance: &mut Self::InstanceData, node_refs: &mut Vec<Option<AnalogReference>>) {
        let desc = self.desc();

        let node_map_base = desc.node_mapping_offset as usize;
        for i in 0..desc.num_nodes as usize {
            let mapping_val: u32 = match node_refs.get(i).and_then(|r| r.as_ref()) {
                Some(r) => match r.idx() { Some(k) => (k + 1) as u32, None => 0 },
                None => 0
            };
            let offset = node_map_base + i * 4;
            instance[offset..offset + 4].copy_from_slice(&mapping_val.to_ne_bytes());
        }

        // Initialize state variable indices so eval correctly addresses prev_state/next_state.
        let num_states = desc.num_states as usize;
        let state_idx_base = desc.state_idx_off as usize;
        if num_states > 0 && state_idx_base != 0xFFFFFFFF {
            for i in 0..num_states {
                let off = state_idx_base + i * 4;
                if off + 4 <= instance.len() {
                    instance[off..off + 4].copy_from_slice(&(i as u32).to_ne_bytes());
                }
            }
        }

        let num_collapsible = desc.num_collapsible as usize;
        if num_collapsible == 0 || desc.collapsible.is_null() { return; }
        let collapsed_base = desc.collapsed_offset as usize;
        let num_terminals = desc.num_terminals as usize;

        for i in 0..num_collapsible {
            if collapsed_base + i >= instance.len() || instance[collapsed_base + i] == 0 { continue; }
            let pair = unsafe { &*desc.collapsible.add(i) };
            let from = pair.node_1 as usize;
            let to_raw = pair.node_2;
            if from < num_terminals { continue; }

            let to_ref = if to_raw == u32::MAX { None } else { node_refs.get(to_raw as usize).and_then(|r| r.clone()) };
            if from < node_refs.len() { node_refs[from] = to_ref; }

            let to_mapping: u32 = if to_raw == u32::MAX { 0 } else {
                let off = node_map_base + to_raw as usize * 4;
                u32::from_ne_bytes(instance[off..off + 4].try_into().unwrap())
            };
            let from_off = node_map_base + from * 4;
            instance[from_off..from_off + 4].copy_from_slice(&to_mapping.to_ne_bytes());
        }
    }

    fn eval(&self, model: &Self::ModelData, instance: &mut Self::InstanceData, sim_info: &mut SimInfo) -> EvalFlags {
        let sp = crate::analog::osdi::ffi::OsdiSimParasOwned::new(
            &crate::solver::Context {
                gmin: sim_info.params.gmin,
                reltol: sim_info.params.reltol,
                vntol: sim_info.params.vntol,
                abstol: sim_info.params.abstol,
                tnom: sim_info.params.tnom,
                ..Default::default()
            },
            sim_info.params.ini_lim,
        );
        let ffi_info = OsdiSimInfo {
            paras: sp.as_raw(),
            abstime: sim_info.abstime,
            prev_solve: sim_info.prev_solve.as_ptr() as *mut f64,
            prev_state: sim_info.prev_state.as_ptr() as *mut f64,
            next_state: sim_info.next_state.as_mut_ptr(),
            flags: sim_info.flags.bits(),
        };
        let result = if let Some(eval_fn) = self.desc().eval {
            unsafe { eval_fn(std::ptr::null_mut(), instance.as_mut_ptr() as *mut c_void, model.as_ptr() as *mut c_void, &ffi_info) }
        } else { 0 };
        EvalFlags::from_bits_truncate(result)
    }

    fn bound_step_hint(&self, instance: &Self::InstanceData) -> f64 {
        let bs_off = self.desc().bound_step_offset as usize;
        if bs_off != 0xFFFFFFFF && bs_off + 8 <= instance.len() {
            f64::from_ne_bytes(instance[bs_off..bs_off + 8].try_into().unwrap())
        } else { f64::INFINITY }
    }

    fn read_opvars(&self, model: &Self::ModelData, instance: &Self::InstanceData) -> Vec<(String, f64)> {
        let desc = self.desc();
        let total = (desc.num_params + desc.num_opvars) as usize;
        if total == 0 || desc.param_opvar.is_null() || desc.access.is_none() { return Vec::new(); }
        let po_slice = unsafe { std::slice::from_raw_parts(desc.param_opvar, total) };
        let access_fn = desc.access.unwrap();
        let mut result = Vec::new();
        for (id, po) in po_slice.iter().enumerate() {
            if po.flags & PARA_KIND_MASK != PARA_KIND_OPVAR { continue; }
            let ptr = unsafe { access_fn(instance.as_ptr() as *mut c_void, model.as_ptr() as *mut c_void, id as u32, ACCESS_FLAG_INSTANCE) };
            if ptr.is_null() { continue; }
            let ty = po.flags & PARA_TY_MASK;
            let value = match ty {
                PARA_TY_INT => unsafe { (ptr as *const i32).read_unaligned() as f64 },
                PARA_TY_REAL => unsafe { (ptr as *const f64).read_unaligned() },
                _ => continue,
            };
            if !po.name.is_null() {
                let name_ptr = unsafe { po.name.add(0).read() };
                if !name_ptr.is_null() {
                    let name = unsafe { std::ffi::CStr::from_ptr(name_ptr) }.to_string_lossy().into_owned();
                    result.push((name, value));
                }
            }
        }
        result
    }

    fn load_residual_resist(&self, model: &Self::ModelData, instance: &Self::InstanceData, rhs: &mut [f64]) {
        if let Some(f) = self.desc().load_residual_resist {
            unsafe { f(instance.as_ptr() as *mut c_void, model.as_ptr() as *mut c_void, rhs.as_mut_ptr()); }
        }
    }

    fn load_residual_react(&self, model: &Self::ModelData, instance: &Self::InstanceData, rhs: &mut [f64]) {
        if let Some(f) = self.desc().load_residual_react {
            unsafe { f(instance.as_ptr() as *mut c_void, model.as_ptr() as *mut c_void, rhs.as_mut_ptr()); }
        }
    }

    fn load_jacobian_resist(&self, model: &Self::ModelData, instance: &Self::InstanceData, jacobian: &mut [f64]) {
        if let Some(f) = self.desc().write_jacobian_array_resist {
            unsafe { f(instance.as_ptr() as *mut c_void, model.as_ptr() as *mut c_void, jacobian.as_mut_ptr()); }
        }
    }

    fn load_jacobian_react(&self, model: &Self::ModelData, instance: &Self::InstanceData, _step: f64, jacobian: &mut [f64]) {
        if let Some(f) = self.desc().write_jacobian_array_react {
            unsafe { f(instance.as_ptr() as *mut c_void, model.as_ptr() as *mut c_void, jacobian.as_mut_ptr()); }
        }
    }

    fn num_noise_sources(&self) -> usize { self.desc().num_noise_src as usize }

    fn noise_source_node_pairs(&self) -> Vec<(usize, usize)> {
        let desc = self.desc();
        let num = desc.num_noise_src as usize;
        if num == 0 || desc.noise_sources.is_null() { return Vec::new(); }
        (0..num).map(|i| {
            let src = unsafe { &*desc.noise_sources.add(i) };
            (src.nodes.node_1 as usize, src.nodes.node_2 as usize)
        }).collect()
    }

    fn load_noise(&self, model: &Self::ModelData, instance: &Self::InstanceData, freq: f64, noise_rhs: &mut [f64]) {
        if let Some(f) = self.desc().load_noise {
            unsafe { f(instance.as_ptr() as *mut c_void, model.as_ptr() as *mut c_void, freq, noise_rhs.as_mut_ptr()); }
        }
    }

    fn get_resist_jac_refs(&self, node_refs: &[Option<AnalogReference>]) -> Vec<(Option<AnalogReference>, Option<AnalogReference>)> {
        let desc = self.desc();
        let num_jac = desc.num_jacobian_entries as usize;
        let mut refs = Vec::new();
        for j in 0..num_jac {
            let entry = unsafe { &*desc.jacobian_entries.add(j) };
            if entry.flags & (JACOBIAN_ENTRY_RESIST | JACOBIAN_ENTRY_RESIST_CONST) != 0 {
                let row = node_refs.get(entry.nodes.node_1 as usize).and_then(|r| r.clone());
                let col = node_refs.get(entry.nodes.node_2 as usize).and_then(|r| r.clone());
                refs.push((row, col));
            }
        }
        refs
    }

    fn get_react_jac_refs(&self, node_refs: &[Option<AnalogReference>]) -> Vec<(Option<AnalogReference>, Option<AnalogReference>)> {
        let desc = self.desc();
        let num_jac = desc.num_jacobian_entries as usize;
        let mut refs = Vec::new();
        for j in 0..num_jac {
            let entry = unsafe { &*desc.jacobian_entries.add(j) };
            if entry.flags & (JACOBIAN_ENTRY_REACT | JACOBIAN_ENTRY_REACT_CONST) != 0 {
                let row = node_refs.get(entry.nodes.node_1 as usize).and_then(|r| r.clone());
                let col = node_refs.get(entry.nodes.node_2 as usize).and_then(|r| r.clone());
                refs.push((row, col));
            }
        }
        refs
    }

    fn get_rhs_indices(&self, instance: &Self::InstanceData) -> Vec<Option<usize>> {
        let desc = self.desc();
        let node_map_base = desc.node_mapping_offset as usize;
        let mut indices = Vec::with_capacity(desc.num_nodes as usize);
        for i in 0..desc.num_nodes as usize {
            let off = node_map_base + i * 4;
            let mapping = u32::from_ne_bytes(instance[off..off + 4].try_into().unwrap()) as usize;
            if mapping == 0 || mapping >= SCRATCH { indices.push(None); }
            else { indices.push(Some(mapping)); }
        }
        indices
    }

    fn build_prev_solve(&self, instance: &Self::InstanceData, node_refs: &[Option<AnalogReference>], state_fn: &dyn Fn(usize) -> f64) -> [f64; SCRATCH] {
        let desc = self.desc();
        let node_map_base = desc.node_mapping_offset as usize;
        let mut prev_solve = [0.0f64; SCRATCH];
        for i in 0..desc.num_nodes as usize {
            let off = node_map_base + i * 4;
            let mapping = u32::from_ne_bytes(instance[off..off + 4].try_into().unwrap()) as usize;
            if mapping == 0 || mapping >= SCRATCH { continue; }
            if let Some(Some(cref)) = node_refs.get(i) {
                if let Some(k) = cref.idx() {
                    prev_solve[mapping] = state_fn(k);
                }
            }
        }
        prev_solve
    }

    fn load_spice_rhs_dc(
        &self,
        model: &Self::ModelData,
        instance: &Self::InstanceData,
        rhs: &mut [f64],
        prev_solve: &[f64],
    ) {
        if let Some(f) = self.desc().load_spice_rhs_dc {
            unsafe { f(instance.as_ptr() as *mut c_void, model.as_ptr() as *mut c_void, rhs.as_mut_ptr(), prev_solve.as_ptr()); }
        }
    }

    fn load_spice_rhs_tran(
        &self,
        model: &Self::ModelData,
        instance: &Self::InstanceData,
        rhs: &mut [f64],
        prev_solve: &[f64],
        alpha: f64,
    ) {
        if let Some(f) = self.desc().load_spice_rhs_tran {
            unsafe { f(instance.as_ptr() as *mut c_void, model.as_ptr() as *mut c_void, rhs.as_mut_ptr(), prev_solve.as_ptr(), alpha); }
        }
    }

    fn collect_rhs_stamps(
        &self,
        instance: &Self::InstanceData,
        rhs: &[f64],
        node_refs: &[Option<AnalogReference>],
        stamps: &mut Vec<crate::math::linear::Stamp<AnalogReference, f64>>,
    ) {
        let desc = self.desc();
        let node_map_base = desc.node_mapping_offset as usize;
        for i in 0..desc.num_nodes as usize {
            let off = node_map_base + i * 4;
            let mapping = u32::from_ne_bytes(instance[off..off + 4].try_into().unwrap()) as usize;
            if mapping == 0 || mapping >= SCRATCH { continue; }
            let val = rhs[mapping];
            if val == 0.0 { continue; }
            if let Some(Some(cref)) = node_refs.get(i) {
                stamps.push(crate::math::linear::Stamp::Rhs(cref.clone(), val));
            }
        }
    }
}
