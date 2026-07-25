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

// ──────────────────── T16/LSP-18: error-accumulating elaboration ──────────────

/// Two independent modules, each with its own undefined-port-type error
/// (the `ElabModules` pass — module elaboration is independent per
/// module), must *both* appear in `elaborate_with_context_accumulating`'s
/// returned `Vec<ElabError>`, not just the first.
#[test]
fn accumulating_elaboration_reports_two_independent_module_errors() {
    let src = "mod M1 ( inout p : NonExistentOne );\nmod M2 ( inout p : NonExistentTwo );\n";
    let source_file = parse_str(src).expect("parse failed");
    let (_, _, errors) = source_file.elaborate_with_context_accumulating(&piperine_lang::SourceMap::dummy());

    assert_eq!(errors.len(), 2, "both independent module errors must be reported, got: {errors:?}");
    let combined = errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(" | ");
    assert!(combined.contains("NonExistentOne"), "M1's error must be present: {combined}");
    assert!(combined.contains("NonExistentTwo"), "M2's error must be present: {combined}");
}

/// A clean elaboration returns an empty error list from the accumulating
/// entry point (no regression: the same source that `elaborate` accepts
/// must still be accepted here).
#[test]
fn accumulating_elaboration_returns_no_errors_on_success() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\nmod M ( inout p : Electrical, inout n: Electrical ) {}\n";
    let source_file = parse_str(src).expect("parse failed");
    let (design, _, errors) = source_file.elaborate_with_context_accumulating(&piperine_lang::SourceMap::dummy());

    assert!(errors.is_empty(), "a cleanly-elaborating source must report no errors, got: {errors:?}");
    assert!(design.module("M").is_some(), "the design must still be fully built on success");
}

