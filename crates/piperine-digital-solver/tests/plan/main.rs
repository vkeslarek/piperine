//! The plan of the digital crate: the discrete half of bind (one writer per net, equal layouts, the
//! pins), the levelized order of the discrete functions, and array nets. Each former target is a module
//! of this one.

mod array_nets;
mod bind;
mod dag;
