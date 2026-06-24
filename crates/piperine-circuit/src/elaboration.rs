use std::collections::HashMap;

use piperine_parser::ast::{self, Expr, Literal, PathSegment, PrefixOp};
use piperine_parser::model::{Document, Module};
use crate::error::ElaborationError;
use crate::registry::HardwareRegistry;
use crate::types::{ParameterValue, ParameterMap, ConnectionMap, parse_si_real};
use crate::hardware::NetResolver;

/// Pre-resolved paramset — preset parameters and the model name/type ready for emission.
struct ParamsetInfo {
    base_module: String,
    /// All preset params (including `model`).
    preset: ParameterMap,
    /// SPICE model card type keyword (from `base.spice_model_type()`), e.g. "NMOS".
    model_type: Option<&'static str>,
}

type ParamsetMap = HashMap<String, ParamsetInfo>;

/// Maps this module's net names → flat SPICE net names.
/// Top-level: empty (nets resolve to themselves).
/// Sub-module: ports → parent's SPICE net; internal nets → mangled.
type NetMap = HashMap<String, String>;

/// Result of elaborating one testbench module.
pub struct ElaborationResult {
    /// SPICE netlist lines without `.end` — caller appends it.
    pub spice_lines: Vec<String>,
    /// The `initial` block body, ready for the interpreter.
    pub initial_statement: ast::Stmt,
    /// Collected `always` block handlers.
    pub always_handlers: AlwaysHandlerSet,
    /// User-defined `function`s from the testbench, for the interpreter to call.
    pub functions: Vec<piperine_parser::model::Function>,
    /// Device instances as `(piperine_name, spice_name)` pairs. The interpreter
    /// binds a `DeviceHandle` under the *piperine* name (what the user writes,
    /// e.g. `load`) but queries the simulator with the *spice* name (`Rload`).
    pub instances: Vec<(String, String)>,
}

/// `AlwaysHandlerSet` — collected from all `always @(...)` blocks in the testbench module.
#[derive(Default, Clone)]
pub struct AlwaysHandlerSet {
    pub initial_step: Vec<ast::Stmt>,
    pub final_step:   Vec<ast::Stmt>,
    pub step:         Vec<ast::Stmt>,
    pub above:        Vec<(ast::Expr, u32, ast::Stmt)>,
    pub cross:        Vec<(ast::Expr, i8, u32, ast::Stmt)>,
}

/// Elaborate the first testbench module found in `document`.
///
/// Uses recursive hierarchical flattening: piperine sub-modules are inlined
/// with `{instance_name}_{net}` mangling so ngspice's flat namespace stays collision-free.
pub fn elaborate(
    document: &Document,
    registry: &HardwareRegistry,
) -> Result<ElaborationResult, ElaborationError> {
    let testbench = find_testbench(document).ok_or(ElaborationError::NoTestbench)?;

    let mut spice_lines = vec![format!("* piperine: {}", testbench.name)];

    // Build paramset map from document declarations.
    let paramsets = build_paramset_map(&document.paramsets, registry)?;

    // Top-level net map is empty: nets resolve to themselves.
    let net_map = NetMap::new();
    let mut instances_list = Vec::new();
    elaborate_instances(
        &testbench.instances, document, registry, &paramsets, "", &net_map, &mut spice_lines, &mut instances_list,
    )?;

    let initial_statement = testbench
        .initial_blocks
        .first()
        .ok_or(ElaborationError::NoTestbench)?
        .stmt
        .clone();

    let mut always_handlers = AlwaysHandlerSet::default();
    let mut crossing_id = 0u32;

    for ab in &testbench.always_blocks {
        match &ab.sensitivity {
            ast::AlwaysSensitivity::InitialStep => always_handlers.initial_step.push(*ab.stmt.clone()),
            ast::AlwaysSensitivity::FinalStep   => always_handlers.final_step.push(*ab.stmt.clone()),
            ast::AlwaysSensitivity::Step        => always_handlers.step.push(*ab.stmt.clone()),
            ast::AlwaysSensitivity::Above(expr) => {
                always_handlers.above.push((expr.clone(), crossing_id, *ab.stmt.clone()));
                crossing_id += 1;
            }
            ast::AlwaysSensitivity::Cross(expr, dir) => {
                always_handlers.cross.push((expr.clone(), *dir, crossing_id, *ab.stmt.clone()));
                crossing_id += 1;
            }
        }
    }
    
    // Auto-save operating-point parameters
    let saves = collect_op_saves(&initial_statement, &always_handlers, &instances_list);
    spice_lines.extend(saves);

    Ok(ElaborationResult {
        spice_lines,
        initial_statement,
        always_handlers,
        functions: testbench.functions.clone(),
        instances: instances_list,
    })
}

