//! The partials verifier over every device of the fixtures crate (SAI-15, Port AC 4): each function of
//! each hand-written device agrees with its central difference at two generic points. The one list
//! below names every `impl Device` of the crate, and `every_device_of_the_crate_is_in_the_list` reads
//! the crate's sources to prove it: a device added without a row, or a row left after its device went,
//! fails.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use piperine_abi::device::Device;
use piperine_abi::raw::{Module, Shape};
use piperine_device_fixtures::analog;
use piperine_device_fixtures::analog::FixtureModules;
use piperine_device_fixtures::analyses;
use piperine_device_fixtures::circuit;
use piperine_device_fixtures::digital;
use piperine_device_fixtures::examples;
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

/// One device of the crate: its label (`module::Type`), its module and how its partials are checked. The
/// label and the module are built from one type, so a row cannot point at another device's module.
struct ListedDevice {
    /// `module::Type`, the path of the device inside the crate, taken from the type itself.
    label: String,
    /// The module the device builds.
    module: &'static Module,
    /// How the partials of the device are checked.
    check: Check,
}

impl ListedDevice {
    /// The row of the device `Type`: its label is the type's path, and `module` must be the module built
    /// from `Type`'s own prototype, or this panics.
    fn of<Type: Device>(module: &'static Module, check: Check) -> ListedDevice {
        let path = std::any::type_name::<Type>();
        let label = path.split_once("::").map_or(path, |(_, inside)| inside).to_string();
        let own = Type::prototype() as *const Shape<'static>;
        assert!(std::ptr::eq(module.prototype_ptr(), own), "the row of {label} holds the module of another device");
        ListedDevice { label, module, check }
    }
}

