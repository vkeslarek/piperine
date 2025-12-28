use crate::ngspice::engine::NgSpiceEngine;
use crate::ngspice::ffi;
use crate::ngspice::structs::{Event, IsrcSync, SyncData, VecInfoAll, VecValuesAll, VsrcSync};
use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::atomic::Ordering;

unsafe fn get_engine_ref<'a>(engine_ptr: *mut c_void) -> Option<&'a NgSpiceEngine> {
    if engine_ptr.is_null() {
        None
    } else {
        Some(&*(engine_ptr as *const NgSpiceEngine))
    }
}

pub(crate) extern "C" fn send_char_callback(
    msg: *mut c_char,
    id: c_int,
    engine_ptr: *mut c_void,
) -> c_int {
    unsafe {
        if msg.is_null() {
            return 0;
        }

        if let Some(engine) = get_engine_ref(engine_ptr) {
            // Check if terminated to avoid sending events during shutdown
            if !engine.terminated.load(Ordering::Relaxed) {
                let str = CStr::from_ptr(msg).to_string_lossy().into_owned();
                let _ = engine.sender().send(Event::SendChar {
                    message: str,
                    id: id.into(),
                });
            }
        }
    }
    0
}

pub(crate) extern "C" fn send_stat_callback(
    msg: *mut c_char,
    id: c_int,
    engine_ptr: *mut c_void,
) -> c_int {
    unsafe {
        if msg.is_null() {
            return 0;
        }

        if let Some(engine) = get_engine_ref(engine_ptr) {
            if !engine.terminated.load(Ordering::Relaxed) {
                let str = CStr::from_ptr(msg).to_string_lossy().into_owned();
                let _ = engine.sender().send(Event::SendStat {
                    message: str,
                    id: id.into(),
                });
            }
        }
    }
    0
}

pub(crate) extern "C" fn controlled_exit_callback(
    status: c_int,
    immediate: bool,
    quit: bool,
    id: c_int,
    engine_ptr: *mut c_void,
) -> c_int {
    unsafe {
        if let Some(engine) = get_engine_ref(engine_ptr) {
            // 1. Mark engine as dead so no new commands can be sent
            engine.terminated.store(true, Ordering::SeqCst);

            // 2. Notify subscribers
            let _ = engine.sender().send(Event::ControlledExit {
                status: status.into(),
                immediate,
                quit,
                id: id.into(),
            });

            // 3. Wake up any thread waiting on wait_for_shutdown()
            let mut terminated_guard = engine.shutdown_lock.lock().unwrap();
            *terminated_guard = true;
            engine.shutdown_cond.notify_all();
        }
    }
    0
}

pub(crate) extern "C" fn send_init_data_callback(
    data: *mut ffi::vecinfoall,
    id: c_int,
    engine_ptr: *mut c_void,
) -> c_int {
    unsafe {
        if data.is_null() {
            return 0;
        }
        if let Some(engine) = get_engine_ref(engine_ptr) {
            if !engine.terminated.load(Ordering::Relaxed) {
                let info = VecInfoAll::from_raw(data);
                let _ = engine.sender().send(Event::SendInitData {
                    info,
                    id: id.into(),
                });
            }
        }
    }
    0
}

pub(crate) extern "C" fn send_data_callback(
    data: *mut ffi::vecvaluesall,
    _: c_int,
    id: c_int,
    engine_ptr: *mut c_void,
) -> c_int {
    unsafe {
        if data.is_null() {
            return 0;
        }
        if let Some(engine) = get_engine_ref(engine_ptr) {
            if !engine.terminated.load(Ordering::Relaxed) {
                let samples = VecValuesAll::from_raw(data);
                let _ = engine.sender.send(Event::SendData {
                    samples,
                    id: id.into(),
                });
            }
        }
    }
    0
}

pub(crate) extern "C" fn background_thread_running_callback(
    running: bool,
    id: c_int,
    engine_ptr: *mut c_void,
) -> c_int {
    unsafe {
        if let Some(engine) = get_engine_ref(engine_ptr) {
            let _ = engine.sender.send(Event::BGThreadRunning {
                running,
                id: id.into(),
            });
        }
    }
    0
}

pub(crate) extern "C" fn get_vsrc_sync(
    voltage: *mut f64,
    time: f64,
    v_name: *mut c_char,
    _id: c_int,
    engine_ptr: *mut c_void,
) -> c_int {
    unsafe {
        if let Some(engine) = get_engine_ref(engine_ptr) {
            if !engine.terminated.load(Ordering::Relaxed) {
                let vsrc_sync = VsrcSync::from_raw(voltage, time, v_name, _id);
                if let Some(new_voltage) = engine.get_vsrc_sync(vsrc_sync) {
                    *voltage = new_voltage.real;
                    *voltage.add(1) = new_voltage.imaginary;
                }
            }
        }
    }

    0
}

pub(crate) extern "C" fn get_isrc_sync(
    current: *mut f64,
    time: f64,
    v_name: *mut c_char,
    _id: c_int,
    engine_ptr: *mut c_void,
) -> c_int {
    unsafe {
        if let Some(engine) = get_engine_ref(engine_ptr) {
            if !engine.terminated.load(Ordering::Relaxed) {
                let isrc_sync = IsrcSync::from_raw(current, time, v_name, _id);
                if let Some(new_current) = engine.get_isrc_sync(isrc_sync) {
                    *current = new_current.real;
                    *current.add(1) = new_current.imaginary;
                }
            }
        }
    }

    0
}

pub(crate) extern "C" fn sync_data(
    actual_time: f64,
    delta_time: *mut f64,
    old_delta_time: f64,
    redustep: c_int,
    after_converge: c_int,
    node_id: c_int,
    engine_ptr: *mut c_void,
) -> c_int {
    unsafe {
        if let Some(engine) = get_engine_ref(engine_ptr) {
            if !engine.terminated.load(Ordering::Relaxed) {
                let sync_data = SyncData::from_raw(
                    actual_time,
                    delta_time,
                    old_delta_time,
                    redustep,
                    after_converge,
                    node_id,
                );
                if let Some(new_delta_time) = engine.get_sync_data(sync_data) {
                    *delta_time = new_delta_time;
                }
            }
        }
    }

    0
}
