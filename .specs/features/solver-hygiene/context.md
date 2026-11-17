# Solver Hygiene Context

Decisions the user made for this feature. They bind the design and the tasks.

## Approvals (user, 2026-11-17, through the orchestrator)

- Lanes: "Topo" (lane C is `solver-hygiene`).
- Spec questions Q0 to Q7: "Aceito todas (Recommended)".
- Tasks: "Pode fazer com Opus por agora, com Sonnet implementando e Opus verificando".

## Answers

| # | Question | Decided |
| - | -------- | ------- |
| Q0 | The silent defect of a nonlinear `ddt` through a `limit` | (a) fixed here as HYG-05, P1 |
| Q1 | Basis of the truncation-error test (V1) | (a) the `ddt` states with ngspice's `CKTterr` tolerance; `chgtol` read; PI target 1 and gate `trtol` 7 unchanged |
| Q2 | The asymmetric Milne skip (V2) | (a) removed |
| Q3 | A step that fails the error at `dt_min` (V3) | (a) one retry at the floor, then `E0508`; C.2.2 item 4 and the `E0508` row amended |
| Q4 | Newton damping (V4) | (a) no damping by default, `NodeDamping` (ngspice `niiter.c`) opt-in, the drivers use the plan's `NewtonStrategy` |
| Q5 | The step after a jump (V11) | (a) at most `0.1·min(last step, gap to the next scheduled time point)`, first order, exempt |
| Q6 | A non-zero start without a captured step (V14) | (a) an operating point at `(t_start, 0)` |
| Q7 | Two existing assertions | (a) `failure_rules.rs` asserts `E0507`; `boundary.rs` runs at the 10 ns cap |

## Conditions on the answers

The golden and pin changes listed under each answer in `spec.md` and in design.md "Goldens, pins
and twins that may move" are authorized ONLY with the before/after deviation table vs ngspice in
the commit and the unchanged tolerance contract (`crates/piperine-parity/src/contract.rs`).
