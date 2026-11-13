//! The reference documentation of the object model, included here so that every
//! `rust` example of `docs/pom/*.md` runs as a doctest (`cargo test -p piperine-lang
//! --doc pom::guide`). The pages are also published with the rest of `docs/` by mkdocs.

/// Overview and quick tour: `docs/pom/index.md`.
#[doc = include_str!("../../../../docs/pom/index.md")]
pub mod index {}

/// Storage reference: `docs/pom/model.md`.
#[doc = include_str!("../../../../docs/pom/model.md")]
pub mod model {}
