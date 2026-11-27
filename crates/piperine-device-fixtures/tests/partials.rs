//! The partials verifier over every device of the fixtures crate (SAI-15, Port AC 4): each function of
//! each hand-written device agrees with its central difference at two generic points. The one list
//! below names every `impl Device` of the crate, and `every_device_of_the_crate_is_in_the_list` reads
//! the crate's sources to prove it: a device added without a row, or a row left after its device went,
//! fails.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use piperine_abi::raw::Module;
use piperine_device_fixtures::analog::FixtureModules;
use piperine_device_fixtures::analyses;
use piperine_device_fixtures::circuit;
use piperine_device_fixtures::digital;
use piperine_device_fixtures::mixed;
use piperine_device_fixtures::partials::{Fixture, PartialsCheck};
use piperine_device_fixtures::solver_devices;

/// How one device is held to its finite differences.
enum Check {
    /// Every function, at the two generic points.
    Generic,
    /// Every function, with read `read` of function `function` held at `value`: a read the device guards
    /// (a time window, a threshold) stays on the side its tests use. Rows are `(function, read, value)`.
    Holding(&'static [(u32, usize, f64)]),
    /// Every function, but those named are fixtures of a failure path and must behave as declared.
    Fixtures(&'static [(u32, Fixture)]),
    /// Not held to the generic points, and why: its function is meaningful only at the points its own tests set.
    Exempt(&'static str),
}

/// One device of the crate: its label (`module::Type`), its module and how its partials are checked.
struct ListedDevice {
    /// `module::Type`, the path of the device inside the crate.
    label: &'static str,
    /// The module the device builds.
    module: &'static Module,
    /// How the partials of the device are checked.
    check: Check,
}

/// The sources of the crate, read to find every `impl Device`.
struct DeviceScan;

impl DeviceScan {
    /// The Rust files under `directory`.
    fn rust_files(directory: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(directory).expect("read the crate sources") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                found.extend(DeviceScan::rust_files(&path));
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
        found
    }

    /// The label (`module::Type`) of every `impl Device for Type` of the crate's sources.
    fn labels() -> BTreeSet<String> {
        let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut labels = BTreeSet::new();
        for path in DeviceScan::rust_files(&sources) {
            let text = std::fs::read_to_string(&path).expect("read a source");
            let relative = path.strip_prefix(&sources).expect("under src").with_extension("");
            let module = relative.components().map(|part| part.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("::");
            for (at, _) in text.match_indices("impl Device for ") {
                let name: String = text[at + "impl Device for ".len()..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                labels.insert(format!("{module}::{name}"));
            }
        }
        labels
    }
}

/// The modules of every device of the crate, by label.
struct EveryDevice;

impl EveryDevice {
    /// The devices of `analog`.
    fn analog(modules: &FixtureModules) -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "analog::AcVoltageSource", module: modules.ac_voltage_source, check: Check::Generic },
            ListedDevice { label: "analog::CapWatch", module: modules.cap_watch_device, check: Check::Generic },
            ListedDevice { label: "analog::ChargeDiode", module: modules.charge_diode, check: Check::Exempt("its exponential overflows at the generic points; its tests set their own") },
            ListedDevice { label: "analog::Clamp", module: modules.clamp, check: Check::Exempt("a limit it owns: the partials against the limit input are meaningful only at the held iterate its tests set") },
            ListedDevice { label: "analog::CorrectionOwner", module: modules.owner, check: Check::Exempt("a limit it owns: the partials against the limit input are meaningful only at the held iterate its tests set") },
            ListedDevice { label: "analog::CountedResistor", module: modules.counted, check: Check::Generic },
            ListedDevice { label: "analog::Cubic", module: modules.cubic, check: Check::Generic },
            ListedDevice { label: "analog::CurrentSource", module: modules.source, check: Check::Generic },
            ListedDevice { label: "analog::DelayLine", module: modules.delay_line, check: Check::Generic },
            ListedDevice { label: "analog::FixedTemperatureNoisyResistor", module: modules.noisy, check: Check::Generic },
            ListedDevice { label: "analog::Flicker", module: modules.flicker, check: Check::Generic },
            ListedDevice { label: "analog::LimitReader", module: modules.reader, check: Check::Generic },
            ListedDevice { label: "analog::ModeFaulty", module: modules.faulty, check: Check::Generic },
            ListedDevice { label: "analog::PlainDiode", module: modules.plain_diode, check: Check::Generic },
            ListedDevice { label: "analog::RecordingConductance", module: modules.probe, check: Check::Generic },
            ListedDevice { label: "analog::SampleHold", module: modules.sample_hold, check: Check::Generic },
            ListedDevice { label: "analog::Split", module: modules.split, check: Check::Generic },
            ListedDevice { label: "analog::ThresholdTracer", module: modules.tracer, check: Check::Generic },
            ListedDevice { label: "analog::TightCapacitor", module: modules.tight_capacitor, check: Check::Generic },
            ListedDevice { label: "analog::Tracker", module: modules.tracker, check: Check::Generic },
            ListedDevice { label: "analog::Trio", module: modules.trio, check: Check::Generic },
        ]
    }

    /// The devices of `analyses`.
    fn analyses() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "analyses::Bias", module: analyses::Bias::module(), check: Check::Generic },
            ListedDevice { label: "analyses::Quadratic", module: analyses::Quadratic::module(), check: Check::Generic },
            ListedDevice { label: "analyses::Wire", module: analyses::Wire::module(), check: Check::Generic },
        ]
    }

    /// The devices of `circuit`.
    fn circuit() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "circuit::AcCurrentSource", module: circuit::AcCurrentSource::module(), check: Check::Generic },
            ListedDevice { label: "circuit::Capacitor", module: circuit::Capacitor::module(), check: Check::Generic },
            ListedDevice { label: "circuit::Diode", module: circuit::Diode::module(), check: Check::Generic },
            ListedDevice { label: "circuit::Inductor", module: circuit::Inductor::module(), check: Check::Generic },
            ListedDevice { label: "circuit::NoisePair", module: circuit::NoisePair::module(), check: Check::Generic },
            ListedDevice { label: "circuit::NoisyResistor", module: circuit::NoisyResistor::module(), check: Check::Generic },
            ListedDevice { label: "circuit::OptionalResistor", module: circuit::OptionalResistor::module(), check: Check::Generic },
            ListedDevice { label: "circuit::RampSource", module: circuit::RampSource::module(), check: Check::Generic },
            ListedDevice { label: "circuit::Resistor", module: circuit::Resistor::module(), check: Check::Generic },
            ListedDevice { label: "circuit::SineSource", module: circuit::SineSource::module(), check: Check::Generic },
            ListedDevice { label: "circuit::VoltageSource", module: circuit::VoltageSource::module(), check: Check::Generic },
        ]
    }

    /// The devices of `digital`.
    fn digital() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "digital::And2", module: digital::And2::module(), check: Check::Generic },
            ListedDevice { label: "digital::BitFaulty", module: digital::BitFaulty::module(), check: Check::Exempt("faults on purpose") },
            ListedDevice { label: "digital::CheckedParameter", module: digital::CheckedParameter::module(), check: Check::Generic },
            ListedDevice { label: "digital::ClockWatch", module: digital::ClockWatch::module(), check: Check::Generic },
            ListedDevice { label: "digital::Counter", module: digital::Counter::module(), check: Check::Generic },
            ListedDevice { label: "digital::DataFlipFlop", module: digital::DataFlipFlop::module(), check: Check::Generic },
            ListedDevice { label: "digital::Inverter", module: digital::Inverter::module(), check: Check::Generic },
            ListedDevice { label: "digital::Lagged", module: digital::Lagged::module(), check: Check::Generic },
            ListedDevice { label: "digital::LaterDelayed", module: digital::LaterDelayed::module(), check: Check::Generic },
            ListedDevice { label: "digital::MultiRecorder", module: digital::MultiRecorder::module(), check: Check::Generic },
            ListedDevice { label: "digital::Odd", module: digital::Odd::module(), check: Check::Generic },
            ListedDevice { label: "digital::Or2", module: digital::Or2::module(), check: Check::Generic },
            ListedDevice { label: "digital::PulseGenerator", module: digital::PulseGenerator::module(), check: Check::Generic },
            ListedDevice { label: "digital::Rearm", module: digital::Rearm::module(), check: Check::Generic },
            ListedDevice { label: "digital::Resolver", module: digital::Resolver::module(), check: Check::Generic },
            ListedDevice { label: "digital::Sampler", module: digital::Sampler::module(), check: Check::Generic },
            ListedDevice { label: "digital::Setter", module: digital::Setter::module(), check: Check::Generic },
            ListedDevice { label: "digital::SettleEvent", module: digital::SettleEvent::module(), check: Check::Generic },
            ListedDevice { label: "digital::SettleRecorder", module: digital::SettleRecorder::module(), check: Check::Generic },
            ListedDevice { label: "digital::Settler", module: digital::Settler::module(), check: Check::Generic },
            ListedDevice { label: "digital::Stub", module: digital::Stub::module(), check: Check::Generic },
            ListedDevice { label: "digital::Successor", module: digital::Successor::module(), check: Check::Generic },
            ListedDevice { label: "digital::Timer", module: digital::Timer::module(), check: Check::Generic },
        ]
    }

    /// The devices of `examples`.
    fn examples(modules: &FixtureModules) -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "examples::DigitalToAnalog", module: modules.digital_to_analog, check: Check::Generic },
            ListedDevice { label: "examples::LimitingDiode", module: modules.diode, check: Check::Exempt("its exponential overflows at the generic points; its tests set their own") },
        ]
    }

    /// The devices of `mixed`.
    fn mixed() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "mixed::BitDigitalToAnalog", module: mixed::BitDigitalToAnalog::module(), check: Check::Generic },
            ListedDevice { label: "mixed::Capped", module: mixed::Capped::module(), check: Check::Generic },
            ListedDevice { label: "mixed::ClockSource", module: mixed::ClockSource::module(), check: Check::Generic },
            ListedDevice { label: "mixed::Comparator", module: mixed::Comparator::module(), check: Check::Generic },
            ListedDevice { label: "mixed::ConductanceTracer", module: mixed::ConductanceTracer::module(), check: Check::Generic },
            ListedDevice { label: "mixed::Delay", module: mixed::Delay::module(), check: Check::Generic },
            ListedDevice { label: "mixed::Finisher", module: mixed::Finisher::module(), check: Check::Generic },
            ListedDevice { label: "mixed::FlakyConductance", module: mixed::FlakyConductance::module(), check: Check::Generic },
            ListedDevice { label: "mixed::Globals", module: mixed::Globals::module(), check: Check::Generic },
            ListedDevice { label: "mixed::SettleCount", module: mixed::SettleCount::module(), check: Check::Generic },
            ListedDevice { label: "mixed::SettleTime", module: mixed::SettleTime::module(), check: Check::Generic },
            ListedDevice { label: "mixed::ToggleFlipFlop", module: mixed::ToggleFlipFlop::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::abi_ac`.
    fn solver_devices_abi_ac() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::abi_ac::Spy", module: solver_devices::abi_ac::Spy::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::abi_dc`.
    fn solver_devices_abi_dc() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::abi_dc::GminProbe", module: solver_devices::abi_dc::GminProbe::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::abi_noise`.
    fn solver_devices_abi_noise() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::abi_noise::NoisyR", module: solver_devices::abi_noise::NoisyR::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::abi_sensitivity`.
    fn solver_devices_abi_sensitivity() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::abi_sensitivity::Gd", module: solver_devices::abi_sensitivity::Gd::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::abi_tran`.
    fn solver_devices_abi_tran() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::abi_tran::ChargedC", module: solver_devices::abi_tran::ChargedC::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::abi_tran::FlakyC", module: solver_devices::abi_tran::FlakyC::module(), check: Check::Holding(&[(0, 3, 0.0)]) },
        ]
    }

    /// The devices of `solver_devices::array_windows`.
    fn solver_devices_array_windows() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::array_windows::Poker", module: solver_devices::array_windows::Poker::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::array_windows::Summer", module: solver_devices::array_windows::Summer::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::bind`.
    fn solver_devices_bind() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::bind::BoundValuesProbe", module: solver_devices::bind::BoundValuesProbe::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::bind::CreateCounter", module: solver_devices::bind::CreateCounter::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::checkpoint_restore`.
    fn solver_devices_checkpoint_restore() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::checkpoint_restore::AgedDiode", module: solver_devices::checkpoint_restore::AgedDiode::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::crossing_rules`.
    fn solver_devices_crossing_rules() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::crossing_rules::Riser", module: solver_devices::crossing_rules::Riser::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::crossing_rules::ToleranceWatch", module: solver_devices::crossing_rules::ToleranceWatch::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::digital_settle_subscriber`.
    fn solver_devices_digital_settle_subscriber() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::digital_settle_subscriber::InitialLowSetter", module: solver_devices::digital_settle_subscriber::InitialLowSetter::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::digital_settle_subscriber::LagInverter", module: solver_devices::digital_settle_subscriber::LagInverter::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::digital_settle_subscriber::Subscriber", module: solver_devices::digital_settle_subscriber::Subscriber::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::environment`.
    fn solver_devices_environment() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::environment::EnvProbe", module: solver_devices::environment::EnvProbe::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::environment::FlagProbe", module: solver_devices::environment::FlagProbe::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::environment::GminLoad", module: solver_devices::environment::GminLoad::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::failure_rules`.
    fn solver_devices_failure_rules() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::failure_rules::JumpOscillator", module: solver_devices::failure_rules::JumpOscillator::module(), check: Check::Holding(&[(0, 3, 0.0)]) },
            ListedDevice { label: "solver_devices::failure_rules::LateOscillator", module: solver_devices::failure_rules::LateOscillator::module(), check: Check::Holding(&[(0, 2, 0.0)]) },
            ListedDevice { label: "solver_devices::failure_rules::LateSource", module: solver_devices::failure_rules::LateSource::module(), check: Check::Holding(&[(0, 3, 0.0)]) },
            ListedDevice { label: "solver_devices::failure_rules::NanSource", module: solver_devices::failure_rules::NanSource::module(), check: Check::Fixtures(&[(0, Fixture::NonFinite)]) },
            ListedDevice { label: "solver_devices::failure_rules::OpenAboveDc", module: solver_devices::failure_rules::OpenAboveDc::module(), check: Check::Holding(&[(0, 2, 0.0)]) },
            ListedDevice { label: "solver_devices::failure_rules::Oscillator", module: solver_devices::failure_rules::Oscillator::module(), check: Check::Fixtures(&[(0, Fixture::Impure)]) },
        ]
    }

    /// The devices of `solver_devices::host_set`.
    fn solver_devices_host_set() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::host_set::CheckedR", module: solver_devices::host_set::CheckedR::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::host_set::Collapsible", module: solver_devices::host_set::Collapsible::module(), check: Check::Holding(&[(0, 0, 1.0e3)]) },
        ]
    }

    /// The devices of `solver_devices::jump_retake`.
    fn solver_devices_jump_retake() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::jump_retake::Gate", module: solver_devices::jump_retake::Gate::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::jump_retake::SettleMirror", module: solver_devices::jump_retake::SettleMirror::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::jump_retake::SettleSetter", module: solver_devices::jump_retake::SettleSetter::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::lifecycle`.
    fn solver_devices_lifecycle() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::lifecycle::Counted", module: solver_devices::lifecycle::Counted::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::lifecycle::Failing", module: solver_devices::lifecycle::Failing::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::limiting_report`.
    fn solver_devices_limiting_report() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::limiting_report::AlwaysLimited", module: solver_devices::limiting_report::AlwaysLimited::module(), check: Check::Fixtures(&[(0, Fixture::AlwaysLimited)]) },
            ListedDevice { label: "solver_devices::limiting_report::LimitedDiode", module: solver_devices::limiting_report::LimitedDiode::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::live_params`.
    fn solver_devices_live_params() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::live_params::Mute", module: solver_devices::live_params::Mute::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::noise_report`.
    fn solver_devices_noise_report() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::noise_report::TwinFlicker", module: solver_devices::noise_report::TwinFlicker::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::parity_baseline`.
    fn solver_devices_parity_baseline() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::parity_baseline::DigitalSource", module: solver_devices::parity_baseline::DigitalSource::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::parity_baseline::FixedDiode", module: solver_devices::parity_baseline::FixedDiode::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::parity_baseline::GatedISink", module: solver_devices::parity_baseline::GatedISink::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::parity_baseline::SineVsrc", module: solver_devices::parity_baseline::SineVsrc::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::parity_baseline::ThresholdComparator", module: solver_devices::parity_baseline::ThresholdComparator::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::probe_selection`.
    fn solver_devices_probe_selection() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::probe_selection::StateAndVariableProbe", module: solver_devices::probe_selection::StateAndVariableProbe::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::protocol_crossings`.
    fn solver_devices_protocol_crossings() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::protocol_crossings::Saturating", module: solver_devices::protocol_crossings::Saturating::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::protocol_crossings::ToleranceComparator", module: solver_devices::protocol_crossings::ToleranceComparator::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::protocol_real`.
    fn solver_devices_protocol_real() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::protocol_real::FlakyG", module: solver_devices::protocol_real::FlakyG::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::protocol_transient`.
    fn solver_devices_protocol_transient() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::protocol_transient::Flipper", module: solver_devices::protocol_transient::Flipper::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::pz_device_fault`.
    fn solver_devices_pz_device_fault() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::pz_device_fault::AcFault", module: solver_devices::pz_device_fault::AcFault::module(), check: Check::Holding(&[(0, 1, 0.0)]) },
        ]
    }

    /// The devices of `solver_devices::sensitivity_fallback`.
    fn solver_devices_sensitivity_fallback() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::sensitivity_fallback::Gan", module: solver_devices::sensitivity_fallback::Gan::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::sensitivity_fallback::Gfb", module: solver_devices::sensitivity_fallback::Gfb::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::settled_value`.
    fn solver_devices_settled_value() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::settled_value::SettledWatch", module: solver_devices::settled_value::SettledWatch::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::settled_value::TwoWrites", module: solver_devices::settled_value::TwoWrites::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::solver_entry`.
    fn solver_devices_solver_entry() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::solver_entry::BadElement", module: solver_devices::solver_entry::BadElement::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::temperature`.
    fn solver_devices_temperature() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::temperature::Thermo", module: solver_devices::temperature::Thermo::module(), check: Check::Generic },
        ]
    }

    /// The devices of `solver_devices::truncation`.
    fn solver_devices_truncation() -> Vec<ListedDevice> {
        vec![
            ListedDevice { label: "solver_devices::truncation::CubicCapacitor", module: solver_devices::truncation::CubicCapacitor::module(), check: Check::Generic },
            ListedDevice { label: "solver_devices::truncation::FaultOnce", module: solver_devices::truncation::FaultOnce::module(), check: Check::Holding(&[(0, 2, 0.0)]) },
            ListedDevice { label: "solver_devices::truncation::StepSource", module: solver_devices::truncation::StepSource::module(), check: Check::Holding(&[(0, 5, 0.0)]) },
        ]
    }

    /// Every device of the crate, in the order of the crate's modules.
    fn all() -> Vec<ListedDevice> {
        let modules = FixtureModules::all();
        let mut every = Vec::new();
        every.extend(EveryDevice::analog(&modules));
        every.extend(EveryDevice::analyses());
        every.extend(EveryDevice::circuit());
        every.extend(EveryDevice::digital());
        every.extend(EveryDevice::examples(&modules));
        every.extend(EveryDevice::mixed());
        every.extend(EveryDevice::solver_devices_abi_ac());
        every.extend(EveryDevice::solver_devices_abi_dc());
        every.extend(EveryDevice::solver_devices_abi_noise());
        every.extend(EveryDevice::solver_devices_abi_sensitivity());
        every.extend(EveryDevice::solver_devices_abi_tran());
        every.extend(EveryDevice::solver_devices_array_windows());
        every.extend(EveryDevice::solver_devices_bind());
        every.extend(EveryDevice::solver_devices_checkpoint_restore());
        every.extend(EveryDevice::solver_devices_crossing_rules());
        every.extend(EveryDevice::solver_devices_digital_settle_subscriber());
        every.extend(EveryDevice::solver_devices_environment());
        every.extend(EveryDevice::solver_devices_failure_rules());
        every.extend(EveryDevice::solver_devices_host_set());
        every.extend(EveryDevice::solver_devices_jump_retake());
        every.extend(EveryDevice::solver_devices_lifecycle());
        every.extend(EveryDevice::solver_devices_limiting_report());
        every.extend(EveryDevice::solver_devices_live_params());
        every.extend(EveryDevice::solver_devices_noise_report());
        every.extend(EveryDevice::solver_devices_parity_baseline());
        every.extend(EveryDevice::solver_devices_probe_selection());
        every.extend(EveryDevice::solver_devices_protocol_crossings());
        every.extend(EveryDevice::solver_devices_protocol_real());
        every.extend(EveryDevice::solver_devices_protocol_transient());
        every.extend(EveryDevice::solver_devices_pz_device_fault());
        every.extend(EveryDevice::solver_devices_sensitivity_fallback());
        every.extend(EveryDevice::solver_devices_settled_value());
        every.extend(EveryDevice::solver_devices_solver_entry());
        every.extend(EveryDevice::solver_devices_temperature());
        every.extend(EveryDevice::solver_devices_truncation());
        every
    }
}