/// A failure in a genuinely order-dependent, fail-fast pass (here:
/// `FoldGlobals` — an unresolvable global const, which every later pass
/// depends on) still stops the pipeline with exactly one error, not a
/// spurious accumulation — the accumulating driver only keeps going past
/// a pass that recorded into `accumulated_errors` itself.
#[test]
fn accumulating_elaboration_still_stops_at_a_fail_fast_pass() {
    let src = "const A : Natural = B; const B : Natural = A;\n";
    let source_file = parse_str(src).expect("parse failed");
    let (_, _, errors) = source_file.elaborate_with_context_accumulating(&piperine_lang::SourceMap::dummy());

    assert_eq!(errors.len(), 1, "a fail-fast precondition pass must not multiply into several errors: {errors:?}");
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
    // Module body uses simple nets (a, b) rather than array wires — the
    // flatten pass defers array-net expansion as gap-3 (see
    // `.specs/features/hierarchy-flattening/spec.md`); this test targets
    // monomorphization, not array-net support.
    let prog = elab(
        "discipline Electrical { potential v: Real; flow i: Real; }
         mod Resistor ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }
         mod RcChain[N] ( inout a : Electrical, inout b : Electrical ) {
             for i in 0..N {
                 Resistor( a, b );
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

#[test]
fn test_two_monomorphs_of_same_base_both_exist() {
    // Regression: `monomorphize` used to drain the mono cache before each
    // insert, so only the last monomorph of a base survived. Two distinct
    // const args must produce two distinct, coexisting monomorphs.
    // Module body avoids array-nets (flatten defers them as gap-3).
    let prog = elab(
        "discipline Electrical { potential v: Real; flow i: Real; }
         mod Resistor ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }
         mod Chain[N] ( inout a : Electrical, inout b : Electrical ) {
             for i in 0..N { Resistor( a, b ); }
         }
         mod Top ( inout a : Electrical, inout b : Electrical ) {
             Chain[4]( a, b );
             Chain[8]( a, b );
         }",
    );
    assert!(prog.module("Chain__4").is_some(), "Chain__4 should exist");
    assert!(prog.module("Chain__8").is_some(), "Chain__8 should coexist with Chain__4");
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
        "pub discipline MyNet { potential v: Real; flow i: Real; }",
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
fn test_prelude_items_map_to_real_header_paths() {
    // LSB-01..03 (T1): every prelude item, including `ddt` (operators.phdl)
    // and `Real` (types.phdl), must map to the real on-disk header file —
    // the file goto-definition will eventually open.
    let source_map = piperine_lang::SourceMap::dummy();
    let mut resolver = Resolver::new(&source_map);
    let _ = resolver.prelude_items();
    let item_files = resolver.take_item_files();

    let ddt_path = item_files.get("ddt").expect("ddt should have a tracked file");
    assert!(
        ddt_path.ends_with("headers/operators.phdl"),
        "ddt should map to headers/operators.phdl, got {}",
        ddt_path.display()
    );
    assert!(ddt_path.is_file(), "{} must exist on disk", ddt_path.display());

    let real_path = item_files.get("Real").expect("Real should have a tracked file");
    assert!(
        real_path.ends_with("headers/types.phdl"),
        "Real should map to headers/types.phdl, got {}",
        real_path.display()
    );
    assert!(real_path.is_file(), "{} must exist on disk", real_path.display());
}

#[test]
fn test_use_loaded_item_maps_to_real_on_disk_path() {
    // LSB-01..03 (T1): items loaded via `use` must map to the real file
    // they were read from (not a hardcoded/embedded path).
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let lib_path = dir.path().join("mylib.phdl");
    std::fs::write(
        &lib_path,
        "pub discipline MyNet { potential v: Real; flow i: Real; }",
    )
    .unwrap();

    let src = "use mylib;\n mod M ( inout a : MyNet );";
    let source = parse_str(src).expect("parse failed");
    let source_map = piperine_lang::SourceMap::new(dir.path().to_path_buf());
    let mut resolver = Resolver::new(&source_map);
    let _ = resolver.prelude_items();
    let _ = resolver.expand(source).expect("expand failed");
    let item_files = resolver.take_item_files();

    let mynet_path = item_files
        .get("MyNet")
        .expect("MyNet should have a tracked file");
    assert_eq!(
        std::fs::canonicalize(mynet_path).unwrap(),
        std::fs::canonicalize(&lib_path).unwrap(),
        "MyNet should map to the real mylib.phdl it was declared in"
    );
}

#[test]
fn test_design_project_item_file_returns_real_header_path() {
    // LSB-01..03 (T2): item_files threaded through elaboration onto
    // Design::project() the same way origins already is.
    let prog = elab("mod M ();");
    let ddt_path = prog
        .project()
        .item_file("ddt")
        .expect("ddt should be tracked on the elaborated design's project");
    assert!(
        ddt_path.ends_with("headers/operators.phdl"),
        "ddt should map to headers/operators.phdl, got {}",
        ddt_path.display()
    );
    assert!(ddt_path.is_file(), "{} must exist on disk", ddt_path.display());
}

#[test]
fn test_ddt_doc_comes_from_the_real_header_content() {
    // LSB-04..06 (T6): `headers/operators.phdl`'s `//` prose directly above
    // `extern operator ddt` was rewritten as `///` so the doc-comment
    // pipeline (T4/T5) has real content to surface on hover, not just a
    // synthetic fixture. Elaborate a real analog body using `ddt`, then
    // confirm the registered `ddt` operator's `.doc()` returns the header's
    // actual authored text (proving the elaborated result reads the real
    // on-disk header, not a hardcoded string in the test).
    let src = "discipline Electrical { potential v: Real; flow i: Real; }
               mod Top ( inout p : Electrical ) { }
               analog Top {
                   I(p) <+ ddt(1.0);
               }";
    let source_file = parse_str(src).expect("parse failed");
    let (_design, ctx) = source_file
        .elaborate_with_context(&piperine_lang::SourceMap::dummy())
        .expect("elaborate ddt-using module against stdlib prelude");

    let ddt = ctx.operators.lookup("ddt").expect("ddt should be registered");
    let doc = ddt.doc().expect("ddt should have a /// doc after T6's header edit");

    let header_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/headers/operators.phdl"
    ))
    .expect("headers/operators.phdl must exist on disk");
    assert!(
        header_text.contains("ddt(qtotal)"),
        "sanity: header should still contain the authored prose this test checks for"
    );
    assert!(
        doc.contains("ddt(qtotal)"),
        "ddt's doc should contain the real header prose, got: {doc:?}"
    );
}

#[test]
fn test_use_transitive() {
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    // a.phdl uses b.phdl
    std::fs::write(
        dir.path().join("b.phdl"),
        "pub discipline NetB { potential v: Real; flow i: Real; }",
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

// ──────────────────────────── pub visibility ────────────────────────────────

#[test]
fn test_pub_item_accessible_from_other_package() {
    use std::path::PathBuf;
    // Set up a temp package with public exports.
    let tmp = std::env::temp_dir().join("piperine_pubtest_pub");
    let lib_dir = tmp.join("lib");
    std::fs::create_dir_all(&lib_dir).unwrap();
    std::fs::write(lib_dir.join("devices.phdl"), "
        pub discipline Electrical { potential v: Real; flow i: Real; }
        pub mod Resistor ( inout p : Electrical, inout n : Electrical ) {
            param r : Real = 1e3;
        }
        analog Resistor { I(p, n) <+ V(p, n) / r; }
    ").unwrap();
    std::fs::write(tmp.join("top.phdl"), "
        use lib::devices;
        mod Top ( inout a : Electrical, inout b : Electrical ) {
            r1 : Resistor ( .p = a, .n = b );
        }
    ").unwrap();

    let mut sm = piperine_lang::SourceMap::new(tmp.clone());
    sm.add_namespace("piperine", PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("headers"));
    sm.add_namespace("lib", lib_dir);

    let src = std::fs::read_to_string(tmp.join("top.phdl")).unwrap();
    let result = parse_and_elaborate(&src, &sm);
    let prog = result.expect("pub items should be accessible");
    assert!(prog.module("Top").is_some());
    assert!(prog.module("Resistor").is_some());
}

#[test]
fn test_private_item_not_accessible_from_other_package() {
    use std::path::PathBuf;
    let tmp = std::env::temp_dir().join("piperine_pubtest_priv");
    let lib_dir = tmp.join("lib");
    std::fs::create_dir_all(&lib_dir).unwrap();
    std::fs::write(lib_dir.join("devices.phdl"), "
        pub discipline Electrical { potential v: Real; flow i: Real; }
        mod SecretHelper ( inout p : Electrical, inout n : Electrical ) {
            param gain : Real = 2.0;
        }
        analog SecretHelper { I(p, n) <+ gain * V(p, n); }
    ").unwrap();
    std::fs::write(tmp.join("top.phdl"), "
        use lib::devices;
        mod Top ( inout a : Electrical, inout b : Electrical ) {
            r1 : SecretHelper ( .p = a, .n = b );
        }
    ").unwrap();

    let mut sm = piperine_lang::SourceMap::new(tmp.clone());
    sm.add_namespace("piperine", PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("headers"));
    sm.add_namespace("lib", lib_dir);

    let src = std::fs::read_to_string(tmp.join("top.phdl")).unwrap();
    let result = parse_and_elaborate(&src, &sm);
    // SecretHelper is private — should not be accessible from top.phdl.
    assert!(result.is_err(), "private module should not be accessible");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("SecretHelper") || err.contains("Undefined") || err.contains("not found"),
        "expected an error about SecretHelper, got: {err}"
    );
}

#[test]
fn test_private_item_accessible_within_same_file() {
    // Within the same file, both pub and non-pub items are accessible.
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod Helper ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1e3; }
        analog Helper { I(p, n) <+ V(p, n) / r; }
        mod Top ( inout a : Electrical, inout b : Electrical ) {
            r1 : Helper ( .p = a, .n = b );
        }
    ";
    let prog = elab(src);
    assert!(prog.module("Helper").is_some());
    assert!(prog.module("Top").is_some());
}

// ──────────────────────────── attribute schemas ─────────────────────────────

#[test]
fn test_registered_schema_attribute_passes() {
    let src = "
        @attribute(schema = \"layout\")
        bundle Layout { min_width : Real = 0.0, layer : String }
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) {
            @layout(layer = \"m3\") wire clk : Electrical;
        }
    ";
    let prog = elab(src);
    let m = prog.module("M").expect("M not found");
    let clk = m.wires().iter().find(|w| w.name == "clk").expect("clk wire not found");
    assert_eq!(clk.attributes().len(), 1);
    assert_eq!(clk.attributes()[0].schema(), "layout");
    assert_eq!(clk.attributes()[0].field("layer"), Some(&piperine_lang::value::Value::Str("m3".into())));
    assert_eq!(clk.attributes()[0].field("min_width"), Some(&piperine_lang::value::Value::Real(0.0)));
}

#[test]
fn test_unknown_schema_attribute_fails() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) {
            @UnknownSchema(foo = 1) wire clk : Electrical;
        }
    ";
    let err = elab_err(src);
    assert!(err.contains("UnknownSchema"), "expected unknown schema error, got: {err}");
}

#[test]
fn test_schema_missing_required_field_fails() {
    let src = "
        @attribute(schema = \"layout\")
        bundle Layout { min_width : Real, layer : String }
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) {
            @layout(layer = \"m3\") wire clk : Electrical;
        }
    ";
    let err = elab_err(src);
    assert!(err.contains("min_width"), "expected missing field error, got: {err}");
    assert!(err.contains("required"), "got: {err}");
}

#[test]
fn test_schema_unknown_field_fails() {
    let src = "
        @attribute(schema = \"layout\")
        bundle Layout { layer : String }
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) {
            @layout(layer = \"m3\", bogus = 42) wire clk : Electrical;
        }
    ";
    let err = elab_err(src);
    assert!(err.contains("bogus"), "expected unknown field error, got: {err}");
}

#[test]
fn test_schema_attribute_on_module() {
    let src = "
        @attribute(schema = \"fp\")
        bundle Floorplan { x : Real = 0.0, y : Real = 0.0 }
        discipline Electrical { potential v: Real; flow i: Real; }
        @fp(x = 10.0, y = 20.0)
        mod M ( inout p : Electrical ) { }
    ";
    let prog = elab(src);
    let m = prog.module("M").expect("M not found");
    assert_eq!(m.attributes().len(), 1);
    assert_eq!(m.attributes()[0].schema(), "fp");
    assert_eq!(m.attributes()[0].field("x"), Some(&piperine_lang::value::Value::Real(10.0)));
    assert_eq!(m.attributes()[0].field("y"), Some(&piperine_lang::value::Value::Real(20.0)));
}

// ──────────────────────────── match exhaustiveness ───────────────────────────

#[test]
fn test_exhaustive_enum_match_passes() {
    let src = "
        enum S { A, B, C }
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) { var x : S = A; }
        digital M {
            match x {
                A => { }
                B => { }
                C => { }
            }
        }
    ";
    let prog = elab(src);
    assert!(prog.module("M").is_some());
}

#[test]
fn test_exhaustive_enum_match_with_wildcard_passes() {
    let src = "
        enum S { A, B, C }
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) { var x : S = A; }
        digital M {
            match x {
                A => { }
                _ => { }
            }
        }
    ";
    let prog = elab(src);
    assert!(prog.module("M").is_some());
}

#[test]
fn test_non_exhaustive_enum_match_fails() {
    let src = "
        enum S { A, B, C }
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) { var x : S = A; }
        digital M {
            match x {
                A => { }
                B => { }
            }
        }
    ";
    let err = elab_err(src);
    assert!(err.contains("non-exhaustive"), "expected non-exhaustive error, got: {err}");
    assert!(err.contains("C"), "error should name the missing variant C, got: {err}");
}

#[test]
fn test_non_exhaustive_enum_match_multiple_missing() {
    let src = "
        enum S { A, B, C, D }
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) { var x : S = A; }
        digital M {
            match x {
                A => { }
            }
        }
    ";
    let err = elab_err(src);
    assert!(err.contains("non-exhaustive"), "got: {err}");
    assert!(err.contains("B"), "missing B: {err}");
    assert!(err.contains("C"), "missing C: {err}");
    assert!(err.contains("D"), "missing D: {err}");
}

#[test]
fn test_bit_pattern_match_without_wildcard_fails() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) { var x : Integer = 0; }
        digital M {
            match x {
                0 => { }
                1 => { }
            }
        }
    ";
    let err = elab_err(src);
    assert!(err.contains("non-exhaustive"), "literal without wildcard should fail: {err}");
    assert!(err.contains("wildcard"), "should mention wildcard requirement: {err}");
}

#[test]
fn test_bit_pattern_match_with_wildcard_passes() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) { var x : Integer = 0; }
        digital M {
            match x {
                1 => { }
                _ => { }
            }
        }
    ";
    let prog = elab(src);
    assert!(prog.module("M").is_some());
}

