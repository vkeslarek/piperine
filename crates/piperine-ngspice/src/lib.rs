mod backend;
mod hardware;
mod tasks;
pub mod expr_serializer;

pub use backend::NgspiceBackend;

/// Path to the `ppr/` directory bundled with this crate.
///
/// Pass to `piperine_parser::parse_with_includes()` so that
/// `` `include "ngspice.ppr" `` resolves correctly:
///
/// ```rust,ignore
/// use piperine_parser::parser::parse_with_includes;
/// use piperine_ngspice::ppr_dir;
///
/// let src = r#"`include "ngspice.ppr"
/// module tb; ... endmodule"#;
///
/// let dirs = vec![ppr_dir(), piperine_parser::bundled_header_dir()];
/// let doc = parse_with_includes(src, &dirs)?;
/// ```
pub fn ppr_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ppr")
}

use std::path::PathBuf;
use piperine_circuit::HardwareRegistry;
use piperine_interpreter::{Plugin, SimulatorBackend, SystemTaskRegistry};
use piperine_coordinator::pool::{ProcessPool, PoolConfig};

/// The ngspice plugin — registers all ngspice-backed hardware definitions,
/// system tasks, and provides the ngspice simulator backend.
///
/// Register this plugin at startup before any other plugin:
/// ```rust
/// runtime.register_plugin(Box::new(NgspicePlugin::default()));
/// ```
#[derive(Default)]
pub struct NgspicePlugin {
    /// Override path to the `piperine-worker` binary. `None` = auto-discover.
    pub worker_binary: Option<PathBuf>,
}

impl Plugin for NgspicePlugin {
    fn name(&self) -> &str { "ngspice" }

    fn register_hardware(&self, registry: &mut HardwareRegistry) {
        use hardware::*;
        registry.register(Box::new(SpiceResistor));
        registry.register(Box::new(SpiceCapacitor));
        registry.register(Box::new(SpiceInductor));
        registry.register(Box::new(SpiceMutual::new()));
        registry.register(Box::new(SpiceVoltageSource));
        registry.register(Box::new(SpiceCurrentSource));
        registry.register(Box::new(SpiceBSourceV::new()));
        registry.register(Box::new(SpiceBSourceI::new()));
        registry.register(Box::new(SpiceVpulse));
        registry.register(Box::new(SpiceIpulse));
        registry.register(Box::new(SpiceVsin));
        registry.register(Box::new(SpiceIsin));
        registry.register(Box::new(SpiceVexp));
        registry.register(Box::new(SpiceIexp));
        registry.register(Box::new(SpiceVpwl));
        registry.register(Box::new(SpiceIpwl));
        registry.register(Box::new(SpiceVsffm));
        registry.register(Box::new(SpiceVam));
        registry.register(Box::new(SpiceVnoise));
        registry.register(Box::new(SpiceVrandom));
        registry.register(Box::new(SpiceVcvs));
        registry.register(Box::new(SpiceVccs));
        registry.register(Box::new(SpiceCcvs));
        registry.register(Box::new(SpiceCccs));
        registry.register(Box::new(SpiceVsw));
        registry.register(Box::new(SpiceIsw));
        registry.register(Box::new(SpiceDiode));
        registry.register(Box::new(SpiceNpn));
        registry.register(Box::new(SpicePnp));
        registry.register(Box::new(SpiceNpn4));
        registry.register(Box::new(SpicePnp4));
        registry.register(Box::new(SpiceNmos));
        registry.register(Box::new(SpicePmos));
        registry.register(Box::new(SpiceJfetN));
        registry.register(Box::new(SpiceJfetP));
        registry.register(Box::new(SpiceMesfetN));
        registry.register(Box::new(SpiceMesfetP));
        registry.register(Box::new(SpiceVdmos));
        registry.register(Box::new(SpiceTline));
        registry.register(Box::new(SpiceLtra));
        registry.register(Box::new(SpiceUrc));
        registry.register(Box::new(SpicePort));
        registry.register(Box::new(SpiceSubckt));
        registry.register(Box::new(SpiceIsffm::new()));
        registry.register(Box::new(SpiceIam::new()));
        registry.register(Box::new(SpiceInoise::new()));
        registry.register(Box::new(SpiceIrandom::new()));
        registry.register(Box::new(SpiceCpl::new()));
        registry.register(Box::new(SpiceTxl::new()));
    }

    fn register_tasks(&self, registry: &mut SystemTaskRegistry) {
        use tasks::*;
        // stdlib tasks ($display, $fatal, $run_error, etc.) are already registered
        // by SystemTaskRegistry::default(). Only ngspice-specific tasks go here.

        // Core analyses
        registry.register(Box::new(OperatingPointTask));
        registry.register(Box::new(TransientTask));
        registry.register(Box::new(AcTask));
        registry.register(Box::new(DcTask));
        // Phase 3 analyses
        registry.register(Box::new(NoiseTask));
        registry.register(Box::new(TfTask));
        registry.register(Box::new(SensTask));
        registry.register(Box::new(SensAcTask));
        registry.register(Box::new(PzTask));
        registry.register(Box::new(DistoTask));
        registry.register(Box::new(PssTask));
        registry.register(Box::new(SpTask));
        // Scalar reads
        registry.register(Box::new(VoltageTask));
        registry.register(Box::new(CurrentTask));
        // $meas family
        registry.register(Box::new(MeasTask));
        registry.register(Box::new(MeasFindAtTask));
        registry.register(Box::new(MeasWhenTask));
        registry.register(Box::new(MeasTrigTargTask));
        registry.register(Box::new(MeasRmsTask));
        registry.register(Box::new(MeasAvgTask));
        registry.register(Box::new(MeasMinTask));
        registry.register(Box::new(MeasMaxTask));
        registry.register(Box::new(MeasMaxAtTask));
        registry.register(Box::new(MeasIntegralTask));
    }

    fn simulator_backend(&self) -> Option<Box<dyn SimulatorBackend>> {
        let config = PoolConfig { size: 1, worker_binary: self.worker_binary.clone() };
        let mut pool = match ProcessPool::spawn(config) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("piperine-ngspice: failed to spawn worker: {e}");
                return None;
            }
        };
        let handle = pool.take_first();
        Some(Box::new(NgspiceBackend::new(handle.cmd, handle.resp)))
    }
}
