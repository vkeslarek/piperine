#![allow(dead_code)]
//! Mathematical and physical constants (`PI`, `E`, Boltzmann, …) shared
//! by devices and kernels.
use num_complex::Complex64;
use std::f64::consts::{E as STD_E, PI as STD_PI};

/// π (3.14159...)
pub const PI: f64 = STD_PI;

/// e (Base of natural logarithms, 2.71828...)
pub const E: f64 = STD_E;

/// i (The square root of -1)
/// Note: This is Complex, unlike the others which are f64.
pub const I: Complex64 = Complex64::new(0.0, 1.0);

/// c (The speed of light in vacuum)
/// Value: 299,792,458 m/s
pub const SPEED_OF_LIGHT: f64 = 299_792_458.0;

/// kelvin (Absolute zero in Celsius)
/// Ngspice defines this as an offset: -273.15
pub const ABSOLUTE_ZERO_CELSIUS: f64 = -273.15;

/// q (The elementary charge of an electron)
/// Ngspice Value: 1.60219e-19 C
pub const ELEMENTARY_CHARGE: f64 = 1.602_19e-19;

/// k (Boltzmann’s constant)
/// Modern Value: 1.380649e-23 J/K
pub const BOLTZMANN_CONSTANT: f64 = 1.380_649e-23;

/// h (Planck’s constant)
/// Ngspice Value: 6.62607e-34 J s
pub const PLANCK_CONSTANT: f64 = 6.626_07e-34;
