//! The analytic voltage of one first-order node under a piecewise-constant drive: the closed form
//! every analog criterion of the suite is judged against.

/// One stretch of the drive: from `start` the node relaxes towards `target` with time constant `tau`
/// (`f64::INFINITY` for a node that holds its value).
#[derive(Debug, Clone, Copy)]
pub struct RcSegment {
    /// The instant the segment begins, in seconds.
    pub start: f64,
    /// The voltage the node relaxes towards, in volts.
    pub target: f64,
    /// The time constant, in seconds.
    pub tau: f64,
}

impl RcSegment {
    /// The node's voltage `elapsed` seconds into the segment, having entered it at `entry`.
    fn value_after(&self, entry: f64, elapsed: f64) -> f64 {
        self.target + (entry - self.target) * (-elapsed / self.tau).exp()
    }
}

/// A node that starts at `initial` volts at the first segment's start and follows its segments.
#[derive(Debug, Clone)]
pub struct PiecewiseRc {
    /// The node's voltage at the first segment's start, in volts.
    initial: f64,
    /// The segments, in time order.
    segments: Vec<RcSegment>,
}

impl PiecewiseRc {
    /// A node at `initial` volts that follows `segments`, given in time order.
    pub fn new(initial: f64, segments: Vec<RcSegment>) -> PiecewiseRc {
        PiecewiseRc { initial, segments }
    }

    /// The node's voltage at `time`, carried through every segment before the one that holds it.
    pub fn at(&self, time: f64) -> f64 {
        let mut entry = self.initial;
        for (index, segment) in self.segments.iter().enumerate() {
            match self.segments.get(index + 1) {
                Some(next) if time >= next.start => entry = segment.value_after(entry, next.start - segment.start),
                _ => return segment.value_after(entry, time - segment.start),
            }
        }
        panic!("a piecewise node has at least one segment")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_node_relaxes_then_holds_then_relaxes_again() {
        let node = PiecewiseRc::new(
            0.0,
            vec![
                RcSegment { start: 0.0, target: 1.0, tau: 1.0 },
                RcSegment { start: 1.0, target: 1.0, tau: f64::INFINITY },
                RcSegment { start: 2.0, target: 0.0, tau: 1.0 },
            ],
        );
        let at_one = 1.0 - (-1.0f64).exp();
        assert!((node.at(1.0) - at_one).abs() < 1e-15);
        assert!((node.at(1.5) - at_one).abs() < 1e-15, "a held node stays");
        assert!((node.at(3.0) - at_one * (-1.0f64).exp()).abs() < 1e-15);
    }
}
