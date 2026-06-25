use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::netlist::{AnalogReference, NodeIdentifier, Netlist};
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::Stamp;
use crate::solver::Context;
use std::collections::VecDeque;
use std::os::raw::c_void;
use std::sync::Arc;

use crate::osdi::ffi::*;
use crate::osdi::loader::OsdiLib;
use std::os::raw::c_char;// ---------------------------------------------------------------------------
// Parameter setup via access()
// ---------------------------------------------------------------------------

/// `model_only`: if true, only set MODEL-kind params; if false, only set INST-kind params/// Set parameters via the access() function.
/// Returns a Vec<bool> indicating which parameters in `params` were found.
fn set_params_via_access(
    desc: &OsdiDescriptor,
    model_data: &mut Vec<u8>,
    inst_data: &mut Vec<u8>,
    params: &[(String, f64)],
    model_only: bool,
) -> Vec<bool> {
    let num_opvars = desc.num_opvars;
    let num_params = desc.num_params;
    let total = (num_params + num_opvars) as usize;

    if total == 0 || desc.param_opvar.is_null() {
        return vec![false; params.len()];
    }

    let po_slice = unsafe { std::slice::from_raw_parts(desc.param_opvar, total) };
    let mut found_vec = Vec::with_capacity(params.len());

    for (param_name, value) in params {
        let mut found = false;
        'outer: for (id, po) in po_slice.iter().enumerate() {
            let kind = po.flags & PARA_KIND_MASK;
            if kind == PARA_KIND_OPVAR { continue; }
            if po.flags & PARA_TY_MASK != PARA_TY_REAL { continue; }

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
                let flags = if is_model_kind {
                    ACCESS_FLAG_SET
                } else {
                    ACCESS_FLAG_SET | ACCESS_FLAG_INSTANCE
                };
                let ptr = unsafe {
                    access_fn(
                        inst_data.as_mut_ptr() as *mut c_void,
                        model_data.as_mut_ptr() as *mut c_void,
                        id as u32,
                        flags,
                    )
                };
                if !ptr.is_null() {
                    let ty = po.flags & PARA_TY_MASK;
                    unsafe {
                        if ty == PARA_TY_INT {
                            (ptr as *mut i32).write_unaligned(*value as i32);
                        } else {
                            (ptr as *mut f64).write_unaligned(*value);
                        }
                    }
                    if is_model_kind {
                        if let Some(f) = desc.given_flag_model {
                            unsafe { f(model_data.as_mut_ptr() as *mut c_void, id as u32); }
                        }
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

/// Set string-valued parameters via access(). `live_cstrings` keeps them alive.
fn set_str_params_via_access(
    desc: &OsdiDescriptor,
    model_data: &mut Vec<u8>,
    inst_data: &mut Vec<u8>,
    str_params: &[(String, String)],
    live_cstrings: &[std::ffi::CString],
) {
    if str_params.is_empty() || desc.param_opvar.is_null() { return; }
    let total = (desc.num_params + desc.num_opvars) as usize;
    if total == 0 { return; }
    let po_slice = unsafe { std::slice::from_raw_parts(desc.param_opvar, total) };

    for ((param_name, _), cstr) in str_params.iter().zip(live_cstrings.iter()) {
        let mut found = false;
        'outer: for (id, po) in po_slice.iter().enumerate() {
            let kind = po.flags & PARA_KIND_MASK;
            if kind == PARA_KIND_OPVAR { continue; }
            if po.flags & PARA_TY_MASK != PARA_TY_STR { continue; }

            let alias_matches = (0..=po.num_alias as usize).any(|k| {
                let alias_ptr = unsafe { po.name.add(k).read_unaligned() };
                if alias_ptr.is_null() { return false; }
                let alias = unsafe { std::ffi::CStr::from_ptr(alias_ptr) };
                alias.to_str().map(|s| s.eq_ignore_ascii_case(param_name)).unwrap_or(false)
            });
            if !alias_matches { continue; }

            if let Some(access_fn) = desc.access {
                let is_model_kind = kind == PARA_KIND_MODEL;
                let flags = if is_model_kind {
                    ACCESS_FLAG_SET
                } else {
                    ACCESS_FLAG_SET | ACCESS_FLAG_INSTANCE
                };
                // access returns *mut *const c_char; write the CString ptr there.
                let ptr = unsafe {
                    access_fn(
                        inst_data.as_mut_ptr() as *mut c_void,
                        model_data.as_mut_ptr() as *mut c_void,
                        id as u32,
                        flags,
                    )
                };
                if !ptr.is_null() {
                    let str_ptr: *const c_char = cstr.as_ptr();
                    unsafe { (ptr as *mut *const c_char).write_unaligned(str_ptr) };
                    if is_model_kind {
                        if let Some(f) = desc.given_flag_model {
                            unsafe { f(model_data.as_mut_ptr() as *mut c_void, id as u32); }
                        }
                    } else if let Some(f) = desc.given_flag_instance {
                        unsafe { f(inst_data.as_mut_ptr() as *mut c_void, id as u32); }
                    }
                    found = true;
                }
            }
            break 'outer;
        }
        if !found {
            eprintln!("[osdi] WARNING: string param '{}' not found", param_name);
        }
    }
}

// ---------------------------------------------------------------------------
// Node collapsing
// ---------------------------------------------------------------------------

/// After setup_instance sets collapsed[] flags in inst_data, update node_refs
/// so that collapsed nodes share the same circuit node as their target.
fn apply_node_collapsing(
    desc: &OsdiDescriptor,
    inst_data: &mut Vec<u8>,
    node_refs: &mut Vec<Option<AnalogReference>>,
    num_terminals: usize,
) {
    let num_collapsible = desc.num_collapsible as usize;
    if num_collapsible == 0 || desc.collapsible.is_null() { return; }

    let collapsed_base = desc.collapsed_offset as usize;
    let node_map_base = desc.node_mapping_offset as usize;

    for i in 0..num_collapsible {
        // collapsed[i] is a bool (1 byte) written by setup_instance
        if collapsed_base + i >= inst_data.len() { break; }
        if inst_data[collapsed_base + i] == 0 { continue; }

        let pair = unsafe { &*desc.collapsible.add(i) };
        let from = pair.node_1 as usize;
        let to_raw = pair.node_2; // u32::MAX means GND

        // Terminals (from < num_terminals) cannot be collapsed.
        if from < num_terminals { continue; }

        let to_ref: Option<AnalogReference> = if to_raw == u32::MAX {
            None // GND
        } else {
            node_refs.get(to_raw as usize).and_then(|r| r.clone())
        };

        if from < node_refs.len() {
            node_refs[from] = to_ref;
        }

        // Patch node_mapping so eval uses the surviving node's solve slot.
        let to_mapping: u32 = if to_raw == u32::MAX {
            0 // GND = 0
        } else {
            let off = node_map_base + to_raw as usize * 4;
            if off + 4 <= inst_data.len() {
                u32::from_ne_bytes(inst_data[off..off + 4].try_into().unwrap())
            } else {
                0
            }
        };
        let from_off = node_map_base + from * 4;
        if from_off + 4 <= inst_data.len() {
            inst_data[from_off..from_off + 4].copy_from_slice(&to_mapping.to_ne_bytes());
        }
    }
}

// ---------------------------------------------------------------------------
// OsdiRuntime
// ---------------------------------------------------------------------------

pub struct OsdiRuntime {
    pub(crate) lib: Arc<OsdiLib>,
    pub(crate) descriptor_idx: usize,
    pub(crate) device_name: String,
    pub(crate) model_data: Vec<u8>,
    pub(crate) inst_data: Vec<u8>,
    /// One entry per OSDI node (terminals first, then internal). None = GND.
    pub(crate) node_refs: Vec<Option<AnalogReference>>,
    /// (row, col) for each RESISTIVE Jacobian entry (indices into node_refs).
    pub(crate) resist_jac_refs: Vec<(Option<AnalogReference>, Option<AnalogReference>)>,
    /// (row, col) for each REACTIVE Jacobian entry.
    pub(crate) react_jac_refs: Vec<(Option<AnalogReference>, Option<AnalogReference>)>,
    /// State variables for ddt() integration.
    pub(crate) prev_state: Vec<f64>,
    pub(crate) next_state: Vec<f64>,
    /// Last known simulation time — used to detect timestep advancement.
    pub(crate) last_time: f64,
    /// Temperature (K) used in last setup_instance call; re-setup when context differs.
    pub(crate) setup_temperature: f64,
    /// Number of terminals (kept for re-setup).
    pub(crate) num_terminals: usize,
    /// Numeric user params (kept for re-setup).
    pub(crate) params: Vec<(String, f64)>,
    /// String params as CStrings — must stay alive as long as the device is active.
    #[allow(dead_code)] // Kept alive to prevent dangling pointers into OSDI instance data.
    pub(crate) str_params_live: Vec<std::ffi::CString>,
    /// Last max timestep suggestion returned by OSDI eval.
    pub(crate) last_bound_step: f64,
    /// History of nodal charges for LTE truncation. Index 0 is most recent.
    pub(crate) charge_history: VecDeque<Vec<f64>>,
    /// True if the last eval returned EVAL_RET_FLAG_LIM.
    pub(crate) limiting_active: bool,
}

impl OsdiRuntime {
    pub fn allocate_osdi(
        lib: Arc<OsdiLib>,
        descriptor_idx: usize,
        device_name: String,
        terminals: &[NodeIdentifier],
        params: &[(String, f64)],
        str_params: &[(String, String)],
        netlist: &mut Netlist,
    ) -> Self {
        let desc = lib.descriptor(descriptor_idx);
        let num_nodes = desc.num_nodes as usize;
        let num_terminals = desc.num_terminals as usize;

        assert_eq!(
            terminals.len(),
            num_terminals,
            "OSDI device: expected {num_terminals} terminals, got {}",
            terminals.len()
        );

        let mut model_data = vec![0u8; desc.model_size as usize];
        let mut inst_data = vec![0u8; desc.instance_size as usize];

        // Connect nodes and write node_mapping into instance data.
        let mut node_refs: Vec<Option<AnalogReference>> = Vec::with_capacity(num_nodes);
        let node_map_base = desc.node_mapping_offset as usize;

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
                let branch_id = crate::circuit::netlist::BranchIdentifier::new(device_name.clone(), node_name);
                netlist.connect_branch(branch_id)
            } else {
                netlist.connect_node(alloc_internal_node())
            };
            let mapping_val: u32 = match cref.idx() {
                Some(k) => (k + 1) as u32,
                None => 0, // GND
            };

            let offset = node_map_base + i * 4;
            inst_data[offset..offset + 4].copy_from_slice(&mapping_val.to_ne_bytes());

            node_refs.push(if cref.idx().is_some() { Some(cref) } else { None });
        }

        // Build separate resist/react Jacobian ref lists based on entry flags.
        let num_jac = desc.num_jacobian_entries as usize;
        let num_res_expected = desc.num_resistive_jacobian_entries as usize;
        let num_react_expected = desc.num_reactive_jacobian_entries as usize;
        let mut resist_jac_refs = Vec::with_capacity(num_res_expected);
        let mut react_jac_refs = Vec::with_capacity(num_react_expected);
        for j in 0..num_jac {
            let entry = unsafe { &*desc.jacobian_entries.add(j) };
            let row = node_refs.get(entry.nodes.node_1 as usize).and_then(|r| r.clone());
            let col = node_refs.get(entry.nodes.node_2 as usize).and_then(|r| r.clone());
            if entry.flags & (JACOBIAN_ENTRY_RESIST | JACOBIAN_ENTRY_RESIST_CONST) != 0
                && resist_jac_refs.len() < num_res_expected
            {
                resist_jac_refs.push((row.clone(), col.clone()));
            }
            if entry.flags & (JACOBIAN_ENTRY_REACT | JACOBIAN_ENTRY_REACT_CONST) != 0
                && react_jac_refs.len() < num_react_expected
            {
                react_jac_refs.push((row, col));
            }
        }
        // If flag-based counting disagrees with OSDI descriptor counts, fall back to index order.
        if resist_jac_refs.len() < num_res_expected {
            resist_jac_refs.resize_with(num_res_expected, || (None, None));
        }
        if react_jac_refs.len() < num_react_expected {
            react_jac_refs.resize_with(num_react_expected, || (None, None));
        }

        let num_states = (desc.num_states as usize).max(1);
        let prev_state = vec![0.0f64; num_states];
        let next_state = vec![0.0f64; num_states];

        let (default_sp, sim_paras) = OsdiSimParasOwned::default_paras();
        let _ = &default_sp;
        let mut init_info = OsdiInitInfo { flags: 0, num_errors: 0, errors: std::ptr::null_mut() };

        if let Some(setup_model) = desc.setup_model {
            unsafe {
                setup_model(
                    std::ptr::null_mut(),
                    model_data.as_mut_ptr() as *mut c_void,
                    &sim_paras,
                    &mut init_info,
                );
            }
            if init_info.num_errors > 0 {
                panic!("[osdi] setup_model failed with {} errors for device '{}'", init_info.num_errors, device_name);
            }
        }
        // Set MODEL params after setup_model (which initializes defaults).
        let found_model = set_params_via_access(desc, &mut model_data, &mut inst_data, params, true);

        const DEFAULT_TEMP: f64 = 300.15;
        if let Some(setup_instance) = desc.setup_instance {
            unsafe {
                setup_instance(
                    std::ptr::null_mut(),
                    inst_data.as_mut_ptr() as *mut c_void,
                    model_data.as_mut_ptr() as *mut c_void,
                    DEFAULT_TEMP,
                    num_terminals as u32,
                    &sim_paras,
                    &mut init_info,
                );
            }
            if init_info.num_errors > 0 {
                panic!("[osdi] setup_instance failed with {} errors for device '{}'", init_info.num_errors, device_name);
            }
        }
        // Set INST params after setup_instance (which initializes instance defaults).
        let found_inst = set_params_via_access(desc, &mut model_data, &mut inst_data, params, false);

        for (i, (param_name, _)) in params.iter().enumerate() {
            if !found_model[i] && !found_inst[i] {
                eprintln!("[osdi] WARNING: param '{}' not found in device '{}'", param_name, device_name);
            }
        }

        // Build CStrings for string params and set them.
        let str_params_live: Vec<std::ffi::CString> = str_params
            .iter()
            .map(|(_, v)| std::ffi::CString::new(v.as_str()).unwrap_or_default())
            .collect();
        set_str_params_via_access(
            desc, &mut model_data, &mut inst_data, str_params, &str_params_live,
        );

        // Apply node collapsing: setup_instance wrote collapsed[] flags; update node_refs.
        apply_node_collapsing(desc, &mut inst_data, &mut node_refs, num_terminals);

        Self {
            lib,
            descriptor_idx,
            device_name,
            model_data,
            inst_data,
            node_refs,
            resist_jac_refs,
            react_jac_refs,
            prev_state,
            next_state,
            last_time: f64::NEG_INFINITY,
            setup_temperature: DEFAULT_TEMP,
            num_terminals,
            params: params.to_vec(),
            str_params_live,
            last_bound_step: f64::INFINITY,
            charge_history: VecDeque::new(),
            limiting_active: false,
        }
    }

    pub(crate) fn desc(&self) -> &OsdiDescriptor {
        self.lib.descriptor(self.descriptor_idx)
    }

    pub(crate) fn build_prev_solve(&self, state: &CircularArrayBuffer2<f64>) -> [f64; SCRATCH] {
        let desc = self.desc();
        let node_map_base = desc.node_mapping_offset as usize;
        let mut prev_solve = [0.0f64; SCRATCH];
        for i in 0..desc.num_nodes as usize {
            let off = node_map_base + i * 4;
            let mapping =
                u32::from_ne_bytes(self.inst_data[off..off + 4].try_into().unwrap()) as usize;
            if mapping == 0 || mapping >= SCRATCH { continue; }
            if let Some(Some(cref)) = self.node_refs.get(i) {
                if let Some(k) = cref.idx() {
                    prev_solve[mapping] =
                        state.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0);
                }
            }
        }
        prev_solve
    }

    pub(crate) fn build_prev_solve_from_dc(&self, dc: &DcAnalysisResult) -> [f64; SCRATCH] {
        let desc = self.desc();
        let node_map_base = desc.node_mapping_offset as usize;
        let mut prev_solve = [0.0f64; SCRATCH];
        for i in 0..desc.num_nodes as usize {
            let off = node_map_base + i * 4;
            let mapping =
                u32::from_ne_bytes(self.inst_data[off..off + 4].try_into().unwrap()) as usize;
            if mapping == 0 || mapping >= SCRATCH { continue; }
            if let Some(Some(cref)) = self.node_refs.get(i) {
                prev_solve[mapping] = dc.get(cref.variable().clone()).unwrap_or(0.0);
            }
        }
        prev_solve
    }



    pub(crate) fn collect_rhs_stamps(
        &self,
        rhs: &[f64; SCRATCH],
        stamps: &mut Vec<Stamp<AnalogReference, f64>>,
    ) {
        let desc = self.desc();
        let node_map_base = desc.node_mapping_offset as usize;
        for i in 0..desc.num_nodes as usize {
            let off = node_map_base + i * 4;
            let mapping =
                u32::from_ne_bytes(self.inst_data[off..off + 4].try_into().unwrap()) as usize;
            if mapping == 0 || mapping >= SCRATCH { continue; }
            let val = rhs[mapping];
            if val == 0.0 { continue; }
            if let Some(Some(cref)) = self.node_refs.get(i) {
                stamps.push(Stamp::Rhs(cref.clone(), val));
            }
        }
    }

    pub(crate) fn add_resist_jac_stamps(&self, stamps: &mut Vec<Stamp<AnalogReference, f64>>) {
        let desc = self.desc();
        let num_res = desc.num_resistive_jacobian_entries as usize;
        if num_res == 0 { return; }
        let mut jac = vec![0.0f64; num_res];
        let inst = self.inst_data.as_ptr() as *mut c_void;
        let model = self.model_data.as_ptr() as *mut c_void;
        unsafe {
            if let Some(f) = desc.write_jacobian_array_resist {
                f(inst, model, jac.as_mut_ptr());
            }
        }
        for (j, (row, col)) in self.resist_jac_refs.iter().enumerate() {
            if j >= num_res { break; }
            let val = jac[j];
            if val == 0.0 { continue; }
            if let (Some(r), Some(c)) = (row, col) {
                stamps.push(Stamp::Matrix(r.clone(), c.clone(), val));
            }
        }
    }

    pub(crate) fn add_react_jac_stamps_scaled(
        &self,
        stamps: &mut Vec<Stamp<AnalogReference, f64>>,
        scale: f64,
    ) {
        if scale == 0.0 { return; }
        let desc = self.desc();
        let num_react = desc.num_reactive_jacobian_entries as usize;
        if num_react == 0 { return; }
        let mut jac = vec![0.0f64; num_react];
        let inst = self.inst_data.as_ptr() as *mut c_void;
        let model = self.model_data.as_ptr() as *mut c_void;
        unsafe {
            if let Some(f) = desc.write_jacobian_array_react {
                f(inst, model, jac.as_mut_ptr());
            }
        }
        for (j, (row, col)) in self.react_jac_refs.iter().enumerate() {
            if j >= num_react { break; }
            let val = jac[j] * scale;
            if val == 0.0 { continue; }
            if let (Some(r), Some(c)) = (row, col) {
                stamps.push(Stamp::Matrix(r.clone(), c.clone(), val));
            }
        }
    }

    pub(crate) fn eval_with_flags(&mut self, flags: u32, prev_solve: &[f64; SCRATCH], context: &Context) -> u32 {
        let sp = OsdiSimParasOwned::new(context, flags & INIT_LIM != 0);
        let paras = sp.as_raw();
        let sim_info = OsdiSimInfo {
            paras,
            abstime: context.time,
            prev_solve: prev_solve.as_ptr() as *mut f64,
            prev_state: self.prev_state.as_ptr() as *mut f64,
            next_state: self.next_state.as_ptr() as *mut f64,
            flags,
        };
        let inst = self.inst_data.as_ptr() as *mut c_void;
        let model = self.model_data.as_ptr() as *mut c_void;
        let ret = unsafe {
            self.desc().eval
                .map(|f| f(std::ptr::null_mut(), inst, model, &sim_info))
                .unwrap_or(0)
        };
        if ret & EVAL_RET_FLAG_FATAL != 0 {
            panic!("[osdi] $fatal called in device '{}' (model: '{}')",
                self.device_name,
                unsafe { std::ffi::CStr::from_ptr(self.desc().name).to_string_lossy() },
            );
        }
        self.limiting_active = (ret & EVAL_RET_FLAG_LIM) != 0;
        ret
    }

    pub(crate) fn eval_tran(
        &mut self,
        abstime: f64,
        prev_solve: &[f64; SCRATCH],
        context: &Context,
    ) -> u32 {
        let sp = OsdiSimParasOwned::new(context, false);
        let paras = sp.as_raw();
        let sim_info = OsdiSimInfo {
            paras,
            abstime,
            prev_solve: prev_solve.as_ptr() as *mut f64,
            prev_state: self.prev_state.as_ptr() as *mut f64,
            next_state: self.next_state.as_ptr() as *mut f64,
            flags: ENABLE_LIM
                | CALC_RESIST_LIM_RHS
                | CALC_REACT_LIM_RHS
                | CALC_RESIST_RESIDUAL
                | CALC_REACT_RESIDUAL
                | CALC_RESIST_JACOBIAN
                | CALC_REACT_JACOBIAN
                | ANALYSIS_TRAN,
        };
        let inst = self.inst_data.as_ptr() as *mut c_void;
        let model = self.model_data.as_ptr() as *mut c_void;
        let ret = unsafe {
            self.desc().eval
                .map(|f| f(std::ptr::null_mut(), inst, model, &sim_info))
                .unwrap_or(0)
        };
        self.limiting_active = (ret & EVAL_RET_FLAG_LIM) != 0;

        let bs_off = self.desc().bound_step_offset as usize;
        if bs_off != 0xFFFFFFFF && bs_off + 8 <= self.inst_data.len() {
            let slice = &self.inst_data[bs_off..bs_off + 8];
            self.last_bound_step = f64::from_ne_bytes(slice.try_into().unwrap());
        }

        ret
    }

    fn rerun_setup_instance(&mut self, temperature: f64) {
        let desc = self.lib.descriptor(self.descriptor_idx);
        let (default_sp, sim_paras) = OsdiSimParasOwned::default_paras();
        let _ = &default_sp;
        let mut init_info = OsdiInitInfo { flags: 0, num_errors: 0, errors: std::ptr::null_mut() };
        if let Some(f) = desc.setup_instance {
            unsafe {
                f(
                    std::ptr::null_mut(),
                    self.inst_data.as_mut_ptr() as *mut c_void,
                    self.model_data.as_mut_ptr() as *mut c_void,
                    temperature,
                    self.num_terminals as u32,
                    &sim_paras,
                    &mut init_info,
                );
            }
            if init_info.num_errors > 0 {
                panic!("[osdi] rerun_setup_instance failed with {} errors for device '{}'", init_info.num_errors, self.device_name);
            }
        }
        let _ = set_params_via_access(desc, &mut self.model_data, &mut self.inst_data, &self.params, false);
        apply_node_collapsing(desc, &mut self.inst_data, &mut self.node_refs, self.num_terminals);
        self.setup_temperature = temperature;
    }
    pub fn update(&mut self, _state: &CircularArrayBuffer2<f64>, context: &Context) {
        // When time advances, carry next_state → prev_state (new timestep begins).
        if (context.time - self.last_time).abs() > 1e-20 {
            self.prev_state.copy_from_slice(&self.next_state);
            self.last_time = context.time;
        }
        // Re-run setup_instance when temperature changes (more than 0.01 K).
        if (context.temperature - self.setup_temperature).abs() > 0.01 {
            self.rerun_setup_instance(context.temperature);
        }
    }

    pub fn accept_timestep(&mut self, _state: &CircularArrayBuffer2<f64>, _context: &Context) {
        let mut rhs = [0.0f64; SCRATCH];
        let inst = self.inst_data.as_ptr() as *mut c_void;
        let model = self.model_data.as_ptr() as *mut c_void;
        unsafe {
            if let Some(f) = self.desc().load_residual_react {
                f(inst, model, rhs.as_mut_ptr());
            }
        }

        let mut charges = Vec::with_capacity(self.node_refs.len());
        let node_map_base = self.desc().node_mapping_offset as usize;
        for i in 0..self.desc().num_nodes as usize {
            let off = node_map_base + i * 4;
            let mapping = u32::from_ne_bytes(self.inst_data[off..off + 4].try_into().unwrap()) as usize;
            if mapping == 0 || mapping >= SCRATCH { 
                charges.push(0.0);
            } else {
                charges.push(rhs[mapping]);
            }
        }

        self.charge_history.push_front(charges);
        if self.charge_history.len() > 10 {
            self.charge_history.pop_back();
        }
    }

    pub fn bound_step_hint(&self) -> f64 { self.last_bound_step }

    /// Read all operating variables from the OSDI device.
    /// Returns a Vec of (name, value) pairs.
    pub fn read_opvars(&self) -> Vec<(String, f64)> {
        let desc = self.desc();
        let total = (desc.num_params + desc.num_opvars) as usize;

        if total == 0 || desc.param_opvar.is_null() || desc.access.is_none() {
            return Vec::new();
        }

        let po_slice = unsafe { std::slice::from_raw_parts(desc.param_opvar, total) };
        let access_fn = desc.access.unwrap();
        let mut result = Vec::new();

        for (id, po) in po_slice.iter().enumerate() {
            let kind = po.flags & PARA_KIND_MASK;
            if kind != PARA_KIND_OPVAR {
                continue;
            }

            // Read without ACCESS_FLAG_SET; instance opvars need ACCESS_FLAG_INSTANCE.
            let ptr = unsafe {
                access_fn(
                    self.inst_data.as_ptr() as *mut c_void,
                    self.model_data.as_ptr() as *mut c_void,
                    id as u32,
                    ACCESS_FLAG_INSTANCE,
                )
            };

            if ptr.is_null() {
                continue;
            }

            let ty = po.flags & PARA_TY_MASK;
            let value = match ty {
                PARA_TY_INT => unsafe { (ptr as *const i32).read_unaligned() as f64 },
                PARA_TY_REAL => unsafe { (ptr as *const f64).read_unaligned() },
                _ => continue,
            };

            // Get the name from the first alias (index 0).
            let name = if !po.name.is_null() {
                let name_ptr = unsafe { po.name.add(0).read() };
                if !name_ptr.is_null() {
                    unsafe { std::ffi::CStr::from_ptr(name_ptr) }
                        .to_string_lossy()
                        .into_owned()
                } else {
                    continue;
                }
            } else {
                continue;
            };

            result.push((name, value));
        }

        result
    }

    /// Set the device temperature, re-running setup_instance if the
    /// new temperature differs from the current one by more than 0.01 K.
    pub fn set_temperature(&mut self, temperature: f64) {
        if (temperature - self.setup_temperature).abs() > 0.01 {
            self.rerun_setup_instance(temperature);
        }
    }
}

// ---------------------------------------------------------------------------

