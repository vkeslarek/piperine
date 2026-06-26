use bitflags::bitflags;
use crate::analog::netlist::AnalogReference;

bitflags! {
    /// Flags passed during setup and evaluation indicating the current simulator state
    /// or the specific components that need calculation.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SimFlags: u32 {
        const CALC_RESIST_RESIDUAL = 1;
        const CALC_REACT_RESIDUAL  = 2;
        const CALC_RESIST_JACOBIAN = 4;
        const CALC_REACT_JACOBIAN  = 8;
        const CALC_NOISE           = 16;
        const CALC_OP              = 32;
        const CALC_RESIST_LIM_RHS  = 64;
        const CALC_REACT_LIM_RHS   = 128;
        const ENABLE_LIM           = 256;
        const INIT_LIM             = 512;
        const ANALYSIS_DC          = 2048;
        const ANALYSIS_AC          = 4096;
        const ANALYSIS_TRAN        = 8192;
    }
}

bitflags! {
    /// Flags returned by the `eval` method to instruct the solver.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct EvalFlags: u32 {
        /// Indicates that the device's equations required limiting to prevent numerical overflow.
        const LIM   = 1;
        /// Indicates a fatal evaluation error (e.g., divide by zero).
        const FATAL = 2;
    }
}

/// Global simulation parameters and tolerances.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimParams {
    pub ini_lim: bool,
    pub gmin: f64,
    pub gdev: f64,
    pub tnom: f64,
    pub simulator_version: f64,
    pub source_scale_factor: f64,
    pub epsmin: f64,
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
}

/// State and timing information passed during an evaluation step.
pub struct SimInfo<'a> {
    pub params: &'a SimParams,
    pub abstime: f64,
    pub prev_solve: &'a [f64],
    pub prev_state: &'a [f64],
    pub next_state: &'a mut [f64],
    pub flags: SimFlags,
}

/// A type-safe identifier for a device parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParameterId(pub u32);

/// A type-safe identifier for an error code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ErrorCode(pub u32);

/// A type-safe wrapper for generic error payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ErrorPayload(pub u32);

/// Specific error codes returned during model or instance initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitError {
    /// The provided parameter name/ID is not recognized by this device.
    UnknownParameter(ParameterId),
    /// A parameter value was outside the valid mathematical or physical range.
    ParameterOutOfRange(ParameterId),
    /// The model data could not be allocated or initialized properly.
    ModelInitializationFailed(ErrorCode),
    /// Unspecified or generic initialization error.
    Generic(ErrorCode, ErrorPayload),
}

/// Initialization context for models and instances.
pub struct InitInfo {
    pub flags: SimFlags,
    pub errors: Vec<InitError>,
}

impl InitInfo {
    pub fn new(flags: SimFlags) -> Self {
        Self {
            flags,
            errors: Vec::new(),
        }
    }
    
    pub fn push_error(&mut self, error: InitError) {
        self.errors.push(error);
    }
}

/// `AnalogDevice` is a native Rust trait designed to act as a safe, drop-in replacement
/// for the dynamically loaded OSDI libraries.
///
/// This trait leverages Rust's Associated Types (`ModelData`, `InstanceData`) 
/// for memory safety and zero-cost abstraction, avoiding FFI entirely.
pub trait AnalogDevice: Send + Sync {
    /// The memory footprint and configuration shared across all instances of this model.
    type ModelData: Send + Sync;
    
    /// The unique memory footprint and state of a single instance of this device.
    type InstanceData: Send + Sync;

    // -----------------------------------------------------------------------
    // Metadata
    // -----------------------------------------------------------------------
    
    /// The name of the device model (e.g., "resistor", "bsim4").
    fn name(&self) -> &str;
    
    /// Number of internal nodes plus external terminals.
    fn num_nodes(&self) -> usize;
    
    /// Number of external terminals connecting to the circuit.
    fn num_terminals(&self) -> usize;
    
    /// Number of state variables needed by the device
    fn num_states(&self) -> usize;

    /// Byte size of the instance data blob (determines pre-allocation size).
    fn instance_size(&self) -> usize;

    /// The number of entries this device contributes to the Resistive (DC) Jacobian.
    fn num_resistive_jacobian_entries(&self) -> usize;
    
    /// The number of entries this device contributes to the Reactive (Transient) Jacobian.
    fn num_reactive_jacobian_entries(&self) -> usize;

    // -----------------------------------------------------------------------
    // Initialization
    // -----------------------------------------------------------------------
    
    /// Initializes shared model parameters based on simulator global settings.
    fn setup_model(
        &self,
        model: &mut Self::ModelData,
        paras: &SimParams,
        info: &mut InitInfo,
    );

    /// Initializes an individual instance, given its parent model configuration.
    fn setup_instance(
        &self,
        model: &Self::ModelData,
        instance: &mut Self::InstanceData,
        temp: f64,
        flags: SimFlags,
        paras: &SimParams,
        info: &mut InitInfo,
    );
    
    fn allocate_nodes(
        &self,
        instance_name: &str,
        terminals: &[crate::analog::netlist::NodeIdentifier],
        netlist: &mut crate::analog::netlist::Netlist
    ) -> Vec<Option<AnalogReference>>;
    
