//! # The Piperine object model
//!
//! One call turns MSL sources into an immutable [`Design`]: arenas of
//! declarations and body nodes addressed by typed ids, read through `Copy`
//! reflection handles. The model mirrors the source: a signal declared in
//! module `abc` is found there and nowhere else.

mod design;
mod ids;
mod reflect;

pub use design::{Design, DesignBuilder, DesignDiagnostic, SourceFileData, SourceFileRepr};
pub use ids::{DeclId, ExprId, FileId, PathId, PatId, StmtId, TypeId};
pub use reflect::SourceFile;
