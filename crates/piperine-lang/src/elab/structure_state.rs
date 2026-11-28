//! The state of structure lowering (§8): [`StructureTables`], the bodies, their plans and variants, and the frames and
//! reads of the placed tree. It goes to `elab/structure/` with the methods that read it.

use std::collections::{BTreeMap, BTreeSet};

use piperine_ir::BodyId;

use super::body::{ModulePlan, ParamPlan};
use super::carriers::Frame;
use super::consts::Bindings;
use super::generation::GeneratedItems;
use super::hoist::PlacementKey;
use super::instances::Instance;
use super::reach::demands::{NestedDemand, NestedDemands};
use super::structural::{BodyKey, BodyVariants, KeyParams};
use crate::pom::{DeclId, ExprId};

/// What one run knows of the module bodies and the tree they are placed in: the body each module gets, its plan and
/// key params, the body being lowered with its frames and generated items, and the reads through children.
pub(in crate::elab) struct StructureTables {
    /// The body each reached module will get: bodies are built in first-reach order, so a
    /// placement can name its child's body before that body exists.
    pub(in crate::elab) body_ids: BTreeMap<DeclId, BodyId>,
    /// The plan of every body lowered, by module and values of its key params.
    pub(super) plans: BTreeMap<BodyKey, ModulePlan>,
    /// The params of each reached module with a body, planned before any body is lowered.
    pub(super) param_plans: BTreeMap<DeclId, Vec<ParamPlan>>,
    /// The params of each instance of a generic module requested so far, planned under its arguments (`generic_bodies.rs`).
    pub(super) instance_param_plans: BTreeMap<Instance, Vec<ParamPlan>>,
    /// How deep each body requested so far sits in the chain of module instances that asked for it (`generic_bodies.rs`).
    pub(super) body_depths: BTreeMap<BodyKey, usize>,
    /// The body being lowered.
    pub(in crate::elab) lowering: Option<BodyKey>,
    /// The key params of each reached module with a body (`structural.rs`).
    pub(in crate::elab) keys: KeyParams,
    /// The variants of keyed bodies requested so far.
    pub(in crate::elab) body_variants: BodyVariants,
    /// The frames of the body being lowered: frame 0 is its own module.
    pub(in crate::elab) frames: Vec<Frame>,
    /// The index each placement of the body being lowered takes among its children (`reach/fields.rs`).
    pub(super) child_indices: BTreeMap<ExprId, u32>,
    /// The items of the body being lowered that are written inside generation, and how each unrolls (`generation.rs`).
    pub(in crate::elab) generated: GeneratedItems,
    /// The values of the key params of the body being lowered: what its initial values fold over.
    pub(in crate::elab) body_keys: Bindings,
    /// The reach frame of each child a frame reads, by the frame that reads it and the placement (`reach/fields.rs`).
    pub(super) reach_frames: BTreeMap<(usize, PlacementKey), usize>,
    /// What the design reads through a child of a child, by module (`reach/demands.rs`).
    pub(in crate::elab) nested_demands: NestedDemands,
    /// The modules refused for a cause of their own that a read through them meets: one that declares a node in generation
    /// (`reach/fields.rs`), a hoisted one that holds a placement (`hoist.rs`). A read through one of them adds no second diagnostic.
    pub(in crate::elab) refused_modules: BTreeSet<DeclId>,
    /// How many placements of a module an unrolled loop has hoisted so far, by the frame that
    /// places them (`unroll.rs`).
    pub(super) unrolled_names: BTreeMap<(usize, DeclId), u32>,
}

impl StructureTables {
    /// The empty tables of a run that knows `demands`, the reads through a child of a child found by the runs before it.
    pub(in crate::elab) fn new(demands: Vec<NestedDemand>) -> Self {
        StructureTables {
            body_ids: BTreeMap::new(),
            plans: BTreeMap::new(),
            param_plans: BTreeMap::new(),
            instance_param_plans: BTreeMap::new(),
            body_depths: BTreeMap::new(),
            lowering: None,
            keys: KeyParams::default(),
            body_variants: BodyVariants::default(),
            frames: Vec::new(),
            child_indices: BTreeMap::new(),
            generated: GeneratedItems::default(),
            body_keys: Bindings::new(),
            reach_frames: BTreeMap::new(),
            nested_demands: NestedDemands::known(demands),
            refused_modules: BTreeSet::new(),
            unrolled_names: BTreeMap::new(),
        }
    }
}
