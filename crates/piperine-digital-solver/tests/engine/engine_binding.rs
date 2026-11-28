//! An engine over placed devices, and the instants it runs.

use piperine_device_fixtures::binding::{Placed, PlanBinding};
use piperine_digital_solver::engine::{Digital, Horizon, Mode, Outcome};
use piperine_digital_solver::limits::DigitalLimits;
use piperine_digital_solver::landing::TimePointRule;
use piperine_digital_solver::net::NetId;
use piperine_digital_solver::tick::{Resolution, Tick};

/// An engine over placed devices: its nets by name, its first instants and the instants it runs.
pub(crate) struct EngineBinding;

impl EngineBinding {
    /// The engine helper `engine`.
    pub(crate) fn engine(devices: Vec<Placed<'_>>) -> Digital {
        Digital::new(PlanBinding::bind_by_name(devices, Resolution::default()).expect("binds"), DigitalLimits::default(), TimePointRule::default())
    }

    /// The net helper `net`.
    pub(crate) fn net(d: &Digital, name: &str) -> NetId {
        PlanBinding::net_named(d.plan(), name)
    }

    /// The instant helper `instant`.
    pub(crate) fn instant(d: &mut Digital, at: Tick) -> Outcome {
        d.run_instant(at, &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).expect("the instant runs")
    }

    /// The started helper `started`.
    pub(crate) fn started(mut d: Digital) -> Digital {
        d.start().unwrap();
        d.run_instant(Tick(0), &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).unwrap();
        d
    }
}
