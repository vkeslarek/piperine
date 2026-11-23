//! `DerivePrinter`: the generated functions that recompute a derived param (features `host-ii`, X9,
//! and `params-and-io`, §8.1.19 and §8.1.23). One `derive_<k>_` per binding of the design's
//! `DerivePlan`, over the parent's params as the bytes of their pins, one after another in the
//! parent's declaration order, each as wide as its pin (`PinLayout::size`), and giving the pin
//! bytes of the child's param: every param crosses as the typed value it is (an integer exactly, a
//! `bool`, an `f64`, an enum's tag, an `Option` or a composite), never as an `f64`. The export
//! `piperine_derive_2(k, params, out)` sits next to `piperine_abi_1` and the host reaches it through
//! `Compiled::derive`: the host lends `out`, as wide as the child's pin, the function writes the pin
//! bytes into it and answers how many it wrote, 0 for a `k` this design has no function for or a
//! parent of another width. Nothing is allocated for the host to keep. The buffers are read and
//! written through the ABI's safe accessors, so the source holds no `unsafe` of its own (RS-05 AC 4).
//! A design that derives nothing prints none of it, so its source is the source it always was.

use piperine_ir::Ir;
use piperine_plan::{Binding, DerivePlan, PinLayout};

use crate::Error;
use crate::print::{Text, ValuePrinter};

/// The symbol the export has: the suffix is the version of this contract, as `piperine_abi_1`'s is
/// the ABI major.
pub const SYMBOL: &str = "piperine_derive_2";

/// Prints the derive functions of a design.
#[derive(Debug)]
pub struct DerivePrinter;

impl DerivePrinter {
    /// The functions and the export of `plan` as source lines; `None` when the plan is empty.
    pub fn text(ir: &Ir, plan: &DerivePlan) -> Result<Option<String>, Error> {
        if plan.bindings().is_empty() {
            return Ok(None);
        }
        let mut t = Text::new();
        let mut arms = Vec::new();
        for binding in plan.bindings() {
            let (text, width) = Self::function(ir, binding)?;
            t.line(&text);
            t.line("");
            arms.push((binding.k, width));
        }
        t.comment("the derive export: the host recomputes a derived param through it", ir.top().body().origin());
        t.line("#[unsafe(no_mangle)]");
        t.line(&format!("pub extern \"C\" fn {SYMBOL}(k: u32, params: List<'_, u8>, mut out: BytesMut<'_>) -> u32 {{"));
        t.indented(|t| {
            t.line("let params: Vec<u8> = (0..params.len()).filter_map(|i| params.get(i).copied()).collect();");
            t.line("// a `k` this design has no function for, or a parent of another width, answers nothing");
            t.line("let answer: Option<Vec<u8>> = match k {");
            t.indented(|t| {
                for (k, width) in &arms {
                    t.line(&format!("{k} if params.len() == {width} => Some({}(&params)),", Text::generated(&format!("derive_{k}"))));
                }
                t.line("_ => None,");
            });
            t.line("};");
            t.line("match answer {");
            t.indented(|t| {
                t.line("Some(bytes) if bytes.len() <= out.len() => {");
                t.indented(|t| {
                    t.line("out.as_mut_slice()[..bytes.len()].copy_from_slice(&bytes);");
                    t.line("bytes.len() as u32");
                });
                t.line("}");
                t.line("_ => 0,");
            });
            t.line("}");
        });
        t.line("}");
        Ok(Some(t.finish()))
    }

    /// `derive_<k>_` of `binding`, and the width in bytes of the parent's params it reads by position.
    fn function(ir: &Ir, binding: &Binding) -> Result<(String, usize), Error> {
        let parent = ir.instances().find(|i| i.path() == binding.parent).map(|i| i.body());
        let Some(parent) = parent else {
            return Err(Error::Unsupported { form: "a derived param whose parent is no instance".into(), path: binding.child.clone(), origin: ir.top().body().origin().clone() });
        };
        let printer = ValuePrinter::of_placement(ir, parent);
        let value = printer.expr(binding.expr)?;
        let params = Text::generated("params");
        let at = parent.origin();
        let unsupported = |form: String| Error::Unsupported { form, path: format!("{}.{}", binding.child, binding.param), origin: at.clone() };
        let mut lets = Vec::new();
        let mut offset = 0usize;
        for p in parent.params() {
            let size = PinLayout::in_ir(ir, &p.ty).map(|layout| layout.size() as usize).ok_or_else(|| unsupported(format!("a derived param that reads the param `{}` of type {}", p.name, ir.written(&p.ty))))?;
            let name = Text::ident(&p.name);
            if printer.uses(&name) {
                let rust = printer.pin_type(&p.ty)?;
                lets.push(format!("let {name} = {};", ValuePrinter::pin_value(&rust, &format!("&{params}[{offset}..{}]", offset + size))));
            }
            offset += size;
        }
        let child = printer.pin_type(&binding.ty)?;
        let mut t = Text::new();
        t.comment(&format!("derive {}: {}.{} from the params of {}", binding.k, binding.child, binding.param, binding.parent), at);
        t.line(&format!("fn {}({params}: &[u8]) -> Vec<u8> {{", Text::generated(&format!("derive_{}", binding.k))));
        t.indented(|t| {
            for line in &lets {
                t.line(line);
            }
            t.line(&format!("<{child} as rt::Carried>::pin({value})"));
        });
        t.line("}");
        Ok((t.finish(), offset))
    }
}
