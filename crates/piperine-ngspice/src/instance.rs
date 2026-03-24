//! Safe wrapper around a single ngspice instance.
//!
//! Each `NgspiceInstance` represents one initialized ngspice shared library context.
//! Because ngspice uses C globals, only ONE instance can exist per process.

use crate::callbacks::{self, CallbackState};
use crate::ffi;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::Arc;

use piperine_api::result::{
    ComplexVector, Plot, PlotType, RealVector, SimulationResult, Vector,
};

/// Error type for ngspice operations.
#[derive(Debug, thiserror::Error)]
pub enum NgspiceError {
    #[error("ngSpice_Init failed with code {0}")]
    InitFailed(i32),
    #[error("ngSpice_Init_Sync failed with code {0}")]
    InitSyncFailed(i32),
    #[error("ngSpice_Command failed: {0}")]
    CommandFailed(String),
    #[error("ngSpice_Circ failed with code {0}")]
    CircFailed(i32),
    #[error("ngspice exited with status {0}")]
    ExitError(i32),
    #[error("null pointer from ngspice: {0}")]
    NullPointer(String),
    #[error("{0}")]
    Other(String),
}

/// A safe wrapper around one ngspice shared library instance.
pub struct NgspiceInstance {
    state: Arc<CallbackState>,
}

impl NgspiceInstance {
    /// Initialize ngspice. Must be called exactly once per process.
    pub fn new() -> Result<Self, NgspiceError> {
        let state = CallbackState::new();

        // We pass a raw pointer to the Arc's inner data as user_data.
        // The Arc is kept alive by this struct.
        let user_data = Arc::as_ptr(&state) as *mut std::ffi::c_void;

        let ret = unsafe {
            ffi::ngSpice_Init(
                Some(callbacks::send_char),
                Some(callbacks::send_stat),
                Some(callbacks::controlled_exit),
                Some(callbacks::send_data),
                Some(callbacks::send_init_data),
                Some(callbacks::bg_thread_running),
                user_data,
            )
        };
        if ret != 0 {
            return Err(NgspiceError::InitFailed(ret));
        }

        // Initialize sync callbacks for external sources
        let ret = unsafe {
            ffi::ngSpice_Init_Sync(
                Some(callbacks::get_vsrc_data),
                Some(callbacks::get_isrc_data),
                Some(callbacks::get_sync_data),
                ptr::null_mut(),
                ptr::null_mut(), // keep user_data from Init
            )
        };
        if ret != 0 {
            return Err(NgspiceError::InitSyncFailed(ret));
        }

        Ok(Self { state })
    }

    /// Send a command to ngspice (e.g. "op", "tran 1u 10m", "quit").
    pub fn command(&self, cmd: &str) -> Result<(), NgspiceError> {
        let c_cmd = CString::new(cmd)
            .map_err(|_| NgspiceError::CommandFailed(format!("invalid command string: {cmd}")))?;
        let ret = unsafe { ffi::ngSpice_Command(c_cmd.as_ptr() as *mut _) };
        if ret != 0 {
            return Err(NgspiceError::CommandFailed(format!("command '{cmd}' returned {ret}")));
        }
        self.check_exit()?;
        Ok(())
    }

    /// Load a circuit from netlist lines (like ngSpice_Circ).
    pub fn load_circuit(&self, lines: &[String]) -> Result<(), NgspiceError> {
        // Build null-terminated array of C strings
        let c_strings: Vec<CString> = lines
            .iter()
            .map(|l| CString::new(l.as_str()).unwrap())
            .collect();
        let mut ptrs: Vec<*mut libc::c_char> = c_strings
            .iter()
            .map(|cs| cs.as_ptr() as *mut _)
            .collect();
        ptrs.push(ptr::null_mut()); // NULL terminator

        let ret = unsafe { ffi::ngSpice_Circ(ptrs.as_mut_ptr()) };
        if ret != 0 {
            return Err(NgspiceError::CircFailed(ret));
        }
        self.check_exit()?;
        Ok(())
    }

    /// Set an external voltage source handler.
    pub fn set_vsrc_handler(&self, f: impl Fn(&str, f64) -> f64 + Send + Sync + 'static) {
        *self.state.vsrc_handler.lock().unwrap() = Some(Box::new(f));
    }

    /// Set an external current source handler.
    pub fn set_isrc_handler(&self, f: impl Fn(&str, f64) -> f64 + Send + Sync + 'static) {
        *self.state.isrc_handler.lock().unwrap() = Some(Box::new(f));
    }

    /// Clear external source handlers.
    pub fn clear_external_handlers(&self) {
        *self.state.vsrc_handler.lock().unwrap() = None;
        *self.state.isrc_handler.lock().unwrap() = None;
    }

    /// Get the name of the current plot.
    pub fn current_plot(&self) -> Result<String, NgspiceError> {
        let ptr = unsafe { ffi::ngSpice_CurPlot() };
        if ptr.is_null() {
            return Err(NgspiceError::NullPointer("ngSpice_CurPlot".into()));
        }
        Ok(unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned())
    }

