//! A placement of a generic module (generics GN-13, GN-16, part III §8, part I §3.8.5 and §3.8.7): the generic arguments
//! written with `::<..>`, else inferred from what the placement connects, then the defaults. A nature parameter is
//! given by the nature of the nodes and carriers connected to the ports typed by it (`Short(a, b)`), a type parameter
//! by the value a `Bit<T>` carrier holds or a value input has, a const parameter by the length of an array connected.
//! Two natures for one nature parameter are `E0180` naming both (DC4); a parameter nothing gives is `E0181`.

use crate::pom::{Arg, DeclId, DeclKind, ExprId, FieldInit, FileId, Resolution, TypeId};

use super::check::Checker;
use super::inference::Inference;
use super::nature_rules::NatureRules;
use super::{GenericSignature, GenericValue, Instantiation, Nominal, Substitution, Ty};

impl Checker<'_> {
    /// The substitution the placement `expr` of the generic module `module` makes, recorded as its instantiation
    /// (`Types::instantiation`); `None` after a diagnostic.
    pub(super) fn generic_placement(&mut self, expr: ExprId, callee: ExprId, module: DeclId, args: &[Arg], fields: &[FieldInit]) -> Option<Substitution> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        let signature = self.generic_signature(module);
        let declared = self.module_port_types(module)?;
        let mut inference = Inference::new(&signature);
        self.explicit_arguments(&mut inference, &signature, callee, module, file, span)?;
        self.learn_from_connections(&mut inference, &declared, args, file)?;
        self.learn_from_templates(&mut inference, module, fields)?;
        self.record_module_instantiation(expr, module, &signature, inference)
    }

    /// The instantiation a template `expr` of the generic module `module` makes (§8.4.3): the arguments written on its head,
    /// `Name::<G> { .. }`, else the ones its param values give, then the defaults; recorded as the template's instantiation. `None`
    /// after a diagnostic, `E0181` for an argument nothing gives.
    pub(super) fn template_instantiation(&mut self, expr: ExprId, module: DeclId, fields: &[FieldInit], expected: Option<&Nominal>) -> Option<Substitution> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        let signature = self.generic_signature(module);
        let mut inference = Inference::new(&signature);
        self.explicit_arguments(&mut inference, &signature, expr, module, file, span)?;
        // `const T: G<2> = G { .. }`: an argument the head leaves out is the one the expected type names
        if let Some(expected) = expected {
            for (index, parameter) in signature.parameters().iter().enumerate() {
                if inference.found().get(parameter.decl).is_none()
                    && let Some(argument) = expected.arguments.get(index)
                {
                    inference.give(index, argument.clone());
                }
            }
        }
        self.learn_from_templates(&mut inference, module, fields)?;
        self.record_module_instantiation(expr, module, &signature, inference)
    }

    /// Completes `inference` into the substitution of the generic module `module`, its bounds checked, and records the arguments as the
    /// instantiation of `expr` (a placement or a template).
    fn record_module_instantiation(&mut self, expr: ExprId, module: DeclId, signature: &GenericSignature, mut inference: Inference<'_>) -> Option<Substitution> {
        let file = self.file_of_expr(expr);
        let span = self.state.design.expr_node(expr).span;
        let substitution = self.complete_arguments(&mut inference, signature, module, module, file, span)?;
        let arguments: Vec<GenericValue> = signature.parameters().iter().filter_map(|parameter| substitution.get(parameter.decl).cloned()).collect();
        self.arguments_satisfy_bounds(signature, &arguments, module, file, span)?;
        self.state.result.instantiations.insert(expr, Instantiation { item: module, arguments });
        Some(substitution)
    }

    /// The arguments of the generic module `module` that the type of the template value `callee` names (`G<2>`, §8.4.3), as the
    /// substitution its ports and result are read under.
    pub(super) fn template_substitution(&mut self, callee: ExprId, module: DeclId) -> Substitution {
        let mut substitution = Substitution::new();
        let Some(Ty::Module(template)) = self.state.result.exprs[callee.index()].clone() else { return substitution };
        for (parameter, argument) in self.generic_signature(module).parameters().iter().zip(template.arguments) {
            substitution.bind(parameter.decl, argument);
        }
        substitution
    }

    /// The type of each port of `module` as it is declared, its generic parameters opaque; `None` when one is refused.
    pub(super) fn module_port_types(&mut self, module: DeclId) -> Option<Vec<Ty>> {
        let ports: Vec<(DeclId, TypeId)> = self.state.design.decl_data(module).children.iter().filter_map(|child| match &self.state.design.decl_data(*child).kind {
            DeclKind::Port(port) => Some((*child, port.ty)),
            _ => None,
        }).collect();
        let outer = std::mem::replace(&mut self.state.generics.generic_item, Some(module));
        // a bundle port is typed as its `struct` with the arguments it is written with (`Stream<T>`), an array of bundles as the
        // array of it (`[Lane; N]`), so a connection can teach `T` and `N`
        let types: Option<Vec<Ty>> = ports.iter().map(|(port, ty)| if self.state.design.is_bundle_owner(*port) { self.bundle_type(*ty) } else { self.port_type(*ty) }).collect();
        self.state.generics.generic_item = outer;
        types
    }

    /// Learns the open parameters from each connection that meets a port: the nodes' and carriers' natures, the values
    /// carriers hold, the lengths of arrays. A nature parameter met by two natures that do not agree is `E0180`.
    fn learn_from_connections(&mut self, inference: &mut Inference<'_>, declared: &[Ty], args: &[Arg], file: FileId) -> Option<()> {
        let mut natures: Vec<(DeclId, DeclId)> = Vec::new();
        for (arg, port) in args.iter().zip(declared) {
            let given = self.declared_port(arg.value).map(|(_, ty)| ty).or_else(|| self.bundle_named_type(arg.value)).or_else(|| self.expressions().infer(arg.value));
            // `gnd` is a node of any nature: it names none, so it teaches nothing
            let Some(given) = given.filter(|ty| *ty != Ty::AnyNode) else { continue };
            let declared_parameter = match port.innermost() {
                Ty::Param(parameter) => Some(parameter),
                Ty::Carrier { nature, .. } if matches!(self.state.design.decl_data(*nature).kind, DeclKind::GenericParam(_)) => Some(nature),
                _ => None,
            };
            if let (Some(parameter), Ty::Node(nature) | Ty::Carrier { nature, .. }) = (declared_parameter, given.innermost()) {
                match natures.iter().find(|(known, _)| known == parameter) {
                    Some((_, first)) if !NatureRules::compatible(self.state.design, *first, *nature) => {
                        let span = self.state.design.expr_node(arg.value).span;
                        self.mismatch(file, span, &Ty::Node(*first), &Ty::Node(*nature));
                        return None;
                    }
                    Some(_) => continue,
                    None => natures.push((*parameter, *nature)),
                }
            }
            inference.unify(port, &given);
        }
        Some(())
    }

    /// Learns a parameter bound by `Fn(P)` from the template a param block binds to a param of its type
    /// (`MemoryBank(wl, bl) { cell: Dram1T1C { cs: 30f } }` gives `C = Dram1T1C`, §8.4.1).
    fn learn_from_templates(&mut self, inference: &mut Inference<'_>, module: DeclId, fields: &[FieldInit]) -> Option<()> {
        for field in fields {
            let Resolution::Decl(param) = field.res else { continue };
            let DeclKind::Param(data) = &self.state.design.decl_data(param).kind else { continue };
            let outer = std::mem::replace(&mut self.state.generics.generic_item, Some(module));
            let declared = self.lower_type(data.ty);
            self.state.generics.generic_item = outer;
            let Some(declared @ Ty::Param(parameter)) = declared else { continue };
            if !self.bounds_of(parameter).iter().any(|bound| matches!(bound, super::Bound::Callable { .. })) {
                continue;
            }
            // a template that did not type was refused where it is written: nothing is left to infer from it
            let given = self.expressions().infer(field.value)?;
            inference.unify(&declared, &given);
        }
        Some(())
    }

    /// The type of a port as a placement sees it: its declared type under the arguments the placement gives.
    pub(super) fn port_type_under_placement(&mut self, ty: TypeId) -> Option<Ty> {
        let declared = match &self.state.generics.placement_substitution {
            Some(_) => {
                let outer = std::mem::replace(&mut self.state.generics.generic_item, self.state.modules.placing.last().copied().flatten());
                let declared = self.port_type(ty);
                self.state.generics.generic_item = outer;
                declared
            }
            None => self.port_type(ty),
        };
        match (&self.state.generics.placement_substitution, declared) {
            (Some(substitution), Some(declared)) => Some(substitution.apply(&declared)),
            (_, declared) => declared,
        }
    }
}
