//! # The Piperine object model
//!
//! One call turns MSL sources into an immutable [`Design`]: arenas of
//! declarations and body nodes addressed by typed ids, read through `Copy`
//! reflection handles. The model mirrors the source: a signal declared in
//! module `abc` is found there and nowhere else.

mod body;
mod decl;
mod design;
mod ids;
mod lower;
mod meta;
mod reflect;
mod resolution;

pub use body::*;
pub use decl::*;
pub use design::{Design, DesignBuilder, DesignDiagnostic, SourceFileData, SourceFileRepr};
pub use ids::{DeclId, ExprId, FileId, PathId, PatId, StmtId, TypeId};
pub use meta::{AttrArgNode, AttrInputNode, AttrNode, Doc, Meta};
pub use reflect::{Attr, Const, Decl, Enum, Expr, Field, FnParam, Function, GenericParam, Path, Pat, Reflect, SourceFile, Stmt, Struct, Type, Variant};
pub use resolution::*;
