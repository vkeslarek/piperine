use crate::circuit::netlist::NodeIdentifier;
use std::sync::Arc;

use crate::osdi::loader::OsdiLib;

// ---------------------------------------------------------------------------
// OsdiDevice — Component
// ---------------------------------------------------------------------------

pub struct OsdiDevice {
    pub name: String,
    pub lib: Arc<OsdiLib>,
    pub descriptor_idx: usize,
    pub terminals: Vec<NodeIdentifier>,
    pub params: Vec<(String, f64)>,
    /// String-valued parameters (e.g. `.type = "nmos"`).
    pub str_params: Vec<(String, String)>,
}

impl OsdiDevice {
    pub fn new(
        name: String,
        lib: Arc<OsdiLib>,
        descriptor_idx: usize,
        terminals: Vec<NodeIdentifier>,
    ) -> Self {
        Self { name, lib, descriptor_idx, terminals, params: Vec::new(), str_params: Vec::new() }
    }

    pub fn new_with_params(
        name: String,
        lib: Arc<OsdiLib>,
        descriptor_idx: usize,
        terminals: Vec<NodeIdentifier>,
        params: Vec<(String, f64)>,
    ) -> Self {
        Self { name, lib, descriptor_idx, terminals, params, str_params: Vec::new() }
    }

    pub fn new_with_all_params(
        name: String,
        lib: Arc<OsdiLib>,
        descriptor_idx: usize,
        terminals: Vec<NodeIdentifier>,
        params: Vec<(String, f64)>,
        str_params: Vec<(String, String)>,
    ) -> Self {
        Self { name, lib, descriptor_idx, terminals, params, str_params }
    }
}


