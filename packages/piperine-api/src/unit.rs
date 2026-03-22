use paste::paste;

pub type Volt = f64;
pub type Ampere = f64;
pub type Ohm = f64;
pub type Farad = f64;
pub type FaradPerMeter = f64;
pub type FaradPerMeterSquared = f64;
pub type Henry = f64;
pub type Hertz = f64;
pub type Meter = f64;
pub type MeterSquared = f64;
pub type MeterPerSecond = f64;
pub type Siemens = f64;
pub type Kelvin = f64;
pub type Celsius = f64;
pub type InvCelsius = f64;
pub type InvCelsiusSquared = f64;
pub type Radian = f64;
pub type Second = f64;
pub type AmpereSquaredSecond = f64;
pub type Dimensionless = f64;
pub type Coulomb = f64;
pub type Joule = f64;
pub type JoulePerKelvin = f64;
pub type JouleSecond = f64;
pub type Watt = f64;

macro_rules! def_unit_ext {
    ($suffix:ident, $type:ty) => {
        paste! {
            fn [< T $suffix >](self) -> $type;
            fn [< G $suffix >](self) -> $type;
            fn [< M $suffix >](self) -> $type;
            fn [< k $suffix >](self) -> $type;
            fn [<$suffix>](self) -> $type;
            fn [< m $suffix >](self) -> $type;
            fn [< u $suffix >](self) -> $type;
            fn [< n $suffix >](self) -> $type;
            fn [< p $suffix >](self) -> $type;
            fn [< f $suffix >](self) -> $type;
        }
    };
}

macro_rules! impl_unit_ext {
    ($suffix:ident, $type:ty) => {
        paste! {
            #[inline(always)] fn [< T $suffix >](self) -> $type { self * 1e12 }
            #[inline(always)] fn [< G $suffix >](self) -> $type { self * 1e9 }
            #[inline(always)] fn [< M $suffix >](self) -> $type { self * 1e6 }
            #[inline(always)] fn [< k $suffix >](self) -> $type { self * 1e3 }

            #[inline(always)] fn [<$suffix>](self) -> $type { self }

            #[inline(always)] fn [< m $suffix >](self) -> $type { self * 1e-3 }
            #[inline(always)] fn [< u $suffix >](self) -> $type { self * 1e-6 }
            #[inline(always)] fn [< n $suffix >](self) -> $type { self * 1e-9 }
            #[inline(always)] fn [< p $suffix >](self) -> $type { self * 1e-12 }
            #[inline(always)] fn [< f $suffix >](self) -> $type { self * 1e-15 }
        }
    };
}

#[allow(non_snake_case)]
pub trait UnitExt {
    def_unit_ext!(V, Volt);
    def_unit_ext!(A, Ampere);
    def_unit_ext!(Ohms, Ohm);
    def_unit_ext!(F, Farad);
    def_unit_ext!(H, Henry);
    def_unit_ext!(Hz, Hertz);
    def_unit_ext!(m, Meter);
    def_unit_ext!(S, Siemens);
    def_unit_ext!(K, Kelvin);
    def_unit_ext!(JpK, JoulePerKelvin);
    def_unit_ext!(A2Sec, AmpereSquaredSecond);
    def_unit_ext!(s, Second);

    fn Week(self) -> Second;
    fn Day(self) -> Second;
    fn Hour(self) -> Second;
    fn Min(self) -> Second;

    fn deg(self) -> Radian;
    fn deg_C(self) -> Celsius;

    fn inv_C(self) -> InvCelsius;
    fn inv_C2(self) -> InvCelsiusSquared;
}

impl UnitExt for f64 {
    impl_unit_ext!(V, Volt);
    impl_unit_ext!(A, Ampere);
    impl_unit_ext!(Ohms, Ohm);
    impl_unit_ext!(F, Farad);
    impl_unit_ext!(H, Henry);
    impl_unit_ext!(Hz, Hertz);
    impl_unit_ext!(m, Meter);
    impl_unit_ext!(S, Siemens);
    impl_unit_ext!(K, Kelvin);
    impl_unit_ext!(JpK, JoulePerKelvin);
    impl_unit_ext!(A2Sec, AmpereSquaredSecond);
    impl_unit_ext!(s, Second);

    #[inline(always)]
    fn Week(self) -> Second {
        self * 604800.0
    }
    #[inline(always)]
    fn Day(self) -> Second {
        self * 86400.0
    }
    #[inline(always)]
    fn Hour(self) -> Second {
        self * 3600.0
    }
    #[inline(always)]
    fn Min(self) -> Second {
        self * 60.0
    }

    #[inline(always)]
    fn deg(self) -> Radian {
        self.to_radians()
    }

    #[inline(always)]
    fn deg_C(self) -> Celsius {
        self
    }

    #[inline(always)]
    fn inv_C(self) -> InvCelsius {
        self
    }

    #[inline(always)]
    fn inv_C2(self) -> InvCelsiusSquared {
        self
    }
}