fn build_paramset_map(
    paramsets: &[ast::ParamsetDecl],
    registry: &HardwareRegistry,
) -> Result<ParamsetMap, ElaborationError> {
    let mut map = ParamsetMap::new();
    for ps in paramsets {
        let base_def = registry.get(&ps.base.0)
            .ok_or_else(|| ElaborationError::UnknownModule { name: ps.base.0.clone() })?;
        let mut preset = ParameterMap::new();
        for entry in &ps.entries {
            let val = ast_expr_to_parameter_value(&entry.value, &entry.name.0, &ps.name.0)?;
            preset.insert(entry.name.0.clone(), val);
        }
        map.insert(ps.name.0.clone(), ParamsetInfo {
            base_module: ps.base.0.clone(),
            preset,
            model_type: base_def.spice_model_type(),
        });
    }
    Ok(map)
}

fn elaborate_instances(
    instances: &[piperine_parser::model::Instance],
    document: &Document,
    registry: &HardwareRegistry,
    paramsets: &ParamsetMap,
    path: &str,
    net_map: &NetMap,
    spice_lines: &mut Vec<String>,
    instances_list: &mut Vec<(String, String)>,
) -> Result<(), ElaborationError> {
    for instance in instances {
        elaborate_instance(instance, instances, document, registry, paramsets, path, net_map, spice_lines, instances_list)?;
    }
    Ok(())
}

fn elaborate_instance(
    instance: &piperine_parser::model::Instance,
    sibling_instances: &[piperine_parser::model::Instance],
    document: &Document,
    registry: &HardwareRegistry,
    paramsets: &ParamsetMap,
    path: &str,
    net_map: &NetMap,
    spice_lines: &mut Vec<String>,
    instances_list: &mut Vec<(String, String)>,
) -> Result<(), ElaborationError> {
    // Resolve port connections to flat SPICE net names using the current net_map.
    let connections = resolve_connections(
        &instance.connections, &instance.name, path, net_map,
    )?;

    if let Some(ps_info) = paramsets.get(&instance.module) {
        // ── Paramset instance: emit .model card + delegate to base hardware ──
        let base_def = registry.get(&ps_info.base_module)
            .ok_or_else(|| ElaborationError::UnknownModule { name: ps_info.base_module.clone() })?;

        // Merge: preset params first, then instance overrides (not including "model").
        let mut merged = ps_info.preset.clone();
        for conn in &instance.params {
            if let piperine_parser::model::Connection::Named { port, expr: Some(expr) } = conn {
                if port != "model" {
                    let val = ast_expr_to_parameter_value(expr, port, &instance.name)?;
                    merged.insert(port.clone(), val);
                }
            }
        }

        // Extract model name (optional — passives like res/cap/ind have no model card).
        let model_name: Option<String> = ps_info.preset.get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Emit .model card only when base module needs one.
        if let Some(spice_type) = ps_info.model_type {
            let name = model_name.as_deref().ok_or_else(|| ElaborationError::MissingParameter {
                parameter: "model".into(),
                instance: instance.name.clone(),
            })?;
            let model_params: String = merged.iter()
                .filter(|(k, _)| *k != "model")
                .map(|(k, v)| format!("{}={}", k, v.to_spice_string()))
                .collect::<Vec<_>>()
                .join(" ");
            if model_params.is_empty() {
                spice_lines.push(format!(".model {} {}", name, spice_type));
            } else {
                spice_lines.push(format!(".model {} {} ({})", name, spice_type, model_params));
            }
        }

        // Insert model name into params only when present.
        if let Some(ref name) = model_name {
            merged.insert("model".into(), ParameterValue::String(format!("\"{}\"", name)));
        }
        let resolver = ConcreteNetResolver { net_map, path, document };
        let hw_instance = base_def.instantiate(&instance.name, &merged, &connections, &resolver)?;
        let lines = hw_instance.spice_lines();
        instances_list.push((instance.name.clone(), spice_element_name(&lines, &instance.name)));
        spice_lines.extend(lines);

    } else if let Some(definition) = registry.get(&instance.module) {
        // ── Leaf hardware (extern module, OSDI device, SPICE primitive) ──────
        let mut parameters = resolve_parameters(
            &instance.params, &instance.name, definition.parameters(),
        )?;
        // Resolve `parameter ref` — bare instance identifiers → SPICE element names.
        resolve_ref_params(
            &instance.params, &instance.name, definition.parameters(),
            sibling_instances, registry, paramsets, &mut parameters,
        )?;
        let resolver = ConcreteNetResolver { net_map, path, document };
        let hw_instance = definition.instantiate(&instance.name, &parameters, &connections, &resolver)?;
        let lines = hw_instance.spice_lines();
        instances_list.push((instance.name.clone(), spice_element_name(&lines, &instance.name)));
        spice_lines.extend(lines);
    } else if let Some(sub_mod) = find_structural_module(document, &instance.module) {
        // ── Piperine sub-module: flatten inline with path-prefixed net names ──
        let sub_path = build_path(path, &instance.name);
        let sub_net_map = build_sub_net_map(sub_mod, &connections, &sub_path);
        elaborate_instances(
            &sub_mod.instances, document, registry, paramsets, &sub_path, &sub_net_map, spice_lines, instances_list,
        )?;
    } else {
        return Err(ElaborationError::UnknownModule { name: instance.module.clone() });
    }

    Ok(())
}

