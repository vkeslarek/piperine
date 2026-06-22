pub mod error;
pub mod hardware;
pub mod registry;
pub mod types;
pub mod elaboration;

pub use elaboration::{
    elaborate, ElaborationResult,
    extract_va_modules, VaModuleInfo,
    eval_default_expr,
};
pub use error::ElaborationError;
pub use hardware::{HardwareDefinition, HardwareInstance, PortDefinition, PortDirection, ParameterDefinition, NetResolver};
pub use registry::HardwareRegistry;
pub use types::{ParameterValue, ParameterMap, ConnectionMap, parse_si_real};
