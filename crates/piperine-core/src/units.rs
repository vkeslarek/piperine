use paste::paste;

// SI unit type aliases (zero-cost, all f64)
pub type Volt = f64;
pub type Ampere = f64;
pub type Ohm = f64;
pub type Farad = f64;
pub type Henry = f64;
pub type Hertz = f64;
pub type Meter = f64;
pub type MeterSquared = f64;
pub type Siemens = f64;
pub type Kelvin = f64;
pub type Celsius = f64;
pub type InvCelsius = f64;
pub type InvCelsiusSquared = f64;
pub type Radian = f64;
pub type Second = f64;
pub type Dimensionless = f64;
pub type Coulomb = f64;
pub type Joule = f64;
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

macro_rules! impl_unit_ext_f64 {
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

macro_rules! impl_unit_ext_i64 {
    ($suffix:ident, $type:ty) => {
        paste! {
            #[inline(always)] fn [< T $suffix >](self) -> $type { (self as f64) * 1e12 }
            #[inline(always)] fn [< G $suffix >](self) -> $type { (self as f64) * 1e9 }
            #[inline(always)] fn [< M $suffix >](self) -> $type { (self as f64) * 1e6 }
            #[inline(always)] fn [< k $suffix >](self) -> $type { (self as f64) * 1e3 }
            #[inline(always)] fn [<$suffix>](self) -> $type { self as f64 }
            #[inline(always)] fn [< m $suffix >](self) -> $type { (self as f64) * 1e-3 }
            #[inline(always)] fn [< u $suffix >](self) -> $type { (self as f64) * 1e-6 }
            #[inline(always)] fn [< n $suffix >](self) -> $type { (self as f64) * 1e-9 }
            #[inline(always)] fn [< p $suffix >](self) -> $type { (self as f64) * 1e-12 }
            #[inline(always)] fn [< f $suffix >](self) -> $type { (self as f64) * 1e-15 }
        }
    };
}

/// Extension trait for engineering notation on numeric types.
///
/// Usage: `1.0.kOhms()` => 1000.0, `100.0.nF()` => 1e-7, `5.0.mV()` => 0.005
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
    def_unit_ext!(W, Watt);
    def_unit_ext!(s, Second);

    fn deg(self) -> Radian;
    fn deg_C(self) -> Celsius;
}

impl UnitExt for f64 {
    impl_unit_ext_f64!(V, Volt);
    impl_unit_ext_f64!(A, Ampere);
    impl_unit_ext_f64!(Ohms, Ohm);
    impl_unit_ext_f64!(F, Farad);
    impl_unit_ext_f64!(H, Henry);
    impl_unit_ext_f64!(Hz, Hertz);
    impl_unit_ext_f64!(m, Meter);
    impl_unit_ext_f64!(S, Siemens);
    impl_unit_ext_f64!(K, Kelvin);
    impl_unit_ext_f64!(W, Watt);
    impl_unit_ext_f64!(s, Second);

    #[inline(always)]
    fn deg(self) -> Radian {
        self.to_radians()
    }

    #[inline(always)]
    fn deg_C(self) -> Celsius {
        self
    }
}

impl UnitExt for i64 {
    impl_unit_ext_i64!(V, Volt);
    impl_unit_ext_i64!(A, Ampere);
    impl_unit_ext_i64!(Ohms, Ohm);
    impl_unit_ext_i64!(F, Farad);
    impl_unit_ext_i64!(H, Henry);
    impl_unit_ext_i64!(Hz, Hertz);
    impl_unit_ext_i64!(m, Meter);
    impl_unit_ext_i64!(S, Siemens);
    impl_unit_ext_i64!(K, Kelvin);
    impl_unit_ext_i64!(W, Watt);
    impl_unit_ext_i64!(s, Second);

    #[inline(always)]
    fn deg(self) -> Radian {
        (self as f64).to_radians()
    }

    #[inline(always)]
    fn deg_C(self) -> Celsius {
        self as f64
    }
}

/// Format a floating-point value using SPICE engineering notation.
pub fn spice_fmt(val: f64) -> String {
    let abs = val.abs();
    if abs == 0.0 {
        "0".to_string()
    } else if abs >= 1e12 {
        format!("{}T", val / 1e12)
    } else if abs >= 1e9 {
        format!("{}G", val / 1e9)
    } else if abs >= 1e6 {
        format!("{}Meg", val / 1e6)
    } else if abs >= 1e3 {
        format!("{}k", val / 1e3)
    } else if abs >= 1.0 {
        format!("{}", val)
    } else if abs >= 1e-3 {
        format!("{}m", val / 1e-3)
    } else if abs >= 1e-6 {
        format!("{}u", val / 1e-6)
    } else if abs >= 1e-9 {
        format!("{}n", val / 1e-9)
    } else if abs >= 1e-12 {
        format!("{}p", val / 1e-12)
    } else {
        format!("{}f", val / 1e-15)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_units() {
        assert_eq!(1.0.kOhms(), 1000.0);
        assert!((100.0.nF() - 1e-7).abs() < 1e-20);
        assert_eq!(5.0.mV(), 0.005);
        assert!((10.0.uH() - 1e-5).abs() < 1e-18);
        assert_eq!(1.0.MHz(), 1e6);
    }

    #[test]
    fn spice_format() {
        assert_eq!(spice_fmt(1000.0), "1k");
        assert_eq!(spice_fmt(1e-9), "1n");
        assert_eq!(spice_fmt(4.7e3), "4.7k");
    }
}