// ────────────────────── hierarchy flattening (FLAT-01) ───────────────────────

/// After `parse_and_elaborate`, every module has a flat form in
/// `flat_modules` — the `FlattenHierarchy` pass runs as the last pass and
/// writes only to that side map. The authored `modules` map is the source
/// of truth; `flat_modules` is the codegen-facing artifact.
#[test]
fn flatten_pass_populates_flat_modules_after_elaboration() {
    let prog = elab(
        "discipline Electrical { potential v: Real; flow i: Real; }
         mod Resistor ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }
         mod Seg ( inout p : Electrical, inout n : Electrical ) {
             wire mid : Electrical;
             r1 : Resistor( .p = p, .n = mid );
             r2 : Resistor( .p = mid, .n = n );
         }
         mod Top ( inout a : Electrical, inout b : Electrical ) {
             x : Seg( .p = a, .n = b );
         }",
    );
    // Every authored module has a flat form recorded.
    assert!(prog.flat_module("Resistor").is_some(), "Resistor flattened");
    assert!(prog.flat_module("Seg").is_some(), "Seg flattened");
    assert!(prog.flat_module("Top").is_some(), "Top flattened");

    // Seg is non-leaf (two Resistor instances); its flat form keeps them.
    let seg_flat = prog.flat_module("Seg").expect("Seg flat form");
    assert_eq!(seg_flat.instances.len(), 2, "Seg flat form has both leaves");
    assert!(seg_flat.instances.iter().all(|i| i.module == "Resistor"));

    // Top's flat form has the inlined leaves (x.r1, x.r2) — Seg is gone.
    let top_flat = prog.flat_module("Top").expect("Top flat form");
    let labels: Vec<&str> = top_flat.instances.iter().map(|i| i.label.as_deref().unwrap()).collect();
    assert_eq!(labels, vec!["x.r1", "x.r2"], "Seg inlined into Top with prefixed labels");
    assert!(top_flat.wires.iter().any(|w| w.name == "x.mid"), "Seg's wire lifted to x.mid");
}

