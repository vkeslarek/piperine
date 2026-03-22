use crate::devices::*;
use crate::model::ModelDef;
use crate::netlist::ToNetlist;
use crate::node::Node;
use crate::options::CircuitOptions;
use crate::subcircuit::{FnSubCircuit, SubCircuitBuilder, SubCircuitDef, subcircuit_to_netlist};
use crate::waveform::{AcSpec, Waveform};

/// A circuit: pure topology and physical properties.
///
/// Does NOT contain analysis configuration — that lives in separate
/// analysis types (OpAnalysis, TranAnalysis, etc.).
#[derive(Debug)]
pub struct Circuit {
    title: String,
    components: Vec<Box<dyn Component>>,
    models: Vec<ModelDef>,
    subcircuits: Vec<SubCircuitEntry>,
    params: Vec<(String, String)>,
    initial_conditions: Vec<(String, f64)>,
    includes: Vec<String>,
    libs: Vec<(String, Option<String>)>,
    options: CircuitOptions,
    raw_lines: Vec<String>,
}

/// Wrapper to hold either a trait-based or closure-based subcircuit.
enum SubCircuitEntry {
    Fn(FnSubCircuit),
    Trait(Box<dyn SubCircuitDefWrapper>),
}

impl std::fmt::Debug for SubCircuitEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubCircuitEntry::Fn(sc) => sc.fmt(f),
            SubCircuitEntry::Trait(sc) => sc.debug_fmt(f),
        }
    }
}

/// Helper trait to erase SubCircuitDef behind Box<dyn>.
trait SubCircuitDefWrapper: Send + Sync {
    fn to_netlist_lines(&self) -> Vec<String>;
    fn debug_fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
}

struct TraitSubCircuit<T: SubCircuitDef + Send + Sync>(T);

impl<T: SubCircuitDef + Send + Sync> SubCircuitDefWrapper for TraitSubCircuit<T> {
    fn to_netlist_lines(&self) -> Vec<String> {
        subcircuit_to_netlist(&self.0)
    }

