use num_complex::Complex;
use paste::paste;
use std::f64::consts::PI;
use uom::si::{ISQ, Quantity, SI};
use uom::typenum::{N1, N2, N3, P1, Z0};

/*******************************************************
TYPE ALIASES -> Measurements
********************************************************/
// Real
pub type Resistance = uom::si::f64::ElectricalResistance;
pub type Capacitance = uom::si::f64::Capacitance;
pub type Inductance = uom::si::f64::Inductance;
pub type Length = uom::si::f64::Length;
pub type Conductance = uom::si::f64::ElectricalConductance;
pub type Frequency = uom::si::f64::Frequency;
pub type Angle = uom::si::f64::Angle;
pub type AngularVelocity = uom::si::f64::AngularVelocity;
pub type Temperature = uom::si::f64::ThermodynamicTemperature;
pub type TemperatureInterval = uom::si::f64::TemperatureInterval;
pub type Ratio = uom::si::f64::Ratio;
pub type Time = uom::si::f64::Time;
pub type Voltage = uom::si::f64::ElectricPotential;
pub type Current = uom::si::f64::ElectricCurrent;

// Complex
pub type ComplexVoltage = uom::si::f64::ElectricPotential;
pub type ComplexCurrent = uom::si::f64::ElectricCurrent;
pub type Impedance = uom::si::complex64::ElectricalResistance;
pub type Admittance = uom::si::complex64::ElectricalConductance;

/*******************************************************
TYPE ALIASES -> Custom Measurements
********************************************************/
// 1/C: [L:0, M:0, T:0, I:0, Th:-1]
pub type LinearTemperatureCoefficient = Quantity<ISQ<Z0, Z0, Z0, Z0, N1, Z0, Z0>, SI<f64>, f64>;

// 1/C^2: [L:0, M:0, T:0, I:0, Th:-2]
pub type QuadraticTemperatureCoefficient = Quantity<ISQ<Z0, Z0, Z0, Z0, N2, Z0, Z0>, SI<f64>, f64>;

// Ohms/m: [L:1, M:1, T:-3, I:-2, Th:0]
pub type LinearResistivity = Quantity<ISQ<P1, P1, N3, N2, Z0, Z0, Z0>, SI<f64>, f64>;

// Ohms/m^2: [L:0, M:1, T:-3, I:-2, Th:0] (Sheet Resistance / Resistivity context)
pub type SheetResistance = Quantity<ISQ<Z0, P1, N3, N2, Z0, Z0, Z0>, SI<f64>, f64>;

/*******************************************************
TYPE ALIASES -> Units
********************************************************/
pub type Volt = uom::si::electric_potential::volt;
pub type Ampere = uom::si::electric_current::ampere;
pub type Ohm = uom::si::electrical_resistance::ohm;
pub type Siemens = uom::si::electrical_conductance::siemens;
pub type Farad = uom::si::capacitance::farad;
pub type Henry = uom::si::inductance::henry;
pub type Hertz = uom::si::frequency::hertz;
pub type Meter = uom::si::length::meter;
pub type Unitless = uom::si::ratio::ratio;
pub type Second = uom::si::time::second;
pub type Minute = uom::si::time::minute;
pub type Hour = uom::si::time::hour;
pub type Day = uom::si::time::day;
pub type Celsius = uom::si::thermodynamic_temperature::degree_celsius;
pub type DeltaCelsius = uom::si::temperature_interval::degree_celsius;
pub type Kelvin = uom::si::thermodynamic_temperature::kelvin;
pub type DeltaKelvin = uom::si::temperature_interval::kelvin;
pub type Radian = uom::si::angle::radian;
pub type Degree = uom::si::angle::degree;
pub type RadianPerSecond = uom::si::angular_velocity::radian_per_second;

/*******************************************************
SCALE METHODS EXT
********************************************************/
macro_rules! def_unit_ext {
    ($meas_type:ty, $method_suffix:ident) => {
        paste! {
            fn [< T $method_suffix >](self) -> $meas_type;
            fn [< G $method_suffix >](self) -> $meas_type;
            fn [< M $method_suffix >](self) -> $meas_type;
            fn [< k $method_suffix >](self) -> $meas_type;
            fn [<$method_suffix>](self) -> $meas_type;
            fn [< m $method_suffix >](self) -> $meas_type;
            fn [< u $method_suffix >](self) -> $meas_type;
            fn [< n $method_suffix >](self) -> $meas_type;
            fn [< p $method_suffix >](self) -> $meas_type;
            fn [< f $method_suffix >](self) -> $meas_type;
        }
    };
}

