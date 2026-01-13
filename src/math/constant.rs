use crate::math::unit::HeatCapacity;
use std::marker::PhantomData;
use uom::si::{SI, heat_capacity};

pub const BOLTZMANN_CONSTANT: HeatCapacity = HeatCapacity {
    dimension: PhantomData::<heat_capacity::Dimension>,
    units: PhantomData::<SI<f64>>,
    value: 1.380_649_E-23,
};
