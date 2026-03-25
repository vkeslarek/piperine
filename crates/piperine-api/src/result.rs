use crate::node::Node;
use crate::spice::{ElementRef, Probe, Measurement};
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
