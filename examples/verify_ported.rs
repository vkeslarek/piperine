use piperine_parser::parser::parse_with_includes;
fn main() {
    let dirs = vec![piperine_ngspice::ppr_dir(), piperine_parser::bundled_header_dir()];
    let mut ok=0; let mut fail=0;
    for p in std::env::args().skip(1) {
        let src = std::fs::read_to_string(&p).unwrap();
        match parse_with_includes(&src, &dirs) {
            Ok(_) => { ok+=1; println!("OK   {p}"); }
            Err(e) => { fail+=1; println!("FAIL {p}: {e}"); }
        }
    }
    println!("--- {ok} ok, {fail} fail");
}
