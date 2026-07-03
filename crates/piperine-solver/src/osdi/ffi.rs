use std::os::raw::{c_char, c_void};
use crate::solver::Context;

// ---------------------------------------------------------------------------
// OSDI 0.4 flags
// ---------------------------------------------------------------------------
pub const CALC_RESIST_RESIDUAL: u32 = 1;
pub const CALC_REACT_RESIDUAL: u32 = 2;
pub const CALC_RESIST_JACOBIAN: u32 = 4;
pub const CALC_REACT_JACOBIAN: u32 = 8;
pub const CALC_NOISE: u32 = 16;
pub const CALC_OP: u32 = 32;
pub const CALC_RESIST_LIM_RHS: u32 = 64;
pub const CALC_REACT_LIM_RHS: u32 = 128;
pub const ENABLE_LIM: u32 = 256;
pub const INIT_LIM: u32 = 512;
pub const ANALYSIS_DC: u32 = 2048;
pub const ANALYSIS_AC: u32 = 4096;
pub const ANALYSIS_TRAN: u32 = 8192;

pub const EVAL_RET_FLAG_LIM: u32 = 1;
pub const EVAL_RET_FLAG_FATAL: u32 = 2;

pub const JACOBIAN_ENTRY_RESIST_CONST: u32 = 1;
pub const JACOBIAN_ENTRY_REACT_CONST: u32 = 2;
pub const JACOBIAN_ENTRY_RESIST: u32 = 4;
pub const JACOBIAN_ENTRY_REACT: u32 = 8;

pub const PARA_TY_MASK: u32 = 3;
pub const PARA_TY_REAL: u32 = 0;
pub const PARA_TY_INT: u32 = 1;
pub const PARA_TY_STR: u32 = 2;
pub const PARA_KIND_MASK: u32 = 3 << 30;
pub const PARA_KIND_MODEL: u32 = 0;
pub const PARA_KIND_OPVAR: u32 = 2 << 30;
pub const ACCESS_FLAG_SET: u32 = 1;
pub const ACCESS_FLAG_INSTANCE: u32 = 4;

// Wrapper to make raw-pointer arrays Sync for use in statics.
pub struct SyncPtrArray<const N: usize>([*const c_char; N]);
unsafe impl<const N: usize> Sync for SyncPtrArray<N> {}

// Sim-param names as C strings (static, pointer-stable).
pub static SIM_PARAM_NAMES: SyncPtrArray<11> = SyncPtrArray([
    c"iniLim".as_ptr(),
    c"gmin".as_ptr(),
    c"gdev".as_ptr(),
    c"tnom".as_ptr(),
    c"simulatorVersion".as_ptr(),
    c"sourceScaleFactor".as_ptr(),
    c"epsmin".as_ptr(),
    c"reltol".as_ptr(),
    c"vntol".as_ptr(),
    c"abstol".as_ptr(),
    std::ptr::null(),
]);
pub static SIM_PARAM_STR_SENTINEL: SyncPtrArray<1> = SyncPtrArray([std::ptr::null()]);

/// Owns the values array backing an OsdiSimParas; must outlive any pointer use.
pub struct OsdiSimParasOwned {
    vals: [f64; 10],
}

impl OsdiSimParasOwned {
    pub fn new(context: &Context, ini_lim: bool) -> Self {
        Self {
            vals: [
                if ini_lim { 1.0 } else { 0.0 },   // iniLim
                context.gmin,                          // gmin
                context.gmin,                          // gdev
                context.tnom - 273.15,                 // tnom (Celsius)
                0.0,                                   // simulatorVersion
                1.0,                                   // sourceScaleFactor
                1e-15,                                 // epsmin
                context.reltol,                        // reltol
                context.vntol,                         // vntol
                context.abstol,                        // abstol
            ],
        }
    }

    pub fn default_paras() -> (Self, OsdiSimParas) {
        let owned = Self {
            vals: [0.0, 1e-12, 1e-12, 27.0, 0.0, 1.0, 1e-15, 1e-3, 1e-6, 1e-12],
        };
        let raw = owned.as_raw();
        (owned, raw)
    }

    pub fn as_raw(&self) -> OsdiSimParas {
        OsdiSimParas {
            names: SIM_PARAM_NAMES.0.as_ptr() as *mut *mut c_char,
            vals: self.vals.as_ptr() as *mut f64,
            names_str: SIM_PARAM_STR_SENTINEL.0.as_ptr() as *mut *mut c_char,
            vals_str: std::ptr::null_mut(),
        }
    }
}

pub const SCRATCH: usize = 1024;

