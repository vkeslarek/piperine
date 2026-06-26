use std::path::Path;
use std::sync::Arc;
use crate::analog::osdi::loader::OsdiLib;

/// An analog device model loaded from an OSDI shared library.
/// Wraps `OsdiLib` and a descriptor index, hiding internal details from clients.
pub struct AnalogModel {
    pub lib: Arc<OsdiLib>,
    pub descriptor_idx: usize,
}

impl AnalogModel {
    /// Load a single-descriptor OSDI model from a .osdi file.
    pub fn load(path: &Path) -> crate::result::Result<Self> {
        let lib = OsdiLib::load(path).map_err(|e| crate::error::Error::simple("Load Error", &e.to_string()))?;
        Ok(Self { lib, descriptor_idx: 0 })
    }

    /// Load all descriptors from an OSDI file as separate models.
    pub fn load_all(path: &Path) -> crate::result::Result<Vec<Self>> {
        let lib = OsdiLib::load(path).map_err(|e| crate::error::Error::simple("Load Error", &e.to_string()))?;
        let count = lib.num_descriptors();
        Ok((0..count).map(|i| Self { lib: lib.clone(), descriptor_idx: i }).collect())
    }

    /// Create from an already-loaded library and descriptor index.
    pub fn from_lib(lib: Arc<OsdiLib>, descriptor_idx: usize) -> Self {
        Self { lib, descriptor_idx }
    }
}
