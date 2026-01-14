use crate::math::rand::Distribution;
use num_complex::Complex;
use std::marker::PhantomData;

pub enum Parameter<Q> {
    Fixed(Q),
    Stochastic(Distribution, PhantomData<Q>),
}

// Logic to sample a UOM Quantity
impl Parameter<f64> {
    pub fn sample(&self) -> f64 {
        match self {
            Parameter::Fixed(q) => q.clone(),
            Parameter::Stochastic(dist, _) => dist.sample(),
        }
    }
}

impl Parameter<Complex<f64>> {
    pub fn sample(&self) -> Complex<f64> {
        match self {
            Parameter::Fixed(q) => q.clone(),
            Parameter::Stochastic(dist, _) => Complex::new(dist.sample(), 0.0),
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