    fn debug_fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Circuit {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            components: Vec::new(),
            models: Vec::new(),
            subcircuits: Vec::new(),
            params: Vec::new(),
            initial_conditions: Vec::new(),
            includes: Vec::new(),
            libs: Vec::new(),
            options: CircuitOptions::default(),
            raw_lines: Vec::new(),
        }
    }

    // === Circuit-level options ===

    pub fn temp(mut self, t: f64) -> Self { self.options.temp = Some(t); self }
    pub fn tnom(mut self, t: f64) -> Self { self.options.tnom = Some(t); self }
    pub fn scale(mut self, s: f64) -> Self { self.options.scale = Some(s); self }
    pub fn savecurrents(mut self) -> Self { self.options.savecurrents = true; self }

    // === Parameters ===

    pub fn param(mut self, name: &str, value: impl std::fmt::Display) -> Self {
        self.params.push((name.to_string(), value.to_string()));
        self
    }

    // === Initial conditions ===

    pub fn ic(mut self, node: &str, voltage: f64) -> Self {
        self.initial_conditions.push((node.to_string(), voltage));
        self
    }

    // === Includes ===

    pub fn include(mut self, path: &str) -> Self {
        self.includes.push(path.to_string());
        self
    }

    pub fn lib(mut self, path: &str, section: Option<&str>) -> Self {
        self.libs.push((path.to_string(), section.map(|s| s.to_string())));
        self
    }

    // === Raw SPICE lines ===

    pub fn raw(mut self, line: &str) -> Self {
        self.raw_lines.push(line.to_string());
        self
    }

    // === Models ===

    pub fn model(mut self, model: ModelDef) -> Self {
        self.models.push(model);
        self
    }

    // === Subcircuits ===

    pub fn subcircuit(mut self, def: impl SubCircuitDef + Send + Sync + 'static) -> Self {
        self.subcircuits.push(SubCircuitEntry::Trait(Box::new(TraitSubCircuit(def))));
        self
    }

    pub fn subcircuit_fn(
        mut self,
        name: &str,
        ports: &[&str],
        f: impl Fn(&mut SubCircuitBuilder) + Send + Sync + 'static,
    ) -> Self {
        self.subcircuits.push(SubCircuitEntry::Fn(FnSubCircuit::new(name, ports, f)));
        self
    }

    // === Passive Components ===

    pub fn resistor(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, value: impl std::fmt::Display) -> Self {
        self.components.push(Box::new(Resistor {
            name: name.to_string(), p: p.into(), n: n.into(),
            value: value.to_string(), model: None,
        }));
        self
    }

    pub fn resistor_model(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, value: impl std::fmt::Display, model: &str) -> Self {
        self.components.push(Box::new(Resistor {
            name: name.to_string(), p: p.into(), n: n.into(),
            value: value.to_string(), model: Some(model.to_string()),
        }));
        self
    }

    pub fn capacitor(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, value: impl std::fmt::Display) -> Self {
        self.components.push(Box::new(Capacitor {
            name: name.to_string(), p: p.into(), n: n.into(),
            value: value.to_string(), ic: None, model: None,
        }));
        self
    }

    pub fn capacitor_ic(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, value: impl std::fmt::Display, ic: f64) -> Self {
        self.components.push(Box::new(Capacitor {
            name: name.to_string(), p: p.into(), n: n.into(),
            value: value.to_string(), ic: Some(ic), model: None,
        }));
        self
    }

    pub fn inductor(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, value: impl std::fmt::Display) -> Self {
        self.components.push(Box::new(Inductor {
            name: name.to_string(), p: p.into(), n: n.into(),
            value: value.to_string(), ic: None, model: None,
        }));
        self
    }

    pub fn inductor_ic(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, value: impl std::fmt::Display, ic: f64) -> Self {
        self.components.push(Box::new(Inductor {
            name: name.to_string(), p: p.into(), n: n.into(),
            value: value.to_string(), ic: Some(ic), model: None,
        }));
        self
    }

    // === Independent Sources ===

    pub fn vdc(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, voltage: f64) -> Self {
        self.components.push(Box::new(VoltageSource {
            name: name.to_string(), p: p.into(), n: n.into(),
            waveform: Some(Waveform::Dc(voltage)), ac: None,
        }));
        self
    }

    pub fn idc(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, current: f64) -> Self {
        self.components.push(Box::new(CurrentSource {
            name: name.to_string(), p: p.into(), n: n.into(),
            waveform: Some(Waveform::Dc(current)), ac: None,
        }));
        self
    }

    pub fn vsource(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, waveform: Waveform) -> Self {
        self.components.push(Box::new(VoltageSource {
            name: name.to_string(), p: p.into(), n: n.into(),
            waveform: Some(waveform), ac: None,
        }));
        self
    }

    pub fn isource(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, waveform: Waveform) -> Self {
        self.components.push(Box::new(CurrentSource {
            name: name.to_string(), p: p.into(), n: n.into(),
            waveform: Some(waveform), ac: None,
        }));
        self
    }

    pub fn vac(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, magnitude: f64) -> Self {
        self.components.push(Box::new(VoltageSource {
            name: name.to_string(), p: p.into(), n: n.into(),
            waveform: None, ac: Some(AcSpec::new(magnitude)),
        }));
        self
    }

    pub fn vac_phase(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, magnitude: f64, phase: f64) -> Self {
        self.components.push(Box::new(VoltageSource {
            name: name.to_string(), p: p.into(), n: n.into(),
            waveform: None, ac: Some(AcSpec::new(magnitude).with_phase(phase)),
        }));
        self
    }

    pub fn v_external(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>) -> Self {
        self.components.push(Box::new(VoltageSource {
            name: name.to_string(), p: p.into(), n: n.into(),
            waveform: Some(Waveform::External), ac: None,
        }));
        self
    }

    pub fn i_external(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>) -> Self {
        self.components.push(Box::new(CurrentSource {
            name: name.to_string(), p: p.into(), n: n.into(),
            waveform: Some(Waveform::External), ac: None,
        }));
        self
    }

    // === Dependent Sources ===

    pub fn vcvs(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, cp: impl Into<Node>, cn: impl Into<Node>, gain: f64) -> Self {
        self.components.push(Box::new(Vcvs {
            name: name.to_string(), p: p.into(), n: n.into(),
            cp: cp.into(), cn: cn.into(), gain,
        }));
        self
    }

    pub fn vccs(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, cp: impl Into<Node>, cn: impl Into<Node>, gm: f64) -> Self {
        self.components.push(Box::new(Vccs {
            name: name.to_string(), p: p.into(), n: n.into(),
            cp: cp.into(), cn: cn.into(), gm,
        }));
        self
    }

    pub fn ccvs(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, vsource: &str, transresistance: f64) -> Self {
        self.components.push(Box::new(Ccvs {
            name: name.to_string(), p: p.into(), n: n.into(),
            vsource: vsource.to_string(), transresistance,
        }));
        self
    }

    pub fn cccs(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, vsource: &str, gain: f64) -> Self {
        self.components.push(Box::new(Cccs {
            name: name.to_string(), p: p.into(), n: n.into(),
            vsource: vsource.to_string(), gain,
        }));
        self
    }

    // === Behavioral Sources ===

    pub fn bsource_v(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, expr: &str) -> Self {
        self.components.push(Box::new(BehavioralSource {
            name: name.to_string(), p: p.into(), n: n.into(),
            kind: BehavioralKind::Voltage, expr: expr.to_string(),
        }));
        self
    }

    pub fn bsource_i(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, expr: &str) -> Self {
        self.components.push(Box::new(BehavioralSource {
            name: name.to_string(), p: p.into(), n: n.into(),
            kind: BehavioralKind::Current, expr: expr.to_string(),
        }));
        self
    }

    // === Semiconductor Devices ===

    pub fn diode(mut self, name: &str, anode: impl Into<Node>, cathode: impl Into<Node>, model: &str) -> Self {
        self.components.push(Box::new(Diode {
            name: name.to_string(), anode: anode.into(), cathode: cathode.into(),
            model: model.to_string(), area: None,
        }));
        self
    }

    pub fn bjt(mut self, name: &str, c: impl Into<Node>, b: impl Into<Node>, e: impl Into<Node>, model: &str) -> Self {
        self.components.push(Box::new(Bjt {
            name: name.to_string(), c: c.into(), b: b.into(), e: e.into(),
            model: model.to_string(), area: None,
        }));
        self
    }

    pub fn mosfet(mut self, name: &str, d: impl Into<Node>, g: impl Into<Node>, s: impl Into<Node>, b: impl Into<Node>, model: &str) -> Self {
        self.components.push(Box::new(Mosfet {
            name: name.to_string(), d: d.into(), g: g.into(), s: s.into(), b: b.into(),
            model: model.to_string(), params: Vec::new(),
        }));
        self
    }

    pub fn mosfet_params(mut self, name: &str, d: impl Into<Node>, g: impl Into<Node>, s: impl Into<Node>, b: impl Into<Node>, model: &str, params: Vec<(&str, &str)>) -> Self {
        self.components.push(Box::new(Mosfet {
            name: name.to_string(), d: d.into(), g: g.into(), s: s.into(), b: b.into(),
            model: model.to_string(),
            params: params.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        }));
        self
    }

    pub fn jfet(mut self, name: &str, d: impl Into<Node>, g: impl Into<Node>, s: impl Into<Node>, model: &str) -> Self {
        self.components.push(Box::new(Jfet {
            name: name.to_string(), d: d.into(), g: g.into(), s: s.into(),
            model: model.to_string(),
        }));
        self
    }

    // === Switches ===

    pub fn vswitch(mut self, name: &str, p: impl Into<Node>, n: impl Into<Node>, cp: impl Into<Node>, cn: impl Into<Node>, model: &str) -> Self {
        self.components.push(Box::new(VSwitch {
            name: name.to_string(), p: p.into(), n: n.into(),
            cp: cp.into(), cn: cn.into(), model: model.to_string(),
        }));
        self
    }

    // === Transmission Lines ===

    pub fn tline(mut self, name: &str, p1: impl Into<Node>, n1: impl Into<Node>, p2: impl Into<Node>, n2: impl Into<Node>, z0: f64, td: f64) -> Self {
        self.components.push(Box::new(TransmissionLine {
            name: name.to_string(), p1: p1.into(), n1: n1.into(),
            p2: p2.into(), n2: n2.into(), z0, td,
        }));
        self
    }

    // === Coupled Inductors ===

    pub fn mutual_inductor(mut self, name: &str, l1: &str, l2: &str, coupling: f64) -> Self {
        self.components.push(Box::new(MutualInductor {
            name: name.to_string(), l1: l1.to_string(), l2: l2.to_string(), coupling,
        }));
        self
    }

    // === Subcircuit Instances ===

    pub fn instance(mut self, name: &str, subckt: &str, nodes: &[impl Into<Node> + Clone]) -> Self {
        self.components.push(Box::new(SubCircuitInstance {
            name: name.to_string(), subckt: subckt.to_string(),
            nodes: nodes.iter().map(|n| n.clone().into()).collect(),
            params: Vec::new(),
        }));
        self
    }

    pub fn instance_params(mut self, name: &str, subckt: &str, nodes: &[impl Into<Node> + Clone], params: Vec<(&str, &str)>) -> Self {
        self.components.push(Box::new(SubCircuitInstance {
            name: name.to_string(), subckt: subckt.to_string(),
            nodes: nodes.iter().map(|n| n.clone().into()).collect(),
            params: params.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        }));
        self
    }
}

