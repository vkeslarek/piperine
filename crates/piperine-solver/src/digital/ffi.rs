use std::os::raw::{c_char, c_void, c_double};

pub const DIGITAL_DIR_INPUT: u32 = 0;
pub const DIGITAL_DIR_OUTPUT: u32 = 1;
pub const DIGITAL_DIR_INOUT: u32 = 2;

pub const DIGITAL_LOGIC_ZERO: u8 = 0;
pub const DIGITAL_LOGIC_ONE: u8 = 1;
pub const DIGITAL_LOGIC_X: u8 = 2;
pub const DIGITAL_LOGIC_Z: u8 = 3;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct DigitalPort {
    pub name: *const c_char,
    pub direction: u32,
    pub width: u32,
}
unsafe impl Send for DigitalPort {}
unsafe impl Sync for DigitalPort {}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct DigitalParam {
    pub name: *const c_char,
    pub type_: u32,
}
unsafe impl Send for DigitalParam {}
unsafe impl Sync for DigitalParam {}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct DigitalEventSink {
    pub handle: *mut c_void,
    pub schedule: Option<unsafe extern "C" fn(handle: *mut c_void, port_idx: u32, value: u8, delay: c_double)>,
    pub cancel: Option<unsafe extern "C" fn(handle: *mut c_void, port_idx: u32)>,
}

/// Simulation parameters passed to setup_model and setup_instance.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct DigitalSimParams {
    pub timescale: c_double,
    pub temperature: c_double,
    pub supply_voltage: c_double,
}
unsafe impl Send for DigitalSimParams {}
unsafe impl Sync for DigitalSimParams {}

impl Default for DigitalSimParams {
    fn default() -> Self {
        Self {
            timescale: 1e-9,
            temperature: 300.15,
            supply_voltage: 1.8,
        }
    }
}

/// The digital device descriptor. One per device type in the .so library.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct DigitalDescriptor {
    pub name: *const c_char,
    pub num_ports: u32,
    pub num_params: u32,
    pub ports: *const DigitalPort,
    pub params: *const DigitalParam,
    pub instance_size: u32,
    pub model_size: u32,

    pub setup_model: Option<unsafe extern "C" fn(
        model_data: *mut c_void,
        sim: *const DigitalSimParams,
    )>,

    pub setup_instance: Option<unsafe extern "C" fn(
        inst_data: *mut c_void,
        model_data: *mut c_void,
        sim: *const DigitalSimParams,
    )>,

    pub eval: Option<unsafe extern "C" fn(
        inst_data: *mut c_void,
        model_data: *mut c_void,
        inputs: *const u8,
        outputs: *mut u8,
        event_sink: *mut DigitalEventSink,
        current_time: c_double,
    ) -> u32>,

    pub access: Option<unsafe extern "C" fn(
        inst_data: *mut c_void,
        model_data: *mut c_void,
        param_id: u32,
        flags: u32,
    ) -> *mut c_void>,
}
unsafe impl Send for DigitalDescriptor {}
unsafe impl Sync for DigitalDescriptor {}
