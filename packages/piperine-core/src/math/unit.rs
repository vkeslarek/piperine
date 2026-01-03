use num_complex::Complex;
use paste::paste;
use std::f64::consts::PI;
use uom::si::{Quantity, ISQ, SI};
use uom::typenum::{N1, N2, N3, P1, P2, Z0};

// --- 1. Type Aliases ---
// Real-valued parameters (f64)
pub type Resistance = uom::si::f64::ElectricalResistance;
pub type Capacitance = uom::si::f64::Capacitance;
pub type Inductance = uom::si::f64::Inductance;
pub type Length = uom::si::f64::Length;
pub type Conductance = uom::si::f64::ElectricalConductance;
pub type Frequency = uom::si::f64::Frequency;
pub type Temperature = uom::si::f64::ThermodynamicTemperature;

// Complex-valued signals (complex64)
pub type Voltage = uom::si::complex64::ElectricPotential;
pub type Current = uom::si::complex64::ElectricCurrent;
pub type Impedance = uom::si::complex64::ElectricalResistance;
pub type Admittance = uom::si::complex64::ElectricalConductance;

// --- 2. The Metric Macro ---
// This macro creates the traits and implements them for f64 or Complex<f64>
macro_rules! impl_uom_metric {
    ($trait_name:ident, $uom_type:ty, $unit_module:ident, $base_unit:ident, $method_suffix:ident, $storage:ty) => {
        paste! {
            pub trait $trait_name {
                fn [< T $method_suffix >](self) -> $uom_type;
                fn [< G $method_suffix >](self) -> $uom_type;
                fn [< M $method_suffix >](self) -> $uom_type;
                fn [< k $method_suffix >](self) -> $uom_type;
                fn [<$method_suffix>](self) -> $uom_type;
                fn [< m $method_suffix >](self) -> $uom_type;
                fn [< u $method_suffix >](self) -> $uom_type;
                fn [< n $method_suffix >](self) -> $uom_type;
                fn [< p $method_suffix >](self) -> $uom_type;
                fn [< f $method_suffix >](self) -> $uom_type;
            }

            impl $trait_name for $storage {
                fn [< T $method_suffix >](self) -> $uom_type { $uom_type::new::<uom::si::$unit_module::$base_unit>(self * 1e12) }
                fn [< G $method_suffix >](self) -> $uom_type { $uom_type::new::<uom::si::$unit_module::$base_unit>(self * 1e9) }
                fn [< M $method_suffix >](self) -> $uom_type { $uom_type::new::<uom::si::$unit_module::$base_unit>(self * 1e6) }
                fn [< k $method_suffix >](self) -> $uom_type { $uom_type::new::<uom::si::$unit_module::$base_unit>(self * 1e3) }
                fn [<$method_suffix>](self) -> $uom_type { $uom_type::new::<uom::si::$unit_module::$base_unit>(self) }
                fn [< m $method_suffix >](self) -> $uom_type { $uom_type::new::<uom::si::$unit_module::$base_unit>(self * 1e-3) }
                fn [< u $method_suffix >](self) -> $uom_type { $uom_type::new::<uom::si::$unit_module::$base_unit>(self * 1e-6) }
                fn [< n $method_suffix >](self) -> $uom_type { $uom_type::new::<uom::si::$unit_module::$base_unit>(self * 1e-9) }
                fn [< p $method_suffix >](self) -> $uom_type { $uom_type::new::<uom::si::$unit_module::$base_unit>(self * 1e-12) }
                fn [< f $method_suffix >](self) -> $uom_type { $uom_type::new::<uom::si::$unit_module::$base_unit>(self * 1e-15) }
            }
        }
    };
}

// --- 3. Applying the Macros ---
pub type Ratio = uom::si::f64::Ratio;
// Real Extensions (f64)
impl_uom_metric!(
    VoltsExt,
    uom::si::f64::ElectricPotential,
    electric_potential,
    volt,
    V,
    f64
);
impl_uom_metric!(
    AmperesExt,
    uom::si::f64::ElectricCurrent,
    electric_current,
    ampere,
    A,
    f64
);
impl_uom_metric!(OhmsExt, Resistance, electrical_resistance, ohm, Ohms, f64);
impl_uom_metric!(FaradsExt, Capacitance, capacitance, farad, F, f64);
impl_uom_metric!(HenrysExt, Inductance, inductance, henry, H, f64);
impl_uom_metric!(HerzExt, Frequency, frequency, hertz, Hz, f64);
impl_uom_metric!(MetersExt, Length, length, meter, m, f64);
impl_uom_metric!(
    ConductanceExt,
    Conductance,
    electrical_conductance,
    siemens,
    S,
    f64
);

// Complex Extensions (Complex<f64>)
impl_uom_metric!(
    ComplexVoltsExt,
    Voltage,
    electric_potential,
    volt,
    V,
    Complex<f64>
);
impl_uom_metric!(
    ComplexAmperesExt,
    Current,
    electric_current,
    ampere,
    A,
    Complex<f64>
);
impl_uom_metric!(
    ComplexOhmsExt,
    Impedance,
    electrical_resistance,
    ohm,
    Ohms,
    Complex<f64>
);

// Ohms/C: [L:2, M:1, T:-3, I:-2, Th:-1]
pub type OhmsPerCelsius = Quantity<ISQ<P2, P1, N3, N2, N1, Z0, Z0>, SI<f64>, f64>;

