//! The sources of `piperine-abi`, embedded (design "AbiSource"; RS-05): generated code links the
//! ABI as an `rlib` built from these with `rustc` alone, so a user needs no `cargo` and no
//! checkout. `tests/abi_source.rs` compares the table with the crate's directory, so a new ABI
//! file cannot be left out.

use std::path::Path;

use crate::error::Error;

/// The embedded ABI crate.
#[derive(Clone, Copy, Debug)]
pub struct AbiSource;

impl AbiSource {
    /// Every file of `crates/piperine-abi/src`, a path relative to it, as (path, text); `lib.rs` is the crate root.
    pub const FILES: &'static [(&'static str, &'static str)] = &[
        ("caller.rs", include_str!("../../piperine-abi/src/caller.rs")),
        ("device.rs", include_str!("../../piperine-abi/src/device.rs")),
        ("layout.rs", include_str!("../../piperine-abi/src/layout.rs")),
        ("lib.rs", include_str!("../../piperine-abi/src/lib.rs")),
        ("raw.rs", include_str!("../../piperine-abi/src/raw.rs")),
        ("raw/calls.rs", include_str!("../../piperine-abi/src/raw/calls.rs")),
        ("raw/constants.rs", include_str!("../../piperine-abi/src/raw/constants.rs")),
        ("raw/shape.rs", include_str!("../../piperine-abi/src/raw/shape.rs")),
        ("raw/views.rs", include_str!("../../piperine-abi/src/raw/views.rs")),
        ("shape.rs", include_str!("../../piperine-abi/src/shape.rs")),
        ("thunk.rs", include_str!("../../piperine-abi/src/thunk.rs")),
        ("validate.rs", include_str!("../../piperine-abi/src/validate.rs")),
        ("verify.rs", include_str!("../../piperine-abi/src/verify.rs")),
    ];

    /// Writes every file into `dir` (created if absent); `dir/lib.rs` is what
    /// `Rustc::rlib` builds.
    pub fn write_to(dir: &Path) -> Result<(), Error> {
        let fail = |e: std::io::Error| Error::Cache { dir: dir.to_path_buf(), cause: e.to_string() };
        std::fs::create_dir_all(dir).map_err(fail)?;
        for (name, text) in AbiSource::FILES {
            let target = dir.join(name);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(fail)?;
            }
            std::fs::write(target, text).map_err(fail)?;
        }
        Ok(())
    }
}
