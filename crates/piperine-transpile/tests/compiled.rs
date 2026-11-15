//! The compiled design as modules and a circuit (feature `ir-rust`, RS-05 AC 1 and 2; checks
//! C26, C27, C28, C29): each running body is bound to the module the library gives under its
//! name, as the interpreter binds its own; every loaded prototype equals the plan's shape entry by
//! entry; a library whose shape differs from its plan is refused naming the entry; and the
//! compiled divider solves to the divider ratio, as the interpreter's does.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};

use piperine_abi::raw::{Cell, List, Module, Shape};
use piperine_interp::Interpreter;
use piperine_ir::{Ir, IrBuilder, NodeRef};
use piperine_plan::{BodyPlan, Modules};
use piperine_solver::prelude::{Context, NodeIdentifier};
use piperine_transpile::{Compiled, Error, Settings, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "../../piperine-interp/tests/support/circuits.rs"]
mod circuits;

use circuits::Circuits;
use devices::Devices;

/// Writing an executable and exec'ing it races with another thread's `fork` holding the write
/// descriptor (ETXTBSY); the tests here take turns.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn compile(ir: &Ir, settings: &Settings) -> Compiled {
    let t = Transpiler::new(Arc::new(ir.clone())).unwrap_or_else(|e| panic!("{e}"));
    t.compile(settings).unwrap_or_else(|e| panic!("{e}"))
}

fn fresh() -> (tempfile::TempDir, Settings) {
    let cache = tempfile::tempdir().unwrap();
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    (cache, settings)
}

/// A design that holds every M1 device body (the top is a resistor alone).
fn all_five() -> Ir {
    let mut b = IrBuilder::new();
    let r = Devices::resistor(&mut b);
    Devices::capacitor(&mut b);
    Devices::vsource(&mut b);
    Devices::isource(&mut b);
    Devices::diode(&mut b);
    let top = Devices::alone(&mut b, r, "R", 1, vec![1000.0]);
    b.finish(top)
}

/// A list's elements, read through the safe accessor.
fn items<T: Copy>(list: &List<'_, T>) -> Vec<T> {
    (0..list.len()).map(|i| *list.get(i).expect("a readable element")).collect()
}

// RS-05 AC 1: a library's modules are read through its symbol and bound by body, like the
// interpreter's: a running body has the module of its name, a body with nothing to run has none.
#[test]
fn modules_come_through_the_symbol_and_bind_like_the_interpreter() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (_cache, settings) = fresh();
    let ir = Circuits::divider();
    let compiled = compile(&ir, &settings);
    let interp = Interpreter::new(Arc::new(ir.clone())).expect("a checked IR");

    let mut running = 0;
    for body in ir.bodies() {
        let ours = Modules::module(&compiled, body.id());
        let theirs = Modules::module(&interp, body.id());
        assert_eq!(ours.is_some(), theirs.is_some(), "{}: both backends run the same bodies", body.name());
        let (Some(ours), Some(theirs)) = (ours, theirs) else { continue };
        running += 1;
        assert_eq!(ours.name().as_str(), body.name());
        assert!(std::ptr::eq(ours, compiled.loaded().module(body.name()).expect("in the library")), "through the symbol");
        let pins = |m: &Module| -> Vec<(String, u32, u32)> {
            items(&m.prototype().pins).iter().map(|p| (p.name.as_str().to_string(), p.kind, p.role)).collect()
        };
        assert_eq!(pins(ours), pins(theirs), "{}: the same pins in the same order", body.name());
    }
    assert_eq!(running, 2, "the divider's V source and resistor run; its top has no analog block");
    assert!(Modules::module(&compiled, ir.top().body().id()).is_none(), "the top body has no module");
    assert_eq!(compiled.loaded().names(), vec!["Vsource", "Resistor"]);
}

/// What a test reads of a shape, entry by entry, as plain values.
struct Entries;

impl Entries {
    fn of(shape: &Shape<'_>) -> Vec<String> {
        let mut out = Vec::new();
        for p in items(&shape.pins) {
            out.push(format!(
                "pin {} {} kind {} role {} global {} flags {} size {} init {:?} check {} abstol {:?}",
                p.name.as_str(), p.origin.as_str(), p.kind, p.role, p.global, p.flags, p.layout.size,
                p.init.as_slice(), p.check, p.abstol
            ));
        }
        for t in items(&shape.terms) {
            out.push(format!("term {} {} op {} {} {}", t.name.as_str(), t.origin.as_str(), t.op, t.plus, t.minus));
        }
        for i in items(&shape.inputs) {
            out.push(format!("input {} {} kind {}", i.name.as_str(), i.origin.as_str(), i.kind));
        }
        for f in items(&shape.functions) {
            let cells = |l: &List<'_, Cell>| items(l).iter().map(|c| (c.kind, c.index, c.part)).collect::<Vec<_>>();
            let partials: Vec<_> = items(&f.partials).iter().map(|p| (p.write, p.to, p.read, p.at)).collect();
            out.push(format!(
                "function {} {} flags {} reads {:?} writes {:?} partials {:?}",
                f.name.as_str(), f.origin.as_str(), f.flags, cells(&f.reads), cells(&f.writes), partials
            ));
        }
        out.push(format!("watches {} reactions {}", shape.watches.len(), shape.reactions.len()));
        out
    }
}

