use std::any::Any;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

mod cap;
mod res;
mod solver;
mod vsrc;

#[derive(Debug, Clone)]
pub enum PiperineProblem {
    ComponentSetupProblem {
        component_type: String,
        component_name: String,
    },
    ModelNotFound {
        name: String,
    },
    NodeCannotBeAllocated {
        reason: String,
    },
    SoaViolation,
    ModelTypeMismatch {
        expected: String,
        found: String,
    },
    DeviceInstantiationFailed {},
}

#[derive(Debug)]
pub struct PiperineError {
    pub title: String,
    pub detail: String,
    pub problems: Vec<PiperineProblem>,
}

impl PiperineError {
    pub fn wrap(self, problem: PiperineProblem) -> Self {
        let PiperineError {
            title,
            detail,
            mut problems,
        } = self;

        problems.push(problem);

        Self {
            title,
            detail,
            problems,
        }
    }
}

pub type PiperineResult<T> = Result<T, PiperineError>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeIdentifier {
    Named(String),
    Indexed(usize),
    Gnd,
}

pub struct NodeReference {
    pub id: usize,
}

pub struct BranchReference {
    pub id: usize,
}

pub enum Analysis {
    OP,
    DC,
}

pub trait Device {
    type ComponentInstance: ComponentInstance;

    const NAME: &'static str;
    const DESCRIPTION: &'static str;
    const PINS: &'static [&'static str];
    const AVAILABLE_ANALYSIS: &'static [Analysis];

    fn name(&self) -> &'static str {
        Self::NAME
    }
    fn description(&self) -> &'static str {
        Self::DESCRIPTION
    }
    fn pin_count(&self) -> usize {
        Self::PINS.len()
    }
    fn pin_names(&self) -> &'static [&'static str] {
        Self::PINS
    }
    fn available_analysis(&self) -> &'static [Analysis] {
        Self::AVAILABLE_ANALYSIS
    }
    fn instantiate(
        params: <Self::ComponentInstance as ComponentInstance>::ComponentParameters,
        circuit_instance: &mut CircuitInstance,
    ) -> PiperineResult<Self::ComponentInstance> {
        Self::ComponentInstance::setup(params, circuit_instance)
            .map_err(|err| err.wrap(PiperineProblem::DeviceInstantiationFailed {}))
    }
}

pub struct CircuitSolution {
    values: Vec<f64>,
}

pub struct CircuitInstance {
    pub solutions: Vec<CircuitSolution>,
    pub model_instances: HashMap<String, Arc<dyn AnyModelInstance>>,
    pub component_instances: Vec<Box<dyn AnyComponentInstance>>,
    pub node_registry: Mutex<HashMap<NodeIdentifier, Arc<NodeReference>>>,
    pub next_node_id: AtomicUsize,
    pub next_branch_id: AtomicUsize,
}

