//! [`Design`] — the POM root, returned by [`SourceFile::elaborate`][crate::parse::SourceFile::elaborate].

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::parse::ast::{BundleDecl, CapabilityDecl, DisciplineDecl, EnumDecl};
use crate::pom::{
    ElabError, ElabErrorKind, Function, ImplBlock, IntrospectionMeta, ModelId, Module,
    OverrideMap, TermMeta, VAR_KINDS, Value, VarMeta, TERMINAL_KINDS,
};

/// A typed staging failure (SPEC Part VI §8.2–§8.4): the plugin layer maps
/// `UndeclaredType` to P0005 ("type not declared") and `Conflict` to P0008.
#[derive(Debug, Clone)]
pub enum StageError {
    /// No-netlist-magic violation: the staged module type was never declared.
    UndeclaredType { label: String, module: String },
    /// Two writers staged different specs under one `(parent, label)`.
    Conflict(crate::pom::staging::StagingConflict),
    Other(String),
}

impl std::fmt::Display for StageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UndeclaredType { label, module } => write!(
                f,
                "staged instance `{label}`: type `{module}` not declared in the design"
            ),
            Self::Conflict(c) => c.fmt(f),
            Self::Other(msg) => msg.fmt(f),
        }
    }
}

/// Project-level metadata carried by the POM. Item provenance (`origins`)
/// is recorded by the `use` resolver during elaboration; the name/version/
/// dependency fields are stamped by whoever knows the `Piperine.toml`
/// (the CLI, the language server) via [`Design::set_project_meta`].
/// Queryable through reflection — the anchor point for the plugin system
/// (spec Part VI).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Project {
    /// The project name from `Piperine.toml`, if available.
    pub name: Option<String>,
    /// The project version from `Piperine.toml`, if available.
    pub version: Option<String>,
    /// Dependency names (from `Piperine.toml [dependencies]`), if available.
    pub dependencies: Vec<String>,
    /// Item provenance: declared item name → the package it was imported
    /// from. Items declared in the project itself are absent.
    pub origins: HashMap<String, String>,
    /// Item-name → the real on-disk file it was declared in (BUG-1/
    /// LSB-01..03), recorded by [`crate::resolve::Resolver::item_files`]
    /// during elaboration. Absence means "no textual declaration is
    /// tracked" (e.g. a native-only registry entry) — goto-definition
    /// declines rather than fabricating a location.
    pub item_files: HashMap<String, PathBuf>,
}

impl Project {
    /// The package `name` was imported from, `None` for a project-local
    /// item. Monomorphized names (`Dac__8`) resolve through their base
    /// (`Dac`).
    pub fn origin_of(&self, name: &str) -> Option<&str> {
        if let Some(pkg) = self.origins.get(name) {
            return Some(pkg);
        }
        let base = name.split("__").next()?;
        self.origins.get(base).map(String::as_str)
    }

    /// The real on-disk file `name` was declared in, `None` if untracked.
    /// Monomorphized names (`Dac__8`) resolve through their base (`Dac`),
    /// mirroring [`Self::origin_of`].
    pub fn item_file(&self, name: &str) -> Option<&Path> {
        if let Some(path) = self.item_files.get(name) {
            return Some(path.as_path());
        }
        let base = name.split("__").next()?;
        self.item_files.get(base).map(PathBuf::as_path)
    }
}

/// A resolved `@rfport(num, z0)` attribute instance (SP-01): the node it
/// decorates, its 1-based port index, and its reference impedance.
#[derive(Debug, Clone, PartialEq)]
pub struct RfPort {
    pub num: u64,
    pub z0: f64,
    pub node: String,
}

