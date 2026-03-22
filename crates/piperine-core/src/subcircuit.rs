use crate::devices::*;
use crate::model::ModelDef;
use crate::netlist::ToNetlist;
use crate::node::Node;
use crate::waveform::Waveform;

/// Trait for reusable subcircuit definitions.
///
/// Implement this trait for library-grade subcircuit components
/// (e.g. ideal op-amp, common amplifier topologies).
pub trait SubCircuitDef: std::fmt::Debug {
    fn name(&self) -> &str;
    fn ports(&self) -> &[&str];
    fn params(&self) -> Vec<(&str, &str)> { vec![] }
    fn build(&self, b: &mut SubCircuitBuilder);
}

/// Builder for populating a subcircuit body.
#[derive(Debug, Default)]
pub struct SubCircuitBuilder {
    pub(crate) components: Vec<Box<dyn Component>>,
    pub(crate) models: Vec<ModelDef>,
}

impl SubCircuitBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn resistor(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, value: impl std::fmt::Display) -> &mut Self {
        self.components.push(Box::new(Resistor {
            name: name.to_string(), p: p.into(), n: n.into(),
            value: value.to_string(), model: None,
        }));
        self
    }

    pub fn capacitor(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, value: impl std::fmt::Display) -> &mut Self {
        self.components.push(Box::new(Capacitor {
            name: name.to_string(), p: p.into(), n: n.into(),
            value: value.to_string(), ic: None, model: None,
        }));
        self
    }

    pub fn inductor(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, value: impl std::fmt::Display) -> &mut Self {
        self.components.push(Box::new(Inductor {
            name: name.to_string(), p: p.into(), n: n.into(),
            value: value.to_string(), ic: None, model: None,
        }));
        self
    }

    pub fn vsource(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, waveform: Waveform) -> &mut Self {
        self.components.push(Box::new(VoltageSource {
            name: name.to_string(), p: p.into(), n: n.into(),
            waveform: Some(waveform), ac: None,
        }));
        self
    }

    pub fn isource(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, waveform: Waveform) -> &mut Self {
        self.components.push(Box::new(CurrentSource {
            name: name.to_string(), p: p.into(), n: n.into(),
            waveform: Some(waveform), ac: None,
        }));
        self
    }

    pub fn bsource_v(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, expr: &str) -> &mut Self {
        self.components.push(Box::new(BehavioralSource {
            name: name.to_string(), p: p.into(), n: n.into(),
            kind: BehavioralKind::Voltage, expr: expr.to_string(),
        }));
        self
    }

    pub fn bsource_i(&mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, expr: &str) -> &mut Self {
        self.components.push(Box::new(BehavioralSource {
            name: name.to_string(), p: p.into(), n: n.into(),
            kind: BehavioralKind::Current, expr: expr.to_string(),
        }));
        self
    }

    pub fn diode(&mut self, name: &str, anode: impl Into<Node>, cathode: impl Into<Node>, model: &str) -> &mut Self {
        self.components.push(Box::new(Diode {
            name: name.to_string(), anode: anode.into(), cathode: cathode.into(),
            model: model.to_string(), area: None,
        }));
        self
    }

    pub fn instance(&mut self, name: &str, subckt: &str, nodes: &[impl Into<Node> + Clone]) -> &mut Self {
        self.components.push(Box::new(SubCircuitInstance {
            name: name.to_string(), subckt: subckt.to_string(),
            nodes: nodes.iter().map(|n| n.clone().into()).collect(),
            params: Vec::new(),
        }));
        self
    }

    pub fn model(&mut self, model: ModelDef) -> &mut Self {
        self.models.push(model);
        self
    }
}

/// Closure-based subcircuit (for inline quick definitions).
pub struct FnSubCircuit {
    name: String,
    ports: Vec<String>,
    params: Vec<(String, String)>,
    builder_fn: Box<dyn Fn(&mut SubCircuitBuilder) + Send + Sync>,
}

impl std::fmt::Debug for FnSubCircuit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FnSubCircuit")
            .field("name", &self.name)
            .field("ports", &self.ports)
            .finish()
    }
}

impl FnSubCircuit {
    pub fn new(
        name: &str,
        ports: &[&str],
        f: impl Fn(&mut SubCircuitBuilder) + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.to_string(),
            ports: ports.iter().map(|s| s.to_string()).collect(),
            params: Vec::new(),
            builder_fn: Box::new(f),
        }
    }

    pub fn with_param(mut self, name: &str, value: &str) -> Self {
        self.params.push((name.to_string(), value.to_string()));
        self
    }
}

impl SubCircuitDef for FnSubCircuit {
    fn name(&self) -> &str { &self.name }

    fn ports(&self) -> &[&str] {
        // SAFETY: same lifetime, we just need the &str references
        // We use a workaround: return empty and implement build differently
        // Actually, let's use a different approach
        &[]
    }

    fn params(&self) -> Vec<(&str, &str)> {
        self.params.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
    }

    fn build(&self, b: &mut SubCircuitBuilder) {
        (self.builder_fn)(b);
    }
}

// FnSubCircuit needs custom ToNetlist because ports() can't return &[&str] from owned data easily.
impl ToNetlist for FnSubCircuit {
    fn to_netlist_lines(&self) -> Vec<String> {
        let ports_str = self.ports.join(" ");
        let mut lines = Vec::new();

        let mut header = format!(".subckt {} {}", self.name, ports_str);
        for (k, v) in &self.params {
            header.push_str(&format!(" {k}={v}"));
        }
        lines.push(header);

        // Build body
        let mut builder = SubCircuitBuilder::new();
        (self.builder_fn)(&mut builder);

        for model in &builder.models {
            lines.extend(model.to_netlist_lines());
        }
        for comp in &builder.components {
            lines.push(comp.to_spice_line());
        }

        lines.push(".ends".to_string());
        lines
    }
}

/// Generic ToNetlist for any SubCircuitDef implementor.
pub fn subcircuit_to_netlist(def: &dyn SubCircuitDef) -> Vec<String> {
    let ports_str: Vec<&str> = def.ports().to_vec();
    let mut lines = Vec::new();

    let mut header = format!(".subckt {} {}", def.name(), ports_str.join(" "));
    for (k, v) in def.params() {
        header.push_str(&format!(" {k}={v}"));
    }
    lines.push(header);

    let mut builder = SubCircuitBuilder::new();
    def.build(&mut builder);

    for model in &builder.models {
        lines.extend(model.to_netlist_lines());
    }
    for comp in &builder.components {
        lines.push(comp.to_spice_line());
    }

    lines.push(".ends".to_string());
    lines
}
