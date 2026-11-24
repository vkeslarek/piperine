//! The pattern of `logic` and `matches_bits` (§22.1.4): one character per level, highest level first, each of `0`, `1`,
//! `x` and `z`. The checker validates a literal pattern, the elaborator reads the planes of the value it names and the
//! masks a match compares.

/// What one character of a pattern names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatternLevel {
    /// `0`.
    Zero,
    /// `1`.
    One,
    /// `x`: the state X in `logic`, any state in `matches_bits`.
    X,
    /// `z`.
    Z,
}

/// A parsed pattern: its levels, highest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogicPattern {
    levels: Vec<PatternLevel>,
}

/// What `matches_bits` compares: `binary & binary_mask == binary_wanted && value & value_mask == value_wanted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PatternMasks {
    /// The levels of `binary` the pattern constrains.
    pub(crate) binary_mask: u128,
    /// What those levels of `binary` must hold.
    pub(crate) binary_wanted: u128,
    /// The levels of `value` the pattern constrains.
    pub(crate) value_mask: u128,
    /// What those levels of `value` must hold.
    pub(crate) value_wanted: u128,
}

impl LogicPattern {
    /// The pattern `text`; the first character that is none of `0`, `1`, `x` and `z` is the error.
    pub(crate) fn parse(text: &str) -> Result<LogicPattern, char> {
        let levels = text
            .chars()
            .map(|character| match character {
                '0' => Ok(PatternLevel::Zero),
                '1' => Ok(PatternLevel::One),
                'x' => Ok(PatternLevel::X),
                'z' => Ok(PatternLevel::Z),
                other => Err(other),
            })
            .collect::<Result<Vec<_>, char>>()?;
        Ok(LogicPattern { levels })
    }

    /// How many levels the pattern names.
    pub(crate) fn width(&self) -> usize {
        self.levels.len()
    }

    /// The bit of level `k` (0 is the lowest) as a mask, with the level's character: the pattern lists the highest first.
    fn levels_from_lowest(&self) -> impl Iterator<Item = (u32, PatternLevel)> + '_ {
        self.levels.iter().rev().enumerate().map(|(k, level)| (k as u32, *level))
    }

    /// The planes `(binary, value)` of the `Logic` the pattern names (§22.1.1): `0` is `(1, 0)`, `1` is `(1, 1)`, X is
    /// `(0, 0)` and Z is `(0, 1)`.
    pub(crate) fn planes(&self) -> (u128, u128) {
        let (mut binary, mut value) = (0u128, 0u128);
        for (k, level) in self.levels_from_lowest() {
            let (b, v) = match level {
                PatternLevel::Zero => (1, 0),
                PatternLevel::One => (1, 1),
                PatternLevel::X => (0, 0),
                PatternLevel::Z => (0, 1),
            };
            binary |= b << k;
            value |= v << k;
        }
        (binary, value)
    }

    /// The masks `matches_bits` compares: `0`, `1` and `z` name one state each, `x` matches any.
    pub(crate) fn masks(&self) -> PatternMasks {
        let mut masks = PatternMasks { binary_mask: 0, binary_wanted: 0, value_mask: 0, value_wanted: 0 };
        for (k, level) in self.levels_from_lowest() {
            let Some((b, v)) = (match level {
                PatternLevel::Zero => Some((1u128, 0u128)),
                PatternLevel::One => Some((1, 1)),
                PatternLevel::Z => Some((0, 1)),
                PatternLevel::X => None,
            }) else {
                continue;
            };
            masks.binary_mask |= 1 << k;
            masks.value_mask |= 1 << k;
            masks.binary_wanted |= b << k;
            masks.value_wanted |= v << k;
        }
        masks
    }
}
