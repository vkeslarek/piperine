use crate::devices::Component;

pub mod bjt;
pub mod capacitor;
pub mod cpl;
pub mod diode;
pub mod inductor;
pub mod jfet;
pub mod ltra;
pub mod mesfet;
pub mod mosfet;
pub mod resistor;
pub mod switch;
pub mod txl;
pub mod urc;
pub mod vdmos;

pub trait Model {
    type ComponentType: Component;
}
