//! Event spec (`@ posedge(clk)`, `@ cross(...)`, ...) → resolved event info.
#![allow(dead_code)]
//! For the POM path, event trigger expressions are POM `Expr` (resolved
//! but not lowered to `IrExpr`).

use piperine_lang::parse::ast::EventSpec;

use crate::lower::*;

use super::expr::resolve_expr;
use super::LowerCtx;

pub(crate) enum LoweredEvent {
    Analog(EventSource),
    Digital(DigitalEvent),
}

/// Convert an event-spec AST node into a vector of [`LoweredEvent`],
/// supporting `@initial`, `@final`, `@cross(...)`, `@above(...)`,
/// `@timer(...)`, and digital edges (`@posedge`, `@negedge`, `@change`).
pub(crate) fn convert_event_spec(spec: &EventSpec, ctx: &mut LowerCtx) -> Vec<LoweredEvent> {
    match spec {
        EventSpec::Initial => {
            if ctx.is_digital {
                vec![LoweredEvent::Digital(DigitalEvent::Initial)]
            } else {
                vec![LoweredEvent::Analog(EventSource::InitialStep)]
            }
        }
        EventSpec::Final => {
            if ctx.is_digital {
                vec![LoweredEvent::Digital(DigitalEvent::Final)]
            } else {
                vec![LoweredEvent::Analog(EventSource::FinalStep)]
            }
        }
        EventSpec::Named { name, arg } => {
            let arg_resolved = resolve_expr(arg, ctx);
            match name.as_str() {
                "cross" => vec![LoweredEvent::Analog(EventSource::Cross { dir: CrossDir::Either, expr: arg_resolved })],
                "above" => vec![LoweredEvent::Analog(EventSource::Above { expr: arg_resolved })],
                "timer" => vec![LoweredEvent::Analog(EventSource::Timer { period: arg_resolved })],
                "posedge"  => vec![LoweredEvent::Digital(DigitalEvent::Posedge(arg_resolved))],
                "negedge"  => vec![LoweredEvent::Digital(DigitalEvent::Negedge(arg_resolved))],
                "change"   => vec![LoweredEvent::Digital(DigitalEvent::Change(arg_resolved))],
                other => unreachable!("event `{other}` passed validation but has no IR lowering"),
            }
        }
        EventSpec::Or(specs) => {
            let mut all = Vec::new();
            let mut digitals = Vec::new();
            for s in specs {
                for ev in convert_event_spec(s, ctx) {
                    match ev {
                        LoweredEvent::Digital(d) => digitals.push(d),
                        a => all.push(a),
                    }
                }
            }
            if !digitals.is_empty() {
                if digitals.len() == 1 {
                    all.push(LoweredEvent::Digital(digitals.pop().unwrap()));
                } else {
                    all.push(LoweredEvent::Digital(DigitalEvent::Or(digitals)));
                }
            }
            all
        }
    }
}