/// The complete output of elaboration — the POM root.
///
/// Fields are `pub(crate)`; external consumers use the public accessor
/// methods that implement the POM reflection interface
/// (`docs/reflection_api.md`).
/// **Serialization (SPEC Part IV §7).** `Design` serializes as itself —
/// there is no separate wire model. The serialized surface is the
/// reflection surface: `modules` (with their ports, params, wires,
/// instances, connections, attributes), `consts`, `project`, and
/// `top_module`. Fields that are compiled ASTs (declarations, functions,
/// behaviors) or live host state (the staging area) are skipped —
/// they are not reflection data and a deserialized `Design` carries their
/// empty defaults. If the reflection surface cannot express something the
/// language has, the POM is incomplete and gets extended — never shadowed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Design {
    pub(crate) modules: HashMap<String, Module>,
    /// Flattened (leaf-only) form of each module — populated by the
    /// `FlattenHierarchy` elaboration pass and consumed only by codegen.
    /// The authored hierarchy lives in `modules` and is never mutated by
    /// flattening (POM navigability mirrors the source); this map is a
    /// derived artifact rebuilt every elaboration, so it is skipped on
    /// serialize — a deserialized `Design` carries an empty `flat_modules`
    /// and the pass is re-runnable.
    #[serde(skip)]
    pub(crate) flat_modules: HashMap<String, Module>,
    #[serde(skip)]
    pub(crate) disciplines: HashMap<String, DisciplineDecl>,
    #[serde(skip)]
    pub(crate) bundles: HashMap<String, BundleDecl>,
    #[serde(skip)]
    pub(crate) enums: HashMap<String, EnumDecl>,
    #[serde(skip)]
    pub(crate) capabilities: HashMap<String, CapabilityDecl>,
    #[serde(skip)]
    pub(crate) functions: HashMap<String, Function>,
    #[serde(skip)]
    pub(crate) impls: Vec<ImplBlock>,
    pub(crate) consts: HashMap<String, Value>,
    /// Project metadata (name, version, dependencies).
    pub(crate) project: Project,
    /// Staged parameter overrides — the single mutation surface in POM.
    /// Writing via `set_param()` stages here; re-elaboration consumes.
    #[serde(skip)]
    pub(crate) overrides: Rc<RefCell<OverrideMap>>,
    /// The top module name, if set by the user or inferred.
    pub(crate) top_module: Option<String>,
}

