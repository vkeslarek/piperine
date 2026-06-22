use std::path::{Path, PathBuf};

use crate::ast;
use crate::grammar::parse_tokens;
use crate::lexer::tokenize;
use crate::model::*;
use crate::preprocessor::Preprocessor;

/// Directory of standard Verilog-AMS headers bundled with the crate.
fn bundled_headers() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("headers")
}

/// Public accessor for the bundled Verilog-AMS headers directory.
/// Callers (e.g. piperine-ngspice) add this to `parse_with_includes` dirs
/// alongside their own header directories.
pub fn bundled_header_dir() -> PathBuf {
    bundled_headers()
}

/// Parse Verilog-A source text. `` `include `` resolves only against bundled standard headers.
pub fn parse(input: &str) -> Result<Document, String> {
    parse_with_includes(input, &[bundled_headers()])
}

/// Parse a Verilog-A file from disk, resolving includes against the file's own
/// directory first, then the bundled standard headers.
pub fn parse_file(path: &Path) -> Result<Document, String> {
    let input = std::fs::read_to_string(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut dirs = Vec::new();
    if let Some(dir) = path.parent() {
        dirs.push(dir.to_path_buf());
    }
    dirs.push(bundled_headers());
    parse_with_includes(&input, &dirs)
}

/// Parse Verilog-A source with an explicit list of include search directories.
pub fn parse_with_includes(input: &str, include_dirs: &[PathBuf]) -> Result<Document, String> {
    let raw = tokenize(input)?;
    let mut pp = Preprocessor::new(include_dirs.to_vec());
    pp.define("__OPENVAF__", "1");
    pp.define("__VAMS_COMPACT_MODELING__", "1");
    let tokens = pp.run(raw)?;
    let source_file = parse_tokens(&tokens)?;

    let mut doc = Document::default();

    for item in source_file.items {
        match item {
            ast::Item::ModuleDecl(decl) => {
                doc.modules.push(convert_module(decl));
            }
            ast::Item::DisciplineDecl(decl) => {
                doc.disciplines.push(convert_discipline(decl));
            }
            ast::Item::NatureDecl(decl) => {
                doc.natures.push(convert_nature(decl));
            }
            ast::Item::ExternModule(decl) => {
                doc.extern_modules.push(decl);
            }
            ast::Item::TypedefEnum(decl) => {
                doc.typedef_enums.push(decl);
            }
            ast::Item::TypedefStruct(decl) => {
                doc.typedef_structs.push(decl);
            }
            ast::Item::ExternClass(decl) => {
                doc.extern_classes.push(decl);
            }
            ast::Item::Paramset(decl) => {
                doc.paramsets.push(decl);
            }
        }
    }

    Ok(doc)
}

fn convert_attrs(attrs: &[ast::Attr]) -> Vec<Attribute> {
    attrs
        .iter()
        .map(|a| Attribute { name: a.name.0.clone(), value: a.val.clone() })
        .collect()
}

fn convert_var_decl(v: &ast::VarDecl, out: &mut Vec<Variable>) {
    let attributes = convert_attrs(&v.attrs);
    for var in &v.vars {
        out.push(Variable {
            name: var.name.0.clone(),
            ty: v.ty.clone(),
            range: var.range.clone(),
            default_value: var.default.clone(),
            attributes: attributes.clone(),
            span: v.span.clone(),
        });
    }
}

fn convert_param_decl(p: &ast::ParamDecl, out: &mut Vec<Parameter>) {
    let attributes = convert_attrs(&p.attrs);
    for param in &p.params {
        out.push(Parameter {
            name: param.name.0.clone(),
            is_local: matches!(p.kind, ast::ParamKind::LocalParam),
            ty: p.ty.clone(),
            default_value: param.default.clone(),
            constraints: param.constraints.clone(),
            attributes: attributes.clone(),
            span: p.span.clone(),
        });
    }
}

fn convert_module(decl: ast::ModuleDecl) -> Module {
    let mut module = Module {
        name: decl.name.0.clone(),
        attributes: convert_attrs(&decl.attrs),
        ports: Vec::new(),
        parameters: Vec::new(),
        aliasparams: Vec::new(),
        nets: Vec::new(),
        variables: Vec::new(),
        branches: Vec::new(),
        instances: Vec::new(),
        functions: Vec::new(),
        analog_blocks: Vec::new(),
        initial_blocks: Vec::new(),
        always_blocks: Vec::new(),
        span: decl.span,
    };

    if let Some(ports) = decl.ports {
        for port in ports {
            match port {
                ast::ModulePort::PortDecl(p) => {
                    let attributes = convert_attrs(&p.attrs);
                    for d in &p.names {
                        module.ports.push(Port {
                            name: d.name.0.clone(),
                            direction: p.dir.clone(),
                            discipline: p.discipline.as_ref().map(|d| d.0.clone()),
                            range: d.range.clone().or_else(|| p.range.clone()),
                            attributes: attributes.clone(),
                            span: p.span.clone(),
                        });
                    }
                }
                ast::ModulePort::Name(name) => {
                    module.ports.push(Port {
                        name: name.0.clone(),
                        direction: ast::Direction::Inout,
                        discipline: None,
                        range: None,
                        attributes: Vec::new(),
                        span: Span { start: 0, end: 0 },
                    });
                }
            }
        }
    }

    for item in decl.items {
        match item {
            ast::ModuleItem::BodyPortDecl(b) => {
                // Body port declarations (e.g. `inout electrical anode, cathode;`) update
                // the direction/discipline of ports already listed in the header. If a port
                // wasn't in the header (non-ANSI style), insert it fresh.
                let attributes = convert_attrs(&b.port.attrs);
                for d in &b.port.names {
                    let existing = module.ports.iter_mut().find(|p| p.name == d.name.0);
                    match existing {
                        Some(p) => {
                            p.direction = b.port.dir.clone();
                            p.discipline = b.port.discipline.as_ref().map(|d| d.0.clone());
                            p.range = d.range.clone().or_else(|| b.port.range.clone()).or(p.range.clone());
                            p.attributes = attributes.clone();
                            p.span = b.port.span.clone();
                        }
                        None => {
                            module.ports.push(Port {
                                name: d.name.0.clone(),
                                direction: b.port.dir.clone(),
                                discipline: b.port.discipline.as_ref().map(|d| d.0.clone()),
                                range: d.range.clone().or_else(|| b.port.range.clone()),
                                attributes: attributes.clone(),
                                span: b.port.span.clone(),
                            });
                        }
                    }
                }
            }
            ast::ModuleItem::NetDecl(n) => {
                let members = n
                    .names
                    .iter()
                    .map(|d| NetMember {
                        name: d.name.0.clone(),
                        range: d.range.clone().or_else(|| n.range.clone()),
                    })
                    .collect();
                module.nets.push(Net {
                    members,
                    discipline: n.discipline.as_ref().map(|d| d.0.clone()),
                    attributes: convert_attrs(&n.attrs),
                    span: n.span,
                });
            }
            ast::ModuleItem::AnalogBehaviour(a) => {
                module.analog_blocks.push(AnalogBlock {
                    is_initial: a.initial,
                    stmt: *a.stmt,
                    attributes: convert_attrs(&a.attrs),
                    span: a.span,
                });
            }
            ast::ModuleItem::Function(f) => {
                let mut func = Function {
                    name: f.name.0.clone(),
                    return_type: f.ty.clone(),
                    args: Vec::new(),
                    variables: Vec::new(),
                    parameters: Vec::new(),
                    body: Vec::new(),
                    attributes: convert_attrs(&f.attrs),
                    span: f.span,
                };
                for f_item in f.items {
                    match f_item {
                        ast::FunctionItem::FunctionArg(a) => {
                            for name in &a.names {
                                func.args.push(FunctionArg {
                                    name: name.0.clone(),
                                    direction: a.dir.clone(),
                                });
                            }
                        }
                        ast::FunctionItem::VarDecl(v) => {
                            convert_var_decl(&v, &mut func.variables);
                        }
                        ast::FunctionItem::ParamDecl(p) => {
                            convert_param_decl(&p, &mut func.parameters);
                        }
                        ast::FunctionItem::Stmt(s) => {
                            func.body.push(s);
                        }
                    }
                }
                module.functions.push(func);
            }
            ast::ModuleItem::BranchDecl(b) => {
                let names = b.names.iter().map(|n| n.0.clone()).collect();
                module.branches.push(Branch {
                    names,
                    ports: b.ports.clone(),
                    attributes: convert_attrs(&b.attrs),
                    span: b.span,
                });
            }
            ast::ModuleItem::VarDecl(v) => {
                convert_var_decl(&v, &mut module.variables);
            }
            ast::ModuleItem::ParamDecl(p) => {
                convert_param_decl(&p, &mut module.parameters);
            }
            ast::ModuleItem::AliasParam(a) => {
                let source = match a.src {
                    ast::ParamRef::Path(p) => ParamSource::Path(path_to_string(&p)),
                    ast::ParamRef::SysFun(s) => ParamSource::SysFun(s),
                };
                module.aliasparams.push(AliasParam {
                    name: a.name.0.clone(),
                    source,
                    attributes: convert_attrs(&a.attrs),
                    span: a.span,
                });
            }
            ast::ModuleItem::Instance(i) => {
                let connections = i.connections.into_iter().map(convert_connection).collect();
                let params = i.params.into_iter().map(convert_connection).collect();
                module.instances.push(Instance {
                    module: i.module.0.clone(),
                    name: i.name.0.clone(),
                    range: i.range.clone(),
                    params,
                    connections,
                    attributes: convert_attrs(&i.attrs),
                    span: i.span,
                });
            }
            ast::ModuleItem::InitialBlock(b) => {
                module.initial_blocks.push(crate::model::InitialBlock {
                    stmt: *b.stmt,
                    span: b.span,
                });
            }
            ast::ModuleItem::AlwaysBlock(ab) => {
                module.always_blocks.push(ab);
            }
        }
    }

    module
}

fn convert_connection(c: ast::Connection) -> Connection {
    match c {
        ast::Connection::Positional(e) => Connection::Positional(e),
        ast::Connection::Named { port, expr } => Connection::Named { port: port.0, expr },
    }
}

fn convert_discipline(decl: ast::DisciplineDecl) -> Discipline {
    let attributes = decl
        .items
        .into_iter()
        .map(|attr| DisciplineAttr {
            name: path_to_string(&attr.name),
            value: attr.val,
        })
        .collect();

    Discipline {
        name: decl.name.0,
        attributes,
        span: decl.span,
    }
}

fn convert_nature(decl: ast::NatureDecl) -> Nature {
    let attributes = decl
        .items
        .into_iter()
        .map(|attr| NatureAttr {
            name: attr.name.0,
            value: attr.val,
        })
        .collect();

    Nature {
        name: decl.name.0,
        parent: decl.parent.map(|p| path_to_string(&p)),
        attributes,
        span: decl.span,
    }
}

fn path_to_string(path: &ast::Path) -> String {
    let mut parts = Vec::new();
    let mut current = path;
    loop {
        match &current.segment {
            ast::PathSegment::Ident(i) => parts.push(i.clone()),
            ast::PathSegment::Root => parts.push("root".to_string()),
        }
        if let Some(qual) = &current.qualifier {
            current = qual;
        } else {
            break;
        }
    }
    parts.reverse();
    parts.join(".")
}