macro_rules! impl_unit_ext {
    ($meas_type:ty, $method_suffix:ident, $unit:ident) => {
        paste! {
            fn [< T $method_suffix >](self) -> $meas_type { $meas_type::new::<$unit>((self * 1e12).into()) }
            fn [< G $method_suffix >](self) -> $meas_type { $meas_type::new::<$unit>((self * 1e9).into()) }
            fn [< M $method_suffix >](self) -> $meas_type { $meas_type::new::<$unit>((self * 1e6).into()) }
            fn [< k $method_suffix >](self) -> $meas_type { $meas_type::new::<$unit>((self * 1e3).into()) }
            fn [<$method_suffix>](self) -> $meas_type { $meas_type::new::<$unit>((self).into()) }
            fn [< m $method_suffix >](self) -> $meas_type { $meas_type::new::<$unit>((self * 1e-3).into()) }
            fn [< u $method_suffix >](self) -> $meas_type { $meas_type::new::<$unit>((self * 1e-6).into()) }
            fn [< n $method_suffix >](self) -> $meas_type { $meas_type::new::<$unit>((self * 1e-9).into()) }
            fn [< p $method_suffix >](self) -> $meas_type { $meas_type::new::<$unit>((self * 1e-12).into()) }
            fn [< f $method_suffix >](self) -> $meas_type { $meas_type::new::<$unit>((self * 1e-15).into()) }
        }
    };
}

pub trait UnitExt {
    def_unit_ext!(Voltage, V);
    def_unit_ext!(Current, A);
    def_unit_ext!(Resistance, Ohms);
    def_unit_ext!(Capacitance, F);
    def_unit_ext!(Inductance, H);
    def_unit_ext!(Frequency, Hz);
    def_unit_ext!(Length, m);
    def_unit_ext!(Conductance, S);
    def_unit_ext!(Temperature, K);

    // Custom ext methods
    fn ratio(self) -> Ratio;

    fn Week(self) -> Time;
    fn Day(self) -> Time;
    fn Hour(self) -> Time;
    fn Min(self) -> Time;
    fn Sec(self) -> Time;
    fn mSec(self) -> Time;
    fn uSec(self) -> Time;
    fn deg(self) -> Angle;

    fn deg_C(self) -> Temperature;
    fn delta_C(self) -> TemperatureInterval;

    fn delta_K(self) -> TemperatureInterval;

    fn inv_C(self) -> LinearTemperatureCoefficient;
    fn inv_C2(self) -> QuadraticTemperatureCoefficient;

    fn Ohms_per_meter(self) -> LinearResistivity;
    fn Ohms_per_meter2(self) -> SheetResistance;
}

impl UnitExt for f64 {
    impl_unit_ext!(Voltage, V, Volt);
    impl_unit_ext!(Current, A, Ampere);
    impl_unit_ext!(Resistance, Ohms, Ohm);
    impl_unit_ext!(Capacitance, F, Farad);
    impl_unit_ext!(Inductance, H, Henry);
    impl_unit_ext!(Frequency, Hz, Hertz);
    impl_unit_ext!(Length, m, Meter);
    impl_unit_ext!(Conductance, S, Siemens);
    impl_unit_ext!(Temperature, K, Kelvin);

    // Custom ext impls
    fn ratio(self) -> Ratio {
        Ratio::new::<Unitless>(self)
    }

    fn Week(self) -> Time {
        Time::new::<Day>(self * 7.0)
    }
    fn Day(self) -> Time {
        Time::new::<Day>(self)
    }
    fn Hour(self) -> Time {
        Time::new::<Hour>(self)
    }
    fn Min(self) -> Time {
        Time::new::<Minute>(self)
    }
    fn Sec(self) -> uom::si::f64::Time {
        Time::new::<Second>(self)
    }
    fn mSec(self) -> uom::si::f64::Time {
        Time::new::<Second>(self * 1e-3)
    }
    fn uSec(self) -> uom::si::f64::Time {
        Time::new::<Second>(self * 1e-6)
    }

    fn deg(self) -> Angle {
        Angle::new::<Degree>(self)
    }

    fn deg_C(self) -> Temperature {
        Temperature::new::<Celsius>(self)
    }

    fn delta_C(self) -> TemperatureInterval {
        TemperatureInterval::new::<DeltaCelsius>(self)
    }

    fn delta_K(self) -> TemperatureInterval {
        TemperatureInterval::new::<DeltaKelvin>(self)
    }

    fn inv_C(self) -> LinearTemperatureCoefficient {
        // We use the direct struct initialization to bypass missing 'new' methods
        // on custom Dimension types.
        LinearTemperatureCoefficient {
            dimension: std::marker::PhantomData,
            units: std::marker::PhantomData,
            value: self,
        }
    }
    fn inv_C2(self) -> QuadraticTemperatureCoefficient {
        QuadraticTemperatureCoefficient {
            dimension: std::marker::PhantomData,
            units: std::marker::PhantomData,
            value: self,
        }
    }

    fn Ohms_per_meter(self) -> LinearResistivity {
        LinearResistivity {
            dimension: std::marker::PhantomData,
            units: std::marker::PhantomData,
            value: self,
        }
    }
    fn Ohms_per_meter2(self) -> SheetResistance {
        SheetResistance {
            dimension: std::marker::PhantomData,
            units: std::marker::PhantomData,
            value: self,
        }
    }
}
