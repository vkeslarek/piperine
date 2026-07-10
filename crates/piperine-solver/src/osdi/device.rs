use crate::digital::LogicValue;
use std::collections::VecDeque;
use std::os::raw::{c_char, c_void};
use std::sync::Arc;

use num_complex::Complex64;

use crate::analysis::ac::AcAnalysisContext;
use crate::analysis::dc::{DcAnalysisResult, DcAnalysisState};
use crate::analysis::noise::Noise;
use crate::analysis::transient::{TransientAnalysisContext, TransientAnalysisState};
use crate::analog::{AnalogReference, BranchIdentifier, NodeIdentifier, Netlist};
use crate::core::device::Device;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::Stamp;
use crate::osdi::ffi::{
    OsdiDescriptor, OsdiInitInfo, OsdiSimInfo, OsdiSimParasOwned, SCRATCH,
    PARA_KIND_MASK, PARA_KIND_OPVAR, PARA_KIND_MODEL, PARA_TY_MASK, PARA_TY_REAL, PARA_TY_STR, PARA_TY_INT,
    ACCESS_FLAG_SET, ACCESS_FLAG_INSTANCE,
    JACOBIAN_ENTRY_RESIST, JACOBIAN_ENTRY_RESIST_CONST, JACOBIAN_ENTRY_REACT, JACOBIAN_ENTRY_REACT_CONST,
    CALC_RESIST_RESIDUAL, CALC_REACT_RESIDUAL, CALC_RESIST_JACOBIAN, CALC_REACT_JACOBIAN,
    CALC_RESIST_LIM_RHS, CALC_REACT_LIM_RHS, ENABLE_LIM, ANALYSIS_DC, ANALYSIS_AC, ANALYSIS_TRAN,
    CALC_NOISE,
};
use crate::osdi::loader::OsdiLib;
use crate::solver::Context;

pub struct OsdiDevice {
    // Config — set at construction, used in Circuit
    pub lib: Arc<OsdiLib>,
    pub descriptor_idx: usize,
    pub name: String,
    pub terminals: Vec<NodeIdentifier>,
    pub params: Vec<(String, f64)>,
    pub str_params: Vec<(String, String)>,

    // Runtime state — populated by initialize()
    pub model_data: Vec<u8>,
    pub inst_data: Vec<u8>,
    pub node_refs: Vec<Option<AnalogReference>>,

    prev_state: Vec<f64>,
    next_state: Vec<f64>,
    last_time: f64,
    setup_temperature: f64,
    charge_history: VecDeque<Vec<f64>>,
    limiting_active: bool,
}

// ---------------------------------------------------------------------------
// Constructors (config only — call initialize() before simulating)
// ---------------------------------------------------------------------------

impl OsdiDevice {
    pub fn new(
        name: String,
        lib: Arc<OsdiLib>,
        descriptor_idx: usize,
        terminals: Vec<NodeIdentifier>,
    ) -> Self {
        Self::with_params(name, lib, descriptor_idx, terminals, Vec::new(), Vec::new())
    }

    pub fn new_with_params(
        name: String,
        lib: Arc<OsdiLib>,
        descriptor_idx: usize,
        terminals: Vec<NodeIdentifier>,
        params: Vec<(String, f64)>,
    ) -> Self {
        Self::with_params(name, lib, descriptor_idx, terminals, params, Vec::new())
    }

    pub fn new_with_all_params(
        name: String,
        lib: Arc<OsdiLib>,
        descriptor_idx: usize,
        terminals: Vec<NodeIdentifier>,
        params: Vec<(String, f64)>,
        str_params: Vec<(String, String)>,
    ) -> Self {
        Self::with_params(name, lib, descriptor_idx, terminals, params, str_params)
    }

