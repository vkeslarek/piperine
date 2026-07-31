//! The `Behavior` family: value from circuit state, with a closed-form
//! Jacobian. `table` (`table.rs`) is extracted first, as the design
//! precedent — a pure piecewise function whose derivative is free.

pub mod table;
