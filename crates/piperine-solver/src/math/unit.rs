#![allow(dead_code)]
//! SI unit type aliases.
//!
//! These are documentation-only aliases over `f64`: they make solver
//! signatures read at a glance (`Ohm`, `Siemens`, `Second`) without introducing
//! any runtime cost or conversion. PHDL delivers already-normalized SI values
//! from the compiler, so the solver never scales units itself — there is no
//! `UnitExt`-style prefix helper here on purpose.

pub type Volt = f64;
pub type Ampere = f64;
pub type Ohm = f64;
pub type Farad = f64;
pub type Henry = f64;
pub type Hertz = f64;
pub type Meter = f64;
pub type MeterPerSecond = f64;
pub type Siemens = f64;
pub type Kelvin = f64;
pub type Celsius = f64;
pub type InvCelsius = f64;
pub type InvCelsiusSquared = f64;
pub type Radian = f64;
pub type Second = f64;
pub type AmpereSquaredSecond = f64;
pub type Dimensionless = f64;
pub type Coulomb = f64;
pub type Joule = f64;
pub type JoulePerKelvin = f64;
pub type JouleSecond = f64;