/// Build the hierarchical path string for a sub-instance.
/// `""` + `"X1"` → `"X1"`, `"X1"` + `"U2"` → `"X1_U2"`.
fn build_path(parent: &str, instance: &str) -> String {
    if parent.is_empty() { instance.to_string() } else { format!("{parent}_{instance}") }
}

/// Mangle a net name with the current hierarchy path.
/// Ground (`"0"`) and empty paths are never mangled.
fn mangle_net(path: &str, net: &str) -> String {
    if net == "0" || path.is_empty() {
        net.to_string()
    } else {
        format!("{path}_{net}")
    }
}

/// Resolve a raw net name from source to its flat SPICE name.
/// `gnd` always maps to `0`. Everything else is looked up in `net_map`;
/// if absent, it belongs to the current module's own namespace → mangle.
fn resolve_net(raw: &str, net_map: &NetMap, path: &str) -> String {
    if raw == "gnd" || raw.is_empty() {
        return "0".to_string();
    }
    net_map.get(raw).cloned().unwrap_or_else(|| mangle_net(path, raw))
}

struct ConcreteNetResolver<'a> {
    net_map: &'a NetMap,
    path: &'a str,
    document: &'a Document,
}

impl<'a> NetResolver for ConcreteNetResolver<'a> {
    fn resolve(&self, raw: &str) -> String {
        resolve_net(raw, self.net_map, self.path)
    }

    fn get_function(&self, name: &str) -> Option<&piperine_parser::model::Function> {
        self.document.modules.iter().flat_map(|m| &m.functions).find(|f| f.name == name)
    }
}

/// Build the NetMap for a sub-module instance.
/// - Port nets → parent's SPICE net (already in `connections`).
/// - Internal nets (declared via `wire`/`electrical`) → mangled with `sub_path`.
fn build_sub_net_map(
    sub_mod: &Module,
    connections: &ConnectionMap,
    sub_path: &str,
) -> NetMap {
    let mut map = NetMap::new();

    // Ports inherit the parent's SPICE names from the resolved connections.
    for port in &sub_mod.ports {
        if let Some(spice_net) = connections.get(&port.name) {
            map.insert(port.name.clone(), spice_net.clone());
        }
    }

    // Internal nets (electrical / wire declarations) get mangled names.
    // Only insert if the net wasn't already mapped as a port.
    for net in &sub_mod.nets {
        for member in &net.members {
            map.entry(member.name.clone())
                .or_insert_with(|| mangle_net(sub_path, &member.name));
        }
    }

    map
}

/// Find a module that can be instantiated as a structural sub-module:
/// no initial block (not a testbench) and no analog block (not a pure VA module
/// — those are compiled by OpenVAF and registered in the HardwareRegistry).
fn find_structural_module<'a>(document: &'a Document, name: &str) -> Option<&'a Module> {
    document.modules.iter().find(|m| {
        m.name == name && m.initial_blocks.is_empty() && m.analog_blocks.is_empty()
    })
}

fn find_testbench(document: &Document) -> Option<&Module> {
    document.modules.iter().find(|m| !m.initial_blocks.is_empty())
}