/// The non-destructive invariant: flattening never mutates `Design::modules`.
/// Authored hierarchy deep-equals before and after the pass (which runs as
/// part of `elaborate`). POM navigability mirrors the source.
#[test]
fn flatten_pass_leaves_authored_modules_untouched() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }
         mod Resistor ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }
         mod Seg ( inout p : Electrical, inout n : Electrical ) {
             wire mid : Electrical;
             r1 : Resistor( .p = p, .n = mid );
             r2 : Resistor( .p = mid, .n = n );
         }
         mod Top ( inout a : Electrical, inout b : Electrical ) {
             x : Seg( .p = a, .n = b );
         }";

    let before = elab(src);
    // Snapshot the authored hierarchy through the public reflection API —
    // `modules` itself is `pub(crate)`, so we compare module-by-module.
    // Sort by name: HashMap iteration order is not stable across runs.
    let mut snapshot: Vec<String> = before.modules().map(|m| format!("{m:?}")).collect();
    snapshot.sort();
    let module_count = before.module_count();

    // Re-elaborate and compare — the authored map is identical.
    let after = elab(src);
    assert_eq!(after.module_count(), module_count, "module count unchanged");
    let mut after_snapshot: Vec<String> = after.modules().map(|m| format!("{m:?}")).collect();
    after_snapshot.sort();
    assert_eq!(
        after_snapshot, snapshot,
        "Design::modules deep-equal before/after flatten (non-destructive)"
    );

    // And Top still has Seg as a direct instance (authored form preserved).
    let top = after.module("Top").expect("Top authored");
    assert_eq!(top.instances.len(), 1, "authored Top still has one instance (Seg)");
    assert_eq!(top.instances[0].module, "Seg", "authored Top instance is Seg, not a spliced leaf");
}