/// The sources of the crate, read to find every `impl Device`. The scan reaches only devices written as
/// `impl Device`: `BadLimit` and `Rogue` build raw modules on purpose and are checked by their own tests.
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
            ListedDevice::of::<analog::AcVoltageSource>(modules.ac_voltage_source, Check::Generic),
            ListedDevice::of::<analog::CapWatch>(modules.cap_watch_device, Check::Generic),
            ListedDevice::of::<analog::ChargeDiode>(modules.charge_diode, Check::Exempt("its exponential overflows at the generic points; its tests set their own")),
            ListedDevice::of::<analog::Clamp>(modules.clamp, Check::Exempt("a limit it owns: the partials against the limit input are meaningful only at the held iterate its tests set")),
            ListedDevice::of::<analog::CorrectionOwner>(modules.owner, Check::Exempt("a limit it owns: the partials against the limit input are meaningful only at the held iterate its tests set")),
            ListedDevice::of::<analog::CountedResistor>(modules.counted, Check::Generic),
            ListedDevice::of::<analog::Cubic>(modules.cubic, Check::Generic),
            ListedDevice::of::<analog::CurrentSource>(modules.source, Check::Generic),
            ListedDevice::of::<analog::DelayLine>(modules.delay_line, Check::Generic),
            ListedDevice::of::<analog::FixedTemperatureNoisyResistor>(modules.noisy, Check::Generic),
            ListedDevice::of::<analog::Flicker>(modules.flicker, Check::Generic),
            ListedDevice::of::<analog::LimitReader>(modules.reader, Check::Generic),
            ListedDevice::of::<analog::ModeFaulty>(modules.faulty, Check::Generic),
            ListedDevice::of::<analog::PlainDiode>(modules.plain_diode, Check::Generic),
            ListedDevice::of::<analog::RecordingConductance>(modules.probe, Check::Generic),
            ListedDevice::of::<analog::SampleHold>(modules.sample_hold, Check::Generic),
            ListedDevice::of::<analog::Split>(modules.split, Check::Generic),
            ListedDevice::of::<analog::ThresholdTracer>(modules.tracer, Check::Generic),
            ListedDevice::of::<analog::TightCapacitor>(modules.tight_capacitor, Check::Generic),
            ListedDevice::of::<analog::Tracker>(modules.tracker, Check::Generic),
            ListedDevice::of::<analog::Trio>(modules.trio, Check::Generic),
        ]
    }

    /// The devices of `analyses`.
    fn analyses() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<analyses::Bias>(analyses::Bias::module(), Check::Generic),
            ListedDevice::of::<analyses::Quadratic>(analyses::Quadratic::module(), Check::Generic),
            ListedDevice::of::<analyses::Wire>(analyses::Wire::module(), Check::Generic),
        ]
    }

    /// The devices of `circuit`.
    fn circuit() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<circuit::AcCurrentSource>(circuit::AcCurrentSource::module(), Check::Generic),
            ListedDevice::of::<circuit::Capacitor>(circuit::Capacitor::module(), Check::Generic),
            ListedDevice::of::<circuit::Diode>(circuit::Diode::module(), Check::Generic),
            ListedDevice::of::<circuit::Inductor>(circuit::Inductor::module(), Check::Generic),
            ListedDevice::of::<circuit::NoisePair>(circuit::NoisePair::module(), Check::Generic),
            ListedDevice::of::<circuit::NoisyResistor>(circuit::NoisyResistor::module(), Check::Generic),
            ListedDevice::of::<circuit::OptionalResistor>(circuit::OptionalResistor::module(), Check::Generic),
            ListedDevice::of::<circuit::RampSource>(circuit::RampSource::module(), Check::Generic),
            ListedDevice::of::<circuit::Resistor>(circuit::Resistor::module(), Check::Generic),
            ListedDevice::of::<circuit::SineSource>(circuit::SineSource::module(), Check::Generic),
            ListedDevice::of::<circuit::VoltageSource>(circuit::VoltageSource::module(), Check::Generic),
        ]
    }

    /// The devices of `digital`.
    fn digital() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<digital::And2>(digital::And2::module(), Check::Generic),
            ListedDevice::of::<digital::BitFaulty>(digital::BitFaulty::module(), Check::Exempt("faults on purpose")),
            ListedDevice::of::<digital::CheckedParameter>(digital::CheckedParameter::module(), Check::Generic),
            ListedDevice::of::<digital::ClockWatch>(digital::ClockWatch::module(), Check::Generic),
            ListedDevice::of::<digital::Counter>(digital::Counter::module(), Check::Generic),
            ListedDevice::of::<digital::DataFlipFlop>(digital::DataFlipFlop::module(), Check::Generic),
            ListedDevice::of::<digital::Inverter>(digital::Inverter::module(), Check::Generic),
            ListedDevice::of::<digital::Lagged>(digital::Lagged::module(), Check::Generic),
            ListedDevice::of::<digital::LaterDelayed>(digital::LaterDelayed::module(), Check::Generic),
            ListedDevice::of::<digital::MultiRecorder>(digital::MultiRecorder::module(), Check::Generic),
            ListedDevice::of::<digital::Odd>(digital::Odd::module(), Check::Generic),
            ListedDevice::of::<digital::Or2>(digital::Or2::module(), Check::Generic),
            ListedDevice::of::<digital::PulseGenerator>(digital::PulseGenerator::module(), Check::Generic),
            ListedDevice::of::<digital::Rearm>(digital::Rearm::module(), Check::Generic),
            ListedDevice::of::<digital::DriveDisjunction>(digital::DriveDisjunction::module(), Check::Generic),
            ListedDevice::of::<digital::Sampler>(digital::Sampler::module(), Check::Generic),
            ListedDevice::of::<digital::Setter>(digital::Setter::module(), Check::Generic),
            ListedDevice::of::<digital::SettleEvent>(digital::SettleEvent::module(), Check::Generic),
            ListedDevice::of::<digital::SettleRecorder>(digital::SettleRecorder::module(), Check::Generic),
            ListedDevice::of::<digital::Settler>(digital::Settler::module(), Check::Generic),
            ListedDevice::of::<digital::Stub>(digital::Stub::module(), Check::Generic),
            ListedDevice::of::<digital::Successor>(digital::Successor::module(), Check::Generic),
            ListedDevice::of::<digital::Timer>(digital::Timer::module(), Check::Generic),
        ]
    }

    /// The devices of `examples`.
    fn examples(modules: &FixtureModules) -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<examples::DigitalToAnalog>(modules.digital_to_analog, Check::Generic),
            ListedDevice::of::<examples::LimitingDiode>(modules.diode, Check::Exempt("its exponential overflows at the generic points; its tests set their own")),
        ]
    }

    /// The devices of `mixed`.
    fn mixed() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<mixed::BitDigitalToAnalog>(mixed::BitDigitalToAnalog::module(), Check::Generic),
            ListedDevice::of::<mixed::Capped>(mixed::Capped::module(), Check::Generic),
            ListedDevice::of::<mixed::ClockSource>(mixed::ClockSource::module(), Check::Generic),
            ListedDevice::of::<mixed::Comparator>(mixed::Comparator::module(), Check::Generic),
            ListedDevice::of::<mixed::ConductanceTracer>(mixed::ConductanceTracer::module(), Check::Generic),
            ListedDevice::of::<mixed::Delay>(mixed::Delay::module(), Check::Generic),
            ListedDevice::of::<mixed::Finisher>(mixed::Finisher::module(), Check::Generic),
            ListedDevice::of::<mixed::FlakyConductance>(mixed::FlakyConductance::module(), Check::Generic),
            ListedDevice::of::<mixed::Globals>(mixed::Globals::module(), Check::Generic),
            ListedDevice::of::<mixed::SettleCount>(mixed::SettleCount::module(), Check::Generic),
            ListedDevice::of::<mixed::SettleTime>(mixed::SettleTime::module(), Check::Generic),
            ListedDevice::of::<mixed::ToggleFlipFlop>(mixed::ToggleFlipFlop::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::abi_ac`.
    fn solver_devices_abi_ac() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::abi_ac::Spy>(solver_devices::abi_ac::Spy::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::abi_dc`.
    fn solver_devices_abi_dc() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::abi_dc::GminProbe>(solver_devices::abi_dc::GminProbe::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::abi_noise`.
    fn solver_devices_abi_noise() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::abi_noise::NoisyR>(solver_devices::abi_noise::NoisyR::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::abi_sensitivity`.
    fn solver_devices_abi_sensitivity() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::abi_sensitivity::Gd>(solver_devices::abi_sensitivity::Gd::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::abi_tran`.
    fn solver_devices_abi_tran() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::abi_tran::ChargedC>(solver_devices::abi_tran::ChargedC::module(), Check::Generic),
            ListedDevice::of::<solver_devices::abi_tran::FlakyC>(solver_devices::abi_tran::FlakyC::module(), Check::Holding(&[(0, 3, 0.0)])),
        ]
    }

    /// The devices of `solver_devices::array_windows`.
    fn solver_devices_array_windows() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::array_windows::Poker>(solver_devices::array_windows::Poker::module(), Check::Generic),
            ListedDevice::of::<solver_devices::array_windows::Summer>(solver_devices::array_windows::Summer::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::bind`.
    fn solver_devices_bind() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::bind::BoundValuesProbe>(solver_devices::bind::BoundValuesProbe::module(), Check::Generic),
            ListedDevice::of::<solver_devices::bind::CreateCounter>(solver_devices::bind::CreateCounter::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::checkpoint_restore`.
    fn solver_devices_checkpoint_restore() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::checkpoint_restore::AgedDiode>(solver_devices::checkpoint_restore::AgedDiode::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::crossing_rules`.
    fn solver_devices_crossing_rules() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::crossing_rules::Riser>(solver_devices::crossing_rules::Riser::module(), Check::Generic),
            ListedDevice::of::<solver_devices::crossing_rules::ToleranceWatch>(solver_devices::crossing_rules::ToleranceWatch::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::digital_settle_subscriber`.
    fn solver_devices_digital_settle_subscriber() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::digital_settle_subscriber::InitialLowSetter>(solver_devices::digital_settle_subscriber::InitialLowSetter::module(), Check::Generic),
            ListedDevice::of::<solver_devices::digital_settle_subscriber::LagInverter>(solver_devices::digital_settle_subscriber::LagInverter::module(), Check::Generic),
            ListedDevice::of::<solver_devices::digital_settle_subscriber::Subscriber>(solver_devices::digital_settle_subscriber::Subscriber::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::environment`.
    fn solver_devices_environment() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::environment::EnvProbe>(solver_devices::environment::EnvProbe::module(), Check::Generic),
            ListedDevice::of::<solver_devices::environment::FlagProbe>(solver_devices::environment::FlagProbe::module(), Check::Generic),
            ListedDevice::of::<solver_devices::environment::GminLoad>(solver_devices::environment::GminLoad::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::failure_rules`.
    fn solver_devices_failure_rules() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::failure_rules::JumpOscillator>(solver_devices::failure_rules::JumpOscillator::module(), Check::Holding(&[(0, 3, 0.0)])),
            ListedDevice::of::<solver_devices::failure_rules::LateOscillator>(solver_devices::failure_rules::LateOscillator::module(), Check::Holding(&[(0, 2, 0.0)])),
            ListedDevice::of::<solver_devices::failure_rules::LateSource>(solver_devices::failure_rules::LateSource::module(), Check::Holding(&[(0, 3, 0.0)])),
            ListedDevice::of::<solver_devices::failure_rules::NanSource>(solver_devices::failure_rules::NanSource::module(), Check::Fixtures(&[(0, Fixture::NonFinite)])),
            ListedDevice::of::<solver_devices::failure_rules::OpenAboveDc>(solver_devices::failure_rules::OpenAboveDc::module(), Check::Holding(&[(0, 2, 0.0)])),
            ListedDevice::of::<solver_devices::failure_rules::Oscillator>(solver_devices::failure_rules::Oscillator::module(), Check::Fixtures(&[(0, Fixture::Impure)])),
        ]
    }

    /// The devices of `solver_devices::host_set`.
    fn solver_devices_host_set() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::host_set::CheckedR>(solver_devices::host_set::CheckedR::module(), Check::Generic),
            ListedDevice::of::<solver_devices::host_set::Collapsible>(solver_devices::host_set::Collapsible::module(), Check::Holding(&[(0, 0, 1.0e3)])),
        ]
    }

    /// The devices of `solver_devices::jump_retake`.
    fn solver_devices_jump_retake() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::jump_retake::Gate>(solver_devices::jump_retake::Gate::module(), Check::Generic),
            ListedDevice::of::<solver_devices::jump_retake::SettleMirror>(solver_devices::jump_retake::SettleMirror::module(), Check::Generic),
            ListedDevice::of::<solver_devices::jump_retake::SettleSetter>(solver_devices::jump_retake::SettleSetter::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::lifecycle`.
    fn solver_devices_lifecycle() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::lifecycle::Counted>(solver_devices::lifecycle::Counted::module(), Check::Generic),
            ListedDevice::of::<solver_devices::lifecycle::Failing>(solver_devices::lifecycle::Failing::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::limiting_report`.
    fn solver_devices_limiting_report() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::limiting_report::AlwaysLimited>(solver_devices::limiting_report::AlwaysLimited::module(), Check::Fixtures(&[(0, Fixture::AlwaysLimited)])),
            ListedDevice::of::<solver_devices::limiting_report::LimitedDiode>(solver_devices::limiting_report::LimitedDiode::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::live_params`.
    fn solver_devices_live_params() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::live_params::Mute>(solver_devices::live_params::Mute::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::noise_report`.
    fn solver_devices_noise_report() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::noise_report::TwinFlicker>(solver_devices::noise_report::TwinFlicker::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::parity_baseline`.
    fn solver_devices_parity_baseline() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::parity_baseline::DigitalSource>(solver_devices::parity_baseline::DigitalSource::module(), Check::Generic),
            ListedDevice::of::<solver_devices::parity_baseline::FixedDiode>(solver_devices::parity_baseline::FixedDiode::module(), Check::Generic),
            ListedDevice::of::<solver_devices::parity_baseline::GatedISink>(solver_devices::parity_baseline::GatedISink::module(), Check::Generic),
            ListedDevice::of::<solver_devices::parity_baseline::SineVsrc>(solver_devices::parity_baseline::SineVsrc::module(), Check::Generic),
            ListedDevice::of::<solver_devices::parity_baseline::ThresholdComparator>(solver_devices::parity_baseline::ThresholdComparator::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::probe_selection`.
    fn solver_devices_probe_selection() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::probe_selection::StateAndVariableProbe>(solver_devices::probe_selection::StateAndVariableProbe::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::protocol_crossings`.
    fn solver_devices_protocol_crossings() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::protocol_crossings::Saturating>(solver_devices::protocol_crossings::Saturating::module(), Check::Generic),
            ListedDevice::of::<solver_devices::protocol_crossings::ToleranceComparator>(solver_devices::protocol_crossings::ToleranceComparator::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::protocol_real`.
    fn solver_devices_protocol_real() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::protocol_real::FlakyG>(solver_devices::protocol_real::FlakyG::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::protocol_transient`.
    fn solver_devices_protocol_transient() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::protocol_transient::Flipper>(solver_devices::protocol_transient::Flipper::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::pz_device_fault`.
    fn solver_devices_pz_device_fault() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::pz_device_fault::AcFault>(solver_devices::pz_device_fault::AcFault::module(), Check::Holding(&[(0, 1, 0.0)])),
        ]
    }

    /// The devices of `solver_devices::sensitivity_fallback`.
    fn solver_devices_sensitivity_fallback() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::sensitivity_fallback::Gan>(solver_devices::sensitivity_fallback::Gan::module(), Check::Generic),
            ListedDevice::of::<solver_devices::sensitivity_fallback::Gfb>(solver_devices::sensitivity_fallback::Gfb::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::settled_value`.
    fn solver_devices_settled_value() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::settled_value::SettledWatch>(solver_devices::settled_value::SettledWatch::module(), Check::Generic),
            ListedDevice::of::<solver_devices::settled_value::TwoWrites>(solver_devices::settled_value::TwoWrites::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::solver_entry`.
    fn solver_devices_solver_entry() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::solver_entry::BadElement>(solver_devices::solver_entry::BadElement::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::temperature`.
    fn solver_devices_temperature() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::temperature::Thermo>(solver_devices::temperature::Thermo::module(), Check::Generic),
        ]
    }

    /// The devices of `solver_devices::truncation`.
    fn solver_devices_truncation() -> Vec<ListedDevice> {
        vec![
            ListedDevice::of::<solver_devices::truncation::CubicCapacitor>(solver_devices::truncation::CubicCapacitor::module(), Check::Generic),
            ListedDevice::of::<solver_devices::truncation::FaultOnce>(solver_devices::truncation::FaultOnce::module(), Check::Holding(&[(0, 2, 0.0)])),
            ListedDevice::of::<solver_devices::truncation::StepSource>(solver_devices::truncation::StepSource::module(), Check::Holding(&[(0, 5, 0.0)])),
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
            Check::Generic => PartialsCheck::verify_every_function(&entry.label, entry.module),
            Check::Holding(held) => PartialsCheck::verify_every_function_with(&entry.label, entry.module, held),
            Check::Fixtures(fixtures) => PartialsCheck::verify_every_function_except(&entry.label, entry.module, fixtures),
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
    let listed: BTreeSet<String> = EveryDevice::all().iter().map(|entry| entry.label.clone()).collect();
    let declared = DeviceScan::labels();
    let missing: Vec<&String> = declared.difference(&listed).collect();
    let stale: Vec<&String> = listed.difference(&declared).collect();
    assert!(missing.is_empty(), "devices no row of EveryDevice names: {missing:?}");
    assert!(stale.is_empty(), "rows whose device is gone: {stale:?}");
    assert!(declared.len() > 100, "the scan found the crate's devices ({})", declared.len());
}

/// HYG-16 AC 3: a check counts for the device its module builds, not for its label. A row whose module
/// is another device's cannot be written: the row panics when it is built.
#[test]
#[should_panic(expected = "holds the module of another device")]
fn a_check_of_another_module_does_not_count() {
    ListedDevice::of::<circuit::Resistor>(circuit::Capacitor::module(), Check::Generic);
}

/// HYG-16 AC 3: the row of a device built from its own module carries the device's own path as its label.
#[test]
fn a_row_takes_its_label_from_its_type() {
    let row = ListedDevice::of::<circuit::Resistor>(circuit::Resistor::module(), Check::Generic);
    assert_eq!(row.label, "circuit::Resistor");
}
