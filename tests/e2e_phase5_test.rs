use std::sync::Arc;

use piperine_circuit::elaboration::ElaborationResult;
use piperine_circuit::registry::HardwareRegistry;
use piperine_parser::parser::parse_with_includes;
use piperine_ngspice::NgspicePlugin;
use piperine_interpreter::Plugin;

fn parse_and_elaborate(src: &str) -> Result<ElaborationResult, String> {
    let full_src = format!("`include \"ngspice.ppr\"\n{}", src);
    let dirs = vec![
        piperine_ngspice::ppr_dir(),
        piperine_parser::bundled_header_dir(),
    ];
    let doc = parse_with_includes(&full_src, &dirs).map_err(|e| format!("{:?}", e))?;
    let mut reg = HardwareRegistry::new();
    let plugin = NgspicePlugin::default();
    plugin.register_hardware(&mut reg);
    piperine_circuit::elaboration::elaborate(&doc, &reg).map_err(|e| format!("{:?}", e))
}

#[test]
fn test_behavioral_bsource() {
    let src = r#"
module tb;
    bsource_v #(.V( V(a)*V(b) )) Bmix(.p(out), .n(gnd));
    bsource_v #(.V( V(in) > 0.0 ? V(in) : 0.0 )) Brect(.p(o), .n(gnd));
    
    real gm_val = 0.5;
    bsource_i #(.I( gm_val * V(g,s) )) Bota(.p(d), .n(s));

    initial begin
    end
endmodule
"#;
    let result = parse_and_elaborate(src).expect("elaborate failed");
    println!("{:#?}", result.spice_lines);
    assert!(result.spice_lines.iter().any(|l| l.starts_with("Bmix out 0 V=") && l.contains("v(a)") && l.contains("v(b)")));
    assert!(result.spice_lines.iter().any(|l| l.starts_with("Brect o 0 V=") && l.contains("v(in)>0.0")));
    assert!(result.spice_lines.iter().any(|l| l.starts_with("Bota d s I=") && l.contains("v(g,s)")));
}

#[test]
fn test_behavioral_nonlinear_eg() {
    let src = r#"
module tb;
    vcvs #(.vol( V(cp,cn) * tanh(V(cp,cn)) )) E1(.p(o), .n(gnd), .cp(a), .cn(b));
    vccs #(.cur( exp(V(cp,cn)/0.026) - 1.0 )) G1(.p(c), .n(e), .cp(b), .cn(e));
    initial begin end
endmodule
"#;
    let result = parse_and_elaborate(src).expect("elaborate failed");
    println!("{:#?}", result.spice_lines);
    assert!(result.spice_lines.iter().any(|l| l.starts_with("E1 o 0 a b VOL={") && l.contains("tanh")));
    assert!(result.spice_lines.iter().any(|l| l.starts_with("G1 c e b e CUR={") && l.contains("exp")));
}

#[test]
fn test_behavioral_nonlinear_passives() {
    let src = r#"
module tb;
    res #(.r_expr( 100.0 * (1.0 + 0.01*(temp - 27.0)) )) Rt(.p(a), .n(b));
    cap #(.q( 1e-12 * V(a,b) )) Cnl(.p(a), .n(b));
    ind #(.flux( 1e-6 * I(Lref) )) L1(.p(a), .n(b));
    initial begin end
endmodule
"#;
    let result = parse_and_elaborate(src).expect("elaborate failed");
    println!("{:#?}", result.spice_lines);
    assert!(result.spice_lines.iter().any(|l| l.starts_with("Rt a b R={") && l.contains("temp") && l.contains("27")));
    assert!(result.spice_lines.iter().any(|l| l.starts_with("Cnl a b Q={") && l.contains("v(a,b)")));
    assert!(result.spice_lines.iter().any(|l| l.starts_with("L1 a b FLUX={") && l.contains("i(Lref)")));
}

#[test]
fn test_behavioral_reject_system_tasks() {
    let src = r#"
module tb;
    bsource_v #(.V( $sin(V(a)) )) B1(.p(out), .n(gnd));
    initial begin end
endmodule
"#;
    let err = match parse_and_elaborate(src) {
        Err(e) => e,
        Ok(_) => panic!("Expected elaboration to fail"),
    };
    assert!(err.contains("system tasks cannot appear in a behavioral expression"));
}

#[test]
fn test_behavioral_function_inlining() {
    let src = r#"
module tb;
    function real my_sigmoid;
        input x;
        real x;
        begin
            return 1.0 / (1.0 + exp(-x));
        end
    endfunction

    bsource_v #(.V( my_sigmoid(V(a)) )) B1(.p(out), .n(gnd));
    initial begin end
endmodule
"#;
    let result = parse_and_elaborate(src).expect("elaborate failed");
    println!("{:#?}", result.spice_lines);
    assert!(result.spice_lines.iter().any(|l| l.starts_with("B1 out 0 V=") && l.contains("exp") && l.contains("v(a)")));
}
