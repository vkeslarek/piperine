use crate::node::Node;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::num::Expr;
use crate::result::SimulationResult;

/// Trait for device instance serialization to a SPICE component line.
pub trait SpiceComponent {
    /// Generates the SPICE instance line (e.g. `R1 in out 10k`).
    fn into_spice(&self) -> String;
}

/// Trait for model serialization to a SPICE `.MODEL` line.
///
/// All device-specific model traits (e.g. `ResistorModel`, `DiodeModel`)
/// must inherit from this trait so that `Arc<dyn XxxModel>` can be upcast
/// to `Arc<dyn SpiceModel>` for deduplication in the circuit.
pub trait SpiceModel: Debug + Send + Sync {
    /// Model name (used for deduplication in the circuit).
    fn model_name(&self) -> &str;

    /// Generates the `.MODEL name type (params...)` line.
    fn to_spice_model_line(&self) -> String;
}

/// Supertrait for anything that can be placed in a [`Circuit`](crate::circuit::Circuit).
///
/// Combines SPICE serialization (`SpiceComponent`) with metadata access.
/// All 18 device types implement this trait.
pub trait SpiceElement: SpiceComponent + Debug + Send + Sync {
    /// Instance name of this element (e.g. `"R1"`, `"M1"`).
    fn element_name(&self) -> &str;

    /// Returns the model used by this device, if any.
    /// Used by the Circuit to collect `.MODEL` lines for the netlist.
    fn spice_model(&self) -> Option<Arc<dyn SpiceModel>> {
        None
    }

    /// Returns a lightweight reference to this element (for use in Probes).
    fn element_ref(&self) -> ElementRef;
}

/// A lightweight, cloneable reference to a circuit element.
///
/// Used by [`Probe::Current`] to reference a device without borrowing it.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ElementRef {
    /// SPICE symbol prefix (e.g. `"R"`, `"V"`, `"M"`).
    pub symbol: &'static str,
    /// Instance name (e.g. `"1"` for `R1`).
    pub name: String,
}

impl ElementRef {
    pub fn new(symbol: &'static str, name: impl Into<String>) -> Self {
        Self {
            symbol,
            name: name.into(),
        }
    }

    /// Full SPICE name (e.g. `"R1"`, `"V1"`).
    pub fn spice_name(&self) -> String {
        format!("{}{}", self.symbol, self.name)
    }
}

/// A typed probe for saving simulation data.
///
/// Replaces string-based `save("V(out)")` with typed references.
#[derive(Debug, Clone)]
pub enum Probe {
    /// Node voltage: `V(node)`.
    Voltage(Node),
    /// Differential voltage: `V(n1, n2)`.
    VoltageDiff(Node, Node),
    /// Branch current through a device: `I(device)`.
    Current(ElementRef),
}

impl Probe {
    pub fn voltage(node: Node) -> Self {
        Probe::Voltage(node)
    }

    pub fn voltage_diff(p: Node, n: Node) -> Self {
        Probe::VoltageDiff(p, n)
    }

    pub fn current(elem: ElementRef) -> Self {
        Probe::Current(elem)
    }

    /// Renders to the SPICE save expression (e.g. `"V(net_1)"`, `"I(R1)"`).
    pub fn to_spice_save(&self) -> String {
        match self {
            Probe::Voltage(n) => format!("V({})", n),
            Probe::VoltageDiff(p, n) => format!("V({},{})", p, n),
            Probe::Current(e) => format!("I({})", e.spice_name()),
        }
    }
}

// ===== Measurement =====

static MEAS_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_meas_name() -> String {
    format!("ppm_{}", MEAS_COUNTER.fetch_add(1, Ordering::Relaxed))
}

/// Variant of a measurement directive.
#[derive(Debug, Clone)]
pub enum MeasKind {
    Max(Probe),
    Min(Probe),
    Avg(Probe),
    Rms(Probe),
    Pp(Probe),
    Integ(Probe),
    /// PARAM: computed from an expression referencing other measurement names.
    Param(Expr),
}

