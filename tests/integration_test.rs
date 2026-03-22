use piperine_core::prelude::*;

#[test]
fn netlist_generation_resistor_divider() {
    let ckt = Circuit::new("Resistor Divider")
        .vdc("in", "in", GND, 10.0)
        .resistor("1", "in", "out", "10k")
        .resistor("2", "out", GND, "10k");

    let lines = ckt.to_netlist_lines();
    assert_eq!(lines[0], "Resistor Divider");
    assert!(lines.iter().any(|l| l == "Vin in 0 DC 10"));
    assert!(lines.iter().any(|l| l == "R1 in out 10k"));
    assert!(lines.iter().any(|l| l == "R2 out 0 10k"));
    assert_eq!(lines.last().unwrap(), ".end");
}

#[test]
fn netlist_generation_with_analysis() {
    let ckt = Circuit::new("AC Test")
        .vac("in", "in", GND, 1.0)
        .resistor("1", "in", "out", "1k")
        .capacitor("1", "out", GND, "1u");

    let ac = AcAnalysis::new(Variation::Dec, 10, 1.0, 1e6)
        .reltol(0.001)
        .save("v(out)")
        .meas("bw", "WHEN vdb(out)=-3");

    let netlist = ckt.to_netlist_lines();
    let control = ac.to_control_commands();

    // Netlist should have circuit elements
    assert!(netlist.iter().any(|l| l.contains("Vin")));
    assert!(netlist.iter().any(|l| l.contains("R1")));
    assert!(netlist.iter().any(|l| l.contains("C1")));

    // Control should have analysis command and options
    assert!(control.iter().any(|l| l.starts_with("ac dec")));
    assert!(control.iter().any(|l| l.contains("reltol=0.001")));
    assert!(control.iter().any(|l| l.contains("save v(out)")));
    assert!(control.iter().any(|l| l.contains(".meas ac bw")));
}

#[test]
fn netlist_generation_transient() {
    let ckt = Circuit::new("Pulse Test")
        .vsource("in", "in", GND,
            Waveform::Pulse(Pulse::new(0.0, 5.0)
                .delay(1e-6)
                .rise(1e-9)
                .fall(1e-9)
                .width(5e-6)
                .period(10e-6)))
        .resistor("1", "in", "out", "1k")
        .capacitor("1", "out", GND, "100p");

    let tran = TranAnalysis::new(1e-9, 50e-6)
        .method(IntegrationMethod::Gear)
        .maxord(2)
        .save("v(out)")
        .save("v(in)");

    let netlist = ckt.to_netlist_lines();
    let control = tran.to_control_commands();

    assert!(netlist.iter().any(|l| l.contains("PULSE(")));
    assert!(control.iter().any(|l| l.starts_with("tran")));
    assert!(control.iter().any(|l| l.contains("method=gear")));
    assert!(control.iter().any(|l| l.contains("maxord=2")));
}

#[test]
fn subcircuit_fn_netlist() {
    let ckt = Circuit::new("SubCkt Test")
        .subcircuit_fn("lpf", &["in", "out"], |b| {
            b.resistor("R", "in", "out", "1k")
             .capacitor("C", "out", "0", "1u");
        })
        .vdc("1", "in", GND, 5.0)
        .instance("1", "lpf", &["in", "out"]);

    let lines = ckt.to_netlist_lines();
    assert!(lines.iter().any(|l| l == ".subckt lpf in out"));
    assert!(lines.iter().any(|l| l == "RR in out 1k"));
    assert!(lines.iter().any(|l| l == ".ends"));
    assert!(lines.iter().any(|l| l == "X1 in out lpf"));
}

#[test]
fn unit_ext_works() {
    assert_eq!(1.0.kOhms(), 1000.0);
    assert_eq!(5.0.mV(), 0.005);
    assert_eq!(1.0.MHz(), 1e6);
    assert_eq!(10.V(), 10.0); // i64
    assert_eq!(1.kOhms(), 1000.0); // i64
}
