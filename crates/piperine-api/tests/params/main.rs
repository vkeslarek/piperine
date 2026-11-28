//! The session options and parameters of a run: the keys of `reltol`, `abstol`, `temp` and the rest, their reach into every analysis, the session parameters a design reads with `simparam`, and the keys and faults of the noise request. Each former target is a module of this one.

mod noise;
mod option_reach;
mod options;
mod session_parameters;
