//! The parity of the solver with ngspice and between the two backends: operating point, DC, AC, noise, transient, temperature, transfer function, pole-zero, sensitivity and periodic steady state, the print grid, the corpus guard, the deviation table and the backend a twin runs on. Each former target is a module of this one.

mod ac;
mod backend;
mod backends;
mod corpus;
mod dc;
mod deviation;
mod grid;
mod noise;
mod op;
mod pss;
mod pz;
mod sensitivity;
mod temp;
mod tf;
mod tran;
