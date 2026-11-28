//! The compiled design as modules and a circuit (feature `ir-rust`, RS-05 AC 1 and 2, RS-06 AC 6;
//! checks C26, C27, C28, C29, C105): each running body is bound to the module the library gives
//! under its name, as the interpreter binds its own; every loaded prototype equals the plan's
//! shape entry by entry; a library whose shape differs from its plan is refused naming the entry;
//! a panic in compiled code comes back as a fault with its message and the instance answers the
//! next call; and the compiled divider solves to the divider ratio, as the interpreter's does.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};

use piperine_abi::caller::Handle;
use piperine_abi::raw::{Cell, List, Module, Shape, status};
use piperine_interp::Interpreter;
use piperine_ir::{Ir, IrBuilder, NodeRef};
use piperine_plan::{BodyPlan, Modules};
use piperine_solver::prelude::{Context, NodeIdentifier};
use piperine_transpile::{Compiled, Error, Settings, Transpiler};

use piperine_ir_fixtures::devices;
use piperine_ir_fixtures::circuits;

use circuits::HandCircuits;
use devices::Devices;

/// Writing an executable and exec'ing it races with another thread's `fork` holding the write
/// descriptor (ETXTBSY); the tests here take turns.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// A design compiled under fresh settings.
struct CompileTarget;

impl CompileTarget {
    fn compile(ir: &Ir, settings: &Settings) -> Compiled {
        let t = Transpiler::new(Arc::new(ir.clone())).unwrap_or_else(|e| panic!("{e}"));
        t.compile(settings).unwrap_or_else(|e| panic!("{e}"))
    }

    fn fresh() -> (tempfile::TempDir, Settings) {
        let cache = tempfile::tempdir().unwrap();
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        (cache, settings)
    }
}

/// The five M1 bodies in one design.
struct AllFiveDesign;

impl AllFiveDesign {
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
}

/// The elements of an ABI list.
struct LoadedItems;

impl LoadedItems {
    /// A list's elements, read through the safe accessor.
    fn items<T: Copy>(list: &List<'_, T>) -> Vec<T> {
        (0..list.len()).map(|i| *list.get(i).expect("a readable element")).collect()
    }
}

