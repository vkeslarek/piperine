//! What the structure tests read off a bench of a project: the output node `o` at the operating
//! point, over an AC sweep and over a transient.

use piperine_solver::prelude::{AcSweepAnalysisOptions, Context, NodeIdentifier};

use crate::mixed::BuiltDesign;
use crate::project::Project;
use crate::waveform::Waveform;

/// The output node `o` of the benches of one project, read at each analysis.
pub struct OutputProbe {
    project: Project,
}

impl OutputProbe {
    /// A probe over the benches of `project`.
    pub fn new(project: Project) -> OutputProbe {
        OutputProbe { project }
    }

    /// The node `<top>.<name>` of an elaborated design.
    pub fn node(ir: &piperine_ir::Ir, top: &str, name: &str) -> NodeIdentifier {
        NodeIdentifier::Anonymous(ir.node_at(&format!("{top}.{name}")).expect("a node").id().index())
    }

    /// `v(o)` of the bench `top` at its operating point.
    pub fn operating_point(&self, top: &str) -> f64 {
        let mut running = self.project.compile(top).unwrap_or_else(|r| panic!("{r}"));
        let out = OutputProbe::node(&running.ir, top, "o");
        running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves").get_node(&out).expect("v(o)")
    }

    /// `|v(o)|` of the bench `top` at each frequency of a sweep of `steps` points from `start` to `stop`.
    pub fn ac_magnitudes(&self, top: &str, start: f64, stop: f64, steps: usize) -> Vec<(f64, f64)> {
        let mut running = self.project.compile(top).unwrap_or_else(|r| panic!("{r}"));
        let out = OutputProbe::node(&running.ir, top, "o");
        let mut ac = running.circuit.ac(Context::default()).expect("ac");
        let sweep = ac.solve_sweep(AcSweepAnalysisOptions { start_frequency: start, stop_frequency: stop, steps, logarithmic: true }).unwrap_or_else(|e| panic!("the sweep does not solve: {e}"));
        sweep.iter().map(|step| (step.frequency, step.get_node(&out).expect("v(o)").norm())).collect()
    }

    /// `v(o)` of the bench `top` over a transient to `stop` with the step cap `step`, as a waveform.
    pub fn transient(&self, top: &str, stop: f64, step: f64) -> Waveform {
        let built = BuiltDesign::of(&self.project, top);
        let out = OutputProbe::node(&built.ir, top, "o");
        let (_, result) = built.run_with(stop, step);
        let steps: Vec<_> = result.iter().collect();
        Waveform::new(steps.iter().map(|s| s.time()).collect(), steps.iter().map(|s| s.get_node(&out).expect("v(o)")).collect())
    }
}
