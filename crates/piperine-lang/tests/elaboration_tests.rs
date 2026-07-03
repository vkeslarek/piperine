/// Integration tests for the elaboration phase.
///
/// These tests verify that `elaborate()` produces a fully resolved `Design`:
/// - types are concrete (`NetType` / `ValueType`, no free expressions)
/// - port connections are `NetRef` (no raw `Expr`)
/// - for loops are unrolled
/// - bundles are expanded to flat ports
/// - generic modules are monomorphized on demand
/// - stdlib prelude is always in scope
/// - `use` declarations are resolved
/// - function and impl bodies are lowered to `BehaviorStmt`
use piperine_lang::{
    pom::{BehaviorStmt, NetType, ValueType},
    parse_and_elaborate, parse_str,
    resolve::Resolver,
};

// ────────────────────────────── helpers ───────────────────────────────────────

fn elab(src: &str) -> piperine_lang::pom::Design {
    parse_str(src).expect("parse failed").elaborate(&piperine_lang::SourceMap::dummy()).expect("elaborate failed")
}

fn elab_err(src: &str) -> String {
    parse_str(src).expect("parse failed").elaborate(&piperine_lang::SourceMap::dummy())
        .err()
        .expect("expected elaboration error")
        .to_string()
}

// ─────────────────────────────── stdlib prelude ───────────────────────────────

#[test]
fn test_stdlib_capabilities_always_in_scope() {
    // Capabilities from stdlib/capabilities.phdl must be present without any `use`.
    let prog = elab("discipline Electrical { potential v: Real; flow i: Real; }");
    assert!(prog.capability("Add").is_some(), "Add not in prelude");
    assert!(prog.capability("Sub").is_some(), "Sub not in prelude");
    assert!(prog.capability("Mul").is_some(), "Mul not in prelude");
    assert!(prog.capability("Div").is_some(), "Div not in prelude");
    assert!(prog.capability("Eq").is_some(), "Eq not in prelude");
    assert!(prog.capability("Ord").is_some(), "Ord not in prelude");
    assert!(prog.capability("Number").is_some(), "Number not in prelude");
    assert!(prog.capability("Not").is_some(), "Not not in prelude");
    assert!(prog.capability("BitAnd").is_some(), "BitAnd not in prelude");
}

#[test]
fn test_stdlib_map_reduce_always_in_scope() {
    // map and reduce from stdlib/collections.phdl must be present without any `use`.
    let prog = elab("discipline Bit { storage Boolean; }");
    assert!(prog.function("map").is_some(), "map not in prelude");
    assert!(prog.function("reduce").is_some(), "reduce not in prelude");
}

// ─────────────────────────── type resolution ──────────────────────────────────

#[test]
fn test_primitive_value_types_resolved() {
    let prog = elab(
        "discipline Bit { storage Boolean; }
         mod M ( input a : Bit ) { param p : Real = 1.0; param n : Natural = 4; }",
    );
    let m = prog.module("M").expect("M not elaborated");
    let p = m.params.iter().find(|x| x.name == "p").expect("param p");
    let n = m.params.iter().find(|x| x.name == "n").expect("param n");
    assert_eq!(p.ty, ValueType::Real);
    assert_eq!(n.ty, ValueType::Natural);
}

#[test]
fn test_discipline_net_type_resolved() {
    let prog = elab(
        "discipline Electrical { potential v: Real; flow i: Real; }
         mod Res ( inout p : Electrical, inout n : Electrical );",
    );
    let m = prog.module("Res").expect("Res not elaborated");
    assert_eq!(m.ports.len(), 2);
    assert_eq!(m.ports[0].ty, NetType::Discipline("Electrical".into()));
    assert_eq!(m.ports[1].ty, NetType::Discipline("Electrical".into()));
}

#[test]
fn test_array_net_type_resolved() {
    let prog = elab(
        "discipline Bit { storage Boolean; }
         mod Bus ( inout data : Bit[8] );",
    );
    let m = prog.module("Bus").expect("Bus not elaborated");
    assert_eq!(m.ports.len(), 1);
    assert_eq!(
        m.ports[0].ty,
        NetType::Array(Box::new(NetType::Discipline("Bit".into())), 8)
    );
}

