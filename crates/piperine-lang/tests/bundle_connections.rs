use piperine_lang::parse_and_elaborate;

#[test]
fn bundle_connections_fan_out() {
    let src = r#"
        discipline Ground {
            potential v: Real;
        }
        bundle Pair {
            a: Ground,
            b: Ground,
        }
        mod top(inout p1: Pair, inout p2: Pair) {
            wire w1: Pair;

            p1 = w1;
        }
    "#;
    
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).unwrap_or_else(|e| panic!("Elaboration failed: {}", e));
    let top = elab.module("top").unwrap();
    
    println!("Wires: {:?}", top.wires());
    println!("Ports: {:?}", top.ports());
    println!("Connections: {:?}", top.connections());
    // let ir = ppr_to_ir(&elab).expect("lowering failed");
}
