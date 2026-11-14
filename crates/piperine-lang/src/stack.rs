//! The one stack-growing helper the recursive walkers of the crate share: a
//! flat chain of up to `MAX_CHAIN` links is a tree that deep, so the printer,
//! the POM lowering, the resolver and `to_ast` run each expression through
//! [`deep`], which moves onto a fresh heap segment when the stack runs low.

/// Run `f`, on a new stack segment when less than the red zone remains.
pub(crate) fn deep<R>(f: impl FnOnce() -> R) -> R {
    const RED_ZONE: usize = 128 * 1024;
    const SEGMENT: usize = 2 * 1024 * 1024;
    stacker::maybe_grow(RED_ZONE, SEGMENT, f)
}
