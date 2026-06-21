
//! Component integration tests.
//! Parses Verilog-AMS containing component instances, elaborates them, and asserts the correct SPICE string.

use piperine_circuit::elaboration::elaborate;
use piperine_circuit::HardwareRegistry;
use piperine_parser::parser::parse_with_includes;
use piperine_interpreter::Plugin;
use piperine_ngspice::NgspicePlugin;

fn test_component(src: &str, expected_spice: &str) {
    let full_src = format!("`include \"ngspice.ppr\"\n{}", src);
    let dirs = vec![
        piperine_ngspice::ppr_dir(),
        piperine_parser::bundled_header_dir(),
    ];
    let doc = parse_with_includes(&full_src, &dirs).unwrap();
    let mut registry = HardwareRegistry::new();
    let plugin = NgspicePlugin::default();
    plugin.register_hardware(&mut registry);
    let result = elaborate(&doc, &registry).unwrap();
    
    let lines: Vec<String> = result.spice_lines.into_iter().filter(|l| !l.starts_with(".subckt") && !l.starts_with(".ends") && !l.is_empty()).collect();
    let mut found = false;
    for line in &lines {
        if line == expected_spice {
            found = true;
            break;
        }
    }
    assert!(found, "Expected SPICE line `{}` not found in generated lines: {:?}", expected_spice, lines);
}

