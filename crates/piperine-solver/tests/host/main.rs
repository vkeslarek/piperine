//! The host surface of the solver: setting and writing values, the host's view of a run, live parameters,
//! probe selection, the bounds a session can set, the solver's entry points, `prelude` on its own, nature abstol
//! inheritance and bind. Each former target is a
//! module of this one.

mod bind;
mod host_set;
mod host_surface;
mod host_writes;
mod live_params;
mod nature_abstol_inheritance;
mod prelude_surface;
mod probe_selection;
mod session_bounds;
mod solver_entry;