// ---------------------------------------------------------------------------
// FFI structures
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct OsdiSimParas {
    pub names: *mut *mut c_char,
    pub vals: *mut f64,
    pub names_str: *mut *mut c_char,
    pub vals_str: *mut *mut c_char,
}

#[repr(C)]
pub struct OsdiSimInfo {
    pub paras: OsdiSimParas,
    pub abstime: f64,
    pub prev_solve: *mut f64,
    pub prev_state: *mut f64,
    pub next_state: *mut f64,
    pub flags: u32,
}

#[repr(C)]
pub struct OsdiInitError {
    pub code: u32,
    pub payload: u32,
}

#[repr(C)]
pub struct OsdiInitInfo {
    pub flags: u32,
    pub num_errors: u32,
    pub errors: *mut OsdiInitError,
}

#[repr(C)]
pub struct OsdiNodePair {
    pub node_1: u32,
    pub node_2: u32,
}

#[repr(C)]
pub struct OsdiJacobianEntry {
    pub nodes: OsdiNodePair,
    pub react_ptr_off: u32,
    pub flags: u32,
}

#[repr(C)]
pub struct OsdiNode {
    pub name: *const c_char,
    pub units: *const c_char,
    pub residual_units: *const c_char,
    pub resist_residual_off: u32,
    pub react_residual_off: u32,
    pub resist_limit_rhs_off: u32,
    pub react_limit_rhs_off: u32,
    pub is_flow: bool,
}

#[repr(C)]
pub struct OsdiParamOpvar {
    pub name: *const *const c_char,
    pub num_alias: u32,
    pub description: *const c_char,
    pub units: *const c_char,
    pub flags: u32,
    pub len: u32,
}

unsafe impl Send for OsdiParamOpvar {}
unsafe impl Sync for OsdiParamOpvar {}

#[repr(C)]
pub struct OsdiNoiseSource {
    pub name: *const c_char,
    pub nodes: OsdiNodePair,
}

unsafe impl Send for OsdiNoiseSource {}
unsafe impl Sync for OsdiNoiseSource {}

#[repr(C)]
pub struct OsdiDescriptor {
    pub name: *const c_char,
    pub num_nodes: u32,
    pub num_terminals: u32,
    pub nodes: *const OsdiNode,
    pub num_jacobian_entries: u32,
    pub jacobian_entries: *const OsdiJacobianEntry,
    pub num_collapsible: u32,
    pub collapsible: *const OsdiNodePair,
    pub collapsed_offset: u32,
    pub noise_sources: *const OsdiNoiseSource,
    pub num_noise_src: u32,
    pub num_params: u32,
    pub num_instance_params: u32,
    pub num_opvars: u32,
    pub param_opvar: *const OsdiParamOpvar,
    pub node_mapping_offset: u32,
    pub jacobian_ptr_resist_offset: u32,
    pub num_states: u32,
    pub state_idx_off: u32,
    pub bound_step_offset: u32,
    pub instance_size: u32,
    pub model_size: u32,
    pub access: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, u32, u32) -> *mut c_void>,
    pub setup_model: Option<
        unsafe extern "C" fn(*mut c_void, *mut c_void, *const OsdiSimParas, *mut OsdiInitInfo),
    >,
    pub setup_instance: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *mut c_void,
            *mut c_void,
            f64,
            u32,
            *const OsdiSimParas,
            *mut OsdiInitInfo,
        ),
    >,
    pub eval:
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *const OsdiSimInfo) -> u32>,
    pub load_noise: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, f64, *mut f64)>,
    pub load_residual_resist: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut f64)>,
    pub load_residual_react: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut f64)>,
    pub load_limit_rhs_resist: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut f64)>,
    pub load_limit_rhs_react: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut f64)>,
    pub load_spice_rhs_dc:
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut f64, *const f64)>,
    pub load_spice_rhs_tran:
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut f64, *const f64, f64)>,
    pub load_jacobian_resist: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    pub load_jacobian_react: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, f64)>,
    pub load_jacobian_tran: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, f64)>,
    pub given_flag_model: Option<unsafe extern "C" fn(*mut c_void, u32) -> u32>,
    pub given_flag_instance: Option<unsafe extern "C" fn(*mut c_void, u32) -> u32>,
    pub num_resistive_jacobian_entries: u32,
    pub num_reactive_jacobian_entries: u32,
    pub write_jacobian_array_resist:
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut f64)>,
    pub write_jacobian_array_react:
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut f64)>,
    pub num_inputs: u32,
    pub inputs: *const OsdiNodePair,
    pub load_jacobian_with_offset_resist:
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void, usize)>,
    pub load_jacobian_with_offset_react:
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void, usize)>,
}

unsafe impl Send for OsdiDescriptor {}
unsafe impl Sync for OsdiDescriptor {}
