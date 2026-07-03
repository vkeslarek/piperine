//! Event spec (`@ posedge(clk)`, `@ cross(...)`, ...) → `IrEventKind`.

use crate::parse::ast::EventSpec;

use piperine_ir::*;

use super::expr::lower_expr;
use super::LowerCtx;


pub(crate) enum LoweredEvent {
    Analog(EventSource),
    Digital(DigitalEvent),
}

/// Convert an event-spec AST node into a vector of [`LoweredEvent`],
/// supporting `@initial`, `@final`, `@cross(...)`, `@above(...)`,
/// `@timer(...)`, and digital edges (`@posedge`, `@negedge`, `@change`).
///
/// Domain-aware (SPEC §10.4): `initial`/`final` work in both analog and
/// digital bodies. In an analog body they lower to `AnalogEvent` with
/// `InitialStep`/`FinalStep`; in a digital body they lower to a
/// `ClockedBlock` with `DigitalEvent::Initial`/`Final` (fires during
/// `init`, never during edge-driven `eval`). `cross`/`above`/`timer` are
/// analog-only; `posedge`/`negedge`/`change` are digital-only.
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
            let arg_ir = lower_expr(arg, ctx);
            match name.as_str() {
                "cross" => vec![LoweredEvent::Analog(EventSource::Cross { dir: CrossDir::Either, expr: arg_ir })],
                "above" => vec![LoweredEvent::Analog(EventSource::Above { expr: arg_ir })],
                "timer" => vec![LoweredEvent::Analog(EventSource::Timer { period: arg_ir })],
                // Digital-style events inside PHDL `digital` blocks.
                "posedge"  => vec![LoweredEvent::Digital(DigitalEvent::Posedge(arg_ir))],
                "negedge"  => vec![LoweredEvent::Digital(DigitalEvent::Negedge(arg_ir))],
                "change"   => vec![LoweredEvent::Digital(DigitalEvent::Change(arg_ir))],
                // Elaboration rejects unregistered event names (UnknownEvent)
                // before lowering runs.
                other => unreachable!("event `{other}` passed validation but has no IR lowering"),
            }
        }
        EventSpec::Or(specs) => {
            // For digital events, we want to group them into DigitalEvent::Or
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