fn resolve_connections(
    source_connections: &[piperine_parser::model::Connection],
    instance_name: &str,
    path: &str,
    net_map: &NetMap,
) -> Result<ConnectionMap, ElaborationError> {
    let mut map = ConnectionMap::new();
    for connection in source_connections {
        match connection {
            piperine_parser::model::Connection::Named { port, expr } => {
                let raw = match expr {
                    Some(Expr::Path(p)) => path_to_net_name(p),
                    None => String::new(),
                    Some(_) => return Err(ElaborationError::ConnectionError {
                        instance: instance_name.to_string(),
                        detail: format!("port `{port}` must connect to a net name, not an expression"),
                    }),
                };
                map.insert(port.clone(), resolve_net(&raw, net_map, path));
            }
            piperine_parser::model::Connection::Positional(_) => {
                return Err(ElaborationError::ConnectionError {
                    instance: instance_name.to_string(),
                    detail: "positional port connections not supported; use named: .p(net)".into(),
                });
            }
        }
    }
    Ok(map)
}

fn resolve_parameters(
    source_connections: &[piperine_parser::model::Connection],
    instance_name: &str,
    definitions: &[crate::hardware::ParameterDefinition],
) -> Result<ParameterMap, ElaborationError> {
    let mut map: ParameterMap = definitions
        .iter()
        .filter_map(|d| d.default.as_ref().map(|v| (d.name.clone(), v.clone())))
        .collect();

    for connection in source_connections {
        match connection {
            piperine_parser::model::Connection::Named { port, expr } => {
                if let Some(expr) = expr {
                    let def = definitions.iter().find(|d| d.name == *port);
                    // Ref params are handled separately by resolve_ref_params.
                    if def.map(|d| d.is_ref).unwrap_or(false) { continue; }
                    let is_expr = def.map(|d| d.is_expr).unwrap_or(false);
                    let value = if is_expr {
                        ParameterValue::Ast(expr.clone())
                    } else {
                        ast_expr_to_parameter_value(expr, port, instance_name)?
                    };
                    map.insert(port.clone(), value);
                }
            }
            piperine_parser::model::Connection::Positional(_) => {
                return Err(ElaborationError::TypeError {
                    parameter: "<positional>".into(),
                    detail: "positional parameter overrides not supported; use named syntax: #(.r(1k))".into(),
                });
            }
        }
    }

    for definition in definitions {
        // Ref params are validated and inserted by resolve_ref_params.
        if definition.is_ref { continue; }
        // Expr params are optional at this layer — the device decides whether the
        // behavioral expression is required (`key_expr` errors) or an optional
        // alternative to a numeric form (`opt_key_expr` falls back).
        if definition.is_expr { continue; }
        if definition.default.is_none() && !map.contains_key(&definition.name) {
            return Err(ElaborationError::MissingParameter {
                parameter: definition.name.clone(),
                instance: instance_name.to_string(),
            });
        }
    }

    Ok(map)
}

/// Resolves `parameter ref` entries: bare instance identifiers → SPICE element names.
fn resolve_ref_params(
    source_connections: &[piperine_parser::model::Connection],
    instance_name: &str,
    definitions: &[crate::hardware::ParameterDefinition],
    sibling_instances: &[piperine_parser::model::Instance],
    registry: &HardwareRegistry,
    paramsets: &ParamsetMap,
    out: &mut ParameterMap,
) -> Result<(), ElaborationError> {
    for def in definitions.iter().filter(|d| d.is_ref) {
        let expr = source_connections.iter().find_map(|c| {
            if let piperine_parser::model::Connection::Named { port, expr: Some(e) } = c {
                if port == &def.name { Some(e) } else { None }
            } else { None }
        });
        let expr = expr.ok_or_else(|| ElaborationError::MissingParameter {
            parameter: def.name.clone(),
            instance: instance_name.to_string(),
        })?;
        let ident = match expr {
            Expr::Path(p) => path_to_net_name(p),
            _ => return Err(ElaborationError::TypeError {
                parameter: def.name.clone(),
                detail: format!(
                    "parameter `{}` on `{}` must be an instance reference (bare identifier), not a literal",
                    def.name, instance_name
                ),
            }),
        };
        // Find the referenced sibling instance and compute its SPICE element name.
        let ref_inst = sibling_instances.iter().find(|i| i.name == ident)
            .ok_or_else(|| ElaborationError::ConnectionError {
                instance: instance_name.to_string(),
                detail: format!("ref `{}` = `{ident}` — no instance named `{ident}` in this module", def.name),
            })?;
        let hw_def = if let Some(ps) = paramsets.get(&ref_inst.module) {
            registry.get(&ps.base_module)
        } else {
            registry.get(&ref_inst.module)
        };
        let spice_ename = get_spice_instance_name(&ident, hw_def.and_then(|d| d.spice_instance_prefix()));
        out.insert(def.name.clone(), ParameterValue::String(format!("\"{spice_ename}\"")));
    }
    Ok(())
}

