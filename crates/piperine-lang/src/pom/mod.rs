//! # The Piperine object model
//!
//! One call turns MSL sources into an immutable [`Design`]: arenas of
//! declarations and body nodes addressed by typed ids, read through `Copy`
//! reflection handles. The model mirrors the source: a signal declared in
//! module `abc` is found there and nowhere else.

pub mod guide;
mod body;
mod builtin;
mod check;
mod decl;
mod design;
mod ids;
mod index;
mod lower;
mod meta;
mod reflect;
mod resolve;
mod resolution;
mod source;
mod to_ast;

pub use body::*;
pub use builtin::{BUILTINS, BuiltinId, BuiltinInfo, BuiltinKind, builtin, builtin_method};
pub use decl::*;
pub use design::{Design, DesignBuilder, DesignDiagnostic, SourceFileData, SourceFileRepr};
pub use index::{Index, ReferenceId, WriterId};
pub use ids::{DeclId, ExprId, FileId, PathId, PatId, StmtId, TypeId};
pub use meta::{AttrArgNode, AttrInputNode, AttrNode, Doc, Meta};
pub use reflect::{Attr, Behavior, Binding, Commit, Const, Contribution, Definition, GenArm, Generation, Instance, Let, Reference, Writer, Module, Nature, Param, Port, Quantity, Reaction, Signal, Decl, Enum, Expr, Field, FnParam, Function, GenericParam, Impl, ImportRef, Namespace, Path, Pat, Reflect, SourceFile, Stmt, Struct, Trait, Type, Use, Variant};
pub use resolution::*;
pub use source::{DiskSources, MemorySources, SourceProvider};
