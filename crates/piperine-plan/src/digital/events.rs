//! Events: the pins and watches of `changed`, `cross`, `at` and `settle` events, delays and bound steps.

use piperine_abi::layout::F64;
use piperine_abi::raw::{DEFAULT, global, role, watch};
use piperine_abi::shape::{PinSpec, ShapeBuf, WatchSpec};
use piperine_ir::{Body, CarrierRole, EventSource, ExprId, Target as Query};

use crate::pin::PinLayout;
use crate::digital::Digital;
use crate::plan::{BodyPlan, DelayedPins};
use crate::resolved::DriveLayout;

impl Digital {
    /// Adds one event pin `event:<k>` per event of `body`; returns their indices.
    pub(crate) fn event_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<u32> {
        body.events()
            .iter()
            .enumerate()
            .map(|(k, e)| s.pin(PinSpec { origin: &Self::at(&e.origin), ..PinSpec::event(&format!("event:{k}"), role::PRIVATE) }))
            .collect()
    }

    /// Adds the pin each guarded `cross` event's watch fires; `None` for every other event.
    pub(crate) fn crossed_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<Option<u32>> {
        body.events()
            .iter()
            .enumerate()
            .map(|(k, e)| match (&e.source, e.guard) {
                (EventSource::Cross { .. }, Some(_)) => {
                    Some(s.pin(PinSpec { origin: &Self::at(&e.origin), ..PinSpec::event(&format!("cross:{k}"), role::PRIVATE) }))
                }
                _ => None,
            })
            .collect()
    }

    /// Adds one `CROSS` watch per `cross` event, firing the event's pin (or, guarded, its
    /// `cross:<k>` pin); returns each event's watch.
    pub(crate) fn watches(
        s: &mut ShapeBuf,
        body: Body<'_>,
        events: &[u32],
        crossed: &[Option<u32>],
        tolerances: &[(Option<u32>, Option<u32>)],
    ) -> Vec<Option<u32>> {
        body.events()
            .iter()
            .enumerate()
            .map(|(k, e)| {
                let EventSource::Cross { .. } = e.source else { return None };
                let event = crossed.get(k).copied().flatten().or_else(|| events.get(k).copied())?;
                Some(s.watch(WatchSpec {
                    name: &format!("cross:{k}"),
                    origin: &Self::at(&e.origin),
                    kind: watch::CROSS,
                    layout: &F64,
                    event,
                    ttol: tolerances.get(k).and_then(|t| t.0).unwrap_or(DEFAULT),
                    etol: tolerances.get(k).and_then(|t| t.1).unwrap_or(DEFAULT),
                }))
            })
            .collect()
    }

    /// Adds the private `F64` pins `ttol:<k>` and `etol:<k>` of each `cross` event that gives a
    /// tolerance, written by a function of the parameter cone.
    pub(crate) fn tolerance_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<(Option<u32>, Option<u32>)> {
        body.events()
            .iter()
            .enumerate()
            .map(|(k, e)| {
                let EventSource::Cross { ttol, etol, .. } = e.source else { return (None, None) };
                let mut pin = |given: Option<ExprId>, name: &str| {
                    given.map(|_| s.pin(PinSpec { origin: &Self::at(&e.origin), ..PinSpec::discrete(&format!("{name}:{k}"), role::PRIVATE, &F64) }))
                };
                (pin(ttol, "ttol"), pin(etol, "etol"))
            })
            .collect()
    }

    /// Adds the private `F64` pin `d<k>.abstol` of each derivative site that gives an `abstol`,
    /// written by a function of the parameter cone and named by the site's `DDT` term.
    pub(crate) fn derivative_abstol_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<Option<u32>> {
        body.derivatives()
            .iter()
            .enumerate()
            .map(|(k, site)| {
                site.abstol.map(|_| s.pin(PinSpec { origin: &Self::at(&site.origin), ..PinSpec::discrete(&format!("d{k}.abstol"), role::PRIVATE, &F64) }))
            })
            .collect()
    }

    /// Adds the private `F64` pin `delay<k>.max` of each delay site, written by a function of the
    /// parameter cone and named by the site's `DELAY` term.
    pub(crate) fn delay_maximum_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<u32> {
        body.delays()
            .iter()
            .enumerate()
            .map(|(k, site)| s.pin(PinSpec { origin: &Self::at(&site.origin), ..PinSpec::discrete(&format!("delay{k}.max"), role::PRIVATE, &F64) }))
            .collect()
    }