// Ohms/C^2: [L:2, M:1, T:-3, I:-2, Th:-2]
pub type OhmsPerCelsiusSquared = Quantity<ISQ<P2, P1, N3, N2, N2, Z0, Z0>, SI<f64>, f64>;

// Ohms/m: [L:1, M:1, T:-3, I:-2, Th:0] (Resistance divided by Length)
pub type OhmsPerMeter = Quantity<ISQ<P1, P1, N3, N2, Z0, Z0, Z0>, SI<f64>, f64>;

// Ohms/m^2: [L:0, M:1, T:-3, I:-2, Th:0] (Sheet Resistance / Resistivity context)
pub type OhmsPerMeterSquared = Quantity<ISQ<Z0, P1, N3, N2, Z0, Z0, Z0>, SI<f64>, f64>;

// --- 4. Custom/Non-Metric Units ---
pub trait DimensionLessExt {
    fn ratio(self) -> Ratio;
}

impl DimensionLessExt for f64 {
    fn ratio(self) -> Ratio {
        uom::si::f64::Ratio::new::<uom::si::ratio::ratio>(self)
    }
}

pub trait SecondsExt {
    fn Week(self) -> uom::si::f64::Time;
    fn Day(self) -> uom::si::f64::Time;
    fn Hour(self) -> uom::si::f64::Time;
    fn Min(self) -> uom::si::f64::Time;
    fn Sec(self) -> uom::si::f64::Time;
    fn mSec(self) -> uom::si::f64::Time;
    fn uSec(self) -> uom::si::f64::Time;
}

impl SecondsExt for f64 {
    fn Week(self) -> uom::si::f64::Time {
        uom::si::f64::Time::new::<uom::si::time::day>(self * 7.0)
    }
    fn Day(self) -> uom::si::f64::Time {
        uom::si::f64::Time::new::<uom::si::time::day>(self)
    }
    fn Hour(self) -> uom::si::f64::Time {
        uom::si::f64::Time::new::<uom::si::time::hour>(self)
    }
    fn Min(self) -> uom::si::f64::Time {
        uom::si::f64::Time::new::<uom::si::time::minute>(self)
    }
    fn Sec(self) -> uom::si::f64::Time {
        uom::si::f64::Time::new::<uom::si::time::second>(self)
    }
    fn mSec(self) -> uom::si::f64::Time {
        uom::si::f64::Time::new::<uom::si::time::millisecond>(self)
    }
    fn uSec(self) -> uom::si::f64::Time {
        uom::si::f64::Time::new::<uom::si::time::microsecond>(self)
    }
}

pub trait TemperatureExt {
    fn degC(self) -> Temperature;
}
impl TemperatureExt for f64 {
    fn degC(self) -> Temperature {
        Temperature::new::<uom::si::thermodynamic_temperature::degree_celsius>(self)
    }
}

pub trait TempCoeffExt {
    fn OhmsPerC(self) -> OhmsPerCelsius;
    fn OhmsPerC2(self) -> OhmsPerCelsiusSquared;
}

impl TempCoeffExt for f64 {
    fn OhmsPerC(self) -> OhmsPerCelsius {
        // We use the direct struct initialization to bypass missing 'new' methods
        // on custom Dimension types.
        OhmsPerCelsius {
            dimension: std::marker::PhantomData,
            units: std::marker::PhantomData,
            value: self,
        }
    }

    fn OhmsPerC2(self) -> OhmsPerCelsiusSquared {
        OhmsPerCelsiusSquared {
            dimension: std::marker::PhantomData,
            units: std::marker::PhantomData,
            value: self,
        }
    }
}

// --- 5. Phase-Specific Conversions (Reactive Elements) ---
pub trait ReactanceConvert {
    fn to_impedance(self, freq: Frequency) -> Impedance;
}

impl ReactanceConvert for Inductance {
    fn to_impedance(self, freq: Frequency) -> Impedance {
        let val = 2.0
            * PI
            * freq.get::<uom::si::frequency::hertz>()
            * self.get::<uom::si::inductance::henry>();
        Impedance::new::<uom::si::electrical_resistance::ohm>(Complex::new(0.0, val))
    }
}

impl ReactanceConvert for Capacitance {
    fn to_impedance(self, freq: Frequency) -> Impedance {
        let val = -1.0
            / (2.0
                * PI
                * freq.get::<uom::si::frequency::hertz>()
                * self.get::<uom::si::capacitance::farad>());
        Impedance::new::<uom::si::electrical_resistance::ohm>(Complex::new(0.0, val))
    }
}

pub trait AdmittanceConvert {
    fn to_admittance(self) -> Admittance;
}

impl AdmittanceConvert for Impedance {
    fn to_admittance(self) -> Admittance {
        let z_raw = self.value;
        let y_raw = z_raw.inv();

        Admittance::new::<uom::si::electrical_conductance::siemens>(y_raw)
    }
}

impl AdmittanceConvert for uom::si::f64::ElectricalConductance {
    fn to_admittance(self) -> Admittance {
        let val = Complex::new(self.get::<uom::si::electrical_conductance::siemens>(), 0.0);
        Admittance::new::<uom::si::electrical_conductance::siemens>(val)
    }
}