impl ToNetlist for Circuit {
    fn to_netlist_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        // Title
        lines.push(self.title.clone());

        // .param
        for (k, v) in &self.params {
            lines.push(format!(".param {k}={v}"));
        }

        // .options (circuit-level: temp, tnom, scale, savecurrents)
        let opts = self.options.to_options_string();
        if !opts.is_empty() {
            lines.push(format!(".options {opts}"));
        }

        // .include / .lib
        for path in &self.includes {
            lines.push(format!(".include {path}"));
        }
        for (path, section) in &self.libs {
            if let Some(sec) = section {
                lines.push(format!(".lib {path} {sec}"));
            } else {
                lines.push(format!(".lib {path}"));
            }
        }

        // .model
        for model in &self.models {
            lines.extend(model.to_netlist_lines());
        }

        // .subckt definitions
        for sc in &self.subcircuits {
            match sc {
                SubCircuitEntry::Fn(fn_sc) => lines.extend(fn_sc.to_netlist_lines()),
                SubCircuitEntry::Trait(t_sc) => lines.extend(t_sc.to_netlist_lines()),
            }
        }

        // Component instances
        for comp in &self.components {
            lines.push(comp.to_spice_line());
        }

        // .ic
        if !self.initial_conditions.is_empty() {
            let ics: Vec<String> = self.initial_conditions.iter()
                .map(|(node, v)| format!("V({node})={v}"))
                .collect();
            lines.push(format!(".ic {}", ics.join(" ")));
        }

