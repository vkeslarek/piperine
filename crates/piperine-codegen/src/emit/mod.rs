//! The reusable Cranelift emission machinery: POM `Expr`/`Stmt` → native
//! code. The [`Codegen`] trait is implemented for POM expression types, so
//! adding a new expression variant requires only one match arm — not
//! separate lowering + emission code in multiple files.
//!
//! The [`Builder`] wraps Cranelift's `FunctionBuilder` and provides
//! high-level methods (arithmetic, quad logic, name resolution, control
//! flow) that the trait impls call.

pub mod abi;
#[allow(unused_imports)]
pub mod analog_emit;
pub mod builder;
pub mod digital_expr;

pub use abi::SimCtx;
pub use builder::{Builder, DigTy, Resolver, Typed, expr_structural_eq};
pub use digital_expr::Codegen;