// ─────────────────────── `///` doc-comment attach (LSP-07/09) ─────────────────

#[test]
fn test_module_doc_attaches_from_triple_slash_run() {
    let src = "
        /// A two-terminal resistor.
        mod Resistor(inout p: Electrical, inout n: Electrical) { param r: Real = 1.0; }
        discipline Electrical { potential v: Real; flow i: Real; }
    ";
    let design = elab(src);
    let m = design.module("Resistor").expect("Resistor exists");
    assert_eq!(m.doc.as_deref(), Some("A two-terminal resistor."));
}

#[test]
fn test_module_without_doc_comment_is_none_no_regression() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod Resistor(inout p: Electrical, inout n: Electrical) { param r: Real = 1.0; }
    ";
    let design = elab(src);
    let m = design.module("Resistor").expect("Resistor exists");
    assert_eq!(m.doc, None);
}

#[test]
fn test_param_wire_var_instance_and_behavior_docs_attach() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod Leaf(inout p: Electrical, inout n: Electrical) {
            /// The resistance in ohms.
            param r: Real = 1.0;
        }
        analog Leaf { I(p, n) <+ V(p, n) / r; }
        mod Top(inout a: Electrical, inout b: Electrical) {
            /// An internal net.
            wire mid: Electrical;
            /// Persistent state.
            var st: Real = 0.0;
            /// The first leg.
            leg1 : Leaf(.p = a, .n = mid) { .r = 1e3 };
        }
        /// The composite behavior.
        analog Top {}
    ";
    let design = elab(src);
    let leaf = design.module("Leaf").expect("Leaf exists");
    let r = leaf.param("r").expect("param r exists");
    assert_eq!(r.doc.as_deref(), Some("The resistance in ohms."));

    let top = design.module("Top").expect("Top exists");
    let mid = top.wire("mid").expect("wire mid exists");
    assert_eq!(mid.doc.as_deref(), Some("An internal net."));

    let st = top.vars().iter().find(|v| v.name == "st").expect("var st exists");
    assert_eq!(st.doc.as_deref(), Some("Persistent state."));

    let leg1 = top.instance("leg1").expect("instance leg1 exists");
    assert_eq!(leg1.doc.as_deref(), Some("The first leg."));

    let behavior = top.behaviors().iter().find(|b| b.is_analog()).expect("analog behavior exists");
    assert_eq!(behavior.doc.as_deref(), Some("The composite behavior."));
}

