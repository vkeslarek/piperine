//! `install` semantics (T20, DVT-32): a [`Wave`] installed mid-run has its
//! **time origin at the install instant**, and replacing an already-active
//! wave **takes effect immediately with its discontinuity registered as an
//! `Exact` breakpoint** — it is never re-anchored from the current value the
//! way [`piperine_ir::ops::Operator::transition`] is (`ideal.md` §4.2).
//! `transition`'s re-anchor is correct *for `transition`*, whose entire job
//! is smoothing; a `Wave` is a literal drive shape, and silently smoothing a
//! step the author asked for would be exactly the kind of helpful magic
//! that makes a result impossible to explain. Continuity is available, but
//! explicit: the caller reads the current value and bakes it into the new
//! wave (`InstallSite::install`'s doc example).

use piperine_solver::abi::AnalogReference;

use piperine_ir::wave::Wave;

use super::super::device::stamp::{Ctx, StampSink};

/// A [`Wave`] pinned to the instant it was installed: `wave.value(t)` reads
/// as `wave.value(t - origin)`, so `Wave::ramp(a, b, dur)` installed at
/// `origin` ramps over `[origin, origin + dur]` — never over `[0, dur]`
/// (DVT-32's time-origin rule).
struct InstalledWave {
    wave: Wave,
    origin: f64,
}

impl InstalledWave {
    fn value(&self, t: f64) -> f64 {
        self.wave.value(t - self.origin)
    }

    fn dc_value(&self) -> f64 {
        self.wave.dc_value()
    }

    fn breakpoints(&self, from: f64, horizon: f64) -> Vec<f64> {
        let mut local = Vec::new();
        self.wave.pending_edges(from - self.origin, from - self.origin + horizon, &mut local);
        local.into_iter().map(|t| t + self.origin).collect()
    }
}

/// The `install`-able force at one two-terminal site (`design.md` §D):
/// drives `(plus, minus)` through a declared series impedance `rout`, one
/// [`Wave`] at a time. `install` replaces the active wave immediately,
/// origin-relative, and registers the swap instant as a breakpoint so the
/// integrator lands on the edge rather than interpolating across it.
pub struct InstallSite {
    plus: AnalogReference,
    minus: AnalogReference,
    rout: f64,
    current: Option<InstalledWave>,
    /// The most recent install instant, still owed one `Exact` breakpoint
    /// report (cleared once `breakpoints` has published it past that
    /// instant — mirrors `Wave`'s own "declare while pending" contract).
    pending_edge: Option<f64>,
}

impl InstallSite {
    /// A site with nothing installed yet: stamps nothing, declares no
    /// breakpoints, until the first `install`.
    pub fn new(plus: AnalogReference, minus: AnalogReference, rout: f64) -> Self {
        Self { plus, minus, rout, current: None, pending_edge: None }
    }

    /// Install `wave` at `t_i`, replacing whatever was active.
    ///
    /// - **Time is install-relative**: `wave`'s origin becomes `t_i`, not
    ///   absolute zero.
    /// - **Takes effect immediately, never re-anchored**: the new wave's
    ///   value at `t_i` is whatever `wave.value(0.0)` says — if that is a
    ///   step away from the old wave's value, the author gets the step
    ///   they asked for, and `t_i` is registered as an `Exact` breakpoint
    ///   so the integrator lands on it.
    ///
    /// Continuity, when wanted, is explicit — the idiom is to read the
    /// current value first and bake it into the new wave:
    ///
    /// ```text
    /// let now = ctx.v(&site_plus) - ctx.v(&site_minus); // or a Measure/read_v
    /// site.install(Wave::ramp(now, 1.8, 100e-6), t_i);   // continuous, because asked for
    /// ```
    pub fn install(&mut self, wave: Wave, t_i: f64) {
        self.current = Some(InstalledWave { wave, origin: t_i });
        self.pending_edge = Some(t_i);
    }

    /// The currently installed wave's value at `t`, before division into a
    /// Norton stamp — the same read a `t.read_v` would report.
    pub fn value(&self, t: f64) -> Option<f64> {
        self.current.as_ref().map(|w| w.value(t))
    }

    /// The installed wave's SPICE-convention DC value (`ideal.md` §12.10),
    /// or `None` if nothing has ever been installed.
    pub fn dc_value(&self) -> Option<f64> {
        self.current.as_ref().map(InstalledWave::dc_value)
    }

    /// Stamp the active wave's Norton contribution at `ctx.t()`, if one is
    /// installed. A site with nothing installed stamps nothing (zero cost).
    ///
    /// At the **DC operating point** (T25, DVT-36) — signaled by `ctx.dt()
    /// == 0.0`, the same way `Ctx::dt`'s own doc states "0.0 at DC, where
    /// there is none," and the one field distinguishing a real transient's
    /// own `t = 0` step (`dt` already the first nonzero step size) from a
    /// DC solve — the wave contributes its declared SPICE-convention
    /// [`Wave::dc_value`], not `value(0.0)`: for a `sigmoid` or `sin`
    /// wave those genuinely differ (`ideal.md` §12.10).
    pub fn stamp(&self, ctx: &Ctx<'_>, sink: &mut StampSink) {
        let Some(wave) = &self.current else { return };
        let g = 1.0 / self.rout;
        let v = if ctx.dt() == 0.0 { wave.dc_value() } else { wave.value(ctx.t()) };
        sink.g(&self.plus, &self.plus, g);
        sink.g(&self.plus, &self.minus, -g);
        sink.g(&self.minus, &self.plus, -g);
        sink.g(&self.minus, &self.minus, g);
        sink.i(&self.plus, g * v);
        sink.i(&self.minus, -g * v);
    }