/// A typed measurement handle.
///
/// Created via `Measurement::max(probe)` etc. Carries an auto-generated internal
/// name so it can look up its own scalar result from `SimulationResult`.
///
/// Clone the handle to pass it to `.meas()` while keeping a copy for later lookup:
/// ```
/// let vmax = Measurement::max(Probe::voltage(out));
/// let analysis = TranAnalysis::new(1e-4, 1e-3).meas(vmax.clone());
/// // ...
/// let v: f64 = vmax.get(&result).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct Measurement {
    name: String,
    kind: MeasKind,
}

impl Measurement {
    fn new(kind: MeasKind) -> Self {
        Self {
            name: next_meas_name(),
            kind,
        }
    }

    pub fn max(probe: Probe) -> Self {
        Self::new(MeasKind::Max(probe))
    }

    pub fn min(probe: Probe) -> Self {
        Self::new(MeasKind::Min(probe))
    }

    pub fn avg(probe: Probe) -> Self {
        Self::new(MeasKind::Avg(probe))
    }

    pub fn rms(probe: Probe) -> Self {
        Self::new(MeasKind::Rms(probe))
    }

    pub fn pp(probe: Probe) -> Self {
        Self::new(MeasKind::Pp(probe))
    }

    pub fn integ(probe: Probe) -> Self {
        Self::new(MeasKind::Integ(probe))
    }

    pub fn param(expr: Expr) -> Self {
        Self::new(MeasKind::Param(expr))
    }

    /// Returns the measurement's internal name (used as the key in `SimulationResult::measurements`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns an `Expr` node that references this measurement's result.
    ///
    /// Use this to build PARAM expressions that depend on the outcome of this measurement:
    /// ```
    /// let gain = Measurement::param(vout_max.expr() / vin_max.expr());
    /// ```
    pub fn expr(&self) -> Expr {
        Expr::MeasResult(self.name.clone())
    }

    /// Generates the ngspice interactive-mode `meas` command.
    /// `analysis_type` is one of: `"op"`, `"dc"`, `"ac"`, `"tran"`, `"noise"`, `"tf"`, `"sens"`.
    pub(crate) fn to_meas_cmd(&self, analysis_type: &str) -> String {
        match &self.kind {
            MeasKind::Max(p) => {
                format!("meas {} {} MAX {}", analysis_type, self.name, p.to_spice_save())
            }
            MeasKind::Min(p) => {
                format!("meas {} {} MIN {}", analysis_type, self.name, p.to_spice_save())
            }
            MeasKind::Avg(p) => {
                format!("meas {} {} AVG {}", analysis_type, self.name, p.to_spice_save())
            }
            MeasKind::Rms(p) => {
                format!("meas {} {} RMS {}", analysis_type, self.name, p.to_spice_save())
            }
            MeasKind::Pp(p) => {
                format!("meas {} {} PP {}", analysis_type, self.name, p.to_spice_save())
            }
            MeasKind::Integ(p) => {
                format!("meas {} {} INTEG {}", analysis_type, self.name, p.to_spice_save())
            }
            MeasKind::Param(e) => format!(
                "meas {} {} PARAM '{}'",
                analysis_type,
                self.name,
                e.to_ngspice()
            ),
        }
    }

    /// Looks up the scalar result in a `SimulationResult`.
    pub fn get(&self, result: &SimulationResult) -> Option<f64> {
        result.get_measurement(self)
    }
}

/// Trait for types that can be serialized to SPICE netlist lines.
///
/// This is the boundary between the typed domain model and the SPICE text format.
pub trait ToSpiceNetlist {
    fn to_spice_netlist(&self) -> Vec<String>;
}

/// Trait for analysis types that can produce SPICE control commands.
///
/// Separates the SPICE serialization concern from the domain analysis types.
pub trait SpiceAnalysis {
    fn to_spice_control_commands(&self) -> Vec<String>;
}
