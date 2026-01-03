use crate::math::rand::Distribution;
use std::marker::PhantomData;
use uom::si::Quantity;

use uom::si::{Dimension, Units};

pub enum Parameter<Q> {
    Fixed(Q),
    Stochastic(Distribution, PhantomData<Q>),
}

// Logic to sample a UOM Quantity
impl<D, U> Parameter<Quantity<D, U, f64>>
where
    D: Dimension + ?Sized,
    U: Units<f64> + ?Sized,
{
    pub fn sample(&self) -> Quantity<D, U, f64> {
        match self {
            Parameter::Fixed(q) => q.clone(),
            Parameter::Stochastic(dist, _) => Quantity {
                dimension: PhantomData,
                units: PhantomData,
                value: dist.sample(),
            },
        }
    }
}

// 1. Define the traits you OWN
pub trait IntoParameter<Q> {
    fn into_parameter(self) -> Parameter<Q>;
}

pub trait IntoOptionalParameter<Q> {
    fn into_optional_parameter(self) -> Option<Parameter<Q>>;
}

// 2. Implement them for the Quantity itself (The "Direct" case)
impl<Q> IntoParameter<Q> for Q {
    fn into_parameter(self) -> Parameter<Q> {
        Parameter::Fixed(self)
    }
}

impl<Q> IntoOptionalParameter<Q> for Q {
    fn into_optional_parameter(self) -> Option<Parameter<Q>> {
        Some(Parameter::Fixed(self))
    }
}

// 3. Implement them for the Parameter itself (The "Already a Parameter" case)
impl<Q> IntoParameter<Q> for Parameter<Q> {
    fn into_parameter(self) -> Parameter<Q> {
        self
    }
}

impl<Q> IntoOptionalParameter<Q> for Parameter<Q> {
    fn into_optional_parameter(self) -> Option<Parameter<Q>> {
        Some(self)
    }
}

// 4. Handle the "Option" case for OptionalParameter
impl<Q> IntoOptionalParameter<Q> for Option<Parameter<Q>> {
    fn into_optional_parameter(self) -> Option<Parameter<Q>> {
        self
    }
}

// --- Traits for the CircuitBuilder API ---

pub type OptionalParameter<Q> = Option<Parameter<Q>>;

// Support direct conversion from a uom Quantity to a Parameter
impl<Q> From<Q> for Parameter<Q> {
    fn from(q: Q) -> Self {
        Parameter::Fixed(q)
    }
}

pub trait SampleOptional<Q> {
    fn sample_opt(&self) -> Option<Q>;
}

impl<D, U> Parameter<Quantity<D, U, f64>>
where
    D: Dimension + ?Sized,
    U: Units<f64> + ?Sized,
{
}
// Specific implementation for UOM Quantities
impl<D, U> SampleOptional<Quantity<D, U, f64>> for OptionalParameter<Quantity<D, U, f64>>
where
    D: Dimension + ?Sized,
    U: Units<f64> + ?Sized,
{
    fn sample_opt(&self) -> Option<Quantity<D, U, f64>> {
        self.as_ref().map(|param| param.sample())
    }
}
