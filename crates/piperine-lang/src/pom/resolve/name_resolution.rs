//! The whole of name resolution: the stages that run after lowering, in order.

use super::super::design::Design;
use super::behaviors::BehaviorBinding;
use super::classify::Classifier;
use super::leaves::LeafResolution;
use super::members::MemberResolution;
use super::names::Names;
use super::shape::DeclarationShapeCheck;
use super::{Ctx, Output};

/// Name resolution over one freshly lowered design.
pub(in super::super) struct NameResolution<'design> {
    design: &'design mut Design,
}

impl<'design> NameResolution<'design> {
    /// The resolution that writes into `design`.
    pub(in super::super) fn new(design: &'design mut Design) -> Self {
        NameResolution { design }
    }

    /// Run every resolution stage over the freshly lowered design.
    pub(in super::super) fn run(&mut self) {
        let design = &mut *self.design;
        design.fill_parents();
        let out = {
            let mut ctx = Ctx::new(design);
            let mut out = Output::default();
            out.diagnostics.extend(ctx.scopes.diagnostics(design));
            ctx.resolve_imports(&mut out);
            ctx.links = BehaviorBinding::new(&ctx).bind(&mut out);
            ctx.impls = ctx.collect_impls();
            Names::resolve_all(&ctx, &mut out);
            out.links = ctx.links;
            out
        };
        for (path, index, res) in out.segments {
            design.set_segment_res(path, index, res);
        }
        for (pat, res) in out.pats {
            design.set_pat_res(pat, res);
        }
        for (behavior, module) in out.links {
            design.bind_behavior(behavior, module);
        }
        for (file, diagnostic) in out.diagnostics {
            design.push_diagnostic(file, diagnostic);
        }
        let classified = Classifier::new(design).run();
        for (port, kind) in classified.port_kinds {
            design.set_port_kind(port, kind);
        }
        for (decl, bundle) in classified.bundles {
            design.set_bundle(decl, bundle);
        }
        for facts in classified.structs {
            design.set_struct(facts.expr, facts.class, &facts.fields);
        }
        for (pat, index, res) in classified.pattern_fields {
            design.set_pattern_field(pat, index, res);
        }
        for facts in classified.calls {
            design.set_call(facts.expr, facts.class, &facts.args, &facts.params);
        }
        for (path, index, res) in classified.segments {
            design.set_segment_res(path, index, res);
        }
        for (module, expr) in classified.instances {
            design.push_instance(module, expr);
        }
        for (file, diagnostic) in classified.diagnostics {
            design.push_diagnostic(file, diagnostic);
        }
        LeafResolution::new(design).run();
        // A fresh context over the resolved design: stage 7 asks it which traits are in scope.
        let members = {
            let ctx = Ctx::new(design);
            MemberResolution::new(design, &|from, tr| ctx.trait_in_scope(from, tr)).resolve()
        };
        for (expr, res) in members.res {
            design.set_member(expr, res);
        }
        for (path, index, res) in members.segments {
            design.set_segment_res(path, index, res);
        }
        for (file, diagnostic) in members.diagnostics {
            design.push_diagnostic(file, diagnostic);
        }
        for (file, diagnostic) in DeclarationShapeCheck::new(design).run() {
            design.push_diagnostic(file, diagnostic);
        }
        design.sort_diagnostics();
        design.index = super::super::index::IndexBuilder::new(design).build();
    }
}
