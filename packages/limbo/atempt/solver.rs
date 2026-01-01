use crate::cap::{Capacitor, CapacitorParameters};
use crate::res::{
    Resistor, ResistorComponentParameters, ResistorModelInstance, ResistorModelParameters,
};
use crate::vsrc::{VoltageSource, VoltageSourceParameters};
use crate::{
    BranchReference, CircuitInstance, CircuitSolution, Device, ModelInstance, NodeIdentifier,
    NodeReference, RealStamper, TransientAnalysisContext,
};
use faer::prelude::Solve;
use faer::sparse::{SparseColMat, Triplet};
use faer::Col;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub struct FaerStamper {
    pub triplets: Vec<Triplet<usize, usize, f64>>,
    pub rhs: Vec<f64>,
    pub node_offset: usize, // To handle the fact that GND (ID 0) is not solved
    pub branch_start: usize,
}

impl FaerStamper {
    fn new(nodes: usize, branches: usize) -> Self {
        // Size is (nodes - 1) + branches
        let size = (nodes - 1) + branches;
        Self {
            triplets: Vec::new(),
            rhs: vec![0.0; size],
            node_offset: 1, // Node 0 is GND, so we map index i -> i-1
            branch_start: nodes - 1,
        }
    }

    // Helper to map our IDs to Matrix Indices
    fn map_node(&self, id: usize) -> Option<usize> {
        if id == 0 { None } else { Some(id - 1) }
    }
}

impl RealStamper for FaerStamper {
    fn nodal_stamp(&mut self, row: &NodeReference, col: &NodeReference, value: f64) {
        if let (Some(r), Some(c)) = (self.map_node(row.id), self.map_node(col.id)) {
            self.triplets.push(Triplet::new(r, c, value));
        }
    }

    fn node_to_branch_stamp(&mut self, node: &NodeReference, branch: &BranchReference, value: f64) {
        if let Some(r) = self.map_node(node.id) {
            let c = self.branch_start + branch.id;
            self.triplets.push(Triplet::new(r, c, value));
        }
    }

    fn branch_to_node_stamp(&mut self, branch: &BranchReference, node: &NodeReference, value: f64) {
        let r = self.branch_start + branch.id;
        if let Some(c) = self.map_node(node.id) {
            self.triplets.push(Triplet::new(r, c, value));
        }
    }

    fn branch_rhs_stamp(&mut self, branch: &BranchReference, value: f64) {
        let idx = self.branch_start + branch.id;
        self.rhs[idx] += value;
    }

    fn nodal_rhs_stamp(&mut self, node: &NodeReference, value: f64) {
        if let Some(idx) = self.map_node(node.id) {
            self.rhs[idx] += value;
        }
    }

    fn branch_to_branch_stamp(&mut self, row: &BranchReference, col: &BranchReference, value: f64) {
        todo!()
    }

    // ... implement other stamps similarly ...
}

pub struct FaerSolver {}

impl FaerSolver {
    pub fn solve_transient(
        &self,
        mut circuit: CircuitInstance,
        stop_time: f64,
        dt: f64,
    ) -> CircuitInstance {
        let mut current_time = 0.0;

        // 1. Initial DC Solve (Time = 0)
        // This sets the starting voltages (e.g., Cap starts at 0V)
        self.solve_step(&mut circuit, &TransientAnalysisContext::dc());

        // 2. Transient Loop
        while current_time < stop_time {
            current_time += dt;
            let ctx = TransientAnalysisContext {
                time: current_time,
                dt,
                is_dc: false,
            };

            self.solve_step(&mut circuit, &ctx);

            let v_cap = circuit.solutions.last().unwrap().values[2]; // Node 2
            println!(
                "Time: {:.2}ns | Cap Voltage: {:.4}V",
                current_time * 1e9,
                v_cap
            );
        }

        circuit
    }

    // Move your existing solve logic into this helper
    fn solve_step(&self, circuit: &mut CircuitInstance, ctx: &TransientAnalysisContext) {
        let n_nodes = circuit.next_node_id.load(Ordering::SeqCst);
        let n_branches = circuit.next_branch_id.load(Ordering::SeqCst);
        let matrix_dim = (n_nodes - 1) + n_branches;

        let mut stamper = FaerStamper::new(n_nodes, n_branches);

        for component in circuit.component_instances.iter_mut() {
            component.temperature();
            // Use our new AnyDcTransientAnalysis trait
        }

        for component in circuit.component_instances.iter() {
            component.load_dc(circuit, ctx, &mut stamper);
        }

        let a_matrix = SparseColMat::<usize, f64>::try_new_from_triplets(
            matrix_dim,
            matrix_dim,
            &stamper.triplets,
        )
        .unwrap();
        let b_vector = Col::from_fn(matrix_dim, |idx| stamper.rhs[idx]);

        let lu = a_matrix.sp_qr().unwrap();
        let x_solution = lu.solve(&b_vector);

        let mut values = vec![0.0; n_nodes + n_branches];
        for i in 0..matrix_dim {
            values[i + 1] = x_solution[i];
        }

        // We return a clone or modify in place? Let's modify in place for history
        circuit.solutions.push(CircuitSolution { values });
    }
}

#[test]
pub fn test() {
    let mut circuit_instance = CircuitInstance {
        solutions: vec![],
        model_instances: HashMap::new(),
        component_instances: vec![],
        node_registry: Mutex::new(HashMap::new()),
        next_node_id: AtomicUsize::new(0),
        next_branch_id: AtomicUsize::new(0),
    };

    let resistor_model = ResistorModelInstance::setup(
        ResistorModelParameters {
            name: "RMODEL_DEFAULT".to_string(),
            tc1: None,
            tc2: None,
            tce: None,
            rsh: None,
            defw: None,
            defl: None,
            narrow: None,
            short: None,
            tnom: None,
            kf: None,
            af: None,
            wf: None,
            lf: None,
            ef: None,
            resistance: None,
        },
        &mut circuit_instance,
    )
    .unwrap();

    circuit_instance
        .model_instances
        .insert("RMODEL_DEFAULT".to_string(), Arc::new(resistor_model));

    let resistor = Resistor::instantiate(
        ResistorComponentParameters {
            model: Some("RMODEL_DEFAULT".to_string()),
            name: "R1".to_string(),
            n_plus: NodeIdentifier::Indexed(1),
            n_minus: NodeIdentifier::Indexed(2),
            resistance: Some(1000.00),
            ..Default::default()
        },
        &mut circuit_instance,
    )
    .unwrap();

    let vsource = VoltageSource::instantiate(
        VoltageSourceParameters {
            name: "VCC".to_string(),
            n_plus: NodeIdentifier::Indexed(1),
            n_minus: NodeIdentifier::Gnd,
            value: 5.0,
            ..Default::default()
        },
        &mut circuit_instance,
    )
    .unwrap();

    let capacitor = Capacitor::instantiate(
        CapacitorParameters {
            name: "C1".to_string(),
            n_plus: NodeIdentifier::Indexed(2),
            n_minus: NodeIdentifier::Gnd,
            value: 1.0e-9,
            ..Default::default()
        },
        &mut circuit_instance,
    )
    .unwrap();

    circuit_instance
        .component_instances
        .push(Box::new(resistor));
    circuit_instance.component_instances.push(Box::new(vsource));
    circuit_instance
        .component_instances
        .push(Box::new(capacitor));

    let solver = FaerSolver {};
    solver.solve_transient(circuit_instance, 0.1, 0.01);
}
