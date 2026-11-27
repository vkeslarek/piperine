//! The numerics of the analog crate: the power-law integral of a noise density (ngspice's quadrature)
//! against closed forms, and the cost of an assembly (an evaluation at another iterate allocates nothing).
//! Each former target is a module of this one.

mod allocation;
mod noise_numerics;
