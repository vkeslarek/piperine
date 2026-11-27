//! The partials verifier over every device of the fixtures crate (SAI-15, Port AC 4): each function of
//! each hand-written device agrees with its central difference at two generic points.

use piperine_abi::raw::Module;
use piperine_device_fixtures::analog::FixtureModules;
use piperine_device_fixtures::analyses;
use piperine_device_fixtures::circuit;
use piperine_device_fixtures::digital;
use piperine_device_fixtures::mixed;
use piperine_device_fixtures::partials::PartialsCheck;

/// The modules of every device of the crate, by name.
struct EveryDevice;

impl EveryDevice {
    /// The modules of the analog assembly's fixtures.
    fn analog() -> Vec<(&'static str, &'static Module)> {
        let modules = FixtureModules::all();
        vec![
            ("analog::resistor", modules.resistor),
            ("analog::diode", modules.diode),
            ("analog::capacitor", modules.capacitor),
            ("analog::flip_flop", modules.flip_flop),
            ("analog::comparator", modules.comparator),
            ("analog::dac", modules.digital_to_analog),
            ("analog::cubic", modules.cubic),
            ("analog::bad_limit", modules.bad_limit),
            ("analog::source", modules.source),
            ("analog::counted", modules.counted),
            ("analog::probe", modules.probe),
            ("analog::faulty", modules.faulty),
            ("analog::plain_diode", modules.plain_diode),
            ("analog::vsource", modules.voltage_source),
            ("analog::clamp", modules.clamp),
            ("analog::owner", modules.owner),
            ("analog::reader", modules.reader),
            ("analog::tracker", modules.tracker),
            ("analog::delay_line", modules.delay_line),
            ("analog::trio", modules.trio),
            ("analog::ac_vsource", modules.ac_voltage_source),
            ("analog::noisy", modules.noisy),
            ("analog::split", modules.split),
            ("analog::flicker", modules.flicker),
            ("analog::capper", modules.cap_watch_device),
            ("analog::tracer", modules.tracer),
            ("analog::sample_hold", modules.sample_hold),
            ("analog::charge_diode", modules.charge_diode),
            ("analog::tight_capacitor", modules.tight_capacitor),
        ]
    }

    /// The modules of the `circuit` fixtures.
    fn circuit() -> Vec<(&'static str, &'static Module)> {
        vec![
            ("circuit::Resistor", circuit::Resistor::module()),
            ("circuit::OptionalResistor", circuit::OptionalResistor::module()),
            ("circuit::Capacitor", circuit::Capacitor::module()),
            ("circuit::Diode", circuit::Diode::module()),
            ("circuit::VoltageSource", circuit::VoltageSource::module()),
            ("circuit::SineSource", circuit::SineSource::module()),
            ("circuit::RampSource", circuit::RampSource::module()),
            ("circuit::AcCurrentSource", circuit::AcCurrentSource::module()),
            ("circuit::Inductor", circuit::Inductor::module()),
            ("circuit::NoisyResistor", circuit::NoisyResistor::module()),
            ("circuit::NoisePair", circuit::NoisePair::module()),
        ]
    }

    /// The modules of the `mixed` fixtures.
    fn mixed() -> Vec<(&'static str, &'static Module)> {
        vec![
            ("mixed::Comparator", mixed::Comparator::module()),
            ("digital::DataFlipFlop", digital::DataFlipFlop::module()),
            ("digital::Inverter", digital::Inverter::module()),
            ("mixed::Dac", mixed::BitDigitalToAnalog::module()),
            ("mixed::SettleCount", mixed::SettleCount::module()),
            ("mixed::Globals", mixed::Globals::module()),
            ("digital::Timer", digital::Timer::module()),
            ("digital::MultiRecorder", digital::MultiRecorder::module()),
            ("digital::Sampler", digital::Sampler::module()),
            ("digital::Stub", digital::Stub::module()),
            ("digital::Counter", digital::Counter::module()),
            ("mixed::SettleTime", mixed::SettleTime::module()),
            ("mixed::Delay", mixed::Delay::module()),
            ("mixed::Finisher", mixed::Finisher::module()),
            ("mixed::ConductanceTracer", mixed::ConductanceTracer::module()),
            ("mixed::FlakyConductance", mixed::FlakyConductance::module()),
            ("mixed::Capped", mixed::Capped::module()),
            ("mixed::ClockSource", mixed::ClockSource::module()),
            ("mixed::ToggleFlipFlop", mixed::ToggleFlipFlop::module()),
        ]
    }

    /// The modules of the `analyses` fixtures.
    fn analyses() -> Vec<(&'static str, &'static Module)> {
        vec![
            ("analyses::Bias", analyses::Bias::module()),
            ("analyses::Wire", analyses::Wire::module()),
            ("analyses::Quadratic", analyses::Quadratic::module()),
        ]
    }

    /// The modules of the `digital` fixtures.
    fn digital() -> Vec<(&'static str, &'static Module)> {
        vec![
            ("digital::Inverter", digital::Inverter::module()),
            ("digital::And2", digital::And2::module()),
            ("digital::Dff", digital::DataFlipFlop::module()),
            ("digital::Timer", digital::Timer::module()),
            ("digital::Counter", digital::Counter::module()),
            ("digital::Resolver", digital::Resolver::module()),
            ("digital::Succ", digital::Successor::module()),
            ("digital::MultiRecorder", digital::MultiRecorder::module()),
            ("digital::SettleRecorder", digital::SettleRecorder::module()),
            ("digital::CheckedParameter", digital::CheckedParameter::module()),
            ("digital::BitFaulty", digital::BitFaulty::module()),
            ("digital::Sampler", digital::Sampler::module()),
            ("digital::Stub", digital::Stub::module()),
            ("digital::Odd", digital::Odd::module()),
            ("digital::Setter", digital::Setter::module()),
            ("digital::Settler", digital::Settler::module()),
            ("digital::SettleEvent", digital::SettleEvent::module()),
            ("digital::LaterDelayed", digital::LaterDelayed::module()),
            ("digital::Lagged", digital::Lagged::module()),
            ("digital::Clockwatch", digital::ClockWatch::module()),
            ("digital::Rearm", digital::Rearm::module()),
        ]
    }

    /// Why a device is not held to the generic points, when it is not: it is a fixture of a failure path,
    /// or its function is meaningful only at the points its own test sets.
    fn exemption(name: &str) -> Option<&'static str> {
        match name {
            "analog::bad_limit" => Some("breaks abi.md A3 on purpose"),
            "digital::BitFaulty" => Some("faults on purpose"),
            "analog::diode" | "analog::charge_diode" => Some("its exponential overflows at the generic points; its tests set their own"),
            "analog::clamp" | "analog::owner" => Some("a limit it owns: the partials against the limit input are meaningful only at the held iterate its tests set"),
            _ => None,
        }
    }

    /// Every module, in the order of the crate's modules.
    fn all() -> Vec<(&'static str, &'static Module)> {
        let mut every = EveryDevice::analog();
        every.extend(EveryDevice::circuit());
        every.extend(EveryDevice::mixed());
        every.extend(EveryDevice::analyses());
        every.extend(EveryDevice::digital());
        every
    }
}

#[test]
fn its_devices_pass_the_partials_verifier() {
    let mut checked = 0;
    for (name, module) in EveryDevice::all() {
        if EveryDevice::exemption(name).is_none() {
            PartialsCheck::verify_every_function(name, module);
            checked += 1;
        }
    }
    assert!(checked > 60, "the check reached {checked} devices only");
}
