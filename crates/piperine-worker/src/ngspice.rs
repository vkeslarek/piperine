mod ffi {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(unused)]

    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

use std::cell::Cell;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use num_complex::Complex64;

use ffi::{
    ngcomplex_t, ngSpice_Init, ngSpice_Command, ngSpice_Circ, ngSpice_CurPlot,
    ngSpice_AllPlots, ngSpice_AllVecs, ngSpice_running, ngSpice_SetBkpt,
    ngSpice_nospinit, ngSpice_nospiceinit, ngGet_Vec_Info,
    pvecvaluesall, pvecinfoall, vecvaluesall, vecinfoall,
    vecvalues, vecinfo, vector_info,
};

// ── Safe Rust types ────────────────────────────────────────────────

impl From<&ngcomplex_t> for Complex64 {
    fn from(c: &ngcomplex_t) -> Self {
        Complex64::new(c.cx_real, c.cx_imag)
    }
}

impl From<ngcomplex_t> for Complex64 {
    fn from(c: ngcomplex_t) -> Self {
        Complex64::new(c.cx_real, c.cx_imag)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum VectorType {
    Notype = 0,
    Time = 1,
    Frequency = 2,
    Voltage = 3,
    Current = 4,
    OutputNDensity = 5,
    OutputNoise = 6,
    InputNDensity = 7,
    InputNoise = 8,
    Pole = 9,
    Zero = 10,
    SParam = 11,
    Temp = 12,
    Res = 13,
    Impedance = 14,
    Admittance = 15,
    Power = 16,
    Phase = 17,
    Db = 18,
    Capacitance = 19,
    Charge = 20,
}

impl VectorType {
    pub fn from_raw(v: i32) -> Self {
        match v {
            0 => Self::Notype,
            1 => Self::Time,
            2 => Self::Frequency,
            3 => Self::Voltage,
            4 => Self::Current,
            5 => Self::OutputNDensity,
            6 => Self::OutputNoise,
            7 => Self::InputNDensity,
            8 => Self::InputNoise,
            9 => Self::Pole,
            10 => Self::Zero,
            11 => Self::SParam,
            12 => Self::Temp,
            13 => Self::Res,
            14 => Self::Impedance,
            15 => Self::Admittance,
            16 => Self::Power,
            17 => Self::Phase,
            18 => Self::Db,
            19 => Self::Capacitance,
            20 => Self::Charge,
            _ => Self::Notype,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VectorFlags(i16);

impl VectorFlags {
    pub const REAL: Self = Self(1 << 0);
    pub const COMPLEX: Self = Self(1 << 1);
    pub const ACCUM: Self = Self(1 << 2);
    pub const PLOT: Self = Self(1 << 3);
    pub const PRINT: Self = Self(1 << 4);
    pub const MINGIVEN: Self = Self(1 << 5);
    pub const MAXGIVEN: Self = Self(1 << 6);
    pub const PERMANENT: Self = Self(1 << 7);

    pub fn from_raw(v: i16) -> Self {
        Self(v)
    }

    pub fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }

    pub fn bits(self) -> i16 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct VectorInfo {
    pub name: String,
    pub v_type: VectorType,
    pub v_flags: VectorFlags,
    pub v_length: i32,
}

impl From<&vector_info> for VectorInfo {
    fn from(vi: &vector_info) -> Self {
        VectorInfo {
            name: unsafe { CStr::from_ptr(vi.v_name) }.to_string_lossy().into_owned(),
            v_type: VectorType::from_raw(vi.v_type),
            v_flags: VectorFlags::from_raw(vi.v_flags),
            v_length: vi.v_length,
        }
    }
}

pub struct DataValuesView<'a> {
    inner: &'a vecvaluesall,
}

impl<'a> From<&'a vecvaluesall> for DataValuesView<'a> {
    fn from(inner: &'a vecvaluesall) -> Self {
        DataValuesView { inner }
    }
}

impl<'a> DataValuesView<'a> {
    pub fn count(&self) -> i32 { self.inner.veccount }
    pub fn index(&self) -> i32 { self.inner.vecindex }

    pub fn iter(&self) -> DataValuesIter<'_> {
        let count = self.inner.veccount as usize;
        let raw = unsafe {
            if self.inner.vecsa.is_null() || count == 0 {
                None
            } else {
                Some(std::slice::from_raw_parts(self.inner.vecsa, count))
            }
        };
        DataValuesIter { raw, idx: 0, count }
    }
}

pub struct DataValuesIter<'a> {
    raw: Option<&'a [*mut vecvalues]>,
    idx: usize,
    count: usize,
}

impl<'a> Iterator for DataValuesIter<'a> {
    type Item = VecValue<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.count { return None; }
        let ptr = self.raw?[self.idx];
        self.idx += 1;
        let v = unsafe { ptr.as_ref()? };
        Some(VecValue::from(v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count.saturating_sub(self.idx);
        (remaining, Some(remaining))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scalar {
    Real(f64),
    Complex(Complex64),
}

impl Scalar {
    pub fn real(&self) -> f64 {
        match self {
            Scalar::Real(r) => *r,
            Scalar::Complex(c) => c.re,
        }
    }

    pub fn imag(&self) -> f64 {
        match self {
            Scalar::Real(_) => 0.0,
            Scalar::Complex(c) => c.im,
        }
    }

    pub fn is_complex(&self) -> bool {
        matches!(self, Scalar::Complex(_))
    }
}

pub struct VecValue<'a> {
    name: &'a str,
    value: Scalar,
    is_scale: bool,
}

impl<'a> From<&'a vecvalues> for VecValue<'a> {
    fn from(v: &'a vecvalues) -> Self {
        let value = if v.is_complex {
            Scalar::Complex(Complex64::new(v.creal, v.cimag))
        } else {
            Scalar::Real(v.creal)
        };
        VecValue {
            name: unsafe { CStr::from_ptr(v.name) }.to_str().unwrap_or(""),
            value,
            is_scale: v.is_scale,
        }
    }
}

impl<'a> VecValue<'a> {
    pub fn name(&self) -> &str { self.name }
    pub fn value(&self) -> Scalar { self.value }
    pub fn is_scale(&self) -> bool { self.is_scale }
}

pub struct PlotInfoView<'a> {
    inner: &'a vecinfoall,
}

impl<'a> From<&'a vecinfoall> for PlotInfoView<'a> {
    fn from(inner: &'a vecinfoall) -> Self {
        PlotInfoView { inner }
    }
}