    /// Adds one `CAP` watch per `bound_step` site of the `analog` block, `cap:<site>`; returns
    /// `(site, watch)`.
    pub(crate) fn cap_watches(s: &mut ShapeBuf, body: Body<'_>) -> Vec<(u32, u32)> {
        BodyPlan::bound_steps(body)
            .into_iter()
            .map(|site| {
                let watch_index = s.watch(WatchSpec {
                    name: &format!("cap:{site}"),
                    origin: &Self::at(body.origin()),
                    kind: watch::CAP,
                    layout: &F64,
                    event: DEFAULT,
                    ttol: DEFAULT,
                    etol: DEFAULT,
                });
                (site, watch_index)
            })
            .collect()
    }

    /// Adds the two pins of each drive with `after`; `None` for a drive without one.
    pub(crate) fn delayed_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<Option<DelayedPins>> {
        body.drives()
            .iter()
            .map(|d| {
                d.after?;
                let name = Self::carrier_name(body, d.carrier);
                let carrier = body.carriers().get(d.carrier as usize)?;
                let at = Self::at(&d.origin);
                let value = match &carrier.role {
                    // the undelayed drive of a resolved carrier is a drive net's layout: its commit copies `driving` too
                    CarrierRole::Resolved { driven, .. } => {
                        let layout = DriveLayout::of(body.ir(), driven)?;
                        let fields = layout.fields();
                        let abi = layout.layout(&fields);
                        s.pin(PinSpec { origin: &at, ..PinSpec::discrete(&format!("drive:{name}.now"), role::PRIVATE, &abi) })
                    }
                    _ => {
                        let layout = PinLayout::in_ir(body.ir(), &carrier.ty)?;
                        let fields = layout.fields();
                        let abi = layout.layout(&fields);
                        s.pin(PinSpec { origin: &at, ..PinSpec::discrete(&format!("drive:{name}.now"), role::PRIVATE, &abi) })
                    }
                };
                let event = s.pin(PinSpec { origin: &at, ..PinSpec::event(&format!("drive:{name}.changed"), role::PRIVATE) });
                Some(DelayedPins { value, event })
            })
            .collect()
    }

    /// Adds the private event pin `at:<k>` the timer of each guarded `at` event fires; `None`
    /// for every other event.
    pub(crate) fn timer_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<Option<u32>> {
        body.events()
            .iter()
            .enumerate()
            .map(|(k, e)| match (&e.source, e.guard) {
                (EventSource::At(_), Some(_)) => {
                    Some(s.pin(PinSpec { origin: &Self::at(&e.origin), ..PinSpec::event(&format!("at:{k}"), role::PRIVATE) }))
                }
                _ => None,
            })
            .collect()
    }

    /// Adds the `GLOBAL` `START` pin when `body` has an `at` event.
    pub(crate) fn start_pin(s: &mut ShapeBuf, body: Body<'_>) -> Option<u32> {
        body.events().iter().any(|e| matches!(e.source, EventSource::At(_))).then(|| {
            s.pin(PinSpec { origin: &Self::at(body.origin()), global: global::START, ..PinSpec::event("start", role::GLOBAL) })
        })
    }

    /// Adds the `GLOBAL` `SETTLE` pin when `body` has a `settle` event (§13 rule 17): the pin the
    /// engine fires in the settle phase (§15 step 4), read by the event function of each
    /// `settle` event (`Source::Settle`).
    pub(crate) fn settle_pin(s: &mut ShapeBuf, body: Body<'_>) -> Option<u32> {
        body.events().iter().any(|e| matches!(e.source, EventSource::Settle)).then(|| {
            s.pin(PinSpec { origin: &Self::at(body.origin()), global: global::SETTLE, ..PinSpec::event("settle", role::GLOBAL) })
        })
    }

    /// Adds, per `at` event, one event pin `at:<k>.<carrier>` per carrier its time reads.
    pub(crate) fn rearm_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<Vec<(u32, u32)>> {
        body.events()
            .iter()
            .enumerate()
            .map(|(k, e)| {
                let EventSource::At(_) = e.source else { return Vec::new() };
                body.carrier_reads(Query::At(k as u32))
                    .into_iter()
                    .map(|c| {
                        let name = format!("at:{k}.{}", Self::carrier_name(body, c));
                        (c, s.pin(PinSpec { origin: &Self::at(&e.origin), ..PinSpec::event(&name, role::PRIVATE) }))
                    })
                    .collect()
            })
            .collect()
    }
}
