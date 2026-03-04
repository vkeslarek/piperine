pub mod ask;
pub mod capacitor;
pub mod diode;
pub mod dynamic;
pub mod inductor;
pub mod resistor;
pub mod soa;
pub mod source;

use crate::analysis::ac::AcAnalysis;
use crate::analysis::dc::DcAnalysis;
use crate::analysis::noise::NoiseSource;
use crate::analysis::transient::TransientAnalysis;
use crate::analysis::truncation::TruncationError;
use crate::circuit::netlist::Netlist;
use crate::devices::ask::Ask;
use crate::devices::soa::SoaCheck;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::expression::Quantity;
use crate::math::num::Scalar;
use crate::solver::Context;
use crate::util::AsAny;
use num_complex::Complex;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

pub trait Component: Any + AsAny + Send + Sync {
    fn name(&self) -> String;

    fn runtime(&self, netlist: &mut Netlist) -> Box<dyn AnyRuntime>;

    fn terminals(&self) -> Vec<String> {
        vec![]
    }

    fn ask_dc(&self, _request: Ask, _solution: &CircularArrayBuffer2<f64>) -> Option<Quantity> {
        None
    }

    fn ask_ac(
        &self,
        _request: Ask,
        _solution: &CircularArrayBuffer2<Complex<f64>>,
    ) -> Option<Quantity> {
        None
    }
}

pub trait Model: Debug + AsAny + Any + Send + Sync {
    type ComponentType: Component;
}

pub trait AnyModel: 'static + AsAny {}

impl<M: 'static + Model> AnyModel for M {}

pub trait Runtime {
    type ComponentType: Component;

    fn allocate(component: Arc<Self::ComponentType>, netlist: &mut Netlist) -> Self
    where
        Self: Sized;

    fn update(&mut self, _: &CircularArrayBuffer2<f64>, _: &Context);

    fn as_dc(&self) -> Option<&dyn DcAnalysis> {
        None
    }

    fn as_ac(&self) -> Option<&dyn AcAnalysis> {
        None
    }

    fn as_transient(&self) -> Option<&dyn TransientAnalysis> {
        None
    }

    fn as_noise_source(&self) -> Option<&dyn NoiseSource> {
        None
    }

    fn as_soa_check(&self) -> Option<&dyn SoaCheck> {
        None
    }

    fn as_truncation_error(&self) -> Option<&dyn TruncationError> {
        None
    }
}

pub trait AnyRuntime {
    fn update(&mut self, state: &CircularArrayBuffer2<f64>, context: &Context);

    fn as_dc(&self) -> Option<&dyn DcAnalysis>;

    fn as_ac(&self) -> Option<&dyn AcAnalysis>;

    fn as_transient(&self) -> Option<&dyn TransientAnalysis>;

    fn as_noise_source(&self) -> Option<&dyn NoiseSource>;

    fn as_soa_check(&self) -> Option<&dyn SoaCheck>;

    fn as_truncation_error(&self) -> Option<&dyn TruncationError>;
}

impl<R: Runtime> AnyRuntime for R
where
    R: Sized,
{
    fn update(&mut self, state: &CircularArrayBuffer2<f64>, context: &Context) {
        R::update(self, state, context);
    }

    fn as_dc(&self) -> Option<&dyn DcAnalysis> {
        R::as_dc(self)
    }

    fn as_ac(&self) -> Option<&dyn AcAnalysis> {
        R::as_ac(self)
    }

    fn as_transient(&self) -> Option<&dyn TransientAnalysis> {
        R::as_transient(self)
    }

    fn as_noise_source(&self) -> Option<&dyn NoiseSource> {
        R::as_noise_source(self)
    }

    fn as_soa_check(&self) -> Option<&dyn SoaCheck> {
        R::as_soa_check(self)
    }

    fn as_truncation_error(&self) -> Option<&dyn TruncationError> {
        R::as_truncation_error(self)
    }
}
