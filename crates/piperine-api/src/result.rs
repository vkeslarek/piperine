use crate::node::Node;
use crate::spice::{ElementRef, Measurement, Probe};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of a simulation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub plots: HashMap<String, Plot>,
    pub measurements: HashMap<String, f64>,
    pub log: Vec<String>,
}

impl SimulationResult {
    /// Get a named measurement value (from `.meas`).
    pub fn measurement(&self, name: &str) -> Option<f64> {
        self.measurements.get(name).copied()
    }

    /// Get a vector by name from the first (or only) plot.
    pub fn vector(&self, name: &str) -> Option<&Vector> {
        self.plots.values().next().and_then(|p| p.vectors.get(name))
    }

    /// Get DC operating point value for a node.
    pub fn dc_value(&self, node: &str) -> Option<f64> {
        self.vector(node).and_then(|v| match v {
            Vector::Real(rv) => rv.data.first().copied(),
            _ => None,
        })
    }

    /// Get real vector data.
    pub fn real_vector(&self, name: &str) -> Option<&[f64]> {
        self.vector(name).and_then(|v| match v {
            Vector::Real(rv) => Some(rv.data.as_slice()),
            _ => None,
        })
    }

    /// Get magnitude of a complex vector (for AC analysis).
    pub fn magnitude(&self, name: &str) -> Option<Vec<f64>> {
        self.vector(name).and_then(|v| match v {
            Vector::Complex(cv) => Some(
                cv.data
                    .iter()
                    .map(|(r, i)| (r * r + i * i).sqrt())
                    .collect(),
            ),
            Vector::Real(rv) => Some(rv.data.iter().map(|v| v.abs()).collect()),
        })
    }

    /// Get magnitude in dB.
    pub fn magnitude_db(&self, name: &str) -> Option<Vec<f64>> {
        self.magnitude(name)
            .map(|m| m.iter().map(|v| 20.0 * v.log10()).collect())
    }

    /// Get phase in degrees of a complex vector.
    pub fn phase_deg(&self, name: &str) -> Option<Vec<f64>> {
        self.vector(name).and_then(|v| match v {
            Vector::Complex(cv) => Some(
                cv.data
                    .iter()
                    .map(|(r, i)| i.atan2(*r).to_degrees())
                    .collect(),
            ),
            _ => None,
        })
    }

    // ===== Typed probe-based lookup =====

    /// Look up voltage at a node across all plots.
    /// Handles ngspice naming variants (case-insensitive, v(name) vs name).
    pub fn voltage(&self, node: &Node) -> Option<&[f64]> {
        if node.is_ground() {
            return None;
        }
        let name = node.spice_name();
        self.find_real_vector_by_node(&name)
    }

    /// Look up current through an element across all plots.
    pub fn current(&self, elem: &ElementRef) -> Option<&[f64]> {
        let spice_name = elem.spice_name();
        self.find_real_vector_by_current(&spice_name)
    }

    /// Look up a vector by Probe (voltage, differential voltage, or current).
    pub fn probe_real(&self, probe: &Probe) -> Option<&[f64]> {
        match probe {
            Probe::Voltage(n) => self.voltage(n),
            Probe::VoltageDiff(p, n) => {
                let key = format!("v({},{})", p.spice_name(), n.spice_name());
                self.find_real_vector_fuzzy(&key)
            }
            Probe::Current(e) => self.current(e),
        }
    }

    /// Look up magnitude of a complex vector via Probe (for AC analysis).
    pub fn magnitude_of(&self, probe: &Probe) -> Option<Vec<f64>> {
        let key = probe.to_spice_save();
        self.magnitude(&key)
    }

    /// Look up magnitude in dB via Probe.
    pub fn magnitude_db_of(&self, probe: &Probe) -> Option<Vec<f64>> {
        let key = probe.to_spice_save();
        self.magnitude_db(&key)
    }

    /// Look up phase in degrees via Probe.
    pub fn phase_deg_of(&self, probe: &Probe) -> Option<Vec<f64>> {
        let key = probe.to_spice_save();
        self.phase_deg(&key)
    }

    // ===== Internal fuzzy lookup helpers =====

    /// Find a real vector by node name. Tries: exact, v(name), name without v().
    fn find_real_vector_by_node(&self, node_name: &str) -> Option<&[f64]> {
        let lower = node_name.to_ascii_lowercase();
        let v_wrapped = format!("v({})", lower);

        for plot in self.plots.values() {
            for (name, vector) in &plot.vectors {
                let lname = name.to_ascii_lowercase();
                if lname == lower || lname == v_wrapped {
                    if let Vector::Real(rv) = vector {
                        return Some(&rv.data);
                    }
                }
            }
        }
        None
    }