/// The SPICE element name an instance actually emits, taken as the first token of
/// its first netlist line — the ground truth regardless of how a device computes
/// its prefix. Falls back to the piperine name if no line was produced.
fn spice_element_name(lines: &[String], fallback: &str) -> String {
    lines.first()
        .and_then(|l| l.split_whitespace().next())
        .unwrap_or(fallback)
        .to_string()
}

fn get_spice_instance_name(ident: &str, prefix: Option<char>) -> String {
    match prefix {
        Some(prefix) => {
            let up = ident.chars().next().map(|c| c.to_ascii_uppercase());
            if up == Some(prefix.to_ascii_uppercase()) {
                ident.to_string()
            } else {
                format!("{prefix}{ident}")
            }
        }
        None => ident.to_string(),
    }
}

fn ast_expr_to_parameter_value(
    expr: &Expr,
    parameter: &str,
    instance: &str,
) -> Result<ParameterValue, ElaborationError> {
    match expr {
        Expr::Literal(Literal::IntNumber(s)) => {
            s.parse::<i64>().map(ParameterValue::Integer).map_err(|_| ElaborationError::TypeError {
                parameter: parameter.into(),
                detail: format!("cannot parse integer: {s}"),
            })
        }
        Expr::Literal(Literal::StdRealNumber(s)) => {
            s.parse::<f64>().map(ParameterValue::Real).map_err(|_| ElaborationError::TypeError {
                parameter: parameter.into(),
                detail: format!("cannot parse real: {s}"),
            })
        }
        Expr::Literal(Literal::SiRealNumber(s)) => {
            parse_si_real(s).map(ParameterValue::Real).ok_or_else(|| ElaborationError::TypeError {
                parameter: parameter.into(),
                detail: format!("cannot parse SI real: {s}"),
            })
        }
        Expr::Literal(Literal::StrLit(s)) => Ok(ParameterValue::String(s.clone())),
        Expr::Prefix(ast::PrefixOp::Neg, inner) => {
            match ast_expr_to_parameter_value(inner, parameter, instance)? {
                ParameterValue::Real(v)    => Ok(ParameterValue::Real(-v)),
                ParameterValue::Integer(v) => Ok(ParameterValue::Integer(-v)),
                _ => Err(ElaborationError::TypeError {
                    parameter: parameter.into(),
                    detail: "cannot negate a string".into(),
                }),
            }
        }
        _ => Err(ElaborationError::TypeError {
            parameter: parameter.into(),
            detail: format!("parameter `{parameter}` on instance `{instance}` must be a literal"),
        }),
    }
}

pub fn path_to_net_name(path: &ast::Path) -> String {
    match &path.segment {
        PathSegment::Ident(s) => s.clone(),
        PathSegment::Root     => "root".to_string(),
    }
}

pub fn path_to_string(path: &ast::Path) -> String {
    let mut parts = Vec::new();
    let mut current = path;
    loop {
        match &current.segment {
            PathSegment::Ident(s) => parts.push(s.clone()),
            PathSegment::Root     => parts.push("root".to_string()),
        }
        match &current.qualifier {
            Some(qualifier) => current = qualifier,
            None            => break,
        }
    }
    parts.reverse();
    parts.join(".")
}

pub struct VaModuleInfo {
    pub module_name: String,
    pub port_names: Vec<String>,
    /// (parameter_name, default_expr)
    pub parameter_defaults: Vec<(String, piperine_parser::ast::Expr)>,
}

/// Find all VA modules (analog block present, no initial block).
// ── Circuit / SOA types ──────────────────────────────────────────────────────

/// Comparison operator for an SOA check (the condition that triggers a violation).
#[derive(Debug, Clone, PartialEq)]
pub enum SoaOp { Gt, Ge, Lt, Le }

/// One SOA (Safe Operating Area) guard compiled from `always @(step)` in a structural module.
#[derive(Debug, Clone)]
pub struct SoaCheck {
    /// `.meas` variable name, e.g. `_soa_0`.
    pub meas_name: String,
    /// Human-readable label from `$run_error("label")`.
    pub label: String,
    /// Threshold value.
    pub threshold: f64,
    /// Comparison that constitutes a violation.
    pub op: SoaOp,
}

