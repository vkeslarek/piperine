//! # The Piperine object model (POM)
//!
//! One call turns MSL sources into an immutable [`Design`]:
//!
//! ```text
//! sources ─► parse ─► lower ─► resolve ─► index ─► Design
//! ```
//!
//! * **Storage.** Arenas of declarations ([`DeclData`]), expressions, statements, patterns,
//!   types and paths, addressed by typed ids ([`DeclId`], [`ExprId`], ..). Nodes never move or
//!   disappear after the build.
//! * **Reflection.** `Copy` handles that borrow a design ([`Module`], [`Port`], [`Signal`],
//!   [`Instance`], ..) and the [`Reflect`] trait they share. Ask a module for its ports, a
//!   signal for the module it is declared in, an instance for the module it places, a
//!   declaration for its [`references`](Reflect::references).
//! * **Resolution.** Names, calls, struct expressions, ports and members are resolved and
//!   classified into slots of the existing nodes (`Resolution`, `CallClass`, `PortKind`, ..);
//!   what needs type inference is `Deferred`. Problems are `diagnostics()`, never a failure:
//!   `build()` always returns a design.
//! * **The one rule.** The model's navigability mirrors the source. Hierarchy and generation are
//!   held as written (a `for` over a thousand items is one declaration), resolution only adds
//!   links, and a lossless [`SourceFile::to_ast`] proves nothing was dropped. A transform that
//!   needs another shape is a side artifact built from a design.
//! * **Serialization.** Every storage type implements `Serialize` and `Deserialize`; the retained
//!   syntax tree is rebuilt from the file text.
//!
//! The reference documentation is under `docs/pom/` (overview, storage model, reflection,
//! construct-to-POM mapping, name resolution), and its `rust` examples are the doctests of
//! [`guide`].

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
pub use builtin::{BUILTINS, BuiltinId, BuiltinInfo, BuiltinKind, builtin, builtin_implicit, builtin_method};
pub use decl::*;
pub use design::{Design, DesignBuilder, DesignDiagnostic, SourceFileData, SourceFileRepr};
pub use index::{Index, ReferenceId, WriterId};
pub use ids::{DeclId, ExprId, FileId, PathId, PatId, StmtId, TypeId};
pub use meta::{AttrArgNode, AttrInputNode, AttrNode, Doc, Meta};
pub use reflect::{Attr, Behavior, Binding, Commit, Const, Contribution, Definition, GenArm, Generation, Instance, Let, Reference, Writer, Module, Nature, Param, Port, Quantity, Reaction, Signal, Decl, Enum, Expr, Field, FnParam, Function, GenericParam, Impl, ImportRef, Namespace, Path, Pat, Reflect, SourceFile, Stmt, Struct, Trait, Type, Use, Variant};
pub use resolution::*;
pub use source::{DiskSources, MemorySources, SourceProvider};
