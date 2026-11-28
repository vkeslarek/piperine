//! What a `noise` run names: the output it measures and the input it refers the noise to
//! (analog-operators AO-05). Both are resolved against the design before anything compiles, so
//! a name that matches nothing is `E0801`, never a solver fault.

use piperine_ir::Ir;
use piperine_solver::prelude::{FlowOutput, GroundName, NodeIdentifier, NoiseAnalysisResult};

use crate::failure::Failure;
use crate::probe::Probe;

/// The output of a noise run, resolved to what the solver measures.
#[derive(Debug, Clone, PartialEq)]
pub enum NoiseOutput {
    /// The potential `plus` against `minus`: a node alone is measured against ground.
    Potential { plus: NodeIdentifier, minus: NodeIdentifier },
    /// A flow: the current of an instance's branch, or of its one flow term.
    Flow(FlowOutput),
}

/// The output and the input of one noise run, as written and as the solver reads them.
#[derive(Debug, Clone, PartialEq)]
pub struct NoiseProbe {
    /// The output as the request wrote it (the column's label).
    pub label: String,
    /// What the solver measures.
    pub output: NoiseOutput,
    /// The full path of the input instance, whose `ac_stim` is the stimulus.
    pub input: Option<String>,
}

impl NoiseProbe {
    /// Resolves `output` (a node, `v(a, b)` or `i(<instance>)`) and `input` (an instance whose
    /// body has an `ac_stim`) against `ir`, each in full or relative to `top`. `E0801` names the
    /// path that matches nothing.
    pub fn resolve(ir: &Ir, top: &str, output: &str, input: Option<&str>) -> Result<NoiseProbe, Failure> {
        Ok(NoiseProbe { label: output.to_string(), output: NoiseProbe::output_of(ir, top, output)?, input: input.map(|path| NoiseProbe::input_of(ir, top, path)).transpose()? })
    }

    /// The unit of the output density: volts or amperes per root hertz.
    pub fn unit(&self) -> &'static str {
        match self.output {
            NoiseOutput::Potential { .. } => "V/sqrt(Hz)",
            NoiseOutput::Flow(_) => "A/sqrt(Hz)",
        }
    }

    /// The output `entry` names.
    fn output_of(ir: &Ir, top: &str, entry: &str) -> Result<NoiseOutput, Failure> {
        let text = entry.trim();
        if let Some(pair) = text.strip_prefix("v(").and_then(|rest| rest.strip_suffix(')')) {
            let node = |path: &str| NoiseProbe::node_of(ir, top, path.trim(), entry);
            return match pair.split_once(',') {
                Some((plus, minus)) => Ok(NoiseOutput::Potential { plus: node(plus)?, minus: node(minus)? }),
                None => Ok(NoiseOutput::Potential { plus: node(pair)?, minus: NodeIdentifier::Gnd }),
            };
        }
        if text.starts_with("i(") {
            return match Probe::resolve(ir, top, text)? {
                Probe::Flow { branch, .. } => Ok(NoiseOutput::Flow(FlowOutput::Branch(branch))),
                Probe::Term { instance, term } => Ok(NoiseOutput::Flow(FlowOutput::Term { instance, term })),
                other => Err(NoiseProbe::nothing(entry, &format!("`{}` is not a flow", other.label()))),
            };
        }
        Ok(NoiseOutput::Potential { plus: NoiseProbe::node_of(ir, top, text, entry)?, minus: NodeIdentifier::Gnd })
    }

    /// The node `path` names; `entry` is the whole request for the fault.
    fn node_of(ir: &Ir, top: &str, path: &str, entry: &str) -> Result<NodeIdentifier, Failure> {
        if GroundName::names(path) {
            return Ok(NodeIdentifier::Gnd);
        }
        if let Some(refusal) = GroundName::refusal(path) {
            return Err(NoiseProbe::nothing(entry, &refusal));
        }
        match Probe::resolve(ir, top, path)? {
            Probe::Node { id, .. } => Ok(id),
            _ => Err(NoiseProbe::nothing(entry, &format!("`{path}` is not a node")))
        }
    }

    /// The full path of the instance `path` names, which must have an `ac_stim`.
    fn input_of(ir: &Ir, top: &str, path: &str) -> Result<String, Failure> {
        let found = [path.to_string(), format!("{top}.{path}")].into_iter().find_map(|full| ir.instance_at(&full));
        let Some(instance) = found else { return Err(NoiseProbe::nothing(path, "no instance of that path")) };
        if instance.body().stims().is_empty() {
            return Err(NoiseProbe::nothing(path, &format!("the instance `{}` has no `ac_stim`, so it cannot be the input of a noise run", instance.path())));
        }
        Ok(instance.path().to_string())
    }

    /// The `E0801` of `entry`, a request that names nothing in the design, for `why`.
    fn nothing(entry: &str, why: &str) -> Failure {
        Failure::Host { code: "E0801", message: format!("`{entry}` names nothing in the design: {why}") }
    }
}

/// The result of a noise run as the host reads it: densities in V/√Hz or A/√Hz (the square
/// root of the solver's power densities), the band totals in V or A, and each group's density
/// (a source's name, else its instance path).
#[derive(Debug, Clone, PartialEq)]
pub struct NoiseReport {
    /// The frequencies of the sweep, hertz.
    pub frequencies: Vec<f64>,
    /// The output density at each frequency.
    pub output: Vec<f64>,
    /// With an input: the output density referred to it, at each frequency.
    pub input: Option<Vec<f64>>,
    /// The band total of the output noise (`onoise_total`).
    pub output_total: f64,
    /// With an input: the band total of the input-referred noise (`inoise_total`).
    pub input_total: Option<f64>,
    /// Each group's density at each frequency, in first-seen order.
    pub groups: Vec<(String, Vec<f64>)>,
    /// The unit of the densities.
    pub unit: &'static str,
}

impl NoiseReport {
    /// The report of the solver's `result` for an output of `unit`: the root of each power
    /// density, and the contributions that share a name summed into one group.
    pub fn of(result: &NoiseAnalysisResult, unit: &'static str) -> NoiseReport {
        let root = |powers: &[f64]| powers.iter().map(|power| power.sqrt()).collect::<Vec<f64>>();
        let mut powers: Vec<(String, Vec<f64>)> = Vec::new();
        for contribution in result.contributions() {
            let name = if contribution.source.is_empty() { contribution.element.clone() } else { contribution.source.clone() };
            match powers.iter_mut().find(|(group, _)| *group == name) {
                Some((_, series)) => series.iter_mut().zip(&contribution.psd).for_each(|(sum, power)| *sum += power),
                None => powers.push((name, contribution.psd.clone())),
            }
        }
        NoiseReport {
            frequencies: result.frequencies.clone(),
            output: root(&result.out_noise_sq),
            input: result.input_noise_sq.as_deref().map(root),
            output_total: result.output_total,
            input_total: result.input_total,
            groups: powers.into_iter().map(|(name, series)| (name, root(&series))).collect(),
            unit,
        }
    }
}