/// Output of elaborating a structural (hardware) module.
pub struct Circuit {
    /// SPICE netlist lines (without `.end`).
    pub spice_lines: Vec<String>,
    /// SOA guards compiled from `always @(step)` blocks.
    pub soa_checks: Vec<SoaCheck>,
}

/// Elaborate a structural module (no `initial` block, no analog block).
///
/// If `module_name` is `Some(name)`, that module is targeted; otherwise the first
/// structural module in the document is used.  `always @(step)` blocks are compiled
/// to `.meas tran` lines + `SoaCheck` entries that the Python bridge can evaluate
/// after each transient run.
pub fn elaborate_circuit(
    document: &Document,
    registry: &HardwareRegistry,
    module_name: Option<&str>,
) -> Result<Circuit, ElaborationError> {
    let module = match module_name {
        Some(name) => find_structural_module(document, name)
            .ok_or_else(|| ElaborationError::NoModule { name: name.to_string() })?,
        None => document.modules.iter()
            .find(|m| m.initial_blocks.is_empty() && m.analog_blocks.is_empty())
            .ok_or(ElaborationError::NoTestbench)?,
    };

    let mut spice_lines = vec![format!("* piperine circuit: {}", module.name)];
    let paramsets = build_paramset_map(&document.paramsets, registry)?;
    let net_map = NetMap::new();
    let mut instances_list = Vec::new();
    elaborate_instances(
        &module.instances, document, registry, &paramsets, "", &net_map,
        &mut spice_lines, &mut instances_list,
    )?;

    let mut soa_checks = Vec::new();
    let mut soa_counter = 0u32;

    for ab in &module.always_blocks {
        if let ast::AlwaysSensitivity::Step = &ab.sensitivity {
            compile_soa_block(&ab.stmt, &mut soa_checks, &mut soa_counter, &mut spice_lines);
        }
    }

    Ok(Circuit { spice_lines, soa_checks })
}

fn compile_soa_block(
    stmt: &ast::Stmt,
    checks: &mut Vec<SoaCheck>,
    counter: &mut u32,
    spice_lines: &mut Vec<String>,
) {
    match stmt {
        ast::Stmt::Block(b) => {
            for item in &b.items {
                if let ast::BlockItem::Stmt(s) = item {
                    compile_soa_block(s, checks, counter, spice_lines);
                }
            }
        }
        ast::Stmt::If(i) if i.else_branch.is_none() => {
            if let Some((expr_str, op, threshold)) = try_extract_soa_condition(&i.condition) {
                if let Some(label) = try_extract_run_error(&i.then_branch) {
                    let meas_name = format!("_soa_{}", counter);
                    *counter += 1;
                    let meas_fn = match op { SoaOp::Gt | SoaOp::Ge => "MAX", SoaOp::Lt | SoaOp::Le => "MIN" };
                    spice_lines.push(format!(".meas tran {} {} {}", meas_name, meas_fn, expr_str));
                    checks.push(SoaCheck { meas_name, label, threshold, op });
                }
            }
        }
        _ => {}
    }
}

fn try_extract_soa_condition(expr: &ast::Expr) -> Option<(String, SoaOp, f64)> {
    let ast::Expr::Binary(lhs, binop, rhs) = expr else { return None; };
    let op = match binop {
        ast::BinOp::Gt => SoaOp::Gt,
        ast::BinOp::Ge => SoaOp::Ge,
        ast::BinOp::Lt => SoaOp::Lt,
        ast::BinOp::Le => SoaOp::Le,
        _ => return None,
    };
    let probe_expr = soa_probe_to_spice(lhs)?;
    let threshold = literal_to_f64(rhs)?;
    Some((probe_expr, op, threshold))
}

fn soa_probe_to_spice(expr: &ast::Expr) -> Option<String> {
    let ast::Expr::Call(ast::FunctionRef::Path(p), args) = expr else { return None; };
    if p.qualifier.is_some() { return None; }
    let name = match &p.segment { PathSegment::Ident(s) => s.as_str(), _ => return None };
    match name {
        "V" | "v" => {
            let nodes: Option<Vec<String>> = args.iter().map(|a| {
                if let ast::CallArg::Positional(ast::Expr::Path(p)) = a {
                    if let PathSegment::Ident(s) = &p.segment { return Some(s.clone()); }
                }
                None
            }).collect();
            Some(format!("v({})", nodes?.join(",")))
        }
        "I" | "i" => {
            if let [ast::CallArg::Positional(ast::Expr::Path(p))] = args.as_slice() {
                if let PathSegment::Ident(s) = &p.segment { return Some(format!("i({})", s)); }
            }
            None
        }
        _ => None,
    }
}

