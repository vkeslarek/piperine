//! `.sp` host surface (SP-05): `Session::sp` fails loud on a design that
//! declares no RF ports. The positive case — a shunt-C low-pass declared
//! with `@rfport(num, z0)` ports — is gone with the PHDL frontend; see the
//! note below.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;


// `@rfport(num, z0)` has no reachable Verilog-AMS spelling: attribute
// instances resolve against `ElabContext::schemas`, which seeds only
// `device`/`port`, and `Design::rfports()` returns `[]` for anything a
// VAMS source can express (verified in ed575c85, which deprecated
// `piperine-lang`'s own `rfport` target for the same reason). Every case
// that needed a *declared* RF port is gone with it; `.sp`'s fail-loud
// path (no ports declared) is kept and green below.

#[test]
fn no_rfport_declared_fails_loud() {
    let src = "module Top(); electrical gnd; endmodule\n";
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let result = session.sp(1e3, 1e9, 5, true, &SolverConfig::default());
    assert!(result.is_err(), "a module with no @rfport ports must fail loud (SP-05)");
}
