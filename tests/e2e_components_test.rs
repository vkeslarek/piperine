//! Hardware component elaboration tests.
//!
//! Each test declares a pure structural module (no `initial` blocks),
//! elaborates it with `elaborate_circuit`, and asserts the correct SPICE line.

use piperine_circuit::{elaborate_circuit, HardwareRegistry};
use piperine_ngspice::register_hardware;
use piperine_parser::parser::parse_with_includes;

fn spice_lines(src: &str) -> Vec<String> {
    let full_src = format!("`include \"ngspice.ppr\"\n{}", src);
    let dirs = vec![
        piperine_ngspice::ppr_dir(),
        piperine_parser::bundled_header_dir(),
    ];
    let doc = parse_with_includes(&full_src, &dirs).expect("parse failed");
    let mut registry = HardwareRegistry::new();
    register_hardware(&mut registry);
    let circuit = elaborate_circuit(&doc, &registry, None).expect("elaborate_circuit failed");
    circuit.spice_lines
}

fn has_line(src: &str, expected: &str) {
    let lines = spice_lines(src);
    assert!(
        lines.iter().any(|l| l == expected),
        "expected `{expected}` not found in:\n{}", lines.join("\n")
    );
}

fn has_line_starting(src: &str, prefix: &str) {
    let lines = spice_lines(src);
    assert!(
        lines.iter().any(|l| l.starts_with(prefix)),
        "expected line starting with `{prefix}` not found in:\n{}", lines.join("\n")
    );
}

fn has_line_containing(src: &str, needle: &str) {
    let lines = spice_lines(src);
    assert!(
        lines.iter().any(|l| l.contains(needle)),
        "expected line containing `{needle}` not found in:\n{}", lines.join("\n")
    );
}

// ── Passives ─────────────────────────────────────────────────────────────────

#[test]
fn test_res() {
    has_line(r#"module tb; res #(.r(1000.0)) R1(.p(a), .n(b)); endmodule"#,
             "R1 a b 1000");
}

#[test]
fn test_res_with_opts() {
    has_line(
        r#"module tb; res #(.r(1000.0), .tc1(0.1), .m(2.0), .noisy(0)) inst(.p(p), .n(n)); endmodule"#,
        "Rinst p n 1000 M=2 TC1=0.1 NOISY=0",
    );
}

