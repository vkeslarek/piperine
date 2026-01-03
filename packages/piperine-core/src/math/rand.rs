use crate::math::param::Parameter;
use faer::rand;
use rand_distr::{Distribution as _, Normal, Uniform};
use std::marker::PhantomData;
use std::ops::RangeInclusive;
use uom::si::{Dimension, Quantity, Units};

#[derive(Clone, Copy, Debug)]
pub enum Distribution {
    Uniform { lower: f64, upper: f64 },
    Gaussian { mean: f64, std_dev: f64 },
    RelativeUniform { nominal: f64, tolerance: f64 },
}

impl Distribution {
    pub fn sample(&self) -> f64 {
        let mut rng = rand::rng();
        match self {
            Distribution::Uniform { lower, upper } => {
                let dist =
                    Uniform::new(lower, upper).expect("Lower limit must be less than upper limit");
                dist.sample(&mut rng)
            }
            Distribution::Gaussian { mean, std_dev } => {
                let dist =
                    Normal::new(*mean, *std_dev).expect("Standard deviation must be positive");
                dist.sample(&mut rng)
            }
            Distribution::RelativeUniform { nominal, tolerance } => {
                let limit = *nominal * *tolerance;
                let dist = Uniform::new(*nominal - limit, *nominal + limit)
                    .expect("Tolerance must be positive");
                dist.sample(&mut rng)
            }
        }
    }
}

pub trait ParameterRangeExt<Q> {
    fn uniform(self) -> Parameter<Q>;
    fn gaussian(self, sigma: f64) -> Parameter<Q>;
}

impl<D, S> ParameterRangeExt<Quantity<D, S, f64>> for RangeInclusive<Quantity<D, S, f64>>
where
    D: Dimension + ?Sized,
    S: Units<f64> + ?Sized,
{
    fn uniform(self) -> Parameter<Quantity<D, S, f64>> {
        Parameter::Stochastic(
            Distribution::Uniform {
                lower: self.start().value,
                upper: self.end().value,
            },
            PhantomData,
        )
    }

    fn gaussian(self, sigma: f64) -> Parameter<Quantity<D, S, f64>> {
        let mean = (self.start().value + self.end().value) / 2.0;
        Parameter::Stochastic(
            Distribution::Gaussian {
                mean,
                std_dev: sigma,
            },
            PhantomData,
        )
    }
}

pub trait ParameterRelativeExt<Q> {
    fn pom(self, tolerance: f64) -> Parameter<Q>;
}

impl<D, S> ParameterRelativeExt<Quantity<D, S, f64>> for Quantity<D, S, f64>
where
    D: Dimension + ?Sized,
    S: Units<f64> + ?Sized,
{
    fn pom(self, tolerance: f64) -> Parameter<Quantity<D, S, f64>> {
        Parameter::Stochastic(
            Distribution::RelativeUniform {
                nominal: self.value,
                tolerance,
            },
            PhantomData,
        )
    }
}