impl<'a> PlotInfoView<'a> {
    pub fn name(&self) -> &str {
        unsafe { CStr::from_ptr(self.inner.name) }.to_str().unwrap_or("")
    }

    pub fn title(&self) -> &str {
        unsafe { CStr::from_ptr(self.inner.title) }.to_str().unwrap_or("")
    }

    pub fn date(&self) -> &str {
        unsafe { CStr::from_ptr(self.inner.date) }.to_str().unwrap_or("")
    }

    pub fn plot_type(&self) -> &str {
        unsafe { CStr::from_ptr(self.inner.type_) }.to_str().unwrap_or("")
    }

    pub fn vector_count(&self) -> i32 { self.inner.veccount }

    pub fn vecs(&self) -> Vec<VecMeta> {
        let count = self.inner.veccount as usize;
        if count == 0 { return Vec::new(); }
        let mut result = Vec::with_capacity(count);
        let raw = unsafe {
            if self.inner.vecs.is_null() { None }
            else { Some(std::slice::from_raw_parts(self.inner.vecs, count)) }
        };
        if let Some(vecs_slice) = raw {
            for i in 0..count {
                if let Some(vi) = unsafe { vecs_slice[i].as_ref() } {
                    result.push(VecMeta::from(vi));
                }
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
pub struct VecMeta {
    pub number: i32,
    pub name: String,
    pub is_real: bool,
}

impl From<&vecinfo> for VecMeta {
    fn from(vi: &vecinfo) -> Self {
        VecMeta {
            number: vi.number,
            name: unsafe { CStr::from_ptr(vi.vecname) }.to_str().unwrap_or("").to_string(),
            is_real: vi.is_real,
        }
    }
}

// ── Handler trait ───────────────────────────────────────────────────

pub trait NgspiceHandler: Send {
    fn on_output(&self, _text: &str) {}
    fn on_status(&self, _status_line: &str) {}
    fn on_exit(&self, _status: i32, _immediate_unload: bool, _on_quit: bool) {}
    fn on_data(&self, _values: &DataValuesView) {}
    fn on_init_data(&self, _info: &PlotInfoView) {}
    fn on_bg_thread(&self, _running: bool) {}

    fn on_step(&self, _time: f64) {}
    fn on_initial_step(&self, _time: f64) {}
    fn on_final_step(&self, _time: f64) {}
}

// ── Internal state / trampolines ────────────────────────────────────

struct State {
    handler: Box<dyn NgspiceHandler>,
    alive: Cell<bool>,
}

macro_rules! trampoline_body {
    ($user:ident) => {
        unsafe { &mut *($user as *mut State) }
    };
}

unsafe extern "C" fn tramp_output(s: *mut c_char, _id: c_int, user: *mut c_void) -> c_int {
    let state = trampoline_body!(user);
    let cstr = unsafe { CStr::from_ptr(s) };
    state.handler.on_output(&cstr.to_string_lossy());
    0
}

unsafe extern "C" fn tramp_status(s: *mut c_char, _id: c_int, user: *mut c_void) -> c_int {
    let state = trampoline_body!(user);
    let cstr = unsafe { CStr::from_ptr(s) };
    state.handler.on_status(&cstr.to_string_lossy());
    0
}

unsafe extern "C" fn tramp_exit(
    status: c_int,
    immediate: bool,
    on_quit: bool,
    _id: c_int,
    user: *mut c_void,
) -> c_int {
    let state = trampoline_body!(user);
    state.alive.set(false);
    state.handler.on_final_step(0.0);
    state.handler.on_exit(status, immediate, on_quit);
    0
}

unsafe extern "C" fn tramp_data(
    values: pvecvaluesall,
    _num_structs: c_int,
    _id: c_int,
    user: *mut c_void,
) -> c_int {
    let state = trampoline_body!(user);
    if let Some(v) = unsafe { values.as_ref() } {
        let view = DataValuesView::from(v);
        state.handler.on_data(&view);
        
        let mut time = 0.0;
        for vec in view.iter() {
            if vec.name() == "time" {
                time = vec.value().real();
                break;
            }
        }
        
        if view.index() == 0 {
            state.handler.on_initial_step(time);
        }
        state.handler.on_step(time);
    }
    0
}

unsafe extern "C" fn tramp_init_data(
    info: pvecinfoall,
    _id: c_int,
    user: *mut c_void,
) -> c_int {
    let state = trampoline_body!(user);
    if let Some(i) = unsafe { info.as_ref() } {
        state.handler.on_init_data(&PlotInfoView::from(i));
    }
    0
}

unsafe extern "C" fn tramp_bg_thread(running: bool, _id: c_int, user: *mut c_void) -> c_int {
    let state = trampoline_body!(user);
    state.handler.on_bg_thread(running);
    0
}

// ── Ngspice handle ──────────────────────────────────────────────────

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut INSTANCE: Option<Weak<Inner>> = None;

struct Inner {
    ptr: *mut State,
}

impl Drop for Inner {
    fn drop(&mut self) {
        let state = unsafe { &*self.ptr };
        if state.alive.get() {
            let cmd = CString::new("quit").unwrap();
            unsafe { ngSpice_Command(cmd.as_ptr() as *mut c_char); }
        }
        unsafe { drop(Box::from_raw(self.ptr)); }
        INITIALIZED.store(false, Ordering::SeqCst);
        unsafe { INSTANCE = None; }
    }
}

#[derive(Clone)]
pub struct Ngspice {
    #[allow(dead_code)]
    inner: Arc<Inner>,
}

pub fn no_spinit() {
    unsafe { ngSpice_nospinit(); }
}

pub fn no_spiceinit() {
    unsafe { ngSpice_nospiceinit(); }
}

impl Ngspice {
    pub fn init(handler: Box<dyn NgspiceHandler>) -> Result<Self, i32> {
        let instance = unsafe { &*std::ptr::addr_of!(INSTANCE) };
        if let Some(weak) = instance {
            if let Some(inner) = weak.upgrade() {
                return Ok(Ngspice { inner });
            }
        }

        let state = Box::new(State {
            handler,
            alive: Cell::new(true),
        });
        let ptr = Box::into_raw(state);

        let rc = unsafe {
            ngSpice_Init(
                Some(tramp_output),
                Some(tramp_status),
                Some(tramp_exit),
                Some(tramp_data),
                Some(tramp_init_data),
                Some(tramp_bg_thread),
                ptr as *mut c_void,
            )
        };

        if rc != 0 {
            unsafe { drop(Box::from_raw(ptr)); }
            return Err(rc);
        }

        let inner = Arc::new(Inner { ptr });
        INITIALIZED.store(true, Ordering::SeqCst);
        unsafe { INSTANCE = Some(Arc::downgrade(&inner)); }
        Ok(Ngspice { inner })
    }

    pub fn is_initialized() -> bool {
        INITIALIZED.load(Ordering::SeqCst)
    }

    pub fn command(&self, cmd: &str) -> Result<(), i32> {
        let c_cmd = CString::new(cmd).map_err(|_| -1)?;
        let rc = unsafe { ngSpice_Command(c_cmd.as_ptr() as *mut c_char) };
        if rc != 0 { return Err(rc); }
        // ngSpice_Command returns before the background simulation thread finishes
        // for analysis commands (dc, tran, ac). Poll until the thread is done.
        // Give it a moment to start, then wait for completion.
        std::thread::sleep(std::time::Duration::from_millis(5));
        while unsafe { ngSpice_running() } {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Ok(())
    }

    pub fn load_circuit(&self, lines: &[&str]) -> Result<(), i32> {
        let c_lines: Vec<CString> = lines
            .iter()
            .map(|s| CString::new(*s))
            .collect::<Result<_, _>>()
            .map_err(|_| -1)?;

        let mut ptrs: Vec<*const c_char> = c_lines.iter().map(|cs| cs.as_ptr()).collect();
        ptrs.push(std::ptr::null());

        let rc = unsafe { ngSpice_Circ(ptrs.as_ptr() as *mut *mut c_char) };
        if rc != 0 { Err(rc) } else { Ok(()) }
    }

    pub fn running(&self) -> bool {
        unsafe { ngSpice_running() }
    }

    pub fn set_breakpoint(&self, time: f64) -> bool {
        unsafe { ngSpice_SetBkpt(time) }
    }

    pub fn cur_plot(&self) -> Option<String> {
        unsafe {
            let ptr = ngSpice_CurPlot();
            if ptr.is_null() { None }
            else { Some(CStr::from_ptr(ptr).to_string_lossy().into_owned()) }
        }
    }

    pub fn all_plots(&self) -> Vec<String> {
        unsafe {
            let ptrs = ngSpice_AllPlots();
            if ptrs.is_null() { return Vec::new(); }
            let mut result = Vec::new();
            for i in 0.. {
                let p = *ptrs.add(i);
                if p.is_null() { break; }
                result.push(CStr::from_ptr(p).to_string_lossy().into_owned());
            }
            result
        }
    }

    pub fn all_vecs(&self, plotname: &str) -> Vec<String> {
        let c_name = match CString::new(plotname) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        unsafe {
            let ptrs = ngSpice_AllVecs(c_name.as_ptr() as *mut c_char);
            if ptrs.is_null() { return Vec::new(); }
            let mut result = Vec::new();
            for i in 0.. {
                let p = *ptrs.add(i);
                if p.is_null() { break; }
                result.push(CStr::from_ptr(p).to_string_lossy().into_owned());
            }
            result
        }
    }

    pub fn vec_info(&self, vecname: &str) -> Option<VectorInfo> {
        let c_name = CString::new(vecname).ok()?;
        unsafe {
            let ptr = ngGet_Vec_Info(c_name.as_ptr() as *mut c_char);
            let vi = ptr.as_ref()?;
            Some(VectorInfo::from(vi))
        }
    }

    pub fn vec_real_data(&self, vecname: &str) -> Option<&[f64]> {
        let c_name = CString::new(vecname).ok()?;
        unsafe {
            let ptr = ngGet_Vec_Info(c_name.as_ptr() as *mut c_char);
            let vi = ptr.as_ref()?;
            if vi.v_realdata.is_null() || vi.v_length <= 0 { return None; }
            Some(std::slice::from_raw_parts(vi.v_realdata, vi.v_length as usize))
        }
    }

    pub fn vec_complex_data(&self, vecname: &str) -> Option<Vec<Complex64>> {
        let c_name = CString::new(vecname).ok()?;
        unsafe {
            let ptr = ngGet_Vec_Info(c_name.as_ptr() as *mut c_char);
            let vi = ptr.as_ref()?;
            if vi.v_compdata.is_null() || vi.v_length <= 0 { return None; }
            let slice = std::slice::from_raw_parts(vi.v_compdata, vi.v_length as usize);
            Some(slice.iter().map(Complex64::from).collect())
        }
    }

    pub fn quit(&self) -> Result<(), i32> {
        self.command("quit")
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    struct CountingHandler {
        output_len: Rc<Cell<usize>>,
    }

    impl CountingHandler {
        fn new(counter: Rc<Cell<usize>>) -> Self {
            Self { output_len: counter }
        }
    }

    impl NgspiceHandler for CountingHandler {
        fn on_output(&self, text: &str) {
            self.output_len.set(self.output_len.get() + text.len());
        }
    }

    #[test]
    fn test_safe_wrappers() {
        let counter = Rc::new(Cell::new(0usize));

        eprintln!("[1] init");
        let handler = Box::new(CountingHandler::new(counter.clone()));
        let ng = Ngspice::init(handler).expect("init");

        eprintln!("[2] version");
        ng.command("version").expect("version");

        eprintln!("[3] running");
        assert!(!ng.running());

        eprintln!("[4] breakpoint");
        let _ = ng.set_breakpoint(1e-9);

        eprintln!("[5] cur_plot");
        let plot = ng.cur_plot().expect("cur_plot");
        assert!(!plot.is_empty());

        eprintln!("[6] all_plots");
        let plots = ng.all_plots();
        assert!(!plots.is_empty());

        eprintln!("[7] all_vecs");
        let _ = ng.all_vecs(&plots[0]);

        eprintln!("[8] load_circuit");
        ng.load_circuit(&[
            "RC",
            "V1 1 0 DC 5",
            "R1 1 0 1k",
            ".end",
        ]).expect("load_circuit");

        eprintln!("[9] op");
        ng.command("op").expect("op");

        eprintln!("[10] vec_info");
        let info = ng.vec_info("1").expect("vec_info node 1");
        assert_eq!(info.v_length, 1);

        eprintln!("[11] vec_real_data");
        let data = ng.vec_real_data("1").expect("real data");
        assert_eq!(data.len(), 1);
        assert!((data[0] - 5.0).abs() < 0.01, "node 1 should be 5V");

        eprintln!("[12] nonexistent");
        assert!(ng.vec_info("nonexistent").is_none());
        assert!(ng.vec_real_data("nonexistent").is_none());
        assert!(ng.vec_complex_data("nonexistent").is_none());

        eprintln!("[13] command error output");
        ng.command("invalid_cmd_xyz").unwrap();

        eprintln!("[14] output captured");
        assert!(counter.get() > 0, "output should have been captured");

        eprintln!("[15] quit");
        let _ = ng.quit();
        drop(ng);
        assert!(!Ngspice::is_initialized());

        eprintln!("[16] done");
    }

    #[test]
    fn test_double_init_returns_same() {
        struct Dummy;
        impl NgspiceHandler for Dummy {}

        let ng1 = Ngspice::init(Box::new(Dummy)).expect("first init");
        assert!(Ngspice::is_initialized());

        let ng2 = Ngspice::init(Box::new(Dummy)).expect("second init returns same");
        assert!(Ngspice::is_initialized());

        ng1.command("version").expect("ng1 version");
        ng2.command("version").expect("ng2 version");

        drop(ng1);
        assert!(Ngspice::is_initialized());

        let _ = ng2.quit();
        drop(ng2);
        assert!(!Ngspice::is_initialized());
    }
}
