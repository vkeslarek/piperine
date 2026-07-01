//! Event spec (`@ posedge(clk)`, `@ cross(...)`, ...) → `IrEventKind`.

use crate::parse::ast::EventSpec;

use piperine_codegen::ir::*;

use super::expr::lower_expr;
use super::LowerCtx;


/// Convert an event-spec AST node into a vector of [`IrEventKind`],
/// supporting `@initial`, `@final`, `@cross(...)`, `@above(...)`,
/// `@timer(...)`, and digital edges (`@posedge`, `@negedge`, `@change`).
pub(crate) fn convert_event_spec(spec: &EventSpec, ctx: &mut LowerCtx) -> Vec<IrEventKind> {
    match spec {
        EventSpec::Initial => vec![IrEventKind::InitialStep],
        EventSpec::Final => vec![IrEventKind::FinalStep],
        EventSpec::Named { name, arg } => {
            let arg_ir = lower_expr(arg, &mut ctx.clone());
            match name.as_str() {
                "cross" => vec![IrEventKind::Cross { dir: 0, expr: Some(arg_ir) }],
                "above" => vec![IrEventKind::Above { expr: Some(arg_ir) }],
                "timer" => vec![IrEventKind::Timer { period: Some(arg_ir) }],
                // Digital-style events inside PHDL `digital` blocks.
                "posedge"  => vec![IrEventKind::Posedge(arg_ir)],
                "negedge"  => vec![IrEventKind::Negedge(arg_ir)],
                "change"   => vec![IrEventKind::Change(arg_ir)],
                _ => vec![IrEventKind::InitialStep],
            }
        }
        EventSpec::Or(specs) => {
            specs.iter().flat_map(|s| convert_event_spec(s, ctx)).collect()
        }
    }
}