impl CircuitInstance {
    /// Fetches a model and downcasts it to the specific type P.
    pub fn get_model_instance<P: ModelInstance + 'static>(
        &self,
        model_name: &Option<String>,
    ) -> PiperineResult<Arc<P>> {
        // 1. Ensure the name exists
        let name = model_name.as_ref().ok_or_else(|| PiperineError {
            title: "Failed to get model".to_string(),
            detail: "The default model was requested but not found in the instances".to_string(),
            problems: vec![PiperineProblem::ModelNotFound {
                name: "None (Default requested but not specified)".to_string(),
            }],
        })?;

        // 2. Lookup in HashMap
        let any_model = self
            .model_instances
            .get(name)
            .ok_or_else(|| PiperineError {
                title: "Model not found".to_string(),
                detail: "The requested model was not found in the instances".to_string(),
                problems: vec![PiperineProblem::ModelNotFound { name: name.clone() }],
            })?;

        // 3. The Downcast Trick:
        // Since we want Arc<P> and we have Arc<dyn AnyModelInstance>,
        // we use a pointer cast after verifying the type with as_any().
        if any_model.as_any().is::<P>() {
            // Safety: We just verified the type ID matches P.
            // We cast the pointer to the underlying data to Arc<P>.
            let raw_ptr = Arc::into_raw(any_model.clone());
            let typed_ptr = raw_ptr as *const P;
            return Ok(unsafe { Arc::from_raw(typed_ptr) });
        }

        Err(PiperineError {
            title: "Failed to cast model".to_string(),
            detail: "The component expects a certain model type and the model we encountered is of a different type".to_string(),
            problems: vec![PiperineProblem::ModelTypeMismatch {
                expected: std::any::type_name::<P>().to_string(),
                found: name.clone(),
            }],
        })
    }

    /// Resolves a NodeIdentifier into a unique NodeReference (ID).
    pub fn get_node_reference(
        &self,
        node_identifier: NodeIdentifier,
    ) -> PiperineResult<Arc<NodeReference>> {
        // Special Case: Ground (GND) is always ID 0
        if let NodeIdentifier::Gnd = node_identifier {
            return Ok(Arc::new(NodeReference { id: 0 }));
        }

        let mut registry = self.node_registry.lock().unwrap();

        // If the node already exists, return the same Arc
        if let Some(existing) = registry.get(&node_identifier) {
            return Ok(existing.clone());
        }

        // Otherwise, allocate a new monotonic ID
        let new_id = self.next_node_id.fetch_add(1, Ordering::SeqCst);
        let reference = Arc::new(NodeReference { id: new_id });

        registry.insert(node_identifier, reference.clone());
        Ok(reference)
    }

    /// Similar to nodes, but branches usually don't have names
    pub fn get_branch_reference(&self) -> PiperineResult<Arc<BranchReference>> {
        let new_id = self.next_branch_id.fetch_add(1, Ordering::SeqCst);
        Ok(Arc::new(BranchReference { id: new_id }))
    }

    // --- State and History Lookups ---

    pub fn get_history_voltage(&self, node: &NodeReference, lookback: usize) -> f64 {
        // lookback 0 = current guess, 1 = previous converged timepoint, etc.
        self.solutions
            .get(self.solutions.len().saturating_sub(lookback + 1))
            .map(|sol| sol.values[node.id])
            .unwrap_or(0.0) // Default to 0V if history doesn't exist
    }

    pub fn get_time_step(&self) -> f64 {
        // In a real solver, this would be calculated based on convergence
        1e-9
    }

    pub fn temp(&self) -> f64 {
        27.0 + 273.15 // 27°C in Kelvin
    }
}

pub trait ComponentInstance: Sized {
    type ComponentParameters;

    fn setup(
        parameters: Self::ComponentParameters,
        circuit_instance: &CircuitInstance,
    ) -> PiperineResult<Self>;

    fn temperature(&mut self);

    fn commit(&mut self) -> PiperineResult<()> {
        Ok(())
    }

    fn rollback(&mut self) -> PiperineResult<()> {
        Ok(())
    }

    fn load_dc(
        &self,
        circuit_instance: &CircuitInstance,
        ctx: &TransientAnalysisContext,
        stamp: &mut dyn RealStamper,
    );

    /// For non-linear devices (Matches DEVconvTest)
    fn check_convergence(
        &self,
        circuit_instance: &CircuitInstance,
        ctx: &TransientAnalysisContext,
    ) -> bool {
        true
    }
}

pub trait AnyComponentInstance {
    fn as_any(&self) -> &dyn Any;

    fn load_dc(
        &self,
        circuit_instance: &CircuitInstance,
        ctx: &TransientAnalysisContext,
        stamp: &mut dyn RealStamper,
    );

    fn temperature(&mut self);
}

impl<CI: 'static + ComponentInstance> AnyComponentInstance for CI {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn load_dc(
        &self,
        circuit_instance: &CircuitInstance,
        ctx: &TransientAnalysisContext,
        stamp: &mut dyn RealStamper,
    ) {
        CI::load_dc(self, circuit_instance, ctx, stamp)
    }

    fn temperature(&mut self) {
        CI::temperature(self);
    }
}

pub trait ModelInstance: Sized {
    type Parameters;
    fn setup(
        parameters: Self::Parameters,
        circuit_instance: &CircuitInstance,
    ) -> PiperineResult<Self>;
}

pub trait AnyModelInstance {
    fn as_any(&self) -> &dyn Any;
}

impl<MI: 'static + ModelInstance> AnyModelInstance for MI {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct TransientAnalysisContext {
    pub time: f64,
    pub dt: f64,
    pub is_dc: bool,
}

impl TransientAnalysisContext {
    pub fn dc() -> Self {
        Self {
            time: 0.0,
            dt: 0.0,
            is_dc: true,
        }
    }
}

pub trait RealStamper {
    /// Area 1: Node Row, Node Col (Conductance)
    fn nodal_stamp(&mut self, row: &NodeReference, col: &NodeReference, value: f64);

    /// Area 2: Node Row, Branch Col (Current variable entering KCL)
    fn node_to_branch_stamp(&mut self, node: &NodeReference, branch: &BranchReference, value: f64);

