use std::collections::HashMap;

use cvaf::ast::{self, Expr, Literal, PathSegment, PrefixOp};
use cvaf::model::{Document, Module};
use crate::error::ElaborationError;
use crate::registry::HardwareRegistry;
use crate::types::{ParameterValue, ParameterMap, ConnectionMap, parse_si_real};

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

    // Top-level net map is empty: nets resolve to themselves.
    let net_map = NetMap::new();
    elaborate_instances(
        &testbench.instances, document, registry, "", &net_map, &mut spice_lines,
    )?;

    let initial_statement = testbench
        .initial_blocks
        .first()
        .ok_or(ElaborationError::NoTestbench)?
        .stmt
        .clone();

    Ok(ElaborationResult { spice_lines, initial_statement })
}

fn elaborate_instances(
    instances: &[cvaf::model::Instance],
    document: &Document,
    registry: &HardwareRegistry,
    path: &str,
    net_map: &NetMap,
    spice_lines: &mut Vec<String>,
) -> Result<(), ElaborationError> {
    for instance in instances {
        elaborate_instance(instance, document, registry, path, net_map, spice_lines)?;
    }
    Ok(())
}

fn elaborate_instance(
    instance: &cvaf::model::Instance,
    document: &Document,
    registry: &HardwareRegistry,
    path: &str,
    net_map: &NetMap,
    spice_lines: &mut Vec<String>,
) -> Result<(), ElaborationError> {
    // Resolve port connections to flat SPICE net names using the current net_map.
    let connections = resolve_connections(
        &instance.connections, &instance.name, path, net_map,
    )?;

    if let Some(definition) = registry.get(&instance.module) {
        // ── Leaf hardware (extern module, OSDI device, SPICE primitive) ──────
        let parameters = resolve_parameters(
            &instance.params, &instance.name, definition.parameters(),
        )?;
        let hw_instance = definition.instantiate(&instance.name, &parameters, &connections)?;
        spice_lines.extend(hw_instance.spice_lines());
    } else if let Some(sub_mod) = find_structural_module(document, &instance.module) {
        // ── Piperine sub-module: flatten inline with path-prefixed net names ──
        let sub_path = build_path(path, &instance.name);
        let sub_net_map = build_sub_net_map(sub_mod, &connections, &sub_path);
        elaborate_instances(
            &sub_mod.instances, document, registry, &sub_path, &sub_net_map, spice_lines,
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
    source_connections: &[cvaf::model::Connection],
    instance_name: &str,
    path: &str,
    net_map: &NetMap,
) -> Result<ConnectionMap, ElaborationError> {
    let mut map = ConnectionMap::new();
    for connection in source_connections {
        match connection {
            cvaf::model::Connection::Named { port, expr } => {
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
            cvaf::model::Connection::Positional(_) => {
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
    source_connections: &[cvaf::model::Connection],
    instance_name: &str,
    definitions: &[crate::hardware::ParameterDefinition],
) -> Result<ParameterMap, ElaborationError> {
    let mut map: ParameterMap = definitions
        .iter()
        .filter_map(|d| d.default.as_ref().map(|v| (d.name.clone(), v.clone())))
        .collect();

    for connection in source_connections {
        match connection {
            cvaf::model::Connection::Named { port, expr } => {
                if let Some(expr) = expr {
                    let value = ast_expr_to_parameter_value(expr, port, instance_name)?;
                    map.insert(port.clone(), value);
                }
            }
            cvaf::model::Connection::Positional(_) => {
                return Err(ElaborationError::TypeError {
                    parameter: "<positional>".into(),
                    detail: "positional parameter overrides not supported; use named syntax: #(.r(1k))".into(),
                });
            }
        }
    }

    for definition in definitions {
        if definition.default.is_none() && !map.contains_key(&definition.name) {
            return Err(ElaborationError::MissingParameter {
                parameter: definition.name.clone(),
                instance: instance_name.to_string(),
            });
        }
    }

    Ok(map)
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
    pub parameter_defaults: Vec<(String, cvaf::ast::Expr)>,
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
