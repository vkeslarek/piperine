//! Verifies the ergonomic model surfaces every parsed feature — no silent drops.

use cvaf::*;

const SRC: &str = r#"
(* my_module_attr = "x" *)
module amp(in, out);
    inout in;
    output [0:3] out;
    electrical in;
    optical [0:1] out;

    (* desc = "gain", units = "1" *)
    parameter real gain = 2.0 from (0:inf) exclude 1.0;
    localparam integer n = 4;
    aliasparam g = gain;

    (* desc = "state" *)
    real state[0:3];
    integer i = 0;

    wire w1, w2;
    electrical bus[3:0], scalar;

    branch (in, out) b1;

    SubModule sub1(in, out[0:1]);

    analog initial begin
        state[0] = 0;
    end

    analog begin
        gain = gain + 1;
    end

    analog function real twice;
        input x;
        real x;
        twice = 2 * x;
    endfunction
endmodule
"#;

#[test]
fn model_surfaces_all_features() {
    let doc = parse(SRC).expect("parse");
    let m = &doc.modules[0];

    // module attribute
    assert_eq!(m.attributes.len(), 1);
    assert_eq!(m.attributes[0].name, "my_module_attr");

    // ports: in (placeholder from header) + the body decls; check the bus range
    let out = m.ports.iter().find(|p| p.name == "out" && p.range.is_some());
    assert!(out.is_some(), "output bus range not captured");

    // parameter with attrs + constraints
    let gain = m.parameters.iter().find(|p| p.name == "gain").unwrap();
    assert!(!gain.is_local);
    assert_eq!(gain.attributes.len(), 2);
    assert_eq!(gain.constraints.len(), 2);
    let n = m.parameters.iter().find(|p| p.name == "n").unwrap();
    assert!(n.is_local);

    // aliasparam
    assert_eq!(m.aliasparams.len(), 1);
    assert_eq!(m.aliasparams[0].name, "g");
    assert!(matches!(&m.aliasparams[0].source, ParamSource::Path(p) if p == "gain"));

    // variable with range + attribute
    let state = m.variables.iter().find(|v| v.name == "state").unwrap();
    assert!(state.range.is_some(), "array range not captured");
    assert_eq!(state.attributes.len(), 1);

    // net with discipline
    let net = m
        .nets
        .iter()
        .find(|nt| nt.members.iter().any(|x| x.name == "w1"))
        .unwrap();
    assert!(net.members.iter().any(|x| x.name == "w2"));

    // per-name range: bus[3:0] has a range, scalar does not
    let busnet = m
        .nets
        .iter()
        .find(|nt| nt.members.iter().any(|x| x.name == "bus"))
        .unwrap();
    let bus = busnet.members.iter().find(|x| x.name == "bus").unwrap();
    let scalar = busnet.members.iter().find(|x| x.name == "scalar").unwrap();
    assert!(bus.range.is_some(), "per-name bus range not captured");
    assert!(scalar.range.is_none(), "scalar should have no range");

    // branch
    assert_eq!(m.branches.len(), 1);
    assert_eq!(m.branches[0].names, vec!["b1".to_string()]);

    // instance with positional connections (one a part-select)
    assert_eq!(m.instances.len(), 1);
    let inst = &m.instances[0];
    assert_eq!(inst.module, "SubModule");
    assert_eq!(inst.name, "sub1");
    assert_eq!(inst.connections.len(), 2);

    // analog blocks: one initial, one not
    assert_eq!(m.analog_blocks.len(), 2);
    assert!(m.analog_blocks.iter().any(|b| b.is_initial));
    assert!(m.analog_blocks.iter().any(|b| !b.is_initial));

    // function with arg + body
    assert_eq!(m.functions.len(), 1);
    let f = &m.functions[0];
    assert_eq!(f.name, "twice");
    assert!(matches!(f.return_type, Some(ast::Type::Real)));
    assert_eq!(f.args.len(), 1);
    assert!(!f.body.is_empty());
}
