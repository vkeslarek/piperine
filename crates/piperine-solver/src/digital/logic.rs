#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LogicValue {
    Zero = 0,
    One = 1,
    X = 2,
    Z = 3,
}

impl LogicValue {
    /// Resolves two driving logic values onto the same net.
    pub fn resolve(a: LogicValue, b: LogicValue) -> LogicValue {
        match (a, b) {
            (LogicValue::Z, other) | (other, LogicValue::Z) => other,
            (LogicValue::Zero, LogicValue::Zero) => LogicValue::Zero,
            (LogicValue::One, LogicValue::One) => LogicValue::One,
            _ => LogicValue::X,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logic_resolution() {
        assert_eq!(LogicValue::resolve(LogicValue::Zero, LogicValue::One), LogicValue::X);
        assert_eq!(LogicValue::resolve(LogicValue::Z, LogicValue::One), LogicValue::One);
        assert_eq!(LogicValue::resolve(LogicValue::X, LogicValue::Zero), LogicValue::X);
        assert_eq!(LogicValue::resolve(LogicValue::One, LogicValue::One), LogicValue::One);
    }
}
