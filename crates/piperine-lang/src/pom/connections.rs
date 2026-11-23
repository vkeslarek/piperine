//! The arguments of a placement read against the ports of its module (part III §8.1.9, §8.3.7): positional ones
//! take the ports in order, a named one (`reset: seat`) the port its name resolves to.

use super::{Arg, DeclId, DeclKind, Design, Resolution};

/// The argument each port of a placed module is bound to, in the order of the ports.
pub(crate) struct PortArguments<'a> {
    /// A port and the argument that binds it, `None` where the placement leaves it out.
    bound: Vec<(DeclId, Option<&'a Arg>)>,
}

impl<'a> PortArguments<'a> {
    /// The ports of `module` bound to `args`: the unnamed arguments in order from the first port, each named one to the
    /// port it names. A name that resolves to no port binds nothing.
    pub(crate) fn bind(design: &Design, module: DeclId, args: &'a [Arg]) -> PortArguments<'a> {
        let mut bound: Vec<(DeclId, Option<&'a Arg>)> = design
            .decl_data(module)
            .children
            .iter()
            .copied()
            .filter(|child| matches!(design.decl_data(*child).kind, DeclKind::Port(_)))
            .map(|port| (port, None))
            .collect();
        let mut positional = args.iter().filter(|arg| arg.name.is_none());
        for (_, slot) in bound.iter_mut() {
            match positional.next() {
                Some(arg) => *slot = Some(arg),
                None => break,
            }
        }
        for arg in args.iter().filter(|arg| arg.name.is_some()) {
            let Resolution::Decl(port) = arg.res else { continue };
            if let Some((_, slot)) = bound.iter_mut().find(|(candidate, slot)| *candidate == port && slot.is_none()) {
                *slot = Some(arg);
            }
        }
        PortArguments { bound }
    }

    /// The argument bound to the port at `index`.
    pub(crate) fn at(&self, index: usize) -> Option<&'a Arg> {
        self.bound.get(index).and_then(|(_, arg)| *arg)
    }
}
