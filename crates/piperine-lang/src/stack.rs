//! The one stack-growing helper the recursive walkers of the crate share: a
//! flat chain of up to `MAX_CHAIN` links is a tree that deep, so the printer,
//! the POM lowering, the resolver and `to_ast` run each expression through
//! [`Stack::deep`], which moves onto a fresh heap segment when the stack runs low.

/// The stack-growing entry points; a unit type so the two walks read as `Stack::deep` and `Stack::wide`.
pub(crate) struct Stack;

impl Stack {
    /// Run `f`, on a new stack segment when less than the red zone remains.
    pub(crate) fn deep<R>(f: impl FnOnce() -> R) -> R {
        const RED_ZONE: usize = 128 * 1024;
        const SEGMENT: usize = 2 * 1024 * 1024;
        stacker::maybe_grow(RED_ZONE, SEGMENT, f)
    }

    /// [`Stack::deep`] for a walk whose steps are heavy, one lowering of a function body inside another: a
    /// larger red zone, so a step never outgrows what is left, and larger segments.
    pub(crate) fn wide<R>(f: impl FnOnce() -> R) -> R {
        const RED_ZONE: usize = 4 * 1024 * 1024;
        const SEGMENT: usize = 32 * 1024 * 1024;
        stacker::maybe_grow(RED_ZONE, SEGMENT, f)
    }
}
