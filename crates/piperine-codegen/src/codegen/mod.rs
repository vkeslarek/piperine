//! Trait-based code generation: POM `Expr`/`Stmt` → Cranelift native code.
//!
//! The [`Codegen`] trait is implemented for POM expression types, so adding
//! a new expression variant requires only one match arm — not separate
//! lowering + emission code in multiple files.
//!
//! The [`Builder`] wraps Cranelift's `FunctionBuilder` and provides
//! high-level methods (arithmetic, quad logic, name resolution, control
//! flow) that the trait impls call.

pub mod builder;
pub mod trait_;

pub use builder::{Builder, DigTy, Resolver, Typed};
pub use trait_::Codegen;