fn literal_to_f64(expr: &ast::Expr) -> Option<f64> {
    match expr {
        Expr::Literal(Literal::StdRealNumber(s)) => s.parse().ok(),
        Expr::Literal(Literal::SiRealNumber(s)) => parse_si_real(s),
        Expr::Literal(Literal::IntNumber(s)) => s.parse::<i64>().ok().map(|v| v as f64),
        Expr::Prefix(PrefixOp::Neg, inner) => literal_to_f64(inner).map(|v| -v),
        _ => None,
    }
}

fn try_extract_run_error(stmt: &ast::Stmt) -> Option<String> {
    match stmt {
        ast::Stmt::Expr(e) => extract_run_error_from_expr(&e.expr),
        ast::Stmt::Block(b) if b.items.len() == 1 => {
            if let ast::BlockItem::Stmt(s) = &b.items[0] { try_extract_run_error(s) } else { None }
        }
        _ => None,
    }
}

fn extract_run_error_from_expr(expr: &ast::Expr) -> Option<String> {
    if let ast::Expr::Call(ast::FunctionRef::SysFun(name), args) = expr {
        if name == "run_error" || name == "$run_error" {
            if let Some(ast::CallArg::Positional(ast::Expr::Literal(ast::Literal::StrLit(s)))) = args.first() {
                return Some(s.trim_matches('"').to_string());
            }
        }
    }
    None
}

pub fn extract_va_modules(document: &Document) -> Vec<VaModuleInfo> {
    document.modules.iter()
        .filter(|m| !m.analog_blocks.is_empty() && m.initial_blocks.is_empty())
        .map(|m| VaModuleInfo {
            module_name: m.name.clone(),
            port_names: m.ports.iter().map(|p| p.name.clone()).collect(),
            parameter_defaults: m.parameters.iter()
                .filter(|p| !p.is_local)
                .map(|p| (p.name.clone(), p.default_value.clone()))
                .collect(),
        })
        .collect()
}

/// Convert a compile-time-constant AST expression to a ParameterValue.
pub fn eval_default_expr(expr: &Expr) -> Option<ParameterValue> {
    match expr {
        Expr::Literal(Literal::StdRealNumber(s)) =>
            s.parse::<f64>().ok().map(ParameterValue::Real),
        Expr::Literal(Literal::SiRealNumber(s)) =>
            parse_si_real(s).map(ParameterValue::Real),
        Expr::Literal(Literal::IntNumber(s)) =>
            s.parse::<i64>().ok().map(ParameterValue::Integer),
        Expr::Literal(Literal::StrLit(s)) =>
            Some(ParameterValue::String(s.clone())),
        Expr::Prefix(PrefixOp::Neg, inner) => match eval_default_expr(inner)? {
            ParameterValue::Real(v)    => Some(ParameterValue::Real(-v)),
            ParameterValue::Integer(v) => Some(ParameterValue::Integer(-v)),
            _ => None,
        },
        _ => None,
    }
}

// ── Auto-save OP parameters ──────────────────────────────────────────────────

