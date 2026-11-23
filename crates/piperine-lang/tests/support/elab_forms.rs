//! What the `elab_aggregates` and `elab_control` targets share besides `support/elab.rs`
//! (`#[path]`-included, never a target itself): the source of a design of `Top` that calls, reacts
//! or holds params, and what is read back from the IR it elaborates to.

use piperine_ir::{Constant, Ir, TypeData};

use crate::support::Elab;

/// The source text of a design of `Top` with some item exercised.
pub struct ElabSource;

impl ElabSource {
    /// A design of `Top` with `items` before it, calling each of `calls` from its `analog` block.
    pub fn reaching(items: &str, calls: &[&str]) -> String {
        let lets: Vec<String> = calls.iter().enumerate().map(|(k, call)| format!("let x{k} = {call};")).collect();
        format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {} }}", lets.join(" "))
    }

    /// A design whose `Top` has the reaction `body` on a clock `clk`.
    pub fn reacting(items: &str, body: &str) -> String {
        format!("{items}\nmodule Top(in clk: Bit, out q: Bit<u8> = 0) {{ }}\ndigital for Top {{ on clk.rise() {{ {body} }} }}")
    }

    /// A design of `Top` whose `analog` block contributes `call` to its node.
    pub fn analog_calling(items: &str, call: &str) -> String {
        format!("{items}\nmodule Top() {{ signal o: Electrical; signal a: Electrical; }}\nanalog for Top {{ i(o) +> {call}; }}")
    }

    /// A design of `Top` with the const items `items` and the params `params` of `Top`.
    pub fn with_params(items: &str, params: &str) -> String {
        format!("{items}\nmodule Top() {{ {params} }}")
    }
}

/// What the IR of an elaborated design holds.
pub struct IrReading;

impl IrReading {
    /// The entry of the type arena named `name`.
    pub fn arena_entry<'a>(ir: &'a Ir, name: &str) -> (usize, &'a TypeData) {
        ir.types().iter().enumerate().find(|(_, data)| data.name == name).unwrap_or_else(|| panic!("no arena type `{name}`: {:?}", ir.types().iter().map(|t| &t.name).collect::<Vec<_>>()))
    }

    /// The text of the function named `name` in the structural print.
    pub fn function_text(ir: &Ir, name: &str) -> String {
        let text = ir.print_structural();
        let head = format!("fn {name}(");
        let start = text.find(&head).unwrap_or_else(|| panic!("no `{head}` in:\n{text}"));
        let rest = &text[start..];
        let end = rest[1..].find("\nfn ").or_else(|| rest[1..].find("\ninstance ")).map_or(rest.len(), |at| at + 1);
        rest[..end].to_string()
    }

    /// The folded value of the `f64` param `name` of `Top`.
    pub fn default_of(ir: &Ir, name: &str) -> Constant {
        let top = ir.bodies().find(|b| b.name() == "Top").expect("Top");
        let param = top.params().iter().find(|p| p.name == name).unwrap_or_else(|| panic!("no param {name}"));
        param.default.unwrap_or_else(|| panic!("no default for {name}"))
    }

    /// The values of the params of `Top` of `src`, printed.
    pub fn param_values(src: &str) -> Vec<String> {
        let ir = Elab::ir(src, "Top");
        ir.instance_at("Top").expect("the top instance").params().iter().map(|c| c.to_string()).collect()
    }
}
