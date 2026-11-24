//! Scope: the run settings of `piperine_lang::elab` (generics phase D, GN-24 and GN-25): `temperature()` lowers to
//! the two environment reads of the `Option` pair (whether the run set it, and the run's kelvin) and nothing scales or
//! offsets a model; the stdlib `multiplicity()` lowers to the session parameter `multiplicity`.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use support::Elab;

const HEAD: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n";

/// The resistor of this target, `r` its own param, and a top placing it with `block` as the param block.
struct Resistors;

impl Resistors {
    /// `module Resistor(p, n) { r }` whose body is `analog`, placed as `Resistor(a, gnd) { block }` from `Top`.
    fn source(analog: &str, block: &str) -> String {
        format!("{HEAD}module Resistor(p: E, n: E) {{ r: f64 = 1k; m: Option<f64>; }}\nanalog for Resistor {{ {analog} }}\nmodule Top() {{ signal a: E; let r1 = Resistor(a, gnd) {{ {block} }}; }}")
    }
}

#[test]
fn the_temperature_call_lowers_to_the_run_read_and_nothing_is_scaled() {
    // GN-24 AC 1, GN-25 AC 1 (§8.5.2, §8.5.3): `temperature().unwrap_or(x)` reads the run's two environment cells and
    // nothing else; the contribution `i(p, n) +> v(p, n) / r` carries no factor of any multiplicity or offset, whether
    // or not the module has a param named `m`
    let ir = Elab::ir(&Resistors::source("let t = temperature().unwrap_or(1.0); i(p, n) +> v(p, n) / r;", "r: 1k, m: Some(4.0)"), "Top");
    let text = ir.print_structural();
    assert!(text.contains("temperature_given()") && text.contains("temperature()"), "the pair of environment reads: {text}");
    // the ambient fallback (§8.5.3) is no special case: `unwrap_or` is the ordinary `Option` method, so the given flag
    // is read and selects between the temperature and the constant
    let ambient = Elab::ir(&Resistors::source("let t = temperature().unwrap_or(AMBIENT_TEMPERATURE); i(p, n) +> v(p, n) / r;", "r: 1k"), "Top").print_structural();
    assert!(ambient.contains("temperature_given()") && ambient.contains("300.15"), "{ambient}");
    let plain = Elab::ir(&Resistors::source("i(p, n) +> v(p, n) / r;", "r: 1k"), "Top").print_structural();
    let scaled = |text: &str| text.lines().filter(|l| l.contains("probe(p, n)")).map(str::to_string).collect::<Vec<_>>();
    assert_eq!(scaled(&text), scaled(&plain), "a bound `m` scales nothing: {text}");
}

#[test]
fn multiplicity_lowers_to_the_session_parameter() {
    // GN-24 AC 1, AC 3 (§8.5.2): the stdlib `multiplicity()` is the session parameter `multiplicity`, read as `simparam`
    // does (`Expr::SessionParameter`), one per body; `m.or(multiplicity()).unwrap_or(DEFAULT_MULTIPLICITY)` reads
    // the module's own `m` first and the run's second
    let body = "let count = m.or(multiplicity()).unwrap_or(DEFAULT_MULTIPLICITY); i(p, n) +> count * v(p, n) / r;";
    let ir = Elab::ir(&Resistors::source(body, "r: 1k, m: Some(4.0)"), "Top");
    let parameters: Vec<String> = ir.bodies().find(|b| b.name() == "Resistor").expect("the resistor body").session_parameters().iter().map(|p| p.name.to_string()).collect();
    assert_eq!(parameters, ["multiplicity"], "one session parameter, the run's");
}
