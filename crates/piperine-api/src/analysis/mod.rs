mod ac;
mod dc;
mod disto;
mod noise;
mod op;
mod pss;
mod pz;
mod sens;
mod sp;
mod tf;
mod tran;

pub use ac::{AcAnalysis, AcOptions};
pub use dc::{DcAnalysis, DcSweep2};
pub use disto::DistortionAnalysis;
pub use noise::NoiseAnalysis;
pub use op::OpAnalysis;
pub use pss::PssAnalysis;
pub use pz::{PoleZeroAnalysis, PzAnalysisMode, PzTransferType};
pub use sens::SensAnalysis;
pub use sp::SParamAnalysis;
pub use tf::TfAnalysis;
pub use tran::{FourierSpec, IntegrationMethod, TranAnalysis, TranOptions};

use crate::node::Node;
use crate::options::SolverOptions;
use crate::spice::Measurement;

/// Emit common control lines: `.options` and `.nodeset` entries.
pub(super) fn emit_common(
    solver: &SolverOptions,
    extra_options: &str,
    nodesets: &[(Node, f64)],
    lines: &mut Vec<String>,
) {
    let solver_opts = solver.to_options_string();
    let all_opts = if extra_options.is_empty() {
        solver_opts
    } else if solver_opts.is_empty() {
        extra_options.to_string()
    } else {
        format!("{solver_opts} {extra_options}")
    };
    if !all_opts.is_empty() {
        lines.push(format!(".options {all_opts}"));
    }
    for (node, voltage) in nodesets {
        lines.push(format!(".nodeset V({})={}", node, voltage));
    }
}

/// Emit `meas` commands for all attached measurements.
pub(super) fn emit_meas(measurements: &[Measurement], analysis_type: &str, lines: &mut Vec<String>) {
    for m in measurements {
        lines.push(m.to_meas_cmd(analysis_type));
    }
}