    fn with_params(
        name: String,
        lib: Arc<OsdiLib>,
        descriptor_idx: usize,
        terminals: Vec<NodeIdentifier>,
        params: Vec<(String, f64)>,
        str_params: Vec<(String, String)>,
    ) -> Self {
        Self {
            lib, descriptor_idx, name, terminals, params, str_params,
            model_data: Vec::new(),
            inst_data: Vec::new(),
            node_refs: Vec::new(),
            prev_state: Vec::new(),
            next_state: Vec::new(),
            last_time: f64::NEG_INFINITY,
            setup_temperature: 300.15,
            charge_history: VecDeque::new(),
            limiting_active: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Initialization (allocates nodes + runs OSDI setup sequence)
// ---------------------------------------------------------------------------

impl OsdiDevice {
    /// Create from a spec `OsdiDevice` (config-only) and immediately initialize it.
    pub fn from_spec(spec: &OsdiDevice, netlist: &mut Netlist, context: &Context) -> Self {
        let mut d = Self::new_with_all_params(
            spec.name.clone(),
            spec.lib.clone(),
            spec.descriptor_idx,
            spec.terminals.clone(),
            spec.params.clone(),
            spec.str_params.clone(),
        );
        d.initialize(netlist, context);
        d
    }

    pub fn initialize(&mut self, netlist: &mut Netlist, context: &Context) {
        const DEFAULT_TEMP: f64 = 300.15;
        let num_states = {
            let d = self.lib.descriptor(self.descriptor_idx);
            (d.num_states as usize).max(1)
        };
        self.node_refs = {
            let d = self.lib.descriptor(self.descriptor_idx);
            Self::alloc_nodes(d, &self.name, &self.terminals, netlist)
        };
        self.prev_state = vec![0.0f64; num_states];
        self.next_state = vec![0.0f64; num_states];

        self.setup_model_internal(context);
        self.setup_instance_internal(DEFAULT_TEMP, 0, context);
        self.bind_nodes_internal();
        let params = self.params.clone();
        let str_params = self.str_params.clone();
        self.set_params_internal(&params, &str_params);
        self.setup_instance_internal(DEFAULT_TEMP, 0, context);
        let params = self.params.clone();
        let str_params = self.str_params.clone();
        self.set_params_internal(&params, &str_params);
        self.bind_nodes_internal();
    }

    pub fn alloc_nodes(
        desc: &OsdiDescriptor,
        instance_name: &str,
        terminals: &[NodeIdentifier],
        netlist: &mut Netlist,
    ) -> Vec<Option<AnalogReference>> {
        let num_nodes = desc.num_nodes as usize;
        let num_terminals = desc.num_terminals as usize;
        let mut node_refs = Vec::with_capacity(num_nodes);

        static INTERNAL_NODE_COUNTER: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(1);
        fn alloc_internal() -> NodeIdentifier {
            NodeIdentifier::Anonymous(
                INTERNAL_NODE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            )
        }

        for i in 0..num_nodes {
            let osdi_node = unsafe { &*desc.nodes.add(i) };
            let cref = if i < num_terminals {
                netlist.connect_node(terminals[i].clone())
            } else if osdi_node.is_flow {
                let node_name = if osdi_node.name.is_null() {
                    format!("br_{}", i)
                } else {
                    unsafe { std::ffi::CStr::from_ptr(osdi_node.name) }.to_string_lossy().into_owned()
                };
                netlist.connect_branch(BranchIdentifier::new(instance_name.to_string(), node_name))
            } else {
                netlist.connect_node(alloc_internal())
            };
            node_refs.push(if cref.idx().is_some() { Some(cref) } else { None });
        }
        node_refs
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

impl OsdiDevice {
    fn desc(&self) -> &OsdiDescriptor { self.lib.descriptor(self.descriptor_idx) }

    fn setup_model_internal(&mut self, context: &Context) {
        let (model_size, setup_fn) = { let d = self.desc(); (d.model_size, d.setup_model) };
        if self.model_data.is_empty() { self.model_data.resize(model_size as usize, 0); }
        let sp = OsdiSimParasOwned::new(context, false);
        let ffi_paras = sp.as_raw();
        let mut ffi_errors = Vec::with_capacity(32);
        let mut ffi_info = OsdiInitInfo { flags: 0, num_errors: 0, errors: ffi_errors.as_mut_ptr() };
        if let Some(setup) = setup_fn {
            unsafe { setup(std::ptr::null_mut(), self.model_data.as_mut_ptr() as *mut c_void, &ffi_paras, &mut ffi_info); }
        }
    }

    fn setup_instance_internal(&mut self, temp: f64, flags: u32, context: &Context) {
        let (instance_size, num_terminals, setup_fn) = {
            let d = self.desc(); (d.instance_size, d.num_terminals, d.setup_instance)
        };
        if self.inst_data.is_empty() { self.inst_data.resize(instance_size as usize, 0); }
        let sp = OsdiSimParasOwned::new(context, false);
        let ffi_paras = sp.as_raw();
        let mut ffi_errors = Vec::with_capacity(32);
        let mut ffi_info = OsdiInitInfo { flags, num_errors: 0, errors: ffi_errors.as_mut_ptr() };
        if let Some(setup) = setup_fn {
            unsafe { setup(std::ptr::null_mut(), self.inst_data.as_mut_ptr() as *mut c_void, self.model_data.as_ptr() as *mut c_void, temp, num_terminals, &ffi_paras, &mut ffi_info); }
        }
    }

    fn set_params_internal(&mut self, params: &[(String, f64)], str_params: &[(String, String)]) {
        let desc = self.lib.descriptor(self.descriptor_idx);
        Self::set_num_params(desc, &mut self.model_data, &mut self.inst_data, params, true);
        Self::set_num_params(desc, &mut self.model_data, &mut self.inst_data, params, false);
        Self::set_str_params(desc, &mut self.model_data, &mut self.inst_data, str_params);
    }

    fn set_num_params(
        desc: &OsdiDescriptor,
        model_data: &mut Vec<u8>,
        inst_data: &mut Vec<u8>,
        params: &[(String, f64)],
        model_only: bool,
    ) {
        let total = (desc.num_params + desc.num_opvars) as usize;
        if total == 0 || desc.param_opvar.is_null() { return; }
        let po_slice = unsafe { std::slice::from_raw_parts(desc.param_opvar, total) };
        for (param_name, value) in params {
            'outer: for (id, po) in po_slice.iter().enumerate() {
                let kind = po.flags & PARA_KIND_MASK;
                let ty = po.flags & PARA_TY_MASK;
                if kind == PARA_KIND_OPVAR || (ty != PARA_TY_REAL && ty != PARA_TY_INT) { continue; }
                let is_model = kind == PARA_KIND_MODEL;
                if model_only != is_model { continue; }
                let alias_matches = (0..=po.num_alias as usize).any(|k| {
                    if po.name.is_null() { return false; }
                    let name_ptr = unsafe { po.name.add(k).read() };
                    if name_ptr.is_null() { return false; }
                    unsafe { std::ffi::CStr::from_ptr(name_ptr) }.to_string_lossy().eq_ignore_ascii_case(param_name)
                });
                if !alias_matches { continue; }
                if let Some(access_fn) = desc.access {
                    let flags = if is_model { ACCESS_FLAG_SET } else { ACCESS_FLAG_SET | ACCESS_FLAG_INSTANCE };
                    let ptr = unsafe { access_fn(inst_data.as_mut_ptr() as *mut c_void, model_data.as_mut_ptr() as *mut c_void, id as u32, flags) };
                    if !ptr.is_null() {
                        match ty {
                            PARA_TY_INT => unsafe { (ptr as *mut i32).write_unaligned(*value as i32); },
                            _ => unsafe { (ptr as *mut f64).write_unaligned(*value); },
                        }
                        if is_model {
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

    fn set_str_params(
        desc: &OsdiDescriptor,
        model_data: &mut Vec<u8>,
        inst_data: &mut Vec<u8>,
        str_params: &[(String, String)],
    ) {
        if str_params.is_empty() || desc.param_opvar.is_null() { return; }
        let total = (desc.num_params + desc.num_opvars) as usize;
        if total == 0 { return; }
        let po_slice = unsafe { std::slice::from_raw_parts(desc.param_opvar, total) };
        let cstrings: Vec<std::ffi::CString> = str_params.iter()
            .map(|(_, v)| std::ffi::CString::new(v.as_str()).unwrap_or_default())
            .collect();
        for ((param_name, _), cstr) in str_params.iter().zip(cstrings.iter()) {
            'outer: for (id, po) in po_slice.iter().enumerate() {
                let kind = po.flags & PARA_KIND_MASK;
                if kind == PARA_KIND_OPVAR || po.flags & PARA_TY_MASK != PARA_TY_STR { continue; }
                let is_model = kind == PARA_KIND_MODEL;
                let alias_matches = (0..=po.num_alias as usize).any(|k| {
                    let alias_ptr = unsafe { po.name.add(k).read_unaligned() };
                    if alias_ptr.is_null() { return false; }
                    unsafe { std::ffi::CStr::from_ptr(alias_ptr) }.to_str()
                        .map(|s| s.eq_ignore_ascii_case(param_name)).unwrap_or(false)
                });
                if !alias_matches { continue; }
                if let Some(access_fn) = desc.access {
                    let flags = if is_model { ACCESS_FLAG_SET } else { ACCESS_FLAG_SET | ACCESS_FLAG_INSTANCE };
                    let ptr = unsafe { access_fn(inst_data.as_mut_ptr() as *mut c_void, model_data.as_mut_ptr() as *mut c_void, id as u32, flags) };
                    if !ptr.is_null() {
                        unsafe { (ptr as *mut *const c_char).write_unaligned(cstr.as_ptr()) };
                        if is_model {
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

    fn bind_nodes_internal(&mut self) {
        let (node_map_base, num_nodes, num_states, state_idx_base,
             num_collapsible, collapsed_base, num_terminals, collapsible_ptr) = {
            let d = self.desc();
            (d.node_mapping_offset as usize, d.num_nodes as usize, d.num_states as usize,
             d.state_idx_off as usize, d.num_collapsible as usize, d.collapsed_offset as usize,
             d.num_terminals as usize, d.collapsible)
        };
        for i in 0..num_nodes {
            let mapping_val: u32 = match self.node_refs.get(i).and_then(|r| r.as_ref()) {
                Some(r) => match r.idx() { Some(k) => (k + 1) as u32, None => 0 },
                None => 0,
            };
            let off = node_map_base + i * 4;
            self.inst_data[off..off + 4].copy_from_slice(&mapping_val.to_ne_bytes());
        }
        if num_states > 0 && state_idx_base != 0xFFFFFFFF {
            for i in 0..num_states {
                let off = state_idx_base + i * 4;
                if off + 4 <= self.inst_data.len() {
                    self.inst_data[off..off + 4].copy_from_slice(&(i as u32).to_ne_bytes());
                }
            }
        }
        if num_collapsible == 0 || collapsible_ptr.is_null() { return; }
        for i in 0..num_collapsible {
            if collapsed_base + i >= self.inst_data.len() || self.inst_data[collapsed_base + i] == 0 { continue; }
            let pair = unsafe { &*collapsible_ptr.add(i) };
            let from = pair.node_1 as usize;
            let to_raw = pair.node_2;
            if from < num_terminals { continue; }
            let to_ref = if to_raw == u32::MAX { None } else { self.node_refs.get(to_raw as usize).and_then(|r| r.clone()) };
            if from < self.node_refs.len() { self.node_refs[from] = to_ref; }
            let to_mapping: u32 = if to_raw == u32::MAX { 0 } else {
                let off = node_map_base + to_raw as usize * 4;
                u32::from_ne_bytes(self.inst_data[off..off + 4].try_into().unwrap())
            };
            let from_off = node_map_base + from * 4;
            self.inst_data[from_off..from_off + 4].copy_from_slice(&to_mapping.to_ne_bytes());
        }
    }

    fn rerun_setup(&mut self, temp: f64, context: &Context) {
        let params = self.params.clone();
        let str_params = self.str_params.clone();
        self.setup_instance_internal(temp, 0, context);
        self.set_params_internal(&params, &str_params);
        self.bind_nodes_internal();
        self.setup_temperature = temp;
    }

    fn eval_with_prev_solve(&mut self, flags: u32, context: &Context, ini_lim: bool, abstime: f64, prev_solve: &[f64; SCRATCH]) -> u32 {
        let sp = OsdiSimParasOwned::new(context, ini_lim);
        let ffi_info = OsdiSimInfo {
            paras: sp.as_raw(),
            abstime,
            prev_solve: prev_solve.as_ptr() as *mut f64,
            prev_state: self.prev_state.as_ptr() as *mut f64,
            next_state: self.next_state.as_mut_ptr(),
            flags,
        };
        if let Some(eval_fn) = self.desc().eval {
            unsafe { eval_fn(std::ptr::null_mut(), self.inst_data.as_mut_ptr() as *mut c_void, self.model_data.as_ptr() as *mut c_void, &ffi_info) }
        } else { 0 }
    }

    fn build_prev_solve(&self, state_fn: &dyn Fn(usize) -> f64) -> [f64; SCRATCH] {
        let d = self.desc();
        let node_map_base = d.node_mapping_offset as usize;
        let mut prev_solve = [0.0f64; SCRATCH];
        for i in 0..d.num_nodes as usize {
            let off = node_map_base + i * 4;
            if off + 4 > self.inst_data.len() { continue; }
            let mapping = u32::from_ne_bytes(self.inst_data[off..off + 4].try_into().unwrap()) as usize;
            if mapping == 0 || mapping >= SCRATCH { continue; }
            if let Some(Some(cref)) = self.node_refs.get(i)
                && let Some(k) = cref.idx() { prev_solve[mapping] = state_fn(k); }
        }
        prev_solve
    }

    fn collect_rhs_stamps(&self, rhs: &[f64; SCRATCH], stamps: &mut Vec<Stamp<AnalogReference, f64>>) {
        let d = self.desc();
        let node_map_base = d.node_mapping_offset as usize;
        for i in 0..d.num_nodes as usize {
            let off = node_map_base + i * 4;
            if off + 4 > self.inst_data.len() { continue; }
            let mapping = u32::from_ne_bytes(self.inst_data[off..off + 4].try_into().unwrap()) as usize;
            if mapping == 0 || mapping >= SCRATCH { continue; }
            let val = rhs[mapping];
            if val == 0.0 { continue; }
            if let Some(Some(cref)) = self.node_refs.get(i) { stamps.push(Stamp::Rhs(cref.clone(), val)); }
        }
    }

    fn resist_jac_refs(&self) -> Vec<(Option<AnalogReference>, Option<AnalogReference>)> {
        let d = self.desc();
        (0..d.num_jacobian_entries as usize).filter_map(|j| {
            let entry = unsafe { &*d.jacobian_entries.add(j) };
            if entry.flags & (JACOBIAN_ENTRY_RESIST | JACOBIAN_ENTRY_RESIST_CONST) != 0 {
                Some((self.node_refs.get(entry.nodes.node_1 as usize).and_then(|r| r.clone()),
                      self.node_refs.get(entry.nodes.node_2 as usize).and_then(|r| r.clone())))
            } else { None }
        }).collect()
    }

    fn react_jac_refs(&self) -> Vec<(Option<AnalogReference>, Option<AnalogReference>)> {
        let d = self.desc();
        (0..d.num_jacobian_entries as usize).filter_map(|j| {
            let entry = unsafe { &*d.jacobian_entries.add(j) };
            if entry.flags & (JACOBIAN_ENTRY_REACT | JACOBIAN_ENTRY_REACT_CONST) != 0 {
                Some((self.node_refs.get(entry.nodes.node_1 as usize).and_then(|r| r.clone()),
                      self.node_refs.get(entry.nodes.node_2 as usize).and_then(|r| r.clone())))
            } else { None }
        }).collect()
    }

    fn rhs_indices(&self) -> Vec<Option<usize>> {
        let d = self.desc();
        let node_map_base = d.node_mapping_offset as usize;
        (0..d.num_nodes as usize).map(|i| {
            let off = node_map_base + i * 4;
            if off + 4 > self.inst_data.len() { return None; }
            let mapping = u32::from_ne_bytes(self.inst_data[off..off + 4].try_into().unwrap()) as usize;
            if mapping == 0 || mapping >= SCRATCH { None } else { Some(mapping) }
        }).collect()
    }

    fn load_resist_jac(&self) -> Vec<f64> {
        let d = self.desc();
        let mut jac = vec![0.0; d.num_resistive_jacobian_entries as usize];
        if let Some(f) = d.write_jacobian_array_resist {
            unsafe { f(self.inst_data.as_ptr() as *mut c_void, self.model_data.as_ptr() as *mut c_void, jac.as_mut_ptr()); }
        }
        jac
    }

    fn load_react_jac(&self) -> Vec<f64> {
        let d = self.desc();
        let mut jac = vec![0.0; d.num_reactive_jacobian_entries as usize];
        if let Some(f) = d.write_jacobian_array_react {
            unsafe { f(self.inst_data.as_ptr() as *mut c_void, self.model_data.as_ptr() as *mut c_void, jac.as_mut_ptr()); }
        }
        jac
    }
}

// ---------------------------------------------------------------------------
// Device implementation
// ---------------------------------------------------------------------------

impl Device for OsdiDevice {
    fn device_name(&self) -> &str { &self.name }
    fn as_analog(&mut self) -> Option<&mut dyn crate::core::device::AnalogDevice> { Some(self) }
    fn as_analog_ref(&self) -> Option<&dyn crate::core::device::AnalogDevice> { Some(self) }
}

impl crate::core::device::AnalogDevice for OsdiDevice {
    fn limiting_active(&self) -> bool { self.limiting_active }

    fn bound_step_hint(&self) -> f64 {
        let bs_off = self.desc().bound_step_offset as usize;
        if bs_off != 0xFFFFFFFF && bs_off + 8 <= self.inst_data.len() {
            f64::from_ne_bytes(self.inst_data[bs_off..bs_off + 8].try_into().unwrap())
        } else { f64::INFINITY }
    }

    fn read_opvars(&self) -> Vec<(String, f64)> {
        let d = self.desc();
        let total = (d.num_params + d.num_opvars) as usize;
        if total == 0 || d.param_opvar.is_null() || d.access.is_none() { return Vec::new(); }
        let po_slice = unsafe { std::slice::from_raw_parts(d.param_opvar, total) };
        let access_fn = d.access.unwrap();
        let mut result = Vec::new();
        for (id, po) in po_slice.iter().enumerate() {
            if po.flags & PARA_KIND_MASK != PARA_KIND_OPVAR { continue; }
            let ptr = unsafe { access_fn(self.inst_data.as_ptr() as *mut c_void, self.model_data.as_ptr() as *mut c_void, id as u32, ACCESS_FLAG_INSTANCE) };
            if ptr.is_null() { continue; }
            let value = match po.flags & PARA_TY_MASK {
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

    fn set_temperature(&mut self, t: f64) {
        if (t - self.setup_temperature).abs() > 0.01 {
            let ctx = Context::default();
            self.rerun_setup(t, &ctx);
        }
    }

    fn update(&mut self, _state: &CircularArrayBuffer2<f64>, context: &Context) {
        if (context.time - self.last_time).abs() > 1e-20 {
            self.prev_state.copy_from_slice(&self.next_state);
            self.last_time = context.time;
        }
        if (context.temperature - self.setup_temperature).abs() > 0.01 {
            self.rerun_setup(context.temperature, context);
        }
    }

    fn accept_timestep(&mut self, _state: &CircularArrayBuffer2<f64>, _ctx: &Context, _nets: &[LogicValue], _sink: &mut dyn crate::digital::interface::EventSink) {
        let mut rhs = [0.0f64; SCRATCH];
        if let Some(f) = self.desc().load_residual_react {
            unsafe { f(self.inst_data.as_ptr() as *mut c_void, self.model_data.as_ptr() as *mut c_void, rhs.as_mut_ptr()); }
        }
        let charges: Vec<f64> = self.rhs_indices().iter()
            .map(|idx| idx.map(|i| rhs[i]).unwrap_or(0.0))
            .collect();
        self.charge_history.push_front(charges);
        if self.charge_history.len() > 10 { self.charge_history.pop_back(); }
    }

    fn load_dc(&mut self, state: &DcAnalysisState, context: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        let flags = ENABLE_LIM | ANALYSIS_DC | CALC_RESIST_RESIDUAL | CALC_RESIST_JACOBIAN | CALC_RESIST_LIM_RHS;
        let prev_solve = self.build_prev_solve(&|k| state.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0));
        let ret = self.eval_with_prev_solve(flags, context, false, context.time, &prev_solve);
        self.limiting_active = ret & 1 != 0;
        let mut stamps = Vec::new();
        let mut rhs = [0.0f64; SCRATCH];
        if let Some(f) = self.desc().load_spice_rhs_dc {
            unsafe { f(self.inst_data.as_ptr() as *mut c_void, self.model_data.as_ptr() as *mut c_void, rhs.as_mut_ptr(), prev_solve.as_ptr()); }
        }
        self.collect_rhs_stamps(&rhs, &mut stamps);
        let jac = self.load_resist_jac();
        for (j, (r, c)) in self.resist_jac_refs().into_iter().enumerate() {
            if j < jac.len() && jac[j] != 0.0
                && let (Some(r), Some(c)) = (r, c) { stamps.push(Stamp::Matrix(r, c, jac[j])); }
        }
        stamps
    }

    fn load_ac(&mut self, dc_op: &DcAnalysisResult, _ac_ctx: &AcAnalysisContext, _context: &Context) -> Vec<Stamp<AnalogReference, Complex64>> {
        let flags = ANALYSIS_AC | CALC_RESIST_JACOBIAN | CALC_REACT_JACOBIAN;
        let ac_ctx = Context { gmin: 0.0, reltol: 1e-3, vntol: 1e-6, abstol: 1e-12, tnom: 300.15, ..Default::default() };
        let prev_solve = self.build_prev_solve(&|k| {
            if let Some(r) = self.node_refs.iter().flatten().find(|r| r.idx() == Some(k)) {
                dc_op.get(r.variable().clone()).unwrap_or(0.0)
            } else { 0.0 }
        });
        self.eval_with_prev_solve(flags, &ac_ctx, false, 0.0, &prev_solve);
        let mut stamps = Vec::new();
        let res_jac = self.load_resist_jac();
        for (j, (r, c)) in self.resist_jac_refs().into_iter().enumerate() {
            if j < res_jac.len() && res_jac[j] != 0.0
                && let (Some(r), Some(c)) = (r, c) { stamps.push(Stamp::Matrix(r, c, Complex64::new(res_jac[j], 0.0))); }
        }
        let react_jac = self.load_react_jac();
        for (j, (r, c)) in self.react_jac_refs().into_iter().enumerate() {
            if j < react_jac.len() && react_jac[j] != 0.0
                && let (Some(r), Some(c)) = (r, c) { stamps.push(Stamp::Matrix(r, c, Complex64::new(0.0, react_jac[j]))); }
        }
        stamps
    }

    fn load_transient(&mut self, states: &TransientAnalysisState, tran_ctx: &TransientAnalysisContext, context: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        let flags = ANALYSIS_TRAN | CALC_RESIST_RESIDUAL | CALC_REACT_RESIDUAL | CALC_RESIST_JACOBIAN | CALC_REACT_JACOBIAN | CALC_RESIST_LIM_RHS | CALC_REACT_LIM_RHS | ENABLE_LIM;
        let dt: f64 = tran_ctx.dt;
        let alpha = 1.0 / dt;
        let prev_solve = self.build_prev_solve(&|k| states.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0));
        let ret = self.eval_with_prev_solve(flags, context, false, context.time, &prev_solve);
        self.limiting_active = ret & 1 != 0;
        let mut stamps = Vec::new();
        let mut rhs = [0.0f64; SCRATCH];
        if let Some(f) = self.desc().load_spice_rhs_tran {
            unsafe { f(self.inst_data.as_ptr() as *mut c_void, self.model_data.as_ptr() as *mut c_void, rhs.as_mut_ptr(), prev_solve.as_ptr(), alpha); }
        }
        self.collect_rhs_stamps(&rhs, &mut stamps);
        let jac = self.load_resist_jac();
        for (j, (r, c)) in self.resist_jac_refs().into_iter().enumerate() {
            if j < jac.len() && jac[j] != 0.0
                && let (Some(r), Some(c)) = (r, c) { stamps.push(Stamp::Matrix(r, c, jac[j])); }
        }
        let react_jac = self.load_react_jac();
        for (j, (r, c)) in self.react_jac_refs().into_iter().enumerate() {
            if j < react_jac.len() && react_jac[j] != 0.0
                && let (Some(r), Some(c)) = (r, c) { stamps.push(Stamp::Matrix(r, c, react_jac[j] * alpha)); }
        }
        stamps
    }

    fn noise_current_psd(&mut self, dc_point: &DcAnalysisResult, ac_context: &AcAnalysisContext) -> Vec<Noise> {
        let num_noise = self.desc().num_noise_src as usize;
        if num_noise == 0 { return Vec::new(); }
        let flags = ANALYSIS_AC | CALC_RESIST_JACOBIAN | CALC_REACT_JACOBIAN | CALC_NOISE;
        let ac_ctx = Context { gmin: 0.0, reltol: 1e-3, vntol: 1e-6, abstol: 1e-12, tnom: 300.15, ..Default::default() };
        let prev_solve = self.build_prev_solve(&|k| {
            if let Some(r) = self.node_refs.iter().flatten().find(|r| r.idx() == Some(k)) {
                dc_point.get(r.variable().clone()).unwrap_or(0.0)
            } else { 0.0 }
        });
        self.eval_with_prev_solve(flags, &ac_ctx, false, 0.0, &prev_solve);
        let mut noise_rhs = vec![0.0f64; num_noise];
        if let Some(f) = self.desc().load_noise {
            unsafe { f(self.inst_data.as_ptr() as *mut c_void, self.model_data.as_ptr() as *mut c_void, ac_context.frequency, noise_rhs.as_mut_ptr()); }
        }
        let d = self.desc();
        let mut noises = Vec::new();
        if d.num_noise_src > 0 && !d.noise_sources.is_null() {
            for (i, &psd) in noise_rhs.iter().enumerate() {
                if psd <= 0.0 { continue; }
                let src = unsafe { &*d.noise_sources.add(i) };
                let ref1 = self.node_refs.get(src.nodes.node_1 as usize).and_then(|r| r.clone())
                    .unwrap_or_else(AnalogReference::ground);
                let ref2 = self.node_refs.get(src.nodes.node_2 as usize).and_then(|r| r.clone())
                    .unwrap_or_else(AnalogReference::ground);
                noises.push(Noise { terminals: (ref1, ref2), value: psd });
            }
        }
        noises
    }
}
