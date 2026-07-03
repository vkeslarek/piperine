//! Parse-check for NGSPICE faithful model files.
//! Only checks parsing (not elaboration/codegen) since the models use
//! ideal-Piperine features not yet implemented.

use piperine_lang::parse::parse_str;

#[test]
fn ngspice_constants_parse() {
    let src = include_str!("../headers/ngspice/constants.phdl");
    parse_str(src).expect("ngspice_constants.phdl should parse");
}

#[test]
fn ngspice_passives_parse() {
    let src = include_str!("../headers/ngspice/passives.phdl");
    parse_str(src).expect("passives.phdl should parse");
}

#[test]
fn ngspice_diode_parse() {
    let src = include_str!("../headers/ngspice/diode.phdl");
    parse_str(src).expect("diode.phdl should parse");
}

#[test]
fn ngspice_bjt_parse() {
    let src = include_str!("../headers/ngspice/bjt.phdl");
    parse_str(src).expect("bjt.phdl should parse");
}

#[test]
fn ngspice_jfet_parse() {
    let src = include_str!("../headers/ngspice/jfet.phdl");
    parse_str(src).expect("jfet.phdl should parse");
}

#[test]
fn ngspice_mos_parse() {
    let src = include_str!("../headers/ngspice/mos.phdl");
    parse_str(src).expect("mos.phdl should parse");
}

#[test]
fn ngspice_switches_parse() {
    let src = include_str!("../headers/ngspice/switches.phdl");
    parse_str(src).expect("switches.phdl should parse");
}

#[test]
fn ngspice_sources_parse() {
    let src = include_str!("../headers/ngspice/sources.phdl");
    parse_str(src).expect("sources.phdl should parse");
}

#[test]
fn ngspice_controlled_parse() {
    let src = include_str!("../headers/ngspice/controlled.phdl");
    parse_str(src).expect("controlled.phdl should parse");
}
