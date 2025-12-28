use crate::ngspice::ffi;
use std::ffi::{c_char, c_int, CStr};

#[derive(Debug, Copy, Clone)]
pub struct VecHandle(usize);

#[derive(Debug)]
pub struct VecInfo {
    pub number: i32,
    pub name: String,
    pub is_real: bool,
    pub vec_pointer_handle: VecHandle,
    pub vec_scale_pointer_handle: VecHandle,
}

impl VecInfo {
    unsafe fn from_raw(raw: ffi::pvecinfo) -> Self {
        unsafe {
            let number = (*raw).number as i32;
            let name = CStr::from_ptr((*raw).vecname)
                .to_string_lossy()
                .into_owned();
            let is_real = (*raw).is_real;
            let vec_pointer_handle = VecHandle((*raw).pdvec as usize);
            let vec_scale_pointer_handle = VecHandle((*raw).pdvecscale as usize);

            Self {
                number,
                name,
                is_real,
                vec_pointer_handle,
                vec_scale_pointer_handle,
            }
        }
    }
}

#[derive(Debug)]
pub struct VecInfoAll {
    pub name: String,
    pub title: String,
    pub date: String,
    pub type_: String,
    pub vecs: Vec<VecInfo>,
}

impl VecInfoAll {
    pub(crate) unsafe fn from_raw(raw: *mut ffi::vecinfoall) -> Self {
        unsafe {
            let name = CStr::from_ptr((*raw).name).to_string_lossy().into_owned();
            let title = CStr::from_ptr((*raw).title).to_string_lossy().into_owned();
            let date = CStr::from_ptr((*raw).date).to_string_lossy().into_owned();
            let type_ = CStr::from_ptr((*raw).type_).to_string_lossy().into_owned();

            let mut vecs = Vec::with_capacity((*raw).veccount as usize);
            let slice = std::slice::from_raw_parts((*raw).vecs, (*raw).veccount as usize);

            for &vecinfo in slice {
                vecs.push(VecInfo::from_raw(vecinfo));
            }

            Self {
                name,
                title,
                date,
                type_,
                vecs,
            }
        }
    }
}

#[derive(Debug)]
pub struct VecValues {
    pub name: String,
    pub real_value: f64,
    pub imaginary_value: f64,
    pub is_scale: bool,
    pub is_complex: bool,
}

impl VecValues {
    unsafe fn from_raw(raw: *mut ffi::vecvalues) -> Self {
        unsafe {
            let name = CStr::from_ptr((*raw).name).to_string_lossy().into_owned();
            Self {
                name,
                real_value: (*raw).creal,
                imaginary_value: (*raw).cimag,
                is_scale: (*raw).is_scale,
                is_complex: (*raw).is_complex,
            }
        }
    }
}

#[derive(Debug)]
pub struct VecValuesAll {
    pub index: usize,
    pub vecs: Vec<VecValues>,
}

impl VecValuesAll {
    pub(crate) unsafe fn from_raw(raw: *mut ffi::vecvaluesall) -> Self {
        unsafe {
            let count = (*raw).veccount as usize;
            let index = (*raw).vecindex as usize;

            let mut vecs = Vec::with_capacity(count);
            if count > 0 {
                let slice = std::slice::from_raw_parts((*raw).vecsa, count);
                for &vecvalues in slice {
                    vecs.push(VecValues::from_raw(vecvalues));
                }
            }
            Self { index, vecs }
        }
    }
}

#[derive(Debug)]
pub struct Complex {
    pub real: f64,
    pub imaginary: f64,
}

impl Complex {
    unsafe fn from_raw(raw: ffi::ngcomplex) -> Self {
        Self {
            real: raw.cx_real,
            imaginary: raw.cx_imag,
        }
    }
}

#[derive(Debug)]
pub struct VectorInfo {
    pub name: String,
    pub type_: i32,
    pub flags: i16,
    pub real_data: Vec<f64>,
    pub complex_data: Vec<Complex>,
    pub length: usize,
}

impl VectorInfo {
    pub(crate) unsafe fn from_raw(raw: *mut ffi::vector_info) -> Self {
        unsafe {
            let name = CStr::from_ptr((*raw).v_name).to_string_lossy().into_owned();
            let length = (*raw).v_length as usize;

            let mut real_data = Vec::new();
            if !(*raw).v_realdata.is_null() {
                let slice = std::slice::from_raw_parts((*raw).v_realdata, length);
                real_data.extend_from_slice(slice);
            }

            let mut complex_data = Vec::new();
            if !(*raw).v_compdata.is_null() {
                let slice = std::slice::from_raw_parts((*raw).v_compdata, length);
                for &item in slice {
                    complex_data.push(Complex::from_raw(item));
                }
            }

            Self {
                name,
                type_: (*raw).v_type,
                flags: (*raw).v_flags,
                real_data,
                complex_data,
                length,
            }
        }
    }
}

#[derive(Debug)]
pub enum Event {
    SendChar {
        message: String,
        id: i32,
    },
    SendStat {
        message: String,
        id: i32,
    },
    ControlledExit {
        status: i32,
        immediate: bool,
        quit: bool,
        id: i32,
    },
    SendInitData {
        info: VecInfoAll,
        id: i32,
    },
    SendData {
        samples: VecValuesAll,
        id: i32,
    },
    BGThreadRunning {
        running: bool,
        id: i32,
    },
}

pub struct VsrcSync {
    pub voltage: Complex,
    pub time: f64,
    pub name: String,
    pub id: i32,
}

impl VsrcSync {
    pub(crate) unsafe fn from_raw(
        voltage: *mut f64,
        time: f64,
        v_name: *mut c_char,
        _id: c_int,
    ) -> Self {
        let name = CStr::from_ptr(v_name).to_string_lossy().into_owned();

        let voltage_real = *voltage;
        let voltage_imag = *voltage.add(1);

        let voltage = Complex {
            real: voltage_real,
            imaginary: voltage_imag,
        };

        Self {
            voltage,
            time,
            name,
            id: _id.into(),
        }
    }
}

pub struct VsrcSyncResponse {
    pub voltage: Option<Complex>,
}

pub struct IsrcSync {
    pub current: Complex,
    pub time: f64,
    pub name: String,
    pub id: i32,
}

impl IsrcSync {
    pub(crate) unsafe fn from_raw(
        current: *mut f64,
        time: f64,
        v_name: *mut c_char,
        _id: c_int,
    ) -> Self {
        let name = CStr::from_ptr(v_name).to_string_lossy().into_owned();

        let current_real = *current;
        let current_imag = *current.add(1);

        let current = Complex {
            real: current_real,
            imaginary: current_imag,
        };

        Self {
            current,
            time,
            name,
            id: _id.into(),
        }
    }
}

pub struct IsrcSyncResponse {
    pub current: Option<Complex>,
}

pub struct SyncData {
    pub actual_time: f64,
    pub delta_time: f64,
    pub old_delta_time: f64,
    pub redustep: i32,
    pub after_converge: i32,
    pub node_id: i32,
}

impl SyncData {
    pub(crate) unsafe fn from_raw(
        actual_time: f64,
        delta_time: *mut f64,
        old_delta_time: f64,
        redustep: c_int,
        after_converge: c_int,
        node_id: c_int,
    ) -> Self {
        Self {
            actual_time,
            delta_time: *delta_time,
            old_delta_time,
            redustep,
            after_converge,
            node_id,
        }
    }
}

pub struct SyncDataResponse {
    pub delta_time: Option<f64>,
}