    /// Find a real vector by current element name. Tries: i(name), name#branch.
    fn find_real_vector_by_current(&self, element_spice_name: &str) -> Option<&[f64]> {
        let lower = element_spice_name.to_ascii_lowercase();
        let i_wrapped = format!("i({})", lower);
        let branch = format!("{}#branch", lower);

        for plot in self.plots.values() {
            for (name, vector) in &plot.vectors {
                let lname = name.to_ascii_lowercase();
                if lname == i_wrapped || lname == branch || lname == lower {
                    if let Vector::Real(rv) = vector {
                        return Some(&rv.data);
                    }
                }
            }
        }
        None
    }

    /// Generic fuzzy vector lookup by key string.
    fn find_real_vector_fuzzy(&self, key: &str) -> Option<&[f64]> {
        let lower = key.to_ascii_lowercase();
        for plot in self.plots.values() {
            for (name, vector) in &plot.vectors {
                if name.to_ascii_lowercase() == lower {
                    if let Vector::Real(rv) = vector {
                        return Some(&rv.data);
                    }
                }
            }
        }
        None
    }

    /// Look up the scalar result of a measurement by its handle.
    pub fn get_measurement(&self, m: &Measurement) -> Option<f64> {
        self.measurements.get(m.name()).copied()
    }

    // ===== Wrapped result accessors =====

    /// Returns a `TimeSeries` for a node voltage — time + values from the same plot.
    ///
    /// Only meaningful for transient (tran) analysis. Returns `None` if no plot
    /// contains both a `time` vector and the requested node.
    pub fn waveform(&self, node: &Node) -> Option<TimeSeries<'_>> {
        if node.is_ground() {
            return None;
        }
        let node_name = node.spice_name();
        for plot in self.plots.values() {
            let time = Self::find_real_in_plot(plot, "time");
            let values = Self::find_node_in_plot(plot, &node_name);
            if let (Some(t), Some(v)) = (time, values) {
                return Some(TimeSeries::new(t, v));
            }
        }
        None
    }

    /// Returns a `TimeSeries` for an arbitrary probe — time + values from the same plot.
    pub fn waveform_of(&self, probe: &Probe) -> Option<TimeSeries<'_>> {
        for plot in self.plots.values() {
            let time = Self::find_real_in_plot(plot, "time");
            let values = match probe {
                Probe::Voltage(n) => {
                    let name = n.spice_name();
                    Self::find_node_in_plot(plot, &name)
                }
                Probe::VoltageDiff(p, n) => {
                    let key = format!("v({},{})", p.spice_name(), n.spice_name());
                    Self::find_real_in_plot(plot, &key)
                }
                Probe::Current(e) => {
                    let spice = e.spice_name();
                    Self::find_current_in_plot(plot, &spice)
                }
            };
            if let (Some(t), Some(v)) = (time, values) {
                return Some(TimeSeries::new(t, v));
            }
        }
        None
    }

    /// Returns an `AcSpectrum` for a probe — frequency axis + complex data from the same plot.
    ///
    /// Only meaningful for AC analysis results.
    pub fn ac_spectrum(&self, probe: &Probe) -> Option<AcSpectrum<'_>> {
        for plot in self.plots.values() {
            let freq = Self::find_real_in_plot(plot, "frequency");
            let complex = match probe {
                Probe::Voltage(n) => {
                    let raw = n.spice_name();
                    let wrapped = format!("v({})", raw.to_ascii_lowercase());
                    Self::find_complex_in_plot(plot, &raw)
                        .or_else(|| Self::find_complex_in_plot(plot, &wrapped))
                }
                Probe::VoltageDiff(p, n) => {
                    let key = format!("v({},{})", p.spice_name(), n.spice_name());
                    Self::find_complex_in_plot(plot, &key)
                }
                Probe::Current(e) => {
                    let name = e.spice_name();
                    Self::find_complex_in_plot(plot, &name)
                }
            };
            if let (Some(f), Some(c)) = (freq, complex) {
                return Some(AcSpectrum::new(f, c));
            }
        }
        None
    }

    // ===== Per-plot lookup helpers =====

    fn find_real_in_plot<'a>(plot: &'a Plot, name: &str) -> Option<&'a [f64]> {
        let lower = name.to_ascii_lowercase();
        for (k, v) in &plot.vectors {
            if k.to_ascii_lowercase() == lower {
                if let Vector::Real(rv) = v {
                    return Some(&rv.data);
                }
            }
        }
        None
    }

    fn find_node_in_plot<'a>(plot: &'a Plot, node_name: &str) -> Option<&'a [f64]> {
        let lower = node_name.to_ascii_lowercase();
        let v_wrapped = format!("v({})", lower);
        for (k, v) in &plot.vectors {
            let lk = k.to_ascii_lowercase();
            if lk == lower || lk == v_wrapped {
                if let Vector::Real(rv) = v {
                    return Some(&rv.data);
                }
            }
        }
        None
    }

    fn find_current_in_plot<'a>(plot: &'a Plot, element_name: &str) -> Option<&'a [f64]> {
        let lower = element_name.to_ascii_lowercase();
        let i_wrapped = format!("i({})", lower);
        let branch = format!("{}#branch", lower);
        for (k, v) in &plot.vectors {
            let lk = k.to_ascii_lowercase();
            if lk == i_wrapped || lk == branch || lk == lower {
                if let Vector::Real(rv) = v {
                    return Some(&rv.data);
                }
            }
        }
        None
    }

    fn find_complex_in_plot<'a>(plot: &'a Plot, name: &str) -> Option<&'a [(f64, f64)]> {
        let lower = name.to_ascii_lowercase();
        for (k, v) in &plot.vectors {
            if k.to_ascii_lowercase() == lower {
                if let Vector::Complex(cv) = v {
                    return Some(&cv.data);
                }
            }
        }
        None
    }
}