    /// Get all plot names.
    pub fn all_plots(&self) -> Vec<String> {
        let ptr = unsafe { ffi::ngSpice_AllPlots() };
        if ptr.is_null() {
            return Vec::new();
        }
        let mut plots = Vec::new();
        let mut i = 0;
        loop {
            let entry = unsafe { *ptr.add(i) };
            if entry.is_null() {
                break;
            }
            plots.push(unsafe { CStr::from_ptr(entry) }.to_string_lossy().into_owned());
            i += 1;
        }
        plots
    }

    /// Get all vector names in a plot.
    pub fn all_vecs(&self, plot_name: &str) -> Vec<String> {
        let c_name = CString::new(plot_name).unwrap();
        let ptr = unsafe { ffi::ngSpice_AllVecs(c_name.as_ptr() as *mut _) };
        if ptr.is_null() {
            return Vec::new();
        }
        let mut vecs = Vec::new();
        let mut i = 0;
        loop {
            let entry = unsafe { *ptr.add(i) };
            if entry.is_null() {
                break;
            }
            vecs.push(unsafe { CStr::from_ptr(entry) }.to_string_lossy().into_owned());
            i += 1;
        }
        vecs
    }

    /// Get vector info and data for a named vector.
    pub fn get_vector(&self, name: &str) -> Result<Vector, NgspiceError> {
        let c_name = CString::new(name).unwrap();
        let info = unsafe { ffi::ngGet_Vec_Info(c_name.as_ptr() as *mut _) };
        if info.is_null() {
            return Err(NgspiceError::NullPointer(format!("vector '{name}'")));
        }
        let info = unsafe { &*info };
        let len = info.v_length as usize;

        if !info.v_compdata.is_null() {
            // Complex vector
            let mut data = Vec::with_capacity(len);
            for i in 0..len {
                let c = unsafe { &*info.v_compdata.add(i) };
                data.push((c.cx_real, c.cx_imag));
            }
            Ok(Vector::Complex(ComplexVector {
                name: name.to_string(),
                data,
            }))
        } else if !info.v_realdata.is_null() {
            // Real vector
            let data = unsafe { std::slice::from_raw_parts(info.v_realdata, len) }.to_vec();
            Ok(Vector::Real(RealVector {
                name: name.to_string(),
                data,
            }))
        } else {
            Err(NgspiceError::NullPointer(format!("vector '{name}' has no data")))
        }
    }

    /// Collect all plots and their vectors into a SimulationResult.
    pub fn collect_results(&self) -> Result<SimulationResult, NgspiceError> {
        let plot_names = self.all_plots();
        let mut plots = HashMap::new();

        for pname in &plot_names {
            let vec_names = self.all_vecs(pname);
            let mut vectors = HashMap::new();

            for vname in &vec_names {
                let full_name = format!("{pname}.{vname}");
                match self.get_vector(&full_name) {
                    Ok(v) => { vectors.insert(vname.clone(), v); }
                    Err(_) => {
                        // Try without plot prefix
                        if let Ok(v) = self.get_vector(vname) {
                            vectors.insert(vname.clone(), v);
                        }
                    }
                }
            }

            let plot_type = classify_plot(pname);
            plots.insert(pname.clone(), Plot {
                name: pname.clone(),
                plot_type,
                vectors,
            });
        }

        // Extract measurements from log
        let measurements = self.extract_measurements();

        let log = self.take_log();

        Ok(SimulationResult {
            plots,
            measurements,
            log,
        })
    }

    /// Take and clear the accumulated log.
    pub fn take_log(&self) -> Vec<String> {
        let mut log = self.state.log.lock().unwrap();
        std::mem::take(&mut *log)
    }

    /// Parse .meas results from the log output.
    fn extract_measurements(&self) -> HashMap<String, f64> {
        let log = self.state.log.lock().unwrap();
        let mut measurements = HashMap::new();
        for line in log.iter() {
            // ngspice outputs measurements as: "name = value"
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let val_str = line[eq_pos + 1..].trim();
                if let Ok(v) = val_str.parse::<f64>() {
                    measurements.insert(key.to_lowercase(), v);
                }
            }
        }
        measurements
    }

    fn check_exit(&self) -> Result<(), NgspiceError> {
        if let Some(status) = *self.state.exit_status.lock().unwrap() {
            if status != 0 {
                return Err(NgspiceError::ExitError(status));
            }
        }
        Ok(())
    }
}

fn classify_plot(name: &str) -> PlotType {
    let lower = name.to_lowercase();
    if lower.contains("op") { PlotType::OpPoint }
    else if lower.contains("dc") { PlotType::DcSweep }
    else if lower.contains("ac") { PlotType::AcAnalysis }
    else if lower.contains("tran") { PlotType::Transient }
    else if lower.contains("noise") { PlotType::Noise }
    else if lower.contains("pz") { PlotType::PoleZero }
    else if lower.contains("sens") { PlotType::Sensitivity }
    else if lower.contains("tf") { PlotType::TransferFunction }
    else if lower.contains("sp") { PlotType::SParameter }
    else { PlotType::Unknown }
}