#[test]
fn test_doc_run_not_immediately_before_a_decl_is_ignored_no_crash_or_misattach() {
    // A `///` run followed by a blank line, then the declaration: per the
    // lexer's attach rule (T1), the run does not reach the declaration —
    // elaboration must not crash and must not misattach it.
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        /// stale, not adjacent

        mod Resistor(inout p: Electrical, inout n: Electrical) { param r: Real = 1.0; }
    ";
    let design = elab(src);
    let m = design.module("Resistor").expect("Resistor elaborates fine despite the dangling run");
    assert_eq!(m.doc, None);
}

// ─────────────────────── `ResolutionIndex` (LSP-03/05) ────────────────────────

use piperine_lang::{BindingKind, ResolutionIndex};

const RESOLUTION_SRC: &str = "
    discipline Electrical { potential v: Real; flow i: Real; }
    /// A two-terminal resistor.
    mod Resistor(inout p: Electrical, inout n: Electrical) {
        /// The resistance in ohms.
        param r: Real = 1.0;
    }
    analog Resistor { I(p, n) <+ V(p, n) / r; }
    mod Top(inout a: Electrical, inout b: Electrical) {
        wire mid: Electrical;
        r1 : Resistor(.p = a, .n = mid) { .r = 1e3 };
    }
";

