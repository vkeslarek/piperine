use libloading::Library;
use std::os::raw::{c_char, c_void};
use std::sync::Arc;
use std::path::Path;
use crate::osdi::ffi::OsdiDescriptor;

// ---------------------------------------------------------------------------
// osdi_log — simulator-provided logging callback required by OSDI .so files
// ---------------------------------------------------------------------------

/// OSDI models call this via a function-pointer slot named `osdi_log` in the .so.
/// We install it in `OsdiLib::load` so $display/$fatal messages don't crash.
unsafe extern "C" fn osdi_log_handler(
    _handle: *mut c_void,
    msg: *const c_char,
    lvl: u32,
) {
    if msg.is_null() { return; }
    let text = unsafe { std::ffi::CStr::from_ptr(msg) }.to_string_lossy();
    eprintln!("[osdi] lvl={lvl}: {text}");
}

// ---------------------------------------------------------------------------
// OsdiLib — loaded shared library
// ---------------------------------------------------------------------------

pub struct OsdiLib {
    _lib: Library,
    pub descriptors: *const OsdiDescriptor,
    pub num_descriptors: u32,
}

unsafe impl Send for OsdiLib {}
unsafe impl Sync for OsdiLib {}

impl OsdiLib {
    pub fn load(path: &Path) -> Result<Arc<Self>, libloading::Error> {
        let lib = unsafe { Library::new(path)? };
        let (descriptors, num_descriptors) = unsafe {
            // OSDI_DESCRIPTORS IS the array in memory (not a pointer-to-pointer).
            // Load as raw *const u8 to bypass libloading's pointer-size check,
            // then reinterpret the symbol address as a pointer to OsdiDescriptor.
            let sym_desc = lib.get::<*const u8>(b"OSDI_DESCRIPTORS\0")?;
            let descriptors = sym_desc.into_raw().as_raw_ptr() as *const OsdiDescriptor;

            let sym_num = lib.get::<*const u8>(b"OSDI_NUM_DESCRIPTORS\0")?;
            let num_descriptors: u32 = *(sym_num.into_raw().as_raw_ptr() as *const u32);

            // Install osdi_log so $display/$fatal message callbacks don't crash.
            // osdi_log is a function-pointer slot in the .so that the simulator must fill.
            if let Ok(log_slot) = lib.get::<*mut unsafe extern "C" fn(*mut c_void, *const c_char, u32)>(b"osdi_log\0") {
                log_slot.write(osdi_log_handler);
            }

            (descriptors, num_descriptors)
        };
        Ok(Arc::new(Self { _lib: lib, descriptors, num_descriptors }))
    }

    pub fn descriptor(&self, idx: usize) -> &OsdiDescriptor {
        assert!(idx < self.num_descriptors as usize, "descriptor index {idx} out of range");
        unsafe { &*self.descriptors.add(idx) }
    }

    pub fn num_descriptors(&self) -> usize {
        self.num_descriptors as usize
    }
}

