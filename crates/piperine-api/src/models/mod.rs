use crate::devices::Component;

pub mod bjt;
pub mod capacitor;
pub mod diode;
pub mod inductor;
pub mod jfet;
pub mod mosfet;
pub mod resistor;
pub mod switch;

pub trait Model {
    type ComponentType: Component;
}