// RS-05 AC 1: a library's modules are read through its symbol and bound by body, like the
// interpreter's: a running body has the module of its name, a body with nothing to run has none.
#[test]
fn modules_come_through_the_symbol_and_bind_like_the_interpreter() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (_cache, settings) = CompileTarget::fresh();
    let ir = HandCircuits::divider();
    let compiled = CompileTarget::compile(&ir, &settings);
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
            LoadedItems::items(&m.prototype().pins).iter().map(|p| (p.name.as_str().to_string(), p.kind, p.role)).collect()
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
        for p in LoadedItems::items(&shape.pins) {
            out.push(format!(
                "pin {} {} kind {} role {} global {} flags {} size {} init {:?} check {} abstol {:?}",
                p.name.as_str(), p.origin.as_str(), p.kind, p.role, p.global, p.flags, p.layout.size,
                p.init.as_slice(), p.check, p.abstol
            ));
        }
        for t in LoadedItems::items(&shape.terms) {
            out.push(format!("term {} {} op {} {} {}", t.name.as_str(), t.origin.as_str(), t.op, t.plus, t.minus));
        }
        for i in LoadedItems::items(&shape.inputs) {
            out.push(format!("input {} {} kind {}", i.name.as_str(), i.origin.as_str(), i.kind));
        }
        for f in LoadedItems::items(&shape.functions) {
            let cells = |l: &List<'_, Cell>| LoadedItems::items(l).iter().map(|c| (c.kind, c.index, c.part)).collect::<Vec<_>>();
            let partials: Vec<_> = LoadedItems::items(&f.partials).iter().map(|p| (p.write, p.to, p.read, p.at)).collect();
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
    let (_cache, settings) = CompileTarget::fresh();
    let ir = AllFiveDesign::all_five();
    let compiled = CompileTarget::compile(&ir, &settings);
    let mut checked = 0;
    for body in ir.bodies() {
        let module = Modules::module(&compiled, body.id()).unwrap_or_else(|| panic!("{} has a module", body.name()));
        let plan = BodyPlan::build(body).expect("the body plans");
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

// RS-06 AC 6: value-level code that panics fails the call with a fault that carries the panic
// message, and the process and the instance stay usable. The compiler here is a wrapper that
// adds `if p > 100.0 { panic!(..) }` to the resistor's generated `terms` before building it: a
// panic in compiled code the transpiler itself never writes.
#[test]
fn a_panic_in_compiled_code_is_a_fault_and_the_instance_survives() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let work = tempfile::tempdir().unwrap();
    let inject = work.path().join("inject.sed");
    std::fs::write(&inject, "s/let p = cx_.f64(0);/let p = cx_.f64(0); if p > 100.0 { panic!(\"boom at {p}\"); }/\n").unwrap();
    let wrapper = work.path().join("rustc");
    let script = format!(
        "#!/bin/sh\nREAL=$(command -v rustc)\n\
        if [ \"$1\" = \"-vV\" ]; then exec \"$REAL\" -vV; fi\n\
        for a in \"$@\"; do shift; case \"$a\" in\n\
          *design.rs) sed -f '{}' \"$a\" > \"$(dirname \"$a\")/mutated.rs\"\n\
                      set -- \"$@\" \"$(dirname \"$a\")/mutated.rs\";;\n\
          *) set -- \"$@\" \"$a\";; esac; done\nexec \"$REAL\" \"$@\"\n",
        inject.display()
    );
    std::fs::write(&wrapper, script).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let settings = Settings { rustc: Some(wrapper), cache: work.path().join("cache") };

    let mut b = IrBuilder::new();
    let body = Devices::resistor(&mut b);
    let top = Devices::alone(&mut b, body, "R", 1, vec![1000.0]);
    let ir = b.finish(top);
    let compiled = CompileTarget::compile(&ir, &settings);
    let module = Modules::module(&compiled, body).expect("the resistor's module");
    let handle = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create: {e}"));
    let k = handle.shape().functions.iter().position(|f| f.name.as_str() == "terms").expect("terms") as u32;
    // terms reads [p, n, r] and writes b0.flow, with two partials
    let run = |p: f64| {
        let input: Vec<[u8; 8]> = [p, 0.0, 1000.0].iter().map(|v| v.to_le_bytes()).collect();
        let reads: Vec<&[u8]> = input.iter().map(|b| b.as_slice()).collect();
        let mut out = [0u8; 8];
        let mut jac = [0.0; 2];
        handle.eval(k, &reads, &mut [&mut out[..]], &[1, 1], &mut jac).map(|()| f64::from_le_bytes(out))
    };

    let fault = run(200.0).expect_err("the injected panic fires above 100 V");
    assert_eq!(fault.status, status::FAULT, "{fault}");
    assert!(fault.message.contains("boom at 200"), "the panic's message is carried: {fault}");
    // the same instance answers the next call, with the resistor's value
    assert_eq!(run(0.5).unwrap_or_else(|e| panic!("the call after the fault: {e}")), 0.5 / 1000.0);
    // and faults again where the code panics, the process still alive
    assert!(run(300.0).is_err_and(|e| e.status == status::FAULT && e.message.contains("boom at 300")));
}

// RS-05 independent test: the compiled resistor divider runs a `Dc` and `mid` reads the divider
// ratio (10 V across 1 k and 3 k: 7.5 V) to 1e-12 relative, the same value the interpreter gives.
#[test]
fn the_compiled_divider_solves() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (_cache, settings) = CompileTarget::fresh();
    let ir = HandCircuits::divider();
    let compiled = CompileTarget::compile(&ir, &settings);
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