/// A simulation plot (one per analysis run).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plot {
    pub name: String,
    pub plot_type: PlotType,
    pub vectors: HashMap<String, Vector>,
}

/// Type of simulation plot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlotType {
    OpPoint,
    DcSweep,
    AcAnalysis,
    Transient,
    Noise,
    PoleZero,
    Sensitivity,
    TransferFunction,
    SParameter,
    Unknown,
}

/// A data vector from simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Vector {
    Real(RealVector),
    Complex(ComplexVector),
}

/// Real-valued vector (time-domain, DC sweep).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealVector {
    pub name: String,
    pub data: Vec<f64>,
}

/// Complex-valued vector (AC analysis).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexVector {
    pub name: String,
    pub data: Vec<(f64, f64)>,
}

/// A time-domain waveform: paired time and value vectors from the same simulation plot.
///
/// Borrows data from a `SimulationResult` — zero-copy.
///
/// ```ignore
/// let wave = result.waveform(&out).unwrap();
/// for (t, v) in wave.iter() { println!("{t:.3e} {v:.4}"); }
/// plotter.plot(&wave);
/// ```
pub struct TimeSeries<'a> {
    time: &'a [f64],
    values: &'a [f64],
}

impl<'a> TimeSeries<'a> {
    pub fn new(time: &'a [f64], values: &'a [f64]) -> Self {
        debug_assert_eq!(
            time.len(),
            values.len(),
            "time and values must have the same length"
        );
        Self { time, values }
    }

    pub fn time(&self) -> &[f64] {
        self.time
    }

    pub fn values(&self) -> &[f64] {
        self.values
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Iterator over `(time, value)` sample pairs.
    pub fn iter(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        self.time
            .iter()
            .copied()
            .zip(self.values.iter().copied())
    }

    /// Sample at a specific index. Returns `None` if out of bounds.
    pub fn get(&self, index: usize) -> Option<(f64, f64)> {
        Some((*self.time.get(index)?, *self.values.get(index)?))
    }
}

impl std::fmt::Debug for TimeSeries<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimeSeries")
            .field("len", &self.len())
            .field("time_range", &(self.time.first(), self.time.last()))
            .finish()
    }
}

/// AC analysis result: frequency axis paired with complex vector data.
///
/// Returned by `SimulationResult::ac_spectrum()`.
pub struct AcSpectrum<'a> {
    frequency: &'a [f64],
    complex_data: &'a [(f64, f64)],
}

impl<'a> AcSpectrum<'a> {
    pub fn new(frequency: &'a [f64], complex_data: &'a [(f64, f64)]) -> Self {
        Self {
            frequency,
            complex_data,
        }
    }

    pub fn frequency(&self) -> &[f64] {
        self.frequency
    }

    pub fn len(&self) -> usize {
        self.frequency.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frequency.is_empty()
    }

    /// Magnitude: `sqrt(re² + im²)` for each sample.
    pub fn magnitude(&self) -> Vec<f64> {
        self.complex_data
            .iter()
            .map(|(r, i)| (r * r + i * i).sqrt())
            .collect()
    }

    /// Magnitude in dB: `20 * log10(|H|)`.
    pub fn magnitude_db(&self) -> Vec<f64> {
        self.magnitude()
            .iter()
            .map(|m| 20.0 * m.log10())
            .collect()
    }

    /// Phase in degrees: `atan2(im, re) * 180/π`.
    pub fn phase_deg(&self) -> Vec<f64> {
        self.complex_data
            .iter()
            .map(|(r, i)| i.atan2(*r).to_degrees())
            .collect()
    }

    /// Iterator over `(frequency, (re, im))` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (f64, (f64, f64))> + '_ {
        self.frequency
            .iter()
            .copied()
            .zip(self.complex_data.iter().copied())
    }
}

impl std::fmt::Debug for AcSpectrum<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcSpectrum")
            .field("len", &self.len())
            .field("freq_range", &(self.frequency.first(), self.frequency.last()))
            .finish()
    }
}