#[test]
fn its_devices_pass_the_partials_verifier() {
    let mut checked = 0;
    for entry in EveryDevice::all() {
        match entry.check {
            Check::Generic => PartialsCheck::verify_every_function(entry.label, entry.module),
            Check::Holding(held) => PartialsCheck::verify_every_function_with(entry.label, entry.module, held),
            Check::Fixtures(fixtures) => PartialsCheck::verify_every_function_except(entry.label, entry.module, fixtures),
            Check::Exempt(reason) => {
                assert!(!reason.is_empty(), "{}: an exemption names its reason", entry.label);
                continue;
            }
        }
        checked += 1;
    }
    assert!(checked > 100, "the check reached {checked} devices only");
}

#[test]
fn every_device_of_the_crate_is_in_the_list() {
    let listed: BTreeSet<String> = EveryDevice::all().iter().map(|entry| entry.label.to_string()).collect();
    let declared = DeviceScan::labels();
    let missing: Vec<&String> = declared.difference(&listed).collect();
    let stale: Vec<&String> = listed.difference(&declared).collect();
    assert!(missing.is_empty(), "devices no row of EveryDevice names: {missing:?}");
    assert!(stale.is_empty(), "rows whose device is gone: {stale:?}");
    assert!(declared.len() > 100, "the scan found the crate's devices ({})", declared.len());
}
