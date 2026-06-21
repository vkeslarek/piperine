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
    elaborate_instances(
        &testbench.instances, document, registry, &paramsets, "", &net_map, &mut spice_lines,
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

    Ok(ElaborationResult { spice_lines, initial_statement, always_handlers })
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
) -> Result<(), ElaborationError> {
    for instance in instances {
        elaborate_instance(instance, instances, document, registry, paramsets, path, net_map, spice_lines)?;
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

        // Extract model name (required in preset).
        let model_name = ps_info.preset.get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ElaborationError::MissingParameter {
                parameter: "model".into(),
                instance: instance.name.clone(),
            })?
            .to_string();

        // Emit .model card before the instance.
        if let Some(spice_type) = ps_info.model_type {
            let model_params: String = merged.iter()
                .filter(|(k, _)| *k != "model")
                .map(|(k, v)| format!("{}={}", k, v.to_spice_string()))
                .collect::<Vec<_>>()
                .join(" ");
            if model_params.is_empty() {
                spice_lines.push(format!(".model {} {}", model_name, spice_type));
            } else {
                spice_lines.push(format!(".model {} {} ({})", model_name, spice_type, model_params));
            }
        }

        // Instantiate via base module with model name in param map.
        merged.insert("model".into(), ParameterValue::String(format!("\"{}\"", model_name)));
        let resolver = ConcreteNetResolver { net_map, path };
        let hw_instance = base_def.instantiate(&instance.name, &merged, &connections, &resolver)?;
        spice_lines.extend(hw_instance.spice_lines());

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
        let resolver = ConcreteNetResolver { net_map, path };
        let hw_instance = definition.instantiate(&instance.name, &parameters, &connections, &resolver)?;
        spice_lines.extend(hw_instance.spice_lines());
    } else if let Some(sub_mod) = find_structural_module(document, &instance.module) {
        // ── Piperine sub-module: flatten inline with path-prefixed net names ──
        let sub_path = build_path(path, &instance.name);
        let sub_net_map = build_sub_net_map(sub_mod, &connections, &sub_path);
        elaborate_instances(
            &sub_mod.instances, document, registry, paramsets, &sub_path, &sub_net_map, spice_lines,
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
}

impl<'a> NetResolver for ConcreteNetResolver<'a> {
    fn resolve(&self, raw: &str) -> String {
        resolve_net(raw, self.net_map, self.path)
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
        let spice_ename = match hw_def.and_then(|d| d.spice_instance_prefix()) {
            Some(prefix) => {
                let up = ident.chars().next().map(|c| c.to_ascii_uppercase());
                if up == Some(prefix.to_ascii_uppercase()) {
                    ident.clone()
                } else {
                    format!("{prefix}{ident}")
                }
            }
            None => ident.clone(),
        };
        out.insert(def.name.clone(), ParameterValue::String(format!("\"{spice_ename}\"")));
    }
    Ok(())
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

pub struct VaModuleInfo {
    pub module_name: String,
    pub port_names: Vec<String>,
    /// (parameter_name, default_expr)
    pub parameter_defaults: Vec<(String, piperine_parser::ast::Expr)>,
}

/// Find all VA modules (analog block present, no initial block).
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
