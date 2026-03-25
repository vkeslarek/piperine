//! C callback implementations for ngspice shared library.
//!
//! These are the `extern "C"` functions passed to `ngSpice_Init` and `ngSpice_Init_Sync`.
//! They forward data to Rust through a user-data pointer to `CallbackState`.

use crate::ffi;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::{Arc, Mutex};

/// State shared between callbacks and the NgspiceInstance.
pub struct CallbackState {
    pub log: Mutex<Vec<String>>,
    pub exit_status: Mutex<Option<i32>>,
    /// If set, called for EXTERNAL voltage sources.
    pub vsrc_handler: Mutex<Option<Box<dyn Fn(&str, f64) -> f64 + Send + Sync>>>,
    /// If set, called for EXTERNAL current sources.
    pub isrc_handler: Mutex<Option<Box<dyn Fn(&str, f64) -> f64 + Send + Sync>>>,
}

impl CallbackState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            log: Mutex::new(Vec::new()),
            exit_status: Mutex::new(None),
            vsrc_handler: Mutex::new(None),
            isrc_handler: Mutex::new(None),
        })
    }
}

/// SendChar callback: captures stdout/stderr from ngspice.
pub unsafe extern "C" fn send_char(msg: *mut c_char, _id: c_int, user_data: *mut c_void) -> c_int {
    if msg.is_null() || user_data.is_null() {
        return 0;
    }
    let state = unsafe { &*(user_data as *const CallbackState) };
    let s = unsafe { CStr::from_ptr(msg) }
        .to_string_lossy()
        .into_owned();
    if let Ok(mut log) = state.log.lock() {
        log.push(s);
    }
    0
}

/// SendStat callback: captures simulation status (we just log it).
pub unsafe extern "C" fn send_stat(msg: *mut c_char, _id: c_int, user_data: *mut c_void) -> c_int {
    if msg.is_null() || user_data.is_null() {
        return 0;
    }
    let state = unsafe { &*(user_data as *const CallbackState) };
    let s = unsafe { CStr::from_ptr(msg) }
        .to_string_lossy()
        .into_owned();
    if let Ok(mut log) = state.log.lock() {
        log.push(format!("[status] {s}"));
    }
    0
}

/// ControlledExit callback: ngspice wants to exit.
pub unsafe extern "C" fn controlled_exit(
    status: c_int,
    _immediate: bool,
    _quit: bool,
    _id: c_int,
    user_data: *mut c_void,
) -> c_int {
    if user_data.is_null() {
        return 0;
    }
    let state = unsafe { &*(user_data as *const CallbackState) };
    if let Ok(mut es) = state.exit_status.lock() {
        *es = Some(status);
    }
    0
}

/// SendData callback: receive vector data during simulation (we ignore for now).
pub unsafe extern "C" fn send_data(
    _data: *mut ffi::vecvaluesall,
    _count: c_int,
    _id: c_int,
    _user_data: *mut c_void,
) -> c_int {
    0
}

/// SendInitData callback: receive vector info at init (we ignore for now).
pub unsafe extern "C" fn send_init_data(
    _data: *mut ffi::vecinfoall,
    _id: c_int,
    _user_data: *mut c_void,
) -> c_int {
    0
}

/// BGThreadRunning callback: signal if background thread is running.
pub unsafe extern "C" fn bg_thread_running(
    _is_running: bool,
    _id: c_int,
    _user_data: *mut c_void,
) -> c_int {
    0
}

/// GetVSRCData callback: ngspice requesting external voltage source value.
pub unsafe extern "C" fn get_vsrc_data(
    value: *mut f64,
    time: f64,
    name: *mut c_char,
    _id: c_int,
    user_data: *mut c_void,
) -> c_int {
    if value.is_null() || name.is_null() || user_data.is_null() {
        return 1;
    }
    let state = unsafe { &*(user_data as *const CallbackState) };
    let source_name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    if let Ok(handler) = state.vsrc_handler.lock() {
        if let Some(ref f) = *handler {
            unsafe { *value = f(&source_name, time) };
            return 0;
        }
    }
    1 // no handler registered
}

/// GetISRCData callback: ngspice requesting external current source value.
pub unsafe extern "C" fn get_isrc_data(
    value: *mut f64,
    time: f64,
    name: *mut c_char,
    _id: c_int,
    user_data: *mut c_void,
) -> c_int {
    if value.is_null() || name.is_null() || user_data.is_null() {
        return 1;
    }
    let state = unsafe { &*(user_data as *const CallbackState) };
    let source_name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    if let Ok(handler) = state.isrc_handler.lock() {
        if let Some(ref f) = *handler {
            unsafe { *value = f(&source_name, time) };
            return 0;
        }
    }
    1
}

/// GetSyncData callback: synchronization (we accept default timing).
pub unsafe extern "C" fn get_sync_data(
    _time: f64,
    _delta: *mut f64,
    _old_delta: f64,
    _redostep: c_int,
    _id: c_int,
    _location: c_int,
    _user_data: *mut c_void,
) -> c_int {
    0
}