// RS-05 AC 2: every loaded module has a shape equal, entry by entry, to the shape
// `piperine-plan` gives the interpreter for the same body.
#[test]
fn loaded_shapes_equal_the_plans_entry_by_entry() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (_cache, settings) = fresh();
    let ir = all_five();
    let compiled = compile(&ir, &settings);
    let mut checked = 0;
    for body in ir.bodies() {
        let module = Modules::module(&compiled, body.id()).unwrap_or_else(|| panic!("{} has a module", body.name()));
        let plan = BodyPlan::new(body);
        let (want, got) = (Entries::of(plan.shape().shape()), Entries::of(module.prototype()));
        assert!(want.len() > 3, "{}: a shape with entries", body.name());
        assert_eq!(got, want, "{}", body.name());
        checked += 1;
    }
    assert_eq!(checked, 5, "Resistor, Capacitor, Vsource, Isource, Diode");
}

// RS-05 AC 2: a library whose shape differs from the plan's is refused with the body and the
// entry named. The compiler here is a wrapper that renames the pin `p` in the generated source
// before building it, a printing defect the check exists to catch.
#[test]
fn a_shape_that_differs_from_its_plan_is_refused() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let work = tempfile::tempdir().unwrap();
    let wrapper = work.path().join("rustc");
    let script = "#!/bin/sh\nREAL=$(command -v rustc)\n\
        if [ \"$1\" = \"-vV\" ]; then exec \"$REAL\" -vV; fi\n\
        for a in \"$@\"; do shift; case \"$a\" in\n\
          *design.rs) sed 's/PinSpec::continuous(\"p\", /PinSpec::continuous(\"pp\", /' \"$a\" > \"$(dirname \"$a\")/mutated.rs\"\n\
                      set -- \"$@\" \"$(dirname \"$a\")/mutated.rs\";;\n\
          *) set -- \"$@\" \"$a\";; esac; done\nexec \"$REAL\" \"$@\"\n";
    std::fs::write(&wrapper, script).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let settings = Settings { rustc: Some(wrapper), cache: work.path().join("cache") };

    let mut b = IrBuilder::new();
    let body = Devices::resistor(&mut b);
    let top = Devices::alone(&mut b, body, "R", 1, vec![1000.0]);
    let t = Transpiler::new(Arc::new(b.finish(top))).unwrap();
    let err = t.compile(&settings).expect_err("the library's shape differs from the plan's");
    let Error::ShapeMismatch { body, entry } = &err else { panic!("{err:?}") };
    assert_eq!(body, "Resistor");
    assert!(entry.starts_with("pin 1:"), "the entry is named: {entry}");
    assert!(entry.contains("`p`") && entry.contains("`pp`"), "both sides are shown: {entry}");
    assert!(err.to_string().contains("transpiler defect"), "{err}");
}

// RS-05 independent test: the compiled resistor divider runs a `Dc` and `mid` reads the divider
// ratio (10 V across 1 k and 3 k: 7.5 V) to 1e-12 relative, the same value the interpreter gives.
#[test]
fn the_compiled_divider_solves() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (_cache, settings) = fresh();
    let ir = Circuits::divider();
    let compiled = compile(&ir, &settings);
    let mid = NodeIdentifier::Anonymous(ir.node_at("Divider.mid").expect("node mid").id().index());
    let vin = ir.node_at("Divider.vin").expect("node vin");
    assert_eq!(ir.instance_at("Divider.v1").expect("v1").binding(0), Some(NodeRef::Node(vin.id())));

    let mut circuit = compiled.circuit().unwrap_or_else(|e| panic!("circuit: {e}"));
    let v = circuit.dc(Context::default()).expect("dc").solve().expect("solve").get_node(&mid).expect("v(mid)");
    let expected = 10.0 * 3000.0 / (1000.0 + 3000.0);
    assert!((v - expected).abs() <= 1e-12 * expected, "v(mid) = {v}, divider ratio {expected}");

    let mut reference = Interpreter::new(Arc::new(ir)).unwrap().circuit().unwrap();
    let r = reference.dc(Context::default()).expect("dc").solve().expect("solve").get_node(&mid).expect("v(mid)");
    assert!((v - r).abs() <= 1e-12 * r.abs(), "compiled {v} vs interpreted {r}");
}