#[test]
fn test_cap() {
    let lines = spice_lines(r#"module tb; cap #(.c(1e-9)) C1(.p(a), .n(b)); endmodule"#);
    assert!(lines.iter().any(|l| l.starts_with("C1 a b")),
        "expected cap C1 in:\n{}", lines.join("\n"));
}

#[test]
fn test_ind() {
    let lines = spice_lines(r#"module tb; ind #(.l(1e-6)) L1(.p(a), .n(b)); endmodule"#);
    assert!(lines.iter().any(|l| l.starts_with("L1 a b")),
        "expected ind L1 in:\n{}", lines.join("\n"));
}

#[test]
fn test_mutual_nodeless() {
    has_line(
        r#"module tb; mutual #(.inductor1("L1"), .inductor2("L2"), .k(0.8)) K1(); endmodule"#,
        "K1 L1 L2 0.8",
    );
}

// ── Voltage / Current sources ─────────────────────────────────────────────────

#[test]
fn test_vsource_dc_ac() {
    has_line(
        r#"module tb; vsource #(.dc(5.0), .acmag(1.0), .acphase(90.0)) inst(.p(p), .n(n)); endmodule"#,
        "Vinst p n DC 5 AC 1 90",
    );
}

#[test]
fn test_vpulse() {
    has_line(
        r#"module tb; vpulse #(.v0(0.0), .v1(1.0), .td(0.0), .tr(1.0), .tf(2.0), .pw(3.0), .per(4.0)) inst(.p(p), .n(n)); endmodule"#,
        "Vinst p n PULSE(0 1 0 1 2 3 4)",
    );
}

#[test]
fn test_ipulse() {
    has_line(
        r#"module tb; ipulse #(.i0(0.0), .i1(1.0), .td(0.0), .tr(1.0), .tf(2.0), .pw(3.0), .per(4.0)) inst(.p(p), .n(n)); endmodule"#,
        "inst p n PULSE(0 1 0 1 2 3 4)",
    );
}

#[test]
fn test_vsin() {
    has_line(
        r#"module tb; vsin #(.vo(0.0), .va(1.0), .freq(100.0), .td(0.0), .theta(0.0), .phi(0.0)) inst(.p(p), .n(n)); endmodule"#,
        "Vinst p n SIN(0 1 100 0 0 0)",
    );
}

#[test]
fn test_isin() {
    has_line(
        r#"module tb; isin #(.io(0.0), .ia(1.0), .freq(100.0), .td(0.0), .theta(0.0), .phi(0.0)) inst(.p(p), .n(n)); endmodule"#,
        "inst p n SIN(0 1 100 0 0 0)",
    );
}

#[test]
fn test_vexp() {
    has_line(
        r#"module tb; vexp #(.v1(0.0), .v2(1.0), .td1(0.0), .tau1(1.0), .td2(2.0), .tau2(3.0)) inst(.p(p), .n(n)); endmodule"#,
        "Vinst p n EXP(0 1 0 1 2 3)",
    );
}

#[test]
fn test_iexp() {
    has_line(
        r#"module tb; iexp #(.i1(0.0), .i2(1.0), .td1(0.0), .tau1(1.0), .td2(2.0), .tau2(3.0)) inst(.p(p), .n(n)); endmodule"#,
        "inst p n EXP(0 1 0 1 2 3)",
    );
}

#[test]
fn test_vpwl() {
    has_line(
        r#"module tb; vpwl #(.points("0 0 1 1")) inst(.p(p), .n(n)); endmodule"#,
        "Vinst p n PWL(0 0 1 1)",
    );
}

#[test]
fn test_ipwl() {
    has_line(
        r#"module tb; ipwl #(.points("0 0 1 1")) inst(.p(p), .n(n)); endmodule"#,
        "inst p n PWL(0 0 1 1)",
    );
}

#[test]
fn test_vsffm() {
    has_line(
        r#"module tb; vsffm #(.vo(0.0), .va(1.0), .fc(100.0), .mdi(0.5), .fs(10.0), .phasec(0.0), .phases(0.0)) inst(.p(p), .n(n)); endmodule"#,
        "Vinst p n SFFM(0 1 100 0.5 10 0 0)",
    );
}

#[test]
fn test_vam() {
    has_line(
        r#"module tb; vam #(.sa(1.0), .fc(100.0), .fm(10.0), .td(0.0), .phases(0.0)) inst(.p(p), .n(n)); endmodule"#,
        "Vinst p n AM(1 100 10 0 0)",
    );
}

#[test]
fn test_vnoise() {
    has_line(
        r#"module tb; vnoise #(.na(1.0), .nt(2.0), .nalpha(3.0), .namp(4.0)) inst(.p(p), .n(n)); endmodule"#,
        "Vinst p n TRNOISE(1 2 3 4)",
    );
}

#[test]
fn test_vrandom() {
    has_line(
        r#"module tb; vrandom #(.rtype(1.0), .ts(0.1), .td(0.0), .param1(2.0), .param2(3.0)) inst(.p(p), .n(n)); endmodule"#,
        "Vinst p n TRRANDOM(1 0.1 0 2 3)",
    );
}

// ── Controlled sources ────────────────────────────────────────────────────────

#[test]
fn test_vcvs() {
    has_line(
        r#"module tb; vcvs #(.gain(2.0)) inst(.p(p), .n(n), .cp(cp), .cn(cn)); endmodule"#,
        "Einst p n cp cn 2",
    );
}

#[test]
fn test_vccs() {
    has_line(
        r#"module tb; vccs #(.gm(2.0)) inst(.p(p), .n(n), .cp(cp), .cn(cn)); endmodule"#,
        "Ginst p n cp cn 2",
    );
}

#[test]
fn test_ccvs() {
    has_line(
        r#"module tb; ccvs #(.vsrc("Vsrc"), .transres(2.0)) inst(.p(p), .n(n)); endmodule"#,
        "Hinst p n Vsrc 2",
    );
}

#[test]
fn test_cccs() {
    has_line(
        r#"module tb; cccs #(.vsrc("Vsrc"), .gain(2.0)) inst(.p(p), .n(n)); endmodule"#,
        "Finst p n Vsrc 2",
    );
}

// ── Behavioral B-sources ──────────────────────────────────────────────────────

#[test]
fn test_bsource_v() {
    has_line(
        r#"module tb; bsource_v #(.V("v(a)*2"), .tc1(0.5)) inst(.p(p), .n(n)); endmodule"#,
        "Binst p n V=v(a)*2 TC1=0.5",
    );
}

#[test]
fn test_bsource_v_expr() {
    let lines = spice_lines(r#"
module tb;
    bsource_v #(.V( V(a)*V(b) )) Bmix(.p(out), .n(gnd));
    bsource_v #(.V( V(in) > 0.0 ? V(in) : 0.0 )) Brect(.p(o), .n(gnd));
endmodule
"#);
    assert!(lines.iter().any(|l| l.starts_with("Bmix out 0 V=") && l.contains("v(a)") && l.contains("v(b)")),
        "missing Bmix line in:\n{}", lines.join("\n"));
    assert!(lines.iter().any(|l| l.starts_with("Brect o 0 V=") && l.contains("v(in)")),
        "missing Brect line in:\n{}", lines.join("\n"));
}

#[test]
fn test_bsource_i_expr() {
    let lines = spice_lines(r#"
module tb;
    bsource_i #(.I( exp(V(b,e)/0.026) - 1.0 )) Bdiode(.p(c), .n(e));
endmodule
"#);
    assert!(lines.iter().any(|l| l.starts_with("Bdiode c e I=") && l.contains("exp")),
        "missing Bdiode B-source line in:\n{}", lines.join("\n"));
}

#[test]
fn test_behavioral_nonlinear_vcvs() {
    let lines = spice_lines(r#"
module tb;
    vcvs #(.vol( V(cp,cn) * tanh(V(cp,cn)) )) E1(.p(o), .n(gnd), .cp(a), .cn(b));
    vccs #(.cur( exp(V(cp,cn)/0.026) - 1.0 )) G1(.p(c), .n(e), .cp(b), .cn(e));
endmodule
"#);
    assert!(lines.iter().any(|l| l.starts_with("E1 o 0 a b VOL={") && l.contains("tanh")),
        "missing E1 line in:\n{}", lines.join("\n"));
    assert!(lines.iter().any(|l| l.starts_with("G1 c e b e CUR={") && l.contains("exp")),
        "missing G1 line in:\n{}", lines.join("\n"));
}

#[test]
fn test_behavioral_nonlinear_passives() {
    let lines = spice_lines(r#"
module tb;
    res #(.r_expr( 100.0 * (1.0 + 0.01*(temp - 27.0)) )) Rt(.p(a), .n(b));
    cap #(.q( 1e-12 * V(a,b) )) Cnl(.p(a), .n(b));
    ind #(.flux( 1e-6 * I(Lref) )) L1(.p(a), .n(b));
endmodule
"#);
    assert!(lines.iter().any(|l| l.starts_with("Rt a b R={") && l.contains("27")),
        "missing Rt line in:\n{}", lines.join("\n"));
    assert!(lines.iter().any(|l| l.starts_with("Cnl a b Q={") && l.contains("v(a,b)")),
        "missing Cnl line in:\n{}", lines.join("\n"));
    assert!(lines.iter().any(|l| l.starts_with("L1 a b FLUX={") && l.contains("i(Lref)")),
        "missing L1 line in:\n{}", lines.join("\n"));
}

// ── Switches ──────────────────────────────────────────────────────────────────

#[test]
fn test_vsw() {
    has_line(r#"
paramset my_vsw vsw; .model = "sw_mod"; endparamset
module tb; my_vsw inst(.p(p), .n(n), .cp(cp), .cn(cn)); endmodule
"#, "Sinst p n cp cn sw_mod");
}

#[test]
fn test_isw() {
    has_line(r#"
paramset my_isw isw; .model = "sw_mod"; .vsrc = "Vsrc"; endparamset
module tb; my_isw inst(.p(p), .n(n)); endmodule
"#, "Winst p n Vsrc sw_mod");
}

// ── Semiconductors ────────────────────────────────────────────────────────────

#[test]
fn test_diode() {
    has_line(r#"
paramset my_d d; .model = "d_mod"; .area = 2.0; endparamset
module tb; my_d inst(.a(a), .c(c)); endmodule
"#, "Dinst a c d_mod AREA=2");
}

#[test]
fn test_npn() {
    has_line(r#"
paramset my_npn npn; .model = "npn_mod"; .area = 2.0; endparamset
module tb; my_npn inst(.c(c), .b(b), .e(e)); endmodule
"#, "Qinst c b e npn_mod AREA=2");
}

#[test]
fn test_pnp() {
    has_line(r#"
paramset my_pnp pnp; .model = "pnp_mod"; .area = 2.0; endparamset
module tb; my_pnp inst(.c(c), .b(b), .e(e)); endmodule
"#, "Qinst c b e pnp_mod AREA=2");
}

#[test]
fn test_npn4() {
    has_line(r#"
paramset my_npn4 npn4; .model = "npn_mod"; endparamset
module tb; my_npn4 inst(.c(c), .b(b), .e(e), .sub(sub)); endmodule
"#, "Qinst c b e sub npn_mod");
}

#[test]
fn test_pnp4() {
    has_line(r#"
paramset my_pnp4 pnp4; .model = "pnp_mod"; endparamset
module tb; my_pnp4 inst(.c(c), .b(b), .e(e), .sub(sub)); endmodule
"#, "Qinst c b e sub pnp_mod");
}

#[test]
fn test_nmos() {
    has_line(r#"
paramset my_nmos nmos; .model = "nmos_mod"; .w = 1.0; .l = 1.0; .nrd = 1.0; .nrs = 1.0; endparamset
module tb; my_nmos inst(.d(d), .g(g), .s(s), .b(b)); endmodule
"#, "Minst d g s b nmos_mod W=1 L=1 NRD=1 NRS=1");
}

#[test]
fn test_pmos() {
    has_line(r#"
paramset my_pmos pmos; .model = "pmos_mod"; .w = 1.0; .l = 1.0; .nrd = 1.0; .nrs = 1.0; endparamset
module tb; my_pmos inst(.d(d), .g(g), .s(s), .b(b)); endmodule
"#, "Minst d g s b pmos_mod W=1 L=1 NRD=1 NRS=1");
}

#[test]
fn test_jfet_n() {
    has_line(r#"
paramset my_jfet_n jfet_n; .model = "njf_mod"; .area = 2.0; endparamset
module tb; my_jfet_n inst(.d(d), .g(g), .s(s)); endmodule
"#, "Jinst d g s njf_mod AREA=2");
}

#[test]
fn test_jfet_p() {
    has_line(r#"
paramset my_jfet_p jfet_p; .model = "pjf_mod"; .area = 2.0; endparamset
module tb; my_jfet_p inst(.d(d), .g(g), .s(s)); endmodule
"#, "Jinst d g s pjf_mod AREA=2");
}

#[test]
fn test_mesfet_n() {
    has_line(r#"
paramset my_mesfet_n mesfet_n; .model = "nmf_mod"; .area = 2.0; endparamset
module tb; my_mesfet_n inst(.d(d), .g(g), .s(s)); endmodule
"#, "Zinst d g s nmf_mod AREA=2");
}

#[test]
fn test_mesfet_p() {
    has_line(r#"
paramset my_mesfet_p mesfet_p; .model = "pmf_mod"; .area = 2.0; endparamset
module tb; my_mesfet_p inst(.d(d), .g(g), .s(s)); endmodule
"#, "Zinst d g s pmf_mod AREA=2");
}

#[test]
fn test_vdmos() {
    has_line(r#"
paramset my_vdmos vdmos; .model = "vdmos_mod"; .w = 1.0; .l = 1.0; endparamset
module tb; my_vdmos inst(.d(d), .g(g), .s(s)); endmodule
"#, "Minst d g s vdmos_mod W=1 L=1");
}

// ── Transmission lines ────────────────────────────────────────────────────────

#[test]
fn test_tline() {
    has_line(
        r#"module tb; tline inst(.ap(ap), .an(an), .bp(bp), .bn(bn)); endmodule"#,
        "Tinst ap an bp bn",
    );
}

#[test]
fn test_ltra() {
    has_line(r#"
paramset my_ltra ltra; .model = "ltra_mod"; endparamset
module tb; my_ltra inst(.ap(ap), .an(an), .bp(bp), .bn(bn)); endmodule
"#, "Oinst ap an bp bn ltra_mod");
}

#[test]
fn test_urc() {
    has_line(r#"
paramset my_urc urc; .model = "urc_mod"; .length = 1.0; endparamset
module tb; my_urc inst(.a(a), .b(b), .ref(ref_)); endmodule
"#, "Uinst a b ref_ urc_mod L=1");
}

// ── Misc ──────────────────────────────────────────────────────────────────────

#[test]
fn test_port() {
    has_line(
        r#"module tb; port inst(.p(p), .n(n)); endmodule"#,
        "Pinst p n",
    );
}

#[test]
fn test_subckt() {
    has_line(
        r#"module tb; subckt #(.subckt_name("my_subckt"), .ports("p n"), .params("param1=1")) inst(); endmodule"#,
        "Xinst p n my_subckt param1=1",
    );
}

// ── Paramset without model (passives) ────────────────────────────────────────

#[test]
fn test_paramset_passive_no_model() {
    has_line(r#"
paramset lpf_r res; .r = 1000.0; endparamset
paramset lpf_c cap; .c = 100e-9;  endparamset
module lpf;
    wire in, out;
    lpf_r R1(.p(in), .n(out));
    lpf_c C1(.p(out), .n(gnd));
endmodule
"#, "R1 in out 1000");
}

// ── Module hierarchy (sub-module flattening) ──────────────────────────────────

#[test]
fn test_submodule_flattening() {
    let lines = spice_lines(r#"
module inv_core(in, out);
    inout in, out;
    res #(.r(500.0)) Rp(.p(out), .n(in));
endmodule

module top;
    wire a, b;
    inv_core U1(.in(a), .out(b));
endmodule
"#);
    // Flattened: net names are path-prefixed (U1/out or similar).
    assert!(lines.iter().any(|l| l.starts_with("R") && l.contains("500")),
        "expected flattened resistor in:\n{}", lines.join("\n"));
}
