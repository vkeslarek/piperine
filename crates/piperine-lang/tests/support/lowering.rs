//! Helper shared by the lowering tests (`#[path]`-included, never a target
//! itself); it sits beside `pom.rs`, which the including target also names.

/// The code of every diagnostic that parsing and lowering produce: all of
/// them but the binding errors name resolution adds on top (POM-08..10:
/// `E0161` for a name, `E0330`, `E0457`, `E0401`..`E0403`, `E0130`,
/// `E0343`). A lowering test over a source that mentions names it does not
/// declare asserts this list is empty.
pub fn lowering_codes(design: &piperine_lang::pom::Design) -> Vec<String> {
    const BINDING: [&str; 8] = ["E0161", "E0330", "E0457", "E0401", "E0402", "E0403", "E0130", "E0343"];
    crate::support::codes(design).into_iter().filter(|c| !BINDING.contains(&c.as_str())).collect()
}