        // Raw lines
        for line in &self.raw_lines {
            lines.push(line.clone());
        }

        // .end
        lines.push(".end".to_string());

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::GND;
    use crate::model::ModelKind;

    #[test]
    fn simple_resistor_divider() {
        let ckt = Circuit::new("Resistor Divider")
            .vdc("in", "in", GND, 10.0)
            .resistor("1", "in", "out", "10k")
            .resistor("2", "out", GND, "10k");

        let lines = ckt.to_netlist_lines();
        assert_eq!(lines[0], "Resistor Divider");
        assert!(lines.contains(&"Vin in 0 DC 10".to_string()));
        assert!(lines.contains(&"R1 in out 10k".to_string()));
        assert!(lines.contains(&"R2 out 0 10k".to_string()));
        assert_eq!(lines.last().unwrap(), ".end");
    }

    #[test]
    fn circuit_with_model() {
        let ckt = Circuit::new("Diode Test")
            .model(ModelDef::new("1N4148", ModelKind::D)
                .param("IS", "2.52e-9")
                .param("RS", "0.568"))
            .vdc("1", "in", GND, 5.0)
            .resistor("1", "in", "a", "1k")
            .diode("1", "a", GND, "1N4148");

        let lines = ckt.to_netlist_lines();
        assert!(lines.iter().any(|l| l.starts_with(".model 1N4148 D")));
        assert!(lines.contains(&"D1 a 0 1N4148".to_string()));
    }

    #[test]
    fn circuit_with_ic() {
        let ckt = Circuit::new("IC Test")
            .ic("out", 2.5)
            .resistor("1", "in", "out", "1k");

        let lines = ckt.to_netlist_lines();
        assert!(lines.iter().any(|l| l.starts_with(".ic V(out)=2.5")));
    }

    #[test]
    fn circuit_with_subcircuit_fn() {
        let ckt = Circuit::new("Subckt Test")
            .subcircuit_fn("buffer", &["in", "out"], |b| {
                b.bsource_v("E1", "out", "0", "V(in)");
            })
            .instance("1", "buffer", &["sig", "buf"]);

        let lines = ckt.to_netlist_lines();
        assert!(lines.contains(&".subckt buffer in out".to_string()));
        assert!(lines.contains(&"BE1 out 0 V=V(in)".to_string()));
        assert!(lines.contains(&".ends".to_string()));
        assert!(lines.contains(&"X1 sig buf buffer".to_string()));
    }
}
