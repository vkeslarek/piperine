//! [`Design`] — the POM root, returned by [`SourceFile::elaborate`][crate::parse::SourceFile::elaborate].

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::parse::ast::{BundleDecl, CapabilityDecl, DisciplineDecl, EnumDecl};
use crate::pom::{BenchBlock, ElabError, ElabErrorKind, Function, ImplBlock, Module, OverrideMap, Value};

/// The complete output of elaboration — the POM root.
///
/// Fields are `pub(crate)`; external consumers use the public accessor
/// methods that implement the POM reflection interface
/// (`docs/reflection_api.md`).
#[derive(Debug, Clone)]
pub struct Design {
    pub(crate) modules: HashMap<String, Module>,
    pub(crate) disciplines: HashMap<String, DisciplineDecl>,
    pub(crate) bundles: HashMap<String, BundleDecl>,
    pub(crate) enums: HashMap<String, EnumDecl>,
    pub(crate) capabilities: HashMap<String, CapabilityDecl>,
    pub(crate) functions: HashMap<String, Function>,
    pub(crate) impls: Vec<ImplBlock>,
    pub(crate) consts: HashMap<String, Value>,
    pub(crate) benches: Vec<BenchBlock>,
    /// Staged parameter overrides — the single mutation surface in POM.
    /// Writing via `set_param()` stages here; re-elaboration consumes.
    pub(crate) overrides: Rc<RefCell<OverrideMap>>,
    /// The top module name, if set by the user or inferred.
    pub(crate) top_module: Option<String>,
}