impl Design {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            flat_modules: HashMap::new(),
            disciplines: HashMap::new(),
            bundles: HashMap::new(),
            enums: HashMap::new(),
            capabilities: HashMap::new(),
            functions: HashMap::new(),
            impls: Vec::new(),
            consts: HashMap::new(),
            project: Project::default(),
            overrides: Rc::new(RefCell::new(OverrideMap::new())),
            top_module: None,
        }
    }

    // ── POM navigation ────────────────────────────────────────────────────

    /// Project metadata (name, version, dependencies, item provenance).
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// Stamp `Piperine.toml`-level metadata onto the design. Called by the
    /// host that knows the project (CLI, language server) — `piperine-lang`
    /// itself never reads the manifest.
    pub fn set_project_meta(
        &mut self,
        name: impl Into<String>,
        version: impl Into<String>,
        dependencies: Vec<String>,
    ) {
        self.project.name = Some(name.into());
        self.project.version = Some(version.into());
        self.project.dependencies = dependencies;
    }

    /// Record item provenance (item name → source package) — called once by
    /// elaboration with the resolver's record.
    pub(crate) fn set_origins(&mut self, origins: HashMap<String, String>) {
        self.project.origins = origins;
    }

    /// Record item-name → real on-disk declaring file (BUG-1/LSB-01..03) —
    /// called once by elaboration with the resolver's record, alongside
    /// [`Self::set_origins`].
    pub(crate) fn set_item_files(&mut self, item_files: HashMap<String, PathBuf>) {
        self.project.item_files = item_files;
    }

    /// The elaborated top module, if set.
    pub fn top(&self) -> Option<&Module> {
        self.top_module.as_ref().and_then(|n| self.modules.get(n))
    }

    /// Evaluate a selector path against this design.
    pub fn select<'a>(&'a self, path: &str) -> Result<crate::pom::selection::NodeSelection<'a>, crate::pom::error::SelectorError> {
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

    /// The flattened (leaf-only) form of module `name`, for codegen. Falls
    /// back to the authored module when no flat form is recorded — a leaf
    /// module's flat form is its authored form, so the pass does not record
    /// one. Returns `None` when `name` is neither a known module nor a
    /// flattened one.
    pub fn flat_module(&self, name: &str) -> Option<&Module> {
        self.flat_modules.get(name).or_else(|| self.modules.get(name))
    }

    /// The mutable module to patch when applying an override — the FLAT form
    /// when present (codegen reads `flat_module` via `lower_bodies`), else
    /// the authored form. For a two-level design the flat form equals the
    /// authored form, so existing override semantics are unchanged; a
    /// flattened root's spliced instance list is what hosts address.
    pub(crate) fn module_for_override_mut(&mut self, name: &str) -> Option<&mut Module> {
        if self.flat_modules.contains_key(name) {
            self.flat_modules.get_mut(name)
        } else {
            self.modules.get_mut(name)
        }
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

    /// Resolve every `@rfport(num, z0)` attribute declared on a wire or port
    /// of `module_name` into a [`RfPort`] (SP-01) — the `.sp` port
    /// declaration path (Part VI attribute-schema machinery, no new device
    /// kind). Fails loud (SP-05): an unknown module, a non-positive `z0`, or
    /// a duplicate port `num`.
    pub fn rfports(&self, module_name: &str) -> Result<Vec<RfPort>, ElabError> {
        let module = self.modules.get(module_name).ok_or_else(|| {
            ElabError::from(ElabErrorKind::Other(format!(
                "@rfport: unknown module `{module_name}`"
            )))
        })?;
        let field_err = |field: &str, reason: String| {
            ElabError::from(ElabErrorKind::AttrSchemaField {
                schema: "rfport".into(),
                field: field.into(),
                reason,
            })
        };
        let candidates = module
            .wires
            .iter()
            .map(|w| (w.name.as_str(), w.attributes.as_slice()))
            .chain(module.ports.iter().map(|p| (p.name.as_str(), p.attributes.as_slice())));
        let mut ports = Vec::new();
        let mut seen_nums = std::collections::HashSet::new();
        for (node, attrs) in candidates {
            for attr in attrs {
                if attr.schema() != "rfport" {
                    continue;
                }
                let num = match attr.field("num") {
                    Some(Value::Nat(n)) => *n,
                    Some(Value::Int(n)) if *n >= 0 => *n as u64,
                    other => {
                        return Err(field_err(
                            "num",
                            format!("expected a non-negative integer port number, got {other:?}"),
                        ));
                    }
                };
                let z0 = match attr.field("z0") {
                    Some(Value::Real(v)) => *v,
                    Some(Value::Nat(n)) => *n as f64,
                    other => return Err(field_err("z0", format!("expected a real z0, got {other:?}"))),
                };
                if z0 <= 0.0 {
                    return Err(field_err("z0", format!("z0 must be positive, got {z0}")));
                }
                if !seen_nums.insert(num) {
                    return Err(field_err("num", format!("duplicate port number {num}")));
                }
                ports.push(RfPort { num, z0, node: node.to_string() });
            }
        }
        Ok(ports)
    }

    /// Resolve the device-introspection metadata sidecar for `module_name`
    /// (phdl-introspection-attributes) — author-declared `@model`/`@name`/
    /// `@unit`/`@description`/`@kind` attributes read off POM nodes into an
    /// [`IntrospectionMeta`] bundle. Mirrors [`Design::rfports`]: iterate the
    /// module's nodes, match each introspection schema, read its fields via
    /// [`Attribute::field`](crate::pom::module::Attribute::field), and fail
    /// loud through [`ElabErrorKind::AttrSchemaField`] (field/enum validation)
    /// or [`ElabErrorKind::Other`] (placement errors) on any violation.
    ///
    /// Every attribute is optional — a module with none yields an empty sidecar
    /// and codegen falls back to its derived defaults (PIA-02/08/12, zero
    /// regression). Placement matrix, `@kind` enum membership, and duplicate
    /// `@name` are validated here so codegen never sees malformed metadata:
    ///
    /// | schema    | module | var | port | wire | enum resolved at this site |
    /// |-----------|:------:|:---:|:----:|:----:|-----------------------------|
    /// | `@model`  | ✓      |     |      |      | —                           |
    /// | `@name`   |        | ✓   | ✓    | ✓    | —                           |
    /// | `@unit`   |        | ✓   |      |      | —                           |
    /// | `@description` | | ✓   | ✓    | ✓    | —                     |
    /// | `@kind`   |        | ✓   | ✓    | ✓    | var→ObservableKind, port/wire→TerminalKind |
    pub fn introspection_meta(&self, module_name: &str) -> Result<IntrospectionMeta, ElabError> {
        let module = self.modules.get(module_name).ok_or_else(|| {
            ElabError::from(ElabErrorKind::Other(format!(
                "introspection: unknown module `{module_name}`"
            )))
        })?;
        let mut meta = IntrospectionMeta::default();

        // Helpers (local closures, scoped to this method): one builds an
        // `AttrSchemaField` error for field/value violations, the other an
        // `Other` error for placement violations — matching `rfports`'s split
        // (AttrSchemaField for field values, Other for the unknown-module case).
        let field_err = |schema: &str, field: &str, reason: String| {
            ElabError::from(ElabErrorKind::AttrSchemaField {
                schema: schema.into(),
                field: field.into(),
                reason,
            })
        };
        let place_err = |schema: &str, node: &str| {
            ElabError::from(ElabErrorKind::Other(format!(
                "`@{schema}` is not valid on a {node} (placement error)"
            )))
        };
        // Read a single-field schema's `value` as a string.
        let str_value = |attr: &crate::pom::module::Attribute, schema: &str| -> Result<String, ElabError> {
            match attr.field("value") {
                Some(Value::Str(s)) => Ok(s.clone()),
                other => Err(field_err(schema, "value", format!("expected a String, got {other:?}"))),
            }
        };

        // @model is legal on the Module only; the other four schemas misplaced here.
        for attr in &module.attributes {
            match attr.schema() {
                "model" => {
                    let type_id = match attr.field("type") {
                        Some(Value::Str(s)) => s.clone(),
                        other => return Err(field_err("model", "type", format!("expected a String, got {other:?}"))),
                    };
                    let version = match attr.field("version") {
                        Some(Value::Str(s)) => s.clone(),
                        other => return Err(field_err("model", "version", format!("expected a String, got {other:?}"))),
                    };
                    meta.model = Some(ModelId { type_id, version });
                }
                "name" | "unit" | "description" | "kind" => return Err(place_err(attr.schema(), "module")),
                _ => {}
            }
        }

        // None of the five introspection schemas target a `param` — any of them
        // here is a placement error (PIA-19). Params carry only their own value.
        for p in &module.params {
            for attr in &p.attributes {
                match attr.schema() {
                    "model" | "name" | "unit" | "description" | "kind" => {
                        return Err(place_err(attr.schema(), "param"));
                    }
                    _ => {}
                }
            }
        }

        // vars: @name/@unit/@description/@kind legal; @model misplaced. The
        // @kind target enum on a var is ObservableKind (VAR_KINDS).
        for v in &module.vars {
            let mut vm = VarMeta::default();
            for attr in &v.attributes {
                match attr.schema() {
                    "name" => vm.name = Some(str_value(attr, "name")?),
                    "unit" => vm.unit = Some(str_value(attr, "unit")?),
                    "description" => vm.description = Some(str_value(attr, "description")?),
                    "kind" => {
                        let raw = str_value(attr, "kind")?;
                        let canonical = raw.to_ascii_lowercase();
                        if !VAR_KINDS.contains(&canonical.as_str()) {
                            return Err(field_err("kind", "value", format!(
                                "`{raw}` is not a valid var @kind (expected one of {VAR_KINDS:?} — an ObservableKind)"
                            )));
                        }
                        vm.kind = Some(canonical);
                    }
                    "model" => return Err(place_err("model", "var")),
                    _ => {}
                }
            }
            if vm.has_any() {
                if let Some(name) = &vm.name {
                    if meta.vars.values().any(|existing| existing.name.as_ref() == Some(name)) {
                        return Err(field_err("name", "value", format!(
                            "duplicate introspection name `{name}` (names must be unique within a module's var catalog)"
                        )));
                    }
                }
                meta.vars.insert(v.name.clone(), vm);
            }
        }

        // ports + wires: @name/@description/@kind legal; @unit/@model misplaced.
        // The @kind target enum on a terminal is TerminalKind (TERMINAL_KINDS).
        let mut collect_terminal = |node_name: &str, attrs: &[crate::pom::module::Attribute], node_kind: &str| -> Result<(), ElabError> {
            let mut tm = TermMeta::default();
            for attr in attrs {
                match attr.schema() {
                    "name" => tm.name = Some(str_value(attr, "name")?),
                    "description" => tm.description = Some(str_value(attr, "description")?),
                    "kind" => {
                        let raw = str_value(attr, "kind")?;
                        let canonical = raw.to_ascii_lowercase();
                        if !TERMINAL_KINDS.contains(&canonical.as_str()) {
                            return Err(field_err("kind", "value", format!(
                                "`{raw}` is not a valid terminal @kind (expected one of {TERMINAL_KINDS:?} — a TerminalKind)"
                            )));
                        }
                        tm.kind = Some(canonical);
                    }
                    "unit" => return Err(place_err("unit", node_kind)),
                    "model" => return Err(place_err("model", node_kind)),
                    _ => {}
                }
            }
            if tm.has_any() {
                meta.terminals.insert(node_name.to_string(), tm);
            }
            Ok(())
        };
        for p in &module.ports {
            collect_terminal(&p.name, &p.attributes, "port")?;
        }
        for w in &module.wires {
            collect_terminal(&w.name, &w.attributes, "wire")?;
        }

        Ok(meta)
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

    /// Stage an instance injection into `parent` (SPEC Part VI §8.2). The
    /// spec's module must be a type declared in this design — a plugin (or
    /// cannot invent a type that was never declared (no-netlist-magic,
    /// Part VI §2). A duplicate label with a different spec is a typed
    /// [`StageError::Conflict`] naming both writers; `staged_by` identifies
    /// this writer (a plugin name).
    pub fn stage_instance(
        &self,
        parent: &str,
        spec: crate::pom::staging::InstanceSpec,
        staged_by: &str,
    ) -> Result<(), StageError> {
        let child = self
            .modules
            .get(&spec.module)
            .ok_or_else(|| StageError::UndeclaredType {
                label: spec.label.clone(),
                module: spec.module.clone(),
            })?;
        if spec.ports.len() != child.ports.len() {
            return Err(StageError::Other(format!(
                "staged instance `{}` connects {} nets, module `{}` has {} ports",
                spec.label,
                spec.ports.len(),
                spec.module,
                child.ports.len()
            )));
        }
        if !self.modules.contains_key(parent) {
            return Err(StageError::Other(format!(
                "staged instance `{}`: parent module `{parent}` not found",
                spec.label
            )));
        }
        self.overrides
            .borrow_mut()
            .add_instance(parent, spec, staged_by)
            .map_err(StageError::Conflict)
    }

    /// Stage a net connection into `parent` (SPEC Part VI §8.2).
    pub fn stage_connection(
        &self,
        parent: &str,
        spec: crate::pom::staging::ConnectionSpec,
    ) -> Result<(), StageError> {
        if !self.modules.contains_key(parent) {
            return Err(StageError::Other(format!(
                "staged connection: parent module `{parent}` not found"
            )));
        }
        self.overrides.borrow_mut().add_connection(parent, spec);
        Ok(())
    }

    /// An independent copy with its own, empty staging area — every other
    /// field is a cheap structural clone. Used to give each host entry
    /// point a fresh view (staged overrides never leak between entry
    /// points).
    pub fn fork(&self) -> Design {
        Design { overrides: Rc::new(RefCell::new(OverrideMap::new())), ..self.clone() }
    }

    /// Consume this design's staged overrides, producing a new `Design`
    /// with them applied to `root_module`'s instances (and, for an empty
    /// path, the module's own params). Non-structural only: the module set
    /// and topology are unchanged, only `Value` defaults are patched —
    /// exactly the effect a differently-written source would have (a param
    /// write is always treated as non-structural).
    ///
    /// The override path is the target instance's bare label within
    /// `root_module` (hosts address a flat, already-monomorphized netlist,
    /// so no hierarchical path is needed). An override naming an unknown
    /// instance or param is a fail-loud error, never a silent no-op.
    pub fn with_overrides_applied(&self, root_module: &str) -> Result<Design, ElabError> {
        let mut design = self.clone();
        let overrides: Vec<(String, String, Value)> =
            self.overrides.borrow().iter().map(|(p, n, v)| (p.clone(), n.clone(), v.clone())).collect();
        // Patch the FLAT form when present (codegen reads `flat_module` via
        // `lower_bodies`); fall back to authored. For a two-level design the
        // flat form equals the authored form, so existing override semantics
        // are unchanged. A flattened root's spliced instance list is what
        // hosts address (e.g. `x.seg0`). FLAT-03.
        let module = design.module_for_override_mut(root_module).ok_or_else(|| {
            ElabError::from(ElabErrorKind::Other(format!("root module `{root_module}` not found")))
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
        // Apply staged instance/connection injections (SPEC Part VI §8.2).
        // The type/arity checks ran at staging time; here the specs become
        // ordinary POM nodes on the parent module — patched into the flat
        // form when present (same retarget as the root override above).
        let staged_instances: Vec<crate::pom::staging::StagedInstance> =
            self.overrides.borrow().added_instances().to_vec();
        let staged_connections: Vec<(String, crate::pom::staging::ConnectionSpec)> =
            self.overrides.borrow().added_connections().to_vec();
        for staged in staged_instances {
            let (parent, spec) = (staged.parent, staged.spec);
            let module = design.module_for_override_mut(&parent).ok_or_else(|| {
                ElabError::from(ElabErrorKind::Other(format!(
                    "staged instance `{}`: parent module `{parent}` not found",
                    spec.label
                )))
            })?;
            if module.instances.iter().any(|i| i.name() == spec.label) {
                return Err(ElabError::from(ElabErrorKind::Other(format!(
                    "staging conflict: `{parent}` already has an instance `{}`",
                    spec.label
                ))));
            }
            module.instances.push(crate::pom::module::Instance {
                span: None,
                attributes: Vec::new(),
                doc: None,
                label: Some(spec.label),
                module: spec.module,
                ports: spec.ports.iter().map(|n| crate::pom::net_type::NetRef::simple(n.clone())).collect(),
                params: spec.params,
            });
        }
        for (parent, spec) in staged_connections {
            let module = design.module_for_override_mut(&parent).ok_or_else(|| {
                ElabError::from(ElabErrorKind::Other(format!(
                    "staged connection: parent module `{parent}` not found"
                )))
            })?;
            module.connections.push(crate::pom::module::Connection {
                span: None,
                lhs: crate::pom::net_type::NetRef::simple(spec.lhs),
                rhs: crate::pom::net_type::NetRef::simple(spec.rhs),
            });
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
    /// Insert a module by name. Test-only: the selector unit tests build
    /// synthetic designs without running the elaborator.
    #[cfg(test)]
    pub(crate) fn insert_module(&mut self, name: String, module: Module) {
        self.modules.insert(name, module);
    }
    /// Insert a flat module by name. Test-only: the flatten unit tests build
    /// synthetic `flat_modules` maps without running the pass.
    #[cfg(test)]
    pub(crate) fn insert_flat_module(&mut self, name: String, module: Module) {
        self.flat_modules.insert(name, module);
    }
}

impl Default for Design {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod flat_module_tests {
    use super::*;
    use crate::pom::{Module, NetType, Wire};

    fn leaf_module(name: &str) -> Module {
        Module::new(
            name.to_string(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    fn module_with_wire(name: &str, wire: &str) -> Module {
        Module::new(
            name.to_string(),
            Vec::new(),
            Vec::new(),
            vec![Wire {
                span: None,
                attributes: Vec::new(),
                doc: None,
                name: wire.to_string(),
                ty: NetType::Discipline("Electrical".to_string()),
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    /// `flat_module` falls back to `modules` when no flat form is recorded —
    /// a leaf module's flat form is its authored form.
    #[test]
    fn flat_module_falls_back_to_authored_when_absent() {
        let mut design = Design::new();
        design.insert_module("Foo".to_string(), leaf_module("Foo"));
        let flat = design.flat_module("Foo").expect("Foo present in modules");
        assert_eq!(flat.name(), "Foo");
        assert!(design.flat_modules.is_empty(), "no flat form recorded");
    }

    /// `flat_module` prefers the flat form over the authored form when both
    /// are present — codegen consumes the flattened netlist.
    #[test]
    fn flat_module_prefers_flat_form_when_present() {
        let mut design = Design::new();
        design.insert_module("Foo".to_string(), leaf_module("Foo"));
        design.insert_flat_module("Foo".to_string(), module_with_wire("Foo", "internal"));
        let returned = design.flat_module("Foo").expect("Foo present");
        assert_eq!(returned.wires.len(), 1, "flat form preferred over authored");
        assert_eq!(returned.wires[0].name, "internal");
    }

    /// `flat_module` returns `None` when the name is neither authored nor
    /// flattened — a fail-loud signal, never a silent default.
    #[test]
    fn flat_module_returns_none_when_unknown() {
        let design = Design::new();
        assert!(design.flat_module("Missing").is_none());
    }

    /// `flat_module` resolves a name present only in `flat_modules` — the
    /// authored map need not contain it (a fully-synthetic flat module).
    #[test]
    fn flat_module_resolves_flat_only_name() {
        let mut design = Design::new();
        design.insert_flat_module("Synth".to_string(), leaf_module("Synth"));
        let flat = design.flat_module("Synth").expect("Synth in flat_modules");
        assert_eq!(flat.name(), "Synth");
        assert!(design.modules.get("Synth").is_none());
    }
}