fn elab_with_index(src: &str) -> (piperine_lang::pom::Design, ResolutionIndex) {
    parse_str(src)
        .expect("parse failed")
        .elaborate_with_index(&piperine_lang::SourceMap::dummy())
        .expect("elaborate_with_index failed")
}

#[test]
fn test_elaborate_with_index_design_matches_plain_elaborate() {
    // Additive: the Design half of elaborate_with_index must be identical
    // to what plain elaborate() produces for the same source.
    let plain = elab(RESOLUTION_SRC);
    let (indexed, _idx) = elab_with_index(RESOLUTION_SRC);
    assert_eq!(indexed.module_count(), plain.module_count());
    let mut plain_snapshot: Vec<String> = plain.modules().map(|m| format!("{m:?}")).collect();
    let mut indexed_snapshot: Vec<String> = indexed.modules().map(|m| format!("{m:?}")).collect();
    plain_snapshot.sort();
    indexed_snapshot.sort();
    assert_eq!(indexed_snapshot, plain_snapshot, "Design unchanged by elaborate_with_index");
}

#[test]
fn test_resolution_index_records_decl_span_kind_doc_and_use_span() {
    let (design, idx) = elab_with_index(RESOLUTION_SRC);
    let resistor = design.module("Resistor").expect("Resistor exists");
    let param_r = resistor.param("r").expect("param r exists");
    let r_span = param_r.span.expect("param r has a span");

    // The cursor landing anywhere inside the param's own decl span resolves
    // to a binding carrying the right kind/name/doc/decl_span.
    let offset = r_span.offset() + 1;
    let id = idx.resolve_at(offset).expect("resolves at param r's decl span");
    let info = idx.binding(id).expect("binding info present");
    assert!(matches!(info.kind, BindingKind::Param));
    assert_eq!(info.name, "r");
    assert_eq!(info.doc.as_deref(), Some("The resistance in ohms."));
    assert_eq!(info.decl_span.offset(), r_span.offset());
    assert_eq!(info.decl_span.len(), r_span.len());
}

#[test]
fn test_resolution_index_decl_and_use_share_one_binding_id() {
    // LSP-03: the declaration span and its own (reflexive) use span must
    // resolve to the *same* BindingId — occurrences() returns that span.
    let (design, idx) = elab_with_index(RESOLUTION_SRC);
    let resistor = design.module("Resistor").expect("Resistor exists");
    let param_r = resistor.param("r").expect("param r exists");
    let r_span = param_r.span.expect("param r has a span");

    let decl_id = idx.resolve_at(r_span.offset()).expect("resolve at decl span start");
    let occ = idx.occurrences(decl_id);
    assert_eq!(occ.len(), 1, "decl site is recorded as a use of itself");
    assert_eq!(occ[0].offset(), r_span.offset());
}

