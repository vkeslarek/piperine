//! Per-analysis data contracts — what an element sees and what a host gets
//! back: analysis states (the read-only view during stamping), options, and
//! result types, one file per analysis. The drivers that *run* the analyses
//! live in `crate::solver`.

pub mod ac;
pub mod dc;
pub mod noise;
pub mod pss;
pub mod sens;
pub mod tf;
pub mod transient;