#[test]
fn test_undefined_type_error() {
    let err = elab_err("mod M ( inout p : NonExistent );");
    assert!(err.contains("NonExistent"), "error should name the undefined type");
}

// ──────────────────────────── bundle expansion ────────────────────────────────

#[test]
fn test_bundle_expanded_to_flat_ports() {
    let prog = elab(
        "discipline Electrical { potential v: Real; flow i: Real; }
         bundle DiffPair { p : Electrical, n : Electrical }
         mod Amp ( inout inp : DiffPair, inout out : Electrical );",
    );
    let m = prog.module("Amp").expect("Amp not elaborated");
    // inp expands to inp_p and inp_n; out stays as out
    let names: Vec<&str> = m.ports.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["inp_p", "inp_n", "out"]);
    for port in &m.ports {
        assert_eq!(port.ty, NetType::Discipline("Electrical".into()));
    }
}

#[test]
fn test_value_bundle_as_net_type_fails() {
    let err = elab_err(
        "bundle Spec { cutoff : Real = 1.0e3 }
         mod M ( inout s : Spec );",
    );
    assert!(
        err.contains("Spec") || err.contains("net"),
        "error should mention Spec or net capability"
    );
}

// ──────────────────────── structural for unrolling ────────────────────────────

#[test]
fn test_structural_for_unrolled_to_instances() {
    let prog = elab(
        "discipline Electrical { potential v: Real; flow i: Real; }
         mod Resistor ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }
         mod Chain ( inout a : Electrical, inout b : Electrical ) {
             wire node : Electrical[3];
             node[0] = a;
             node[3] = b;
             for i in 0..3 {
                 Resistor( node[i], node[i] ) { .r = 1.0e3 };
             }
         }",
    );
    let m = prog.module("Chain").expect("Chain not elaborated");
    // for 0..3 unrolled → 3 Resistor instances
    assert_eq!(m.instances.len(), 3, "expected 3 unrolled instances");
    // Each module name is just "Resistor" (no const args)
    for inst in &m.instances {
        assert_eq!(inst.module, "Resistor");
    }
}

#[test]
fn test_structural_for_port_connections_are_net_refs() {
    let prog = elab(
        "discipline Electrical { potential v: Real; flow i: Real; }
         mod Resistor ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }
         mod Chain ( inout a : Electrical, inout b : Electrical ) {
             wire node : Electrical[3];
             for i in 0..3 {
                 Resistor( node[i], node[i] );
             }
         }",
    );
    let m = prog.module("Chain").expect("Chain not elaborated");
    // First instance: node[0], node[0]
    let inst0 = &m.instances[0];
    assert_eq!(inst0.ports[0].net, "node");
    assert_eq!(inst0.ports[0].index, Some(0));
}

#[test]
fn test_net_connection_resolved_to_net_ref() {
    let prog = elab(
        "discipline Electrical { potential v: Real; flow i: Real; }
         mod M ( inout a : Electrical, inout b : Electrical ) {
             a = b;
         }",
    );
    let m = prog.module("M").expect("M not elaborated");
    assert_eq!(m.connections.len(), 1);
    assert_eq!(m.connections[0].lhs.net, "a");
    assert_eq!(m.connections[0].rhs.net, "b");
}

// ────────────────────────── generic monomorphization ─────────────────────────

#[test]
fn test_generic_module_monomorphized_on_demand() {
    let prog = elab(
        "discipline Electrical { potential v: Real; flow i: Real; }
         mod Resistor ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }
         mod RcChain[N] ( inout a : Electrical, inout b : Electrical ) {
             wire node : Electrical[N];
             for i in 0..N {
                 Resistor( node[i], node[i] );
             }
         }
         mod Top ( inout a : Electrical, inout b : Electrical ) {
             RcChain[4]( a, b );
         }",
    );
    // RcChain[4] must have been monomorphized and present in modules.
    assert!(
        prog.module("RcChain__4").is_some(),
        "RcChain__4 not in program modules; got: {:?}",
        prog.modules().map(|m| m.name()).collect::<Vec<_>>()
    );
    // Top instances reference the mangled name.
    let top = prog.module("Top").expect("Top not elaborated");
    assert_eq!(top.instances[0].module, "RcChain__4");
}