    /// Area 3: Branch Row, Node Col (Voltage affecting branch equation)
    fn branch_to_node_stamp(&mut self, branch: &BranchReference, node: &NodeReference, value: f64);

    /// Area 4: Branch Row, Branch Col (Branch interdependence)
    fn branch_to_branch_stamp(&mut self, row: &BranchReference, col: &BranchReference, value: f64);

    /// RHS for Nodes (Current sources)
    fn nodal_rhs_stamp(&mut self, node: &NodeReference, value: f64);

    /// RHS for Branches (Voltage sources)
    fn branch_rhs_stamp(&mut self, branch: &BranchReference, value: f64);
}
//
// /// 3. DC & TRANSIENT (Matches DEVload)
// pub trait DcTransientAnalysis: ComponentInstance {
//     /// The "Hot Path" - Stamping the Real matrix
//     fn load(
//         &self,
//         circuit_instance: &CircuitInstance,
//         ctx: &TransientAnalysisContext,
//         stamp: &mut dyn RealStamper,
//     );
//
//     /// For non-linear devices (Matches DEVconvTest)
//     fn check_convergence(
//         &self,
//         circuit_instance: &CircuitInstance,
//         ctx: &TransientAnalysisContext,
//     ) -> bool {
//         true
//     }
// }
//
// pub trait AnyDcTransientAnalysis {
//     fn load(
//         &self,
//         circuit_instance: &CircuitInstance,
//         ctx: &TransientAnalysisContext,
//         stamp: &mut dyn RealStamper,
//     );
//
//     /// For non-linear devices (Matches DEVconvTest)
//     fn check_convergence(
//         &self,
//         circuit_instance: &CircuitInstance,
//         ctx: &TransientAnalysisContext,
//     ) -> bool {
//         true
//     }
// }
//
// impl<DTA: DcTransientAnalysis> AnyDcTransientAnalysis for DTA {
//     fn load(
//         &self,
//         circuit_instance: &CircuitInstance,
//         ctx: &TransientAnalysisContext,
//         stamp: &mut dyn RealStamper,
//     ) {
//         DTA::load(self, circuit_instance, ctx, stamp);
//     }
//
//     fn check_convergence(
//         &self,
//         circuit_instance: &CircuitInstance,
//         ctx: &TransientAnalysisContext,
//     ) -> bool {
//         DTA::check_convergence(self, circuit_instance, ctx)
//     }
// }

pub trait ComplexStamper {}

/// 4. AC ANALYSIS (Matches DEVacLoad)
pub trait AcAnalysis: Device {
    /// Stamping the Complex matrix
    fn load_ac(
        component_instance: &Self::ComponentInstance,
        circuit_instance: &CircuitInstance,
        omega: f64,
        stamp: &mut dyn ComplexStamper,
    );
}

/// 5. NOISE & SENSITIVITY (Matches DEVnoise, DEVsenLoad)
pub trait AdvancedAnalysis: Device {
    fn noise(instance: &Self::ComponentInstance, ctx: &TransientAnalysisContext) -> f64;
    fn sensitivity_load(
        component_instance: &Self::ComponentInstance,
        circuit_instance: &CircuitInstance,
        ctx: &TransientAnalysisContext,
        stamp: &mut dyn RealStamper,
    );
}
//
// pub trait NonLinearAnalysis: DcTransientAnalysis {
//     /// Prevents exponential explosion by limiting the voltage/current step
//     fn limit_step(&self, circuit: &mut CircuitInstance);
// }

pub enum AskParameter {
    Power,
    Temperature,
    // etc ...
}

pub trait Interrogatable: Device {
    /// Allows the UI or a .print command to get a parameter by ID
    fn ask(&self, instance: &Self::ComponentInstance, parameter: AskParameter) -> f64;
}

/// 6. SAFETY & MONITORING (Matches DEVsoaCheck)
pub trait SecurityMonitor: Device {
    /// Safe Operating Area Check (e.g., checking if V > BV_MAX)
    fn check_soa(
        component_instance: &Self::ComponentInstance,
        circuit_instance: &CircuitInstance,
        ctx: &TransientAnalysisContext,
    ) -> PiperineResult<()>;
}

pub trait TimestepControl: Device {
    /// Predict the maximum allowable next time-step based on local truncation error (LTE)
    fn truncate(
        component_instance: &Self::ComponentInstance,
        circuit_instance: &CircuitInstance,
        ctx: &TransientAnalysisContext,
    ) -> f64;
}
