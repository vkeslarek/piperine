//! The four-state digital value and the IEEE 1364 edge predicate, owned
//! once.
//!
//! Extracted from `piperine-codegen`'s `device/digital.rs:286,329` inline
//! `match edge { .. }` blocks, semantics unchanged — that rule is the
//! correct one: `Rising` fires on `0→1`, `X→1` and `Z→1`; `Falling` fires
//! on `1→0`, `X→0` and `Z→0`; `Either` fires on any change. Before this
//! module existed, `piperine-api`'s host sequencer implemented a narrower
//! `Zero→One`-only rule, so a host `wait(edge(clk, Rising))` could miss the
//! `X→1` transition every digital net starts at — a live silent-stall bug
//! (`codegen-simplification` spec §0 item 3).
//!
//! [`Logic`]'s discriminants are pinned to `piperine_solver::abi::LogicValue`
//! and to codegen's `Quad` encoding (`device/digital.rs:20`) so both
//! conversions are a plain `match`, never a transmute — this crate stays
//! dependency-free (the purity guard forbids seeing `piperine-solver`), so
//! it defines its own type and both sides convert explicitly.

/// A four-state digital value: `Zero`/`One`/`X` (unknown)/`Z` (high-impedance).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Logic {
    Zero = 0,
    One = 1,
    X = 2,
    Z = 3,
}

/// Which transition an edge-triggered wait or clocked block is watching for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDirection {
    Rising,
    Falling,
    Either,
}

impl EdgeDirection {
    /// Whether this direction fires for the transition `before → after`,
    /// per IEEE 1364: `Rising` also fires on `X→1`/`Z→1`; `Falling` also
    /// fires on `X→0`/`Z→0`; `Either` fires on any change at all.
    pub fn fires(self, before: Logic, after: Logic) -> bool {
        match self {
            EdgeDirection::Rising => before != Logic::One && after == Logic::One,
            EdgeDirection::Falling => before != Logic::Zero && after == Logic::Zero,
            EdgeDirection::Either => before != after,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Logic; 4] = [Logic::Zero, Logic::One, Logic::X, Logic::Z];

    #[test]
    fn rising_fires_only_into_one_from_a_non_one_state() {
        for &before in &ALL {
            for &after in &ALL {
                let expect = before != Logic::One && after == Logic::One;
                assert_eq!(
                    EdgeDirection::Rising.fires(before, after),
                    expect,
                    "Rising({before:?} -> {after:?})"
                );
            }
        }
        // Named explicitly per spec AC 29.
        assert!(EdgeDirection::Rising.fires(Logic::X, Logic::One), "X -> 1 is a rising edge");
        assert!(EdgeDirection::Rising.fires(Logic::Z, Logic::One), "Z -> 1 is a rising edge");
        assert!(!EdgeDirection::Rising.fires(Logic::One, Logic::One), "1 -> 1 is not an edge");
    }

    #[test]
    fn falling_fires_only_into_zero_from_a_non_zero_state() {
        for &before in &ALL {
            for &after in &ALL {
                let expect = before != Logic::Zero && after == Logic::Zero;
                assert_eq!(
                    EdgeDirection::Falling.fires(before, after),
                    expect,
                    "Falling({before:?} -> {after:?})"
                );
            }
        }
        // Named explicitly per spec AC 29.
        assert!(EdgeDirection::Falling.fires(Logic::X, Logic::Zero), "X -> 0 is a falling edge");
        assert!(EdgeDirection::Falling.fires(Logic::Z, Logic::Zero), "Z -> 0 is a falling edge");
        assert!(!EdgeDirection::Falling.fires(Logic::Zero, Logic::Zero), "0 -> 0 is not an edge");
    }

    #[test]
    fn either_fires_on_any_change_and_only_on_change() {
        for &before in &ALL {
            for &after in &ALL {
                let expect = before != after;
                assert_eq!(
                    EdgeDirection::Either.fires(before, after),
                    expect,
                    "Either({before:?} -> {after:?})"
                );
            }
        }
    }

    #[test]
    fn discriminants_are_pinned_to_logic_value_and_quad() {
        assert_eq!(Logic::Zero as i64, 0);
        assert_eq!(Logic::One as i64, 1);
        assert_eq!(Logic::X as i64, 2);
        assert_eq!(Logic::Z as i64, 3);
    }
}