#[test]
fn test_non_instantiated_generic_not_in_modules() {
    let prog = elab(
        "discipline Bit { storage Boolean; }
         mod Generic[N] ( inout a : Bit );",
    );
    // Generic[N] is declared but never instantiated → should NOT appear in modules.
    let generic_present = prog.modules().any(|m| m.name().starts_with("Generic"));
    assert!(!generic_present, "un-instantiated generic should not be in modules");
}

// ──────────────────────────── behavior elaboration ────────────────────────────

#[test]
fn test_analog_behavior_elaborated() {
    let prog = elab(
        "discipline Electrical { potential v: Real; flow i: Real; }
         mod Res ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }
         analog Res { V(p, n) <+ r * I(p, n); }",
    );
    assert_eq!(prog.module("Res").unwrap().behaviors().len(), 1);
    assert_eq!(prog.module("Res").unwrap().behaviors()[0].name, "Res");
}

#[test]
fn test_digital_behavior_elaborated() {
    let prog = elab(
        "discipline Bit { storage Boolean; }
         mod SrLatch ( input s : Bit, input r : Bit, output q : Bit ) { var st : Bit = 0; }
         digital SrLatch {
             q <- st;
             @ (posedge(s) | posedge(r)) {
                 if (s == 1) { st = 1; } else { st = 0; }
             }
         }",
    );
    assert_eq!(prog.module("SrLatch").unwrap().behaviors().len(), 1);
    let b = &prog.module("SrLatch").unwrap().behaviors()[0];
    assert_eq!(b.body.len(), 2);
}

#[test]
fn test_behavioral_for_unrolled() {
    let prog = elab(
        "discipline Bit { storage Boolean; }
         mod M ( inout a : Bit ) {}
         analog M {
             for i in 0..3 { V(a) <+ 1.0; }
         }",
    );
    let b = &prog.module("M").unwrap().behaviors()[0];
    assert_eq!(b.body.len(), 3);
    for stmt in &b.body {
        assert!(matches!(stmt, BehaviorStmt::Bind { .. }));
    }
}

#[test]
fn test_const_if_folded_in_behavior() {
    let prog = elab(
        "discipline Bit { storage Boolean; }
         mod M ( inout a : Bit ) {}
         analog M {
             if (1 == 1) { V(a) <+ 1.0; } else { V(a) <+ 0.0; }
         }",
    );
    let b = &prog.module("M").unwrap().behaviors()[0];
    assert_eq!(b.body.len(), 1);
    assert!(matches!(b.body[0], BehaviorStmt::Bind { .. }));
}

#[test]
fn test_contrib_in_digital_rejected() {
    let err = elab_err(
        "discipline Bit { storage Boolean; }
         mod M ( inout a : Bit ) {}
         digital M { V(a) <+ 1.0; }",
    );
    assert!(
        err.contains("contribution") || err.contains("digital"),
        "error should mention contribution or digital"
    );
}

// ─────────────────────────── function elaboration ─────────────────────────────

#[test]
fn test_function_body_lowered() {
    let prog = elab(
        "fn double(x: Real) -> Real {
             var y : Real = 0.0;
             return x + x;
         }",
    );
    let f = prog.function("double").expect("double not elaborated");
    // body should have VarDecl + Expr (return value)
    assert!(!f.body.is_empty(), "function body should be non-empty");
    assert!(matches!(f.body[0], BehaviorStmt::VarDecl { .. }));
}

#[test]
fn test_function_param_types_resolved() {
    let prog = elab("fn add(a: Real, b: Real) -> Real { return a + b; }");
    let f = prog.function("add").expect("add not elaborated");
    assert_eq!(f.params.len(), 2);
    assert_eq!(f.ret, piperine_lang::pom::TypeRef::Value(ValueType::Real));
}