#[test]
fn test_resolution_index_covers_module_port_param_wire_instance_behavior() {
    let (_design, idx) = elab_with_index(RESOLUTION_SRC);
    let kinds: std::collections::HashSet<_> = idx
        .bindings()
        .map(|(_, info)| std::mem::discriminant(&info.kind))
        .collect();
    for expected in [
        BindingKind::Module,
        BindingKind::Port,
        BindingKind::Param,
        BindingKind::Wire,
        BindingKind::Instance,
        BindingKind::Behavior,
    ] {
        assert!(
            kinds.contains(&std::mem::discriminant(&expected)),
            "ResolutionIndex missing a binding of kind {expected:?}"
        );
    }
}

#[test]
fn test_resolution_index_module_doc_carried_on_binding() {
    let (design, idx) = elab_with_index(RESOLUTION_SRC);
    let resistor = design.module("Resistor").expect("Resistor exists");
    let m_span = resistor.span.expect("module has a span");
    let id = idx.resolve_at(m_span.offset()).expect("resolves at module decl span");
    let info = idx.binding(id).expect("binding present");
    assert!(matches!(info.kind, BindingKind::Module));
    assert_eq!(info.doc.as_deref(), Some("A two-terminal resistor."));
}

// ─────────────────────── instance token-level spans (LSB-07..10, T7) ──────────

/// A labeled instance's `label_span` covers exactly the label token's bytes
/// and `type_span` covers exactly the type-name token's bytes — not the
/// whole 56-byte multi-line statement. Fixture mirrors spec.md's exact
/// reported repro shape.
#[test]
fn test_labeled_instance_label_and_type_spans_are_token_tight() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
               mod RampSource ( inout p : Electrical, inout n : Electrical ) { param slope: Real = 0.0; }\n\
               mod Top ( inout vin : Electrical, inout gnd : Electrical ) {\n\
                   src : RampSource(.p = vin, .n = gnd) { .slope = 4.0e5 };\n\
               }";
    let design = elab(src);
    let top = design.module("Top").expect("Top exists");
    let inst = top.instance("src").expect("instance src exists");

    let label_span = inst.label_span.expect("labeled instance has a label_span");
    assert_eq!(label_span.len(), 3, "label_span should cover only `src` (3 bytes)");
    assert_eq!(&src[label_span.offset()..label_span.offset() + label_span.len()], "src");

    let type_span = inst.type_span.expect("labeled instance has a type_span");
    assert_eq!(type_span.len(), 10, "type_span should cover only `RampSource` (10 bytes)");
    assert_eq!(
        &src[type_span.offset()..type_span.offset() + type_span.len()],
        "RampSource"
    );
}

/// An unlabeled instance has no `label_span`, and its `type_span` is tight
/// to the single identifier token (not the whole statement).
#[test]
fn test_unlabeled_instance_has_no_label_span_and_tight_type_span() {
    let src = "discipline Electrical { potential v: Real; flow i: Real; }\n\
               mod RampSource ( inout p : Electrical, inout n : Electrical ) { param slope: Real = 0.0; }\n\
               mod Top ( inout vin : Electrical, inout gnd : Electrical ) {\n\
                   RampSource(.p = vin, .n = gnd);\n\
               }";
    let design = elab(src);
    let top = design.module("Top").expect("Top exists");
    let inst = top.instances().first().expect("Top has exactly one instance");
    assert_eq!(inst.label, None, "instance should be unlabeled");

    assert_eq!(inst.label_span, None, "unlabeled instance must have no label_span");

    let type_span = inst.type_span.expect("unlabeled instance still has a type_span");
    assert_eq!(type_span.len(), 10, "type_span should cover only `RampSource` (10 bytes)");
    assert_eq!(
        &src[type_span.offset()..type_span.offset() + type_span.len()],
        "RampSource"
    );
}
