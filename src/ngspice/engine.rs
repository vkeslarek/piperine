use crate::ngspice::structs::{
    Complex, Event, IsrcSync, IsrcSyncResponse, SyncData, SyncDataResponse, VectorInfo, VsrcSync,
    VsrcSyncResponse,
};
use crate::ngspice::{callbacks, ffi};
use once_cell::sync::OnceCell;
use std::ffi::{c_void, CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub trait NgSpiceSyncData: Send + Sync {
    fn get_vsrc_sync(&self, vsrc_sync: &VsrcSync) -> Option<VsrcSyncResponse>;
    fn get_isrc_sync(&self, isrc_sync: &IsrcSync) -> Option<IsrcSyncResponse>;
    fn sync_data(&self, sync_data: &SyncData) -> Option<SyncDataResponse>;
}

#[derive(Debug)]
pub enum NgSpiceError {
    EngineNotRunning,
    NoCurrentPlotAvailable,
}

pub struct NgSpiceEngine {
    receiver: crossbeam::channel::Receiver<Event>,
    initialized: AtomicBool,
    pub(crate) sender: crossbeam::channel::Sender<Event>,
    pub(crate) terminated: AtomicBool,
    pub(crate) shutdown_lock: Mutex<bool>,
    pub(crate) shutdown_cond: Condvar,
    pub(crate) sync_data: Vec<Arc<dyn NgSpiceSyncData>>,
}

impl NgSpiceEngine {
    fn new() -> Arc<Self> {
        let (sender, receiver) = crossbeam::channel::unbounded();

        let engine = Arc::new(Self {
            sender,
            receiver,
            initialized: AtomicBool::new(false),
            terminated: AtomicBool::new(false),
            shutdown_lock: Mutex::new(false),
            shutdown_cond: Condvar::new(),
            sync_data: Vec::new(),
        });

        engine.init_if_needed();
        engine
    }

    pub fn instance() -> Arc<Self> {
        static GLOBAL: OnceCell<Arc<NgSpiceEngine>> = OnceCell::new();
        GLOBAL.get_or_init(|| Self::new()).clone()
    }

    fn init_if_needed(self: &Arc<Self>) {
        if self
            .initialized
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            unsafe {
                let user_data = Arc::into_raw(self.clone()) as *mut c_void;
                ffi::ngSpice_Init(
                    Some(callbacks::send_char_callback),
                    Some(callbacks::send_stat_callback),
                    Some(callbacks::controlled_exit_callback),
                    Some(callbacks::send_data_callback),
                    Some(callbacks::send_init_data_callback),
                    Some(callbacks::background_thread_running_callback),
                    user_data,
                );

                ffi::ngSpice_Init_Sync(
                    Some(callbacks::get_vsrc_sync),
                    Some(callbacks::get_isrc_sync),
                    Some(callbacks::sync_data),
                    ptr::null_mut() as *mut c_int,
                    user_data,
                );
            }
        }
    }

    // Helper to check lifecycle
    fn check_alive(&self) -> Result<(), NgSpiceError> {
        if self.terminated.load(Ordering::SeqCst) {
            return Err(NgSpiceError::EngineNotRunning);
        }

        Ok(())
    }

    pub fn sender(&self) -> &crossbeam::channel::Sender<Event> {
        &self.sender
    }

    pub fn receiver(&self) -> crossbeam::channel::Receiver<Event> {
        self.receiver.clone()
    }

    pub fn send_command(self: &Arc<Self>, command: &str) -> Result<(), NgSpiceError> {
        self.check_alive()?;

        unsafe {
            self.init_if_needed();
            let command_cstr = CString::new(command).unwrap();
            ffi::ngSpice_Command(command_cstr.as_ptr() as *mut c_char);
        }
        Ok(())
    }

    pub fn vector_info(
        self: &Arc<Self>,
        vec_name: &str,
    ) -> Result<Option<VectorInfo>, NgSpiceError> {
        self.check_alive()?;
        unsafe {
            self.init_if_needed();
            let name = CString::new(vec_name).unwrap();
            let vec_info_ptr = ffi::ngGet_Vec_Info(name.as_ptr() as *mut c_char);

            if vec_info_ptr.is_null() {
                return Ok(None);
            }

            Ok(Some(VectorInfo::from_raw(vec_info_ptr)))
        }
    }

    pub fn circuit(self: &Arc<Self>, circuit: Vec<String>) -> Result<i32, NgSpiceError> {
        self.check_alive()?;
        unsafe {
            self.init_if_needed();
            let cstrings: Vec<CString> = circuit
                .iter()
                .map(|s| CString::new(s.as_str()).unwrap())
                .collect();
            let mut ptrs: Vec<*mut c_char> =
                cstrings.iter().map(|c| c.as_ptr() as *mut c_char).collect();
            ptrs.push(std::ptr::null_mut());

            Ok(ffi::ngSpice_Circ(ptrs.as_mut_ptr()))
        }
    }

    pub fn current_plot(self: &Arc<Self>) -> Result<String, NgSpiceError> {
        self.check_alive()?;
        unsafe {
            self.init_if_needed();
            let plot_name = ffi::ngSpice_CurPlot();
            if plot_name.is_null() {
                return Err(NgSpiceError::NoCurrentPlotAvailable);
            }

            Ok(CStr::from_ptr(plot_name).to_string_lossy().into_owned())
        }
    }

    pub fn all_plots(self: &Arc<Self>) -> Result<Vec<String>, NgSpiceError> {
        self.check_alive()?;

        unsafe {
            self.init_if_needed();

            let mut result = Vec::new();

            let plots = ffi::ngSpice_AllPlots();

            if plots.is_null() {
                return Ok(result);
            }

            let mut i = 0;

            loop {
                let ptr = *plots.add(i);

                if ptr.is_null() {
                    break;
                }

                let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();

                result.push(s);

                i += 1;
            }

            Ok(result)
        }
    }

    pub fn all_vectors(self: &Arc<Self>, plotname: String) -> Result<Vec<String>, NgSpiceError> {
        self.check_alive()?;

        unsafe {
            self.init_if_needed();

            let plotname_cstr = CString::new(plotname).unwrap();

            let vecs = ffi::ngSpice_AllVecs(plotname_cstr.as_ptr() as *mut c_char);
            let mut result = Vec::new();

            if vecs.is_null() {
                return Ok(result);
            }

            let mut i = 0;

            loop {
                let ptr = *vecs.add(i);

                if ptr.is_null() {
                    break;
                }

                let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();

                result.push(s);

                i += 1;
            }

            Ok(result)
        }
    }

    pub fn sync_quit(self: &Arc<Self>) -> Result<(), NgSpiceError> {
        self.send_command("quit")?;

        let mut terminated = self.shutdown_lock.lock().unwrap();
        if !*terminated {
            let mut result;
            (terminated, result) = self
                .shutdown_cond
                .wait_timeout(terminated, Duration::from_secs(10))
                .unwrap();

            if result.timed_out() {
                return Err(NgSpiceError::EngineNotRunning);
            }
        }

        Ok(())
    }

    pub(crate) fn get_vsrc_sync(&self, vsrc_sync: VsrcSync) -> Option<Complex> {
        self.sync_data
            .iter()
            .find_map(|sync_data| sync_data.get_vsrc_sync(&vsrc_sync))
            .and_then(|vsrc_sync| vsrc_sync.voltage)
    }

    pub(crate) fn get_isrc_sync(&self, isrc_sync: IsrcSync) -> Option<Complex> {
        self.sync_data
            .iter()
            .find_map(|sync_data| sync_data.get_isrc_sync(&isrc_sync))
            .and_then(|isrc_sync| isrc_sync.current)
    }

    pub(crate) fn get_sync_data(&self, sync_data: SyncData) -> Option<f64> {
        self.sync_data
            .iter()
            .find_map(|sync_data_trait| sync_data_trait.sync_data(&sync_data))
            .and_then(|sync_data| sync_data.delta_time)
    }
}
