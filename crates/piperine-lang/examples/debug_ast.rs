fn main() {
    let src = std::fs::read_to_string("examples/test_03_quad_conflict.phdl").unwrap();
    let elab = piperine_lang::parse_and_elaborate(&src).unwrap();
    let ir = piperine_lang::ppr_to_ir(&elab);
    for module in ir.modules {
        if module.name == "Driver1" {
            println!("IR for Driver1: {:#?}", module);
        }
    }
}