    /// This site's breakpoints within `(from, from + horizon]`: the active
    /// wave's own self-declared edges (origin-shifted) plus, once, the
    /// most recent `install` instant itself.
    pub fn breakpoints(&self, from: f64, horizon: f64) -> Vec<f64> {
        let mut out = Vec::new();
        if let Some(wave) = &self.current {
            out.extend(wave.breakpoints(from, horizon));
        }
        if let Some(edge) = self.pending_edge
            && edge > from
        {
            out.push(edge);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use piperine_solver::abi::{AnalogVariable, NodeIdentifier};
    use std::sync::Arc;

    fn node(i: usize) -> AnalogReference {
        AnalogReference::new(Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(i))), i)
    }

    /// DVT-32's time-origin rule, asserted numerically: `Wave::ramp(a, b,
    /// dur)` installed at `t_i` reads `a` at `t_i`, `b` at `t_i + dur`, and
    /// the linear midpoint at `t_i + dur/2` — never as if `t = 0` were the
    /// origin.
    #[test]
    fn a_ramp_installed_mid_run_ramps_over_install_instant_to_install_instant_plus_duration() {
        let mut site = InstallSite::new(node(0), node(1), 1.0);
        let t_i = 5e-6;
        let dur = 2e-6;
        site.install(Wave::ramp(0.0, 1.0, dur), t_i);

        assert_eq!(site.value(t_i), Some(0.0), "value at the install instant is the ramp start");
        assert_eq!(site.value(t_i + dur), Some(1.0), "value at install + dur is the ramp end");
        assert!(
            (site.value(t_i + dur / 2.0).unwrap() - 0.5).abs() < 1e-12,
            "the ramp's midpoint lands at the midpoint of [t_i, t_i+dur], not [0, dur]"
        );
        // Absolute t=0 (before the install instant) must not see any of
        // the ramp — proof the origin really moved, not merely offset the
        // read.
        assert_eq!(site.value(0.0), Some(0.0), "before t_i the ramp has not started (clamped at its start value)");
    }

    /// DVT-32's non-re-anchor rule: replacing `const(10e-3)` with
    /// `const(100e-3)` takes effect immediately at the install instant —
    /// the new value, not a blend — and registers that instant as a
    /// breakpoint.
    #[test]
    fn replacing_a_wave_takes_effect_immediately_and_registers_an_exact_breakpoint() {
        let mut site = InstallSite::new(node(0), node(1), 1.0);
        site.install(Wave::constant(10e-3), 0.0);
        assert_eq!(site.value(1e-6), Some(10e-3));

        let t_i = 3e-6;
        site.install(Wave::constant(100e-3), t_i);

        // Immediate effect, never re-anchored from the 10e-3 that was
        // active a moment before: the read right at t_i is the new value.
        assert_eq!(site.value(t_i), Some(100e-3), "the replacement's value at t_i is the new wave's, not a blend");
        assert_eq!(site.value(t_i + 1e-9), Some(100e-3));

        // The swap instant is a declared, Exact-priority breakpoint.
        let bps = site.breakpoints(t_i - 1e-9, 1e-6);
        assert!(bps.iter().any(|&t| (t - t_i).abs() < 1e-15), "the install instant must be a declared breakpoint: {bps:?}");
    }

    /// Continuity is available, but only when the author asks for it
    /// explicitly — the documented idiom (`InstallSite::install`'s doc
    /// example): read the current value, bake it into the new wave's
    /// start.
    #[test]
    fn continuity_across_an_install_is_explicit_not_automatic() {
        let mut site = InstallSite::new(node(0), node(1), 1.0);
        site.install(Wave::constant(0.6), 0.0);
        let now = site.value(2e-6).unwrap();
        assert_eq!(now, 0.6);

        let t_i = 2e-6;
        // The explicit-continuity idiom: read `now`, bake it into the
        // ramp's start.
        site.install(Wave::ramp(now, 1.8, 100e-6), t_i);
        assert_eq!(site.value(t_i), Some(now), "an explicitly continuous install matches the read value at t_i");
        assert_eq!(site.value(t_i + 100e-6), Some(1.8));

        // The contrast: install WITHOUT reading `now` first is a step,
        // proving continuity was never automatic — it took the explicit
        // read to get it.
        let mut stepped = InstallSite::new(node(0), node(1), 1.0);
        stepped.install(Wave::constant(0.6), 0.0);
        stepped.install(Wave::ramp(0.0, 1.8, 100e-6), t_i);
        assert_eq!(stepped.value(t_i), Some(0.0), "without the explicit read, the new wave starts wherever it says — a step here");
    }
}
