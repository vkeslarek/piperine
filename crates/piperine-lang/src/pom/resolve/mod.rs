//! Name resolution: the stages that run after lowering and write only into
//! resolution slots and the diagnostics list. Each stage computes its answers
//! from the immutable design, then applies them.

mod scopes;

use super::design::Design;

/// Run every resolution stage over a freshly lowered design.
pub(super) fn run(design: &mut Design) {
    let scopes = scopes::Scopes::build(design);
    let diagnostics = scopes.diagnostics(design);
    for (file, diagnostic) in diagnostics {
        design.push_diagnostic(file, diagnostic);
    }
}