fn collect_op_saves(initial: &ast::Stmt, always: &AlwaysHandlerSet, devices: &[(String, String)]) -> Vec<String> {
    let mut saves = std::collections::HashSet::new();

    fn walk_stmt(stmt: &ast::Stmt, saves: &mut std::collections::HashSet<String>, devices: &[(String, String)]) {
        match stmt {
            ast::Stmt::Block(b) => {
                for item in &b.items {
                    match item {
                        ast::BlockItem::Stmt(s) => walk_stmt(s, saves, devices),
                        ast::BlockItem::VarDecl(v) => {
                            for var in &v.vars {
                                if let Some(e) = &var.default { walk_expr(e, saves, devices); }
                            }
                        }
                        ast::BlockItem::ParamDecl(p) => {
                            for param in &p.params {
                                walk_expr(&param.default, saves, devices);
                            }
                        }
                    }
                }
            }
            ast::Stmt::Expr(e) => walk_expr(&e.expr, saves, devices),
            ast::Stmt::Assign(a) => {
                walk_expr(&a.assign.rval, saves, devices);
                walk_expr(&a.assign.lval, saves, devices);
            }
            ast::Stmt::If(i) => {
                walk_expr(&i.condition, saves, devices);
                walk_stmt(&i.then_branch, saves, devices);
                if let Some(e) = &i.else_branch {
                    walk_stmt(e, saves, devices);
                }
            }
            ast::Stmt::While(w) => {
                walk_expr(&w.condition, saves, devices);
                walk_stmt(&w.body, saves, devices);
            }
            ast::Stmt::For(f) => {
                walk_stmt(&f.init, saves, devices);
                walk_expr(&f.condition, saves, devices);
                walk_stmt(&f.incr, saves, devices);
                walk_stmt(&f.for_body, saves, devices);
            }
            ast::Stmt::Repeat(r) => {
                walk_expr(&r.count, saves, devices);
                walk_stmt(&r.body, saves, devices);
            }
            ast::Stmt::Forever(f) => walk_stmt(&f.body, saves, devices),
            ast::Stmt::Foreach(f) => {
                walk_expr(&f.array, saves, devices);
                walk_stmt(&f.body, saves, devices);
            }
            ast::Stmt::Return(r) => {
                if let Some(e) = &r.value {
                    walk_expr(e, saves, devices);
                }
            }
            ast::Stmt::Case(c) => {
                walk_expr(&c.discriminant, saves, devices);
                for case in &c.cases {
                    if let ast::CaseItem::Exprs(exprs) = &case.item {
                        for e in exprs { walk_expr(e, saves, devices); }
                    }
                    walk_stmt(&case.stmt, saves, devices);
                }
            }
            ast::Stmt::Assert(a) => {
                walk_expr(&a.condition, saves, devices);
                if let Some(m) = &a.message { walk_expr(m, saves, devices); }
            }
            ast::Stmt::AssertRun(a) => {
                walk_expr(&a.condition, saves, devices);
                if let Some(m) = &a.message { walk_expr(m, saves, devices); }
            }
            ast::Stmt::AssertWarn(a) => {
                walk_expr(&a.condition, saves, devices);
                if let Some(m) = &a.message { walk_expr(m, saves, devices); }
            }
            _ => {}
        }
    }

    fn walk_expr(expr: &ast::Expr, saves: &mut std::collections::HashSet<String>, devices: &[(String, String)]) {
        match expr {
            ast::Expr::Path(p) => {
                let s = path_to_string(p);
                if let Some((inst, param)) = s.split_once('.') {
                    if let Some((_, spice)) = devices.iter().find(|(piperine, _)| piperine == inst) {
                        saves.insert(format!(".save @{}[{}]", spice, param));
                    }
                }
            }
            ast::Expr::Call(ast::FunctionRef::Path(p), args) => {
                let s = path_to_string(p);
                if let Some((inst, param)) = s.split_once('.') {
                    if args.is_empty() {
                        if let Some((_, spice)) = devices.iter().find(|(piperine, _)| piperine == inst) {
                            saves.insert(format!(".save @{}[{}]", spice, param));
                        }
                    }
                }
                for arg in args {
                    match arg {
                        ast::CallArg::Positional(e) => walk_expr(e, saves, devices),
                        ast::CallArg::Named(_, e) => walk_expr(e, saves, devices),
                    }
                }
            }
            ast::Expr::Call(_, args) => {
                for arg in args {
                    match arg {
                        ast::CallArg::Positional(e) => walk_expr(e, saves, devices),
                        ast::CallArg::Named(_, e) => walk_expr(e, saves, devices),
                    }
                }
            }
            ast::Expr::Paren(e) => walk_expr(e, saves, devices),
            ast::Expr::Prefix(_, e) => walk_expr(e, saves, devices),
            ast::Expr::Binary(l, _, r) => {
                walk_expr(l, saves, devices);
                walk_expr(r, saves, devices);
            }
            ast::Expr::Select(c, t, e) => {
                walk_expr(c, saves, devices);
                walk_expr(t, saves, devices);
                walk_expr(e, saves, devices);
            }
            ast::Expr::Array(arr) => {
                for e in arr { walk_expr(e, saves, devices); }
            }
            ast::Expr::Index(b, i) => {
                walk_expr(b, saves, devices);
                walk_expr(i, saves, devices);
            }
            _ => {}
        }
    }

    walk_stmt(initial, &mut saves, devices);
    for s in &always.initial_step { walk_stmt(s, &mut saves, devices); }
    for s in &always.step { walk_stmt(s, &mut saves, devices); }
    for s in &always.final_step { walk_stmt(s, &mut saves, devices); }
    for (_, _, s) in &always.above { walk_stmt(s, &mut saves, devices); }
    for (_, _, _, s) in &always.cross { walk_stmt(s, &mut saves, devices); }

    let mut result: Vec<_> = saves.into_iter().collect();
    result.sort();
    result
}
