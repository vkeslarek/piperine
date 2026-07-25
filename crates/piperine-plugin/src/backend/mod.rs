//! Backend loaders. Native is the only dlopen backend (MD-21: native +
//! embedded Python; the WASM and process backends are removed).

pub mod native;
pub mod process;
pub mod wire_hosted;