impl Design {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            disciplines: HashMap::new(),
            bundles: HashMap::new(),
            enums: HashMap::new(),
            capabilities: HashMap::new(),
            functions: HashMap::new(),
            impls: Vec::new(),
            consts: HashMap::new(),
            benches: Vec::new(),
            overrides: Rc::new(RefCell::new(OverrideMap::new())),
            top_module: None,
        }
    }

    // ── POM navigation ────────────────────────────────────────────────────

    /// The elaborated top module, if set.
    pub fn top(&self) -> Option<&Module> {
        self.top_module.as_ref().and_then(|n| self.modules.get(n))
    }

    /// Evaluate a selector path against this design.
    pub fn select<'a>(&'a self, path: &str) -> Result<crate::pom::selection::NodeSelection<'a>, String> {
        let sel = path.parse::<crate::pom::selector::Selector>()?;
        let initial_context = if sel.absolute {
            crate::pom::selection::NodeSelection::new()
        } else {
            // If relative, start from top module
            if let Some(top) = self.top() {
                crate::pom::selection::NodeSelection::from_vec(vec![crate::pom::node::Node::Module(top)])
            } else {
                crate::pom::selection::NodeSelection::new()
            }
        };
        crate::pom::selector::Evaluator::new(self).evaluate(&sel, initial_context)
    }

    /// Set the top module by name.
    pub fn set_top(&mut self, name: &str) {
        self.top_module = Some(name.into());
    }

    /// Look up a module by name.
    pub fn module(&self, name: &str) -> Option<&Module> {
        self.modules.get(name)
    }

    /// Every elaborated (monomorphized) module.
    pub fn modules(&self) -> impl Iterator<Item = &Module> {
        self.modules.values()
    }

    /// Number of elaborated modules.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Look up a function by name.
    pub fn function(&self, name: &str) -> Option<&Function> {
        self.functions.get(name)
    }

    /// Every discipline declaration.
    pub fn disciplines(&self) -> impl Iterator<Item = (&String, &DisciplineDecl)> {
        self.disciplines.iter()
    }

    /// Look up a discipline declaration by name.
    pub fn discipline(&self, name: &str) -> Option<&DisciplineDecl> {
        self.disciplines.get(name)
    }

    /// Every bundle declaration.
    pub fn bundles(&self) -> impl Iterator<Item = (&String, &BundleDecl)> {
        self.bundles.iter()
    }

    /// Look up a bundle declaration by name.
    pub fn bundle(&self, name: &str) -> Option<&BundleDecl> {
        self.bundles.get(name)
    }

    /// Every enum declaration.
    pub fn enums(&self) -> impl Iterator<Item = (&String, &EnumDecl)> {
        self.enums.iter()
    }

    /// Look up an enum declaration by name.
    pub fn enum_(&self, name: &str) -> Option<&EnumDecl> {
        self.enums.get(name)
    }

    /// Every enum variant's discriminant, keyed bare (`Idle`) and qualified
    /// (`SarState::Idle`). Values default sequential from zero, continuing
    /// after an explicit discriminant (SPEC §6.4). Non-constant explicit
    /// discriminants were rejected during elaboration.
    pub fn enum_value_map(&self) -> HashMap<String, i64> {
        let mut map = HashMap::new();
        for (enum_name, decl) in &self.enums {
            let mut next: i64 = 0;
            for variant in &decl.variants {
                let value = variant
                    .value
                    .as_ref()
                    .and_then(|expr| crate::elab::const_eval::ConstEnv::new().eval(expr).ok())
                    .map_or(next, |val| match val {
                        crate::value::Value::Int(v) => v,
                        crate::value::Value::Nat(v) => v as i64,
                        _ => next,
                    });
                map.insert(variant.name.clone(), value);
                map.insert(format!("{enum_name}::{}", variant.name), value);
                next = value + 1;
            }
        }
        map
    }

    /// Every capability declaration.
    pub fn capabilities(&self) -> impl Iterator<Item = (&String, &CapabilityDecl)> {
        self.capabilities.iter()
    }

    /// Look up a capability declaration by name.
    pub fn capability(&self, name: &str) -> Option<&CapabilityDecl> {
        self.capabilities.get(name)
    }

    /// Every global function.
    pub fn functions(&self) -> impl Iterator<Item = &Function> {
        self.functions.values()
    }

    /// Look up a global constant by name.
    pub fn const_(&self, name: &str) -> Option<&Value> {
        self.consts.get(name)
    }

    /// Every global constant.
    pub fn consts(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.consts.iter()
    }

    /// Every impl block.
    pub fn impls(&self) -> &[ImplBlock] { &self.impls }

    /// Every `bench` block.
    pub fn benches(&self) -> impl Iterator<Item = &BenchBlock> {
        self.benches.iter()
    }

    /// The `bench` rooted at `module`, if one exists.
    pub fn bench(&self, module: &str) -> Option<&BenchBlock> {
        self.benches.iter().find(|b| b.module == module)
    }

    // ── Staging layer ─────────────────────────────────────────────────────

    /// Stage a parameter override. Does NOT mutate the elaborated design —
    /// a subsequent re-elaboration consumes the override purely.
    pub fn set_param(&self, path: &str, param: &str, value: Value) {
        self.overrides.borrow_mut().set(path, param, value);
    }

    /// Look up a staged override.
    pub fn get_override(&self, path: &str, param: &str) -> Option<Value> {
        self.overrides.borrow().get(path, param).cloned()
    }

    /// True if any overrides are staged.
    pub fn has_overrides(&self) -> bool {
        !self.overrides.borrow().is_empty()
    }

    /// Clear all staged overrides.
    pub fn clear_overrides(&self) {
        self.overrides.borrow_mut().clear();
    }

    /// An independent copy with its own, empty staging area — every other
    /// field is a cheap structural clone. Used to give each `bench` entry
    /// point a fresh view (SPEC_BENCH.md §9: staged overrides never leak
    /// between entry points).
    pub fn fork(&self) -> Design {
        Design { overrides: Rc::new(RefCell::new(OverrideMap::new())), ..self.clone() }
    }

    /// Consume this design's staged overrides, producing a new `Design`
    /// with them applied to `root_module`'s instances (and, for an empty
    /// path, the module's own params). Non-structural only: the module set
    /// and topology are unchanged, only `Value` defaults are patched —
    /// exactly the effect `ppr_to_ir` would see from a differently-written
    /// source (SPEC_BENCH.md §6.2 "the engine decides"; milestone 1 always
    /// treats a param write as non-structural).
    ///
    /// The override path is the target instance's bare label within
    /// `root_module` (SPEC_BENCH.md §3 name resolution — benches address a
    /// flat, already-monomorphized netlist, so no hierarchical path is
    /// needed). An override naming an unknown instance or param is a
    /// fail-loud error, never a silent no-op.
    pub fn with_overrides_applied(&self, root_module: &str) -> Result<Design, ElabError> {
        let mut design = self.clone();
        let overrides: Vec<(String, String, Value)> =
            self.overrides.borrow().iter().map(|(p, n, v)| (p.clone(), n.clone(), v.clone())).collect();
        let module = design.modules.get_mut(root_module).ok_or_else(|| {
            ElabError::from(ElabErrorKind::Other(format!("bench root module `{root_module}` not found")))
        })?;
        for (path, param, value) in overrides {
            if !value.is_const_scalar() {
                return Err(ElabError::from(ElabErrorKind::Other(format!(
                    "cannot stage a {} value for `{param}`",
                    value.type_name()
                ))));
            }
            let const_val = value;
            if path.is_empty() {
                let p = module.params.iter_mut().find(|p| p.name == param).ok_or_else(|| {
                    ElabError::from(ElabErrorKind::Other(format!(
                        "unknown param `{param}` on module `{root_module}`"
                    )))
                })?;
                p.default = Some(const_val);
                continue;
            }
            let instance = module.instances.iter_mut().find(|i| i.name() == path).ok_or_else(|| {
                ElabError::from(ElabErrorKind::Other(format!(
                    "unknown instance `{path}` in `{root_module}`"
                )))
            })?;
            match instance.params.iter_mut().find(|(name, _)| name == &param) {
                Some((_, v)) => *v = const_val,
                None => instance.params.push((param, const_val)),
            }
        }
        Ok(design)
    }

    // ── Internal access (pub(crate)) ──────────────────────────────────────

    /// Mutable access to the modules map. For internal elaboration use.
    pub(crate) fn modules_map_mut(&mut self) -> &mut HashMap<String, Module> {
        &mut self.modules
    }
    /// Mutable access to the disciplines map. For internal elaboration use.
    pub(crate) fn disciplines_map_mut(&mut self) -> &mut HashMap<String, DisciplineDecl> {
        &mut self.disciplines
    }
    /// Mutable access to the bundles map. For internal elaboration use.
    pub(crate) fn bundles_map_mut(&mut self) -> &mut HashMap<String, BundleDecl> {
        &mut self.bundles
    }
    /// Mutable access to the enums map. For internal elaboration use.
    pub(crate) fn enums_map_mut(&mut self) -> &mut HashMap<String, EnumDecl> {
        &mut self.enums
    }
    /// Mutable access to the capabilities map. For internal elaboration use.
    pub(crate) fn capabilities_map_mut(&mut self) -> &mut HashMap<String, CapabilityDecl> {
        &mut self.capabilities
    }
    /// Mutable access to the functions map. For internal elaboration use.
    pub(crate) fn functions_map_mut(&mut self) -> &mut HashMap<String, Function> {
        &mut self.functions
    }
    /// Mutable access to the impl blocks vec. For internal elaboration use.
    pub(crate) fn impls_vec_mut(&mut self) -> &mut Vec<ImplBlock> {
        &mut self.impls
    }
    /// Mutable access to the consts map. For internal elaboration use.
    pub(crate) fn consts_map_mut(&mut self) -> &mut HashMap<String, Value> {
        &mut self.consts
    }
    /// Mutable access to the bench blocks vec. For internal elaboration use.
    pub(crate) fn benches_vec_mut(&mut self) -> &mut Vec<BenchBlock> {
        &mut self.benches
    }

    /// Insert a module by name. Used internally by the elaborator and by
    /// the digital interpreter bridge to build synthetic modules for
    /// digital-only test scenarios.
    pub(crate) fn insert_module(&mut self, name: String, module: Module) {
        self.modules.insert(name, module);
    }
}

impl Default for Design {
    fn default() -> Self { Self::new() }
}
