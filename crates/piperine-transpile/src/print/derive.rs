//! `DerivePrinter`: the generated functions that recompute a derived param (feature `host-ii`,
//! X9). One `derive_<k>_` per binding of the design's `DerivePlan`, over the parent's params as
//! one `&[f64]` in the parent's declaration order (a `bool` crosses as exactly 0.0 or 1.0), and
//! the export `piperine_derive_1(k, params)` next to `piperine_abi_1`, which the host reaches
//! through `Compiled::derive`; `params` is the ABI's `List<f64>`, read through its safe accessor, so
//! the source holds no `unsafe` of its own (RS-05 AC 4). A design that derives nothing prints none of it, so its source is
//! the source it always was.

use piperine_ir::{Ir, ValueType};
use piperine_plan::{Binding, DerivePlan};

use crate::Error;
use crate::print::{Text, ValuePrinter};

/// The symbol the export has: the suffix is the version of this contract, as `piperine_abi_1`'s is
/// the ABI major.
pub const SYMBOL: &str = "piperine_derive_1";

/// Prints the derive functions of a design.
#[derive(Debug)]
pub struct DerivePrinter;

impl DerivePrinter {
    /// The functions and the export of `plan` as source lines; `None` when the plan is empty.
    pub fn text(ir: &Ir, plan: &DerivePlan) -> Result<Option<String>, Error> {
        if plan.is_empty() {
            return Ok(None);
        }
        let mut t = Text::new();
        let mut arms = Vec::new();
        for binding in plan.bindings() {
            let (text, count) = Self::function(ir, binding)?;
            t.line(&text);
            t.line("");
            arms.push((binding.k, count));
        }
        t.comment("the derive export: the host recomputes a derived param through it", ir.top().body().origin());
        t.line("#[unsafe(no_mangle)]");
        t.line(&format!("pub extern \"C\" fn {SYMBOL}(k: u32, params: List<'_, f64>) -> f64 {{"));
        t.indented(|t| {
            t.line("let params: Vec<f64> = (0..params.len()).filter_map(|i| params.get(i).copied()).collect();");
            t.line("// a `k` this design has no function for, or a parent with another number of params, is NaN");
            t.line("match k {");
            t.indented(|t| {
                for (k, count) in &arms {
                    t.line(&format!("{k} if params.len() == {count} => {}(&params),", Text::generated(&format!("derive_{k}"))));
                }
                t.line("_ => f64::NAN,");
            });
            t.line("}");
        });
        t.line("}");
        Ok(Some(t.finish()))
    }

    /// `derive_<k>_` of `binding`, and the number of params of the parent it reads by position.
    fn function(ir: &Ir, binding: &Binding) -> Result<(String, usize), Error> {
        let parent = ir.instances().find(|i| i.path() == binding.parent).map(|i| i.body());
        let Some(parent) = parent else {
            return Err(Error::Unsupported { form: "a derived param whose parent is no instance".into(), path: binding.child.clone(), origin: ir.top().body().origin().clone() });
        };
        let printer = ValuePrinter::of_placement(ir, parent);
        let value = printer.expr_as(binding.expr, &binding.ty)?;
        let params = Text::generated("params");
        let at = parent.origin();
        let unsupported = |form: String| Error::Unsupported { form, path: format!("{}.{}", binding.child, binding.param), origin: at.clone() };
        let mut lets = Vec::new();
        for (i, p) in parent.params().iter().enumerate() {
            let name = Text::ident(&p.name);
            if !printer.uses(&name) {
                continue;
            }
            lets.push(match p.ty {
                ValueType::F64 => format!("let {name} = {params}[{i}];"),
                ValueType::Bool => format!("let {name} = {params}[{i}] != 0.0;"),
                _ => return Err(unsupported(format!("a derived param that reads the integer param `{}`", p.name))),
            });
        }
        let tail = match binding.ty {
            ValueType::F64 => value,
            ValueType::Bool => format!("if {value} {{ 1.0 }} else {{ 0.0 }}"),
            _ => return Err(unsupported(format!("a derived param of type {}", binding.ty))),
        };
        let mut t = Text::new();
        t.comment(&format!("derive {}: {}.{} from the params of {}", binding.k, binding.child, binding.param, binding.parent), at);
        t.line(&format!("fn {}({params}: &[f64]) -> f64 {{", Text::generated(&format!("derive_{}", binding.k))));
        t.indented(|t| {
            for line in &lets {
                t.line(line);
            }
            t.line(&tail);
        });
        t.line("}");
        Ok((t.finish(), parent.params().len()))
    }
}