#[test]
fn test_vpulse() {
    test_component(r#"
module tb;
    vpulse #(.v0(0.0), .v1(1.0), .td(0.0), .tr(1.0), .tf(2.0), .pw(3.0), .per(4.0)) inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "Vinst p n PULSE(0 1 0 1 2 3 4)");
}

#[test]
fn test_ipulse() {
    test_component(r#"
module tb;
    ipulse #(.i0(0.0), .i1(1.0), .td(0.0), .tr(1.0), .tf(2.0), .pw(3.0), .per(4.0)) inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "inst p n PULSE(0 1 0 1 2 3 4)");
}

#[test]
fn test_vsin() {
    test_component(r#"
module tb;
    vsin #(.vo(0.0), .va(1.0), .freq(100.0), .td(0.0), .theta(0.0), .phi(0.0)) inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "Vinst p n SIN(0 1 100 0 0 0)");
}

#[test]
fn test_isin() {
    test_component(r#"
module tb;
    isin #(.io(0.0), .ia(1.0), .freq(100.0), .td(0.0), .theta(0.0), .phi(0.0)) inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "inst p n SIN(0 1 100 0 0 0)");
}

#[test]
fn test_vexp() {
    test_component(r#"
module tb;
    vexp #(.v1(0.0), .v2(1.0), .td1(0.0), .tau1(1.0), .td2(2.0), .tau2(3.0)) inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "Vinst p n EXP(0 1 0 1 2 3)");
}

#[test]
fn test_iexp() {
    test_component(r#"
module tb;
    iexp #(.i1(0.0), .i2(1.0), .td1(0.0), .tau1(1.0), .td2(2.0), .tau2(3.0)) inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "inst p n EXP(0 1 0 1 2 3)");
}

#[test]
fn test_vpwl() {
    test_component(r#"
module tb;
    vpwl #(.points("0 0 1 1")) inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "Vinst p n PWL(0 0 1 1)");
}

#[test]
fn test_ipwl() {
    test_component(r#"
module tb;
    ipwl #(.points("0 0 1 1")) inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "inst p n PWL(0 0 1 1)");
}

#[test]
fn test_vsffm() {
    test_component(r#"
module tb;
    vsffm #(.vo(0.0), .va(1.0), .fc(100.0), .mdi(0.5), .fs(10.0), .phasec(0.0), .phases(0.0)) inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "Vinst p n SFFM(0 1 100 0.5 10 0 0)");
}

#[test]
fn test_vam() {
    test_component(r#"
module tb;
    vam #(.sa(1.0), .fc(100.0), .fm(10.0), .td(0.0), .phases(0.0)) inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "Vinst p n AM(1 100 10 0 0)");
}

#[test]
fn test_vnoise() {
    test_component(r#"
module tb;
    vnoise #(.na(1.0), .nt(2.0), .nalpha(3.0), .namp(4.0)) inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "Vinst p n TRNOISE(1 2 3 4)");
}

#[test]
fn test_vrandom() {
    test_component(r#"
module tb;
    vrandom #(.rtype(1.0), .ts(0.1), .td(0.0), .param1(2.0), .param2(3.0)) inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "Vinst p n TRRANDOM(1 0.1 0 2 3)");
}

#[test]
fn test_vcvs() {
    test_component(r#"
module tb;
    vcvs #(.gain(2.0)) inst(.p(p), .n(n), .cp(cp), .cn(cn));
    initial begin end
endmodule
"#, "Einst p n cp cn 2");
}

#[test]
fn test_vccs() {
    test_component(r#"
module tb;
    vccs #(.gm(2.0)) inst(.p(p), .n(n), .cp(cp), .cn(cn));
    initial begin end
endmodule
"#, "Ginst p n cp cn 2");
}

#[test]
fn test_ccvs() {
    test_component(r#"
module tb;
    ccvs #(.vsrc("Vsrc"), .transres(2.0)) inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "Hinst p n Vsrc 2");
}

#[test]
fn test_cccs() {
    test_component(r#"
module tb;
    cccs #(.vsrc("Vsrc"), .gain(2.0)) inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "Finst p n Vsrc 2");
}

#[test]
fn test_vsw() {
    test_component(r#"
paramset my_vsw vsw;
    .model = "sw_mod";
endparamset
module tb;
    my_vsw inst(.p(p), .n(n), .cp(cp), .cn(cn));
    initial begin end
endmodule
"#, "Sinst p n cp cn sw_mod");
}

#[test]
fn test_isw() {
    test_component(r#"
paramset my_isw isw;
    .model = "sw_mod";
    .vsrc = "Vsrc";
endparamset
module tb;
    my_isw inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "Winst p n Vsrc sw_mod");
}

#[test]
fn test_diode() {
    test_component(r#"
paramset my_d d;
    .model = "d_mod";
    .area = 2.0;
endparamset
module tb;
    my_d inst(.a(a), .c(c));
    initial begin end
endmodule
"#, "Dinst a c d_mod AREA=2");
}

#[test]
fn test_npn() {
    test_component(r#"
paramset my_npn npn;
    .model = "npn_mod";
    .area = 2.0;
endparamset
module tb;
    my_npn inst(.c(c), .b(b), .e(e));
    initial begin end
endmodule
"#, "Qinst c b e npn_mod AREA=2");
}

#[test]
fn test_pnp() {
    test_component(r#"
paramset my_pnp pnp;
    .model = "pnp_mod";
    .area = 2.0;
endparamset
module tb;
    my_pnp inst(.c(c), .b(b), .e(e));
    initial begin end
endmodule
"#, "Qinst c b e pnp_mod AREA=2");
}

#[test]
fn test_npn4() {
    test_component(r#"
paramset my_npn4 npn4;
    .model = "npn_mod";
endparamset
module tb;
    my_npn4 inst(.c(c), .b(b), .e(e), .sub(sub));
    initial begin end
endmodule
"#, "Qinst c b e sub npn_mod");
}

#[test]
fn test_pnp4() {
    test_component(r#"
paramset my_pnp4 pnp4;
    .model = "pnp_mod";
endparamset
module tb;
    my_pnp4 inst(.c(c), .b(b), .e(e), .sub(sub));
    initial begin end
endmodule
"#, "Qinst c b e sub pnp_mod");
}

#[test]
fn test_nmos() {
    test_component(r#"
paramset my_nmos nmos;
    .model = "nmos_mod";
    .w = 1.0;
    .l = 1.0;
    .nrd = 1.0;
    .nrs = 1.0;
endparamset
module tb;
    my_nmos inst(.d(d), .g(g), .s(s), .b(b));
    initial begin end
endmodule
"#, "Minst d g s b nmos_mod W=1 L=1 NRD=1 NRS=1");
}

#[test]
fn test_pmos() {
    test_component(r#"
paramset my_pmos pmos;
    .model = "pmos_mod";
    .w = 1.0;
    .l = 1.0;
    .nrd = 1.0;
    .nrs = 1.0;
endparamset
module tb;
    my_pmos inst(.d(d), .g(g), .s(s), .b(b));
    initial begin end
endmodule
"#, "Minst d g s b pmos_mod W=1 L=1 NRD=1 NRS=1");
}

#[test]
fn test_jfet_n() {
    test_component(r#"
paramset my_jfet_n jfet_n;
    .model = "njf_mod";
    .area = 2.0;
endparamset
module tb;
    my_jfet_n inst(.d(d), .g(g), .s(s));
    initial begin end
endmodule
"#, "Jinst d g s njf_mod AREA=2");
}

#[test]
fn test_jfet_p() {
    test_component(r#"
paramset my_jfet_p jfet_p;
    .model = "pjf_mod";
    .area = 2.0;
endparamset
module tb;
    my_jfet_p inst(.d(d), .g(g), .s(s));
    initial begin end
endmodule
"#, "Jinst d g s pjf_mod AREA=2");
}

#[test]
fn test_mesfet_n() {
    test_component(r#"
paramset my_mesfet_n mesfet_n;
    .model = "nmf_mod";
    .area = 2.0;
endparamset
module tb;
    my_mesfet_n inst(.d(d), .g(g), .s(s));
    initial begin end
endmodule
"#, "Zinst d g s nmf_mod AREA=2");
}

#[test]
fn test_mesfet_p() {
    test_component(r#"
paramset my_mesfet_p mesfet_p;
    .model = "pmf_mod";
    .area = 2.0;
endparamset
module tb;
    my_mesfet_p inst(.d(d), .g(g), .s(s));
    initial begin end
endmodule
"#, "Zinst d g s pmf_mod AREA=2");
}

#[test]
fn test_vdmos() {
    test_component(r#"
paramset my_vdmos vdmos;
    .model = "vdmos_mod";
    .w = 1.0;
    .l = 1.0;
endparamset
module tb;
    my_vdmos inst(.d(d), .g(g), .s(s));
    initial begin end
endmodule
"#, "Minst d g s vdmos_mod W=1 L=1");
}

#[test]
fn test_tline() {
    test_component(r#"
module tb;
    tline inst(.ap(ap), .an(an), .bp(bp), .bn(bn));
    initial begin end
endmodule
"#, "Tinst ap an bp bn");
}

#[test]
fn test_ltra() {
    test_component(r#"
paramset my_ltra ltra;
    .model = "ltra_mod";
endparamset
module tb;
    my_ltra inst(.ap(ap), .an(an), .bp(bp), .bn(bn));
    initial begin end
endmodule
"#, "Oinst ap an bp bn ltra_mod");
}

#[test]
fn test_urc() {
    test_component(r#"
paramset my_urc urc;
    .model = "urc_mod";
    .length = 1.0;
endparamset
module tb;
    my_urc inst(.a(a), .b(b), .ref(ref_));
    initial begin end
endmodule
"#, "Uinst a b ref_ urc_mod L=1");
}

#[test]
fn test_port() {
    test_component(r#"
module tb;
    port inst(.p(p), .n(n));
    initial begin end
endmodule
"#, "Pinst p n");
}

#[test]
fn test_subckt() {
    test_component(r#"
module tb;
    subckt #(.subckt_name("my_subckt"), .ports("p n"), .params("param1=1")) inst();
    initial begin end
endmodule
"#, "Xinst p n my_subckt param1=1");
}
