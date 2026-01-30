use crate::circuit::netlist::NodeIdentifier;
use crate::devices::{Component, Dynamic};
use crate::num::Scalar;
use crate::unit::{Ampere, Celsius, Dimensionless, Hertz, Volt};

#[derive(Clone, Debug)]
pub struct TableData {
    pub input: Dynamic<Dimensionless>,
    pub pairs: Vec<(f64, f64)>,
}

#[derive(Clone, Debug)]
pub struct LaplaceTransfer {
    pub input: Dynamic<Dimensionless>,
    pub s_domain_expr: String,
}

#[derive(Clone, Debug)]
pub enum FreqDomainRepresentation {
    MagDeg,
    MagRad,
    Decibel,
    RealImag,
}

#[derive(Clone, Debug)]
pub struct FrequencyResponse {
    pub input: Dynamic<Dimensionless>,
    pub representation: FreqDomainRepresentation,
    pub points: Vec<(Hertz, f64, f64)>,
}

#[derive(Clone, Debug)]
pub struct Polynomial {
    pub inputs: Vec<Dynamic<Dimensionless>>,
    pub coefficients: Vec<f64>,
}

#[derive(Clone, Debug)]
pub enum Behavior<T: Scalar> {
    Expression(Dynamic<T>),
    Table(TableData),
    Laplace(LaplaceTransfer),
    Frequency(FrequencyResponse),
    Poly(Polynomial),
}

impl<T: Scalar> From<Dynamic<T>> for Behavior<T> {
    fn from(d: Dynamic<T>) -> Self {
        Behavior::Expression(d)
    }
}

#[derive(Clone)]
pub struct BehavioralSource {
    name: String,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,

    voltage: Option<Behavior<Volt>>,
    current: Option<Behavior<Ampere>>,

    tc1: Option<Dimensionless>,
    tc2: Option<Dimensionless>,
    temp: Option<Celsius>,
    dtemp: Option<Celsius>,
}

impl BehavioralSource {
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<NodeIdentifier>,
        node_minus: impl Into<NodeIdentifier>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            voltage: None,
            current: None,
            tc1: None,
            tc2: None,
            temp: None,
            dtemp: None,
        }
    }

    pub fn with_voltage(&mut self, expr: impl Into<Behavior<Volt>>) -> &mut Self {
        self.voltage = Some(expr.into());
        self.current = None;
        self
    }

    pub fn with_current(&mut self, expr: impl Into<Behavior<Ampere>>) -> &mut Self {
        self.current = Some(expr.into());
        self.voltage = None;
        self
    }

    pub fn with_voltage_table(
        &mut self,
        input: impl Into<Dynamic<Dimensionless>>,
        pairs: Vec<(f64, f64)>,
    ) -> &mut Self {
        self.voltage = Some(Behavior::Table(TableData {
            input: input.into(),
            pairs,
        }));
        self.current = None;
        self
    }

    pub fn with_current_table(
        &mut self,
        input: impl Into<Dynamic<Dimensionless>>,
        pairs: Vec<(f64, f64)>,
    ) -> &mut Self {
        self.current = Some(Behavior::Table(TableData {
            input: input.into(),
            pairs,
        }));
        self.voltage = None;
        self
    }

    pub fn with_voltage_laplace(
        &mut self,
        input: impl Into<Dynamic<Dimensionless>>,
        s_expr: impl Into<String>,
    ) -> &mut Self {
        self.voltage = Some(Behavior::Laplace(LaplaceTransfer {
            input: input.into(),
            s_domain_expr: s_expr.into(),
        }));
        self.current = None;
        self
    }

    pub fn with_voltage_freq_response(
        &mut self,
        input: impl Into<Dynamic<Dimensionless>>,
        repr: FreqDomainRepresentation,
        points: Vec<(Hertz, f64, f64)>,
    ) -> &mut Self {
        self.voltage = Some(Behavior::Frequency(FrequencyResponse {
            input: input.into(),
            representation: repr,
            points,
        }));
        self.current = None;
        self
    }

    pub fn with_voltage_poly(
        &mut self,
        inputs: Vec<Dynamic<Dimensionless>>,
        coefficients: Vec<f64>,
    ) -> &mut Self {
        self.voltage = Some(Behavior::Poly(Polynomial {
            inputs,
            coefficients,
        }));
        self.current = None;
        self
    }

    pub fn with_current_poly(
        &mut self,
        inputs: Vec<Dynamic<Dimensionless>>,
        coefficients: Vec<f64>,
    ) -> &mut Self {
        self.current = Some(Behavior::Poly(Polynomial {
            inputs,
            coefficients,
        }));
        self.voltage = None;
        self
    }

    pub fn with_tc1(&mut self, tc1: impl Into<Dimensionless>) -> &mut Self {
        self.tc1 = Some(tc1.into());
        self
    }

    pub fn with_tc2(&mut self, tc2: impl Into<Dimensionless>) -> &mut Self {
        self.tc2 = Some(tc2.into());
        self
    }

    pub fn with_temp(&mut self, temp: impl Into<Celsius>) -> &mut Self {
        self.temp = Some(temp.into());
        self
    }

    pub fn with_dtemp(&mut self, dtemp: impl Into<Celsius>) -> &mut Self {
        self.dtemp = Some(dtemp.into());
        self
    }
}

impl Component for BehavioralSource {
    fn name(&self) -> &String {
        &self.name
    }
}