    /// Instructs the device to collapse any nodes if applicable, and binds the circuit references.
    /// The device can optionally update `node_refs` in place.
    fn bind_nodes(
        &self,
        instance: &mut Self::InstanceData,
        node_refs: &mut Vec<Option<AnalogReference>>
    );

    /// Set parameters
    fn set_params(&self, model: &mut Self::ModelData, instance: &mut Self::InstanceData, params: &[(String, f64)], str_params: &[(String, String)]);

    // -----------------------------------------------------------------------
    // Evaluation
    // -----------------------------------------------------------------------
    
    /// Evaluates limits, intermediate states, and prepares internal variables 
    /// for a Newton-Raphson iteration.
    /// 
    /// Returns a set of status flags (e.g., `EvalFlags::LIM` or `EvalFlags::FATAL`).
    fn eval(
        &self,
        model: &Self::ModelData,
        instance: &mut Self::InstanceData,
        sim_info: &mut SimInfo,
    ) -> EvalFlags;

    fn bound_step_hint(&self, instance: &Self::InstanceData) -> f64;

    fn read_opvars(&self, model: &Self::ModelData, instance: &Self::InstanceData) -> Vec<(String, f64)>;

    // -----------------------------------------------------------------------
    // Loading (Residuals / Right-Hand Side)
    // -----------------------------------------------------------------------
    
    /// Loads the resistive (DC) residual currents/charges into the RHS vector.
    fn load_residual_resist(
        &self,
        model: &Self::ModelData,
        instance: &Self::InstanceData,
        rhs: &mut [f64],
    );

    /// Loads the reactive (Transient) residual currents/charges into the RHS vector.
    fn load_residual_react(
        &self,
        model: &Self::ModelData,
        instance: &Self::InstanceData,
        rhs: &mut [f64],
    );

    // -----------------------------------------------------------------------
    // Loading (Jacobian / Derivatives)
    // -----------------------------------------------------------------------
    
    /// Writes the partial derivatives (conductances) for the DC operating point.
    fn load_jacobian_resist(
        &self,
        model: &Self::ModelData,
        instance: &Self::InstanceData,
        jacobian: &mut [f64],
    );

    /// Writes the partial derivatives (capacitances/inductances) for transient analysis.
    fn load_jacobian_react(
        &self,
        model: &Self::ModelData,
        instance: &Self::InstanceData,
        step: f64,
        jacobian: &mut [f64],
    );
    
    fn get_resist_jac_refs(&self, node_refs: &[Option<AnalogReference>]) -> Vec<(Option<AnalogReference>, Option<AnalogReference>)>;
    fn get_react_jac_refs(&self, node_refs: &[Option<AnalogReference>]) -> Vec<(Option<AnalogReference>, Option<AnalogReference>)>;
    fn get_rhs_indices(&self, instance: &Self::InstanceData) -> Vec<Option<usize>>;
    fn build_prev_solve(&self, instance: &Self::InstanceData, node_refs: &[Option<AnalogReference>], state_fn: &dyn Fn(usize) -> f64) -> [f64; crate::analog::osdi::ffi::SCRATCH];

    // -----------------------------------------------------------------------
    // Noise and Advanced Analysis
    // -----------------------------------------------------------------------
    
    /// Number of noise sources this device exposes (0 = noiseless).
    fn num_noise_sources(&self) -> usize { 0 }

    /// Returns (osdi_node_idx_1, osdi_node_idx_2) pairs for each noise source.
    fn noise_source_node_pairs(&self) -> Vec<(usize, usize)> { Vec::new() }

    /// Contributes spectral noise density to the RHS for AC Noise analysis.
    /// `noise_rhs` must have length `num_noise_sources()`.
    fn load_noise(
        &self,
        _model: &Self::ModelData,
        _instance: &Self::InstanceData,
        _freq: f64,
        _noise_rhs: &mut [f64],
    ) {
        // Default implementation: noiseless device
    }

    // -----------------------------------------------------------------------
    // SPICE-style RHS (combined residual + limiting)
    // -----------------------------------------------------------------------

    /// Compute the SPICE-style DC right-hand side: includes J*x - f(x) formulation.
    /// This is the function OSDI calls `load_spice_rhs_dc(inst, model, rhs, prev_solve)`.
    fn load_spice_rhs_dc(
        &self,
        model: &Self::ModelData,
        instance: &Self::InstanceData,
        rhs: &mut [f64],
        prev_solve: &[f64],
    );

    /// Compute the SPICE-style transient right-hand side: includes J*x - f(x)
    /// combined with charge integration via alpha (1/dt).
    /// This is the function OSDI calls `load_spice_rhs_tran(inst, model, rhs, prev_solve, alpha)`.
    fn load_spice_rhs_tran(
        &self,
        model: &Self::ModelData,
        instance: &Self::InstanceData,
        rhs: &mut [f64],
        prev_solve: &[f64],
        alpha: f64,
    );

    /// Collect RHS stamps by reading from the node_mapping stored in instance data
    /// and mapping each OSDI node to the corresponding circuit AnalogReference.
    fn collect_rhs_stamps(
        &self,
        instance: &Self::InstanceData,
        rhs: &[f64],
        node_refs: &[Option<AnalogReference>],
        stamps: &mut Vec<crate::math::linear::Stamp<AnalogReference, f64>>,
    );
}
