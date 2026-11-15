//! Scope: the analog block of `piperine_lang::elab` (ELAB-04): value code and functions lowered
//! to the IR node of the same form (each function once), probes and environment reads, sites
//! numbered in source order, and, with T7, branches and the split of a contribution.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use std::path::PathBuf;

use support::Elab;

/// One function block of a golden text of `piperine-ir`: the `fn` line and its indented lines.
struct Golden;

impl Golden {
    fn function(file: &str, name: &str) -> String {
        let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "piperine-ir", "tests", "golden", file].iter().collect();
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut block = Vec::new();
        for line in text.lines().skip_while(|l| !l.starts_with(&format!("fn {name}("))) {
            if !block.is_empty() && !line.starts_with("  ") {
                break;
            }
            block.push(line);
        }
        assert!(!block.is_empty(), "no `fn {name}` in {file}");
        block.join("\n") + "\n"
    }
}

#[test]
fn value_code_lowers_to_the_same_forms_and_functions_once() {
    // Two bodies call `pnjlim` (std, §22.2): one function, however many callers.
    let src = "
module A() { }
analog for A { let y = pnjlim(1.0, 0.5, 0.026, 0.6); }
module B() { k: f64 = 2.0; }
analog for B { let z = pnjlim(k, 0.5, 0.026, 0.6) + 1.0; let w = if z > 1.0 { z } else { -z }; }
module Top() { let a = A { }(); let b = B { }(); }";
    let ir = Elab::ir(src, "Top");
    assert_eq!(ir.functions().filter(|f| f.name() == "pnjlim").count(), 1, "pnjlim is one FunctionId");
    assert_eq!(ir.functions().count(), 1);

    // The function is the form the hand-built golden of `piperine-ir` pins, line for line:
    // `if`/`else`, blocks with `let`s, `abs`/`ln` as `Math`, unary `-`, every local.
    let text = ir.print_structural();
    let golden = Golden::function("diode.txt", "pnjlim");
    assert!(text.contains(&golden), "the lowered pnjlim differs from the golden:\n{golden}\nin\n{text}");

    // The calls, the `if` expression and the locals of the bodies.
    assert!(text.contains("let y = pnjlim(1.0, 0.5, 0.026, 0.6)"), "{text}");
    assert!(text.contains("let z = (pnjlim(k, 0.5, 0.026, 0.6) + 1.0)"), "{text}");
    assert!(text.contains("let w = if (z > 1.0) { z } else { (-z) }"), "{text}");
}

#[test]
fn a_lowered_expression_carries_its_type_and_the_origin_of_its_construct() {
    let src = "module A() { }\nanalog for A { let y = pnjlim(1.0, 0.5, 0.026, 0.6); }\nmodule Top() { let a = A { }(); }";
    let ir = Elab::ir(src, "Top");
    let body = ir.bodies().find(|b| b.name() == "A").expect("body A");
    let first = body.analog().expect("an analog block")[0];
    let piperine_ir::Stmt::Let { init, .. } = ir.stmt(first) else { panic!("a `let`") };
    let call_col = src.lines().nth(1).unwrap().find("pnjlim").unwrap() as u32 + 1;
    let origin = ir.expr_origin(*init);
    assert_eq!((origin.file.as_str(), origin.line, origin.col), ("top.msl", 2, call_col), "the start of the call");
    assert_eq!(ir.expr_type(*init), &piperine_ir::ValueType::F64);
}