// ────────────────────────────── impl elaboration ──────────────────────────────

#[test]
fn test_impl_methods_elaborated() {
    let prog = elab(
        "capability Greet { fn hello(self) -> Boolean; }
         discipline Bit { storage Boolean; }
         mod Widget ( inout a : Bit );
         impl Greet for Widget {
             fn hello(self) -> Boolean { return 1; }
         }",
    );
    assert_eq!(prog.impls().len(), 1);
    let i = &prog.impls()[0];
    assert_eq!(i.capability, Some("Greet".into()));
    assert_eq!(i.ty, "Widget");
    assert_eq!(i.methods.len(), 1);
    // method body should be lowered
    assert!(!i.methods[0].body.is_empty());
}

// ─────────────────────────── use resolution ──────────────────────────────────

#[test]
fn test_use_resolution_file_based() {
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let lib_path = dir.path().join("mylib.phdl");
    std::fs::write(
        &lib_path,
        "discipline MyNet { potential v: Real; flow i: Real; }",
    )
    .unwrap();

    let src = "use mylib;\n mod M ( inout a : MyNet );";
    let source = parse_str(src).expect("parse failed");
    let source_map = piperine_lang::SourceMap::new(dir.path().to_path_buf());
    let mut resolver = Resolver::new(&source_map);
    let prog =
        source.elaborate_with(&mut resolver).expect("elab failed");

    assert!(
        prog.discipline("MyNet").is_some(),
        "MyNet discipline should be resolved from mylib.phdl"
    );
    assert!(prog.module("M").is_some(), "M should be elaborated");
}

#[test]
fn test_use_piperine_capabilities_explicit() {
    // Explicit use of stdlib should work (and not double-inject, just idempotent).
    let prog = elab("use piperine::capabilities; discipline Bit { storage Boolean; }");
    assert!(prog.capability("Add").is_some());
}

#[test]
fn test_use_transitive() {
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    // a.phdl uses b.phdl
    std::fs::write(
        dir.path().join("b.phdl"),
        "discipline NetB { potential v: Real; flow i: Real; }",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.phdl"), "use b;").unwrap();

    let src = "use a;\n mod M ( inout x : NetB );";
    let source = parse_str(src).expect("parse");
    let source_map = piperine_lang::SourceMap::new(dir.path().to_path_buf());
    let mut resolver = Resolver::new(&source_map);
    let prog =
        source.elaborate_with(&mut resolver).expect("elab");

    assert!(prog.discipline("NetB").is_some(), "NetB should be transitively resolved");
}

// ───────────────────────── example file round-trips ──────────────────────────

#[test]
fn test_elab_sr_latch_example() {
    let src = include_str!("examples/sr_latch.phdl");
    // sr_latch uses Bit discipline — provide it inline since there's no `use`.
    let full = format!(
        "discipline Bit {{ storage Boolean; }}\n{}",
        src
    );
    let prog = elab(&full);
    assert!(prog.module("SrLatch").is_some());
    assert_eq!(prog.module("SrLatch").unwrap().behaviors().len(), 1);
    let ports: Vec<&str> = prog.module("SrLatch").unwrap().ports().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(ports, vec!["s", "r", "q"]);
}

#[test]
fn test_parse_and_elaborate_api() {
    let result = parse_and_elaborate(
        "discipline Electrical { potential v: Real; flow i: Real; }
         mod R ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }",
        &piperine_lang::SourceMap::dummy());
    let prog = result.expect("parse_and_elaborate failed");
    assert!(prog.module("R").is_some());
}

#[test]
fn test_global_const_evaluated() {
    let result = parse_and_elaborate(
        "const MY_CONST : Natural = 42; const ANOTHER : Natural = MY_CONST + 1;",
        &piperine_lang::SourceMap::dummy());
    let prog = result.expect("parse_and_elaborate failed");
    assert_eq!(prog.const_("MY_CONST").unwrap().as_natural().unwrap(), 42);
    assert_eq!(prog.const_("ANOTHER").unwrap().as_natural().unwrap(), 43);
}
