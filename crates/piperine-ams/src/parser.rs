use std::path::{Path, PathBuf};

use crate::ast;
use crate::grammar::Parser as GrammarParser;
use crate::lexer::Lexer;
use crate::model::*;
use crate::preprocessor::Preprocessor;

impl Document {
    fn bundled_headers() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("headers")
    }

    pub fn bundled_header_dir() -> PathBuf {
        Self::bundled_headers()
    }

    pub fn parse(input: &str) -> Result<Document, String> {
        Self::parse_with_includes(input, &[Self::bundled_headers()])
    }

    pub fn parse_file(path: &Path) -> Result<Document, String> {
        let input = std::fs::read_to_string(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let mut dirs = Vec::new();
        if let Some(dir) = path.parent() {
            dirs.push(dir.to_path_buf());
        }
        dirs.push(Self::bundled_headers());
        Self::parse_with_includes(&input, &dirs)
    }

    pub fn parse_with_includes(input: &str, include_dirs: &[PathBuf]) -> Result<Document, String> {
        let raw = Lexer::tokenize(input)?;
        let mut pp = Preprocessor::new(include_dirs.to_vec());
        pp.define("__OPENVAF__", "1");
        pp.define("__VAMS_COMPACT_MODELING__", "1");
        let tokens = pp.run(raw)?;
        let source_file = GrammarParser::parse(&tokens)?;

    let mut doc = Document::default();

    for item in source_file.items {
        match item {
            ast::Item::ModuleDecl(decl) => {
                doc.modules.push(Self::convert_module(decl));
            }
            ast::Item::DisciplineDecl(decl) => {
                doc.disciplines.push(Self::convert_discipline(decl));
            }
            ast::Item::NatureDecl(decl) => {
                doc.natures.push(Self::convert_nature(decl));
            }
            ast::Item::Paramset(p) => {
                let mut paramset = Paramset {
                    name: p.name.0.clone(),
                    base: p.base.0.clone(),
                    parameters: Vec::new(),
                    aliasparams: Vec::new(),
                    variables: Vec::new(),
                    statements: p.statements,
                    attributes: Self::convert_attrs(&p.attrs),
                    span: p.span,
                };
                for decl in p.item_decls {
                    match decl {
                        ast::ParamsetItemDecl::Parameter(param) | ast::ParamsetItemDecl::LocalParameter(param) => {
                            Self::convert_param_decl(&param, &mut paramset.parameters);
                        }
                        ast::ParamsetItemDecl::AliasParam(alias) => {
                            let source = match alias.src {
                                ast::ParamRef::Path(path) => ParamSource::Path(Self::path_to_string(&path)),
                                ast::ParamRef::SysFun(s) => ParamSource::SysFun(s),
                            };
                            paramset.aliasparams.push(AliasParam {
                                name: alias.name.0.clone(),
                                source,
                                attributes: Self::convert_attrs(&alias.attrs),
                                span: alias.span,
                            });
                        }
                        ast::ParamsetItemDecl::IntegerDecl(var) | ast::ParamsetItemDecl::RealDecl(var) => {
                            Self::convert_var_decl(&var, &mut paramset.variables);
                        }
                    }
                }
                doc.paramsets.push(paramset);
            }
            ast::Item::Connectrules(c) => {
                doc.connectrules.push(c);
            }
            ast::Item::Config(c) => {
                doc.configs.push(c);
            }
            ast::Item::Primitive(p) => {
                doc.primitives.push(p);
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
        let attributes = Self::convert_attrs(&v.attrs);
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
        let attributes = Self::convert_attrs(&p.attrs);
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
            attributes: Self::convert_attrs(&decl.attrs),
        ports: Vec::new(),
        parameters: Vec::new(),
        aliasparams: Vec::new(),
        nets: Vec::new(),
        variables: Vec::new(),
        branches: Vec::new(),
        functions: Vec::new(),
        tasks: Vec::new(),
        analog_blocks: Vec::new(),
        instances: Vec::new(),
        ground_decls: Vec::new(),
        events: Vec::new(),
        defparams: Vec::new(),
        continuous_assigns: Vec::new(),
        span: decl.span,
    };

        if let Some(ports) = decl.ports {
            for port in ports {
                match port {
                    ast::ModulePort::PortDecl(p) => {
                        let attributes = Self::convert_attrs(&p.attrs);
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
                    ast::ModulePort::NamedExternal { port, .. } => {
                        module.ports.push(Port {
                            name: port.0.clone(),
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
                ast::ModuleItem::PortDecl(b) => {
                    let attributes = Self::convert_attrs(&b.attrs);
                    for d in &b.names {
                        let existing = module.ports.iter_mut().find(|p| p.name == d.name.0);
                        match existing {
                            Some(p) => {
                                p.direction = b.dir.clone();
                                p.discipline = b.discipline.as_ref().map(|d| d.0.clone());
                                p.range = d.range.clone().or_else(|| b.range.clone()).or(p.range.clone());
                                p.attributes = attributes.clone();
                                p.span = b.span.clone();
                            }
                            None => {
                                module.ports.push(Port {
                                    name: d.name.0.clone(),
                                    direction: b.dir.clone(),
                                    discipline: b.discipline.as_ref().map(|d| d.0.clone()),
                                    range: d.range.clone().or_else(|| b.range.clone()),
                                    attributes: attributes.clone(),
                                    span: b.span.clone(),
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
                        ty: n.ty.clone(),
                        attributes: Self::convert_attrs(&n.attrs),
                        span: n.span,
                    });
                }
                ast::ModuleItem::AnalogBehaviour(a) => {
                    module.analog_blocks.push(AnalogBlock {
                        is_initial: a.initial,
                        stmt: *a.stmt,
                        attributes: Self::convert_attrs(&a.attrs),
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
                        attributes: Self::convert_attrs(&f.attrs),
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
                                Self::convert_var_decl(&v, &mut func.variables);
                            }
                            ast::FunctionItem::ParamDecl(p) => {
                                Self::convert_param_decl(&p, &mut func.parameters);
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
                        attributes: Self::convert_attrs(&b.attrs),
                        span: b.span,
                    });
                }
                ast::ModuleItem::VarDecl(v) => {
                    Self::convert_var_decl(&v, &mut module.variables);
                }
                ast::ModuleItem::ParamDecl(p) => {
                    Self::convert_param_decl(&p, &mut module.parameters);
                }
                ast::ModuleItem::AliasParam(a) => {
                    let source = match a.src {
                        ast::ParamRef::Path(p) => ParamSource::Path(Self::path_to_string(&p)),
                        ast::ParamRef::SysFun(s) => ParamSource::SysFun(s),
                    };
                    module.aliasparams.push(AliasParam {
                        name: a.name.0.clone(),
                        source,
                        attributes: Self::convert_attrs(&a.attrs),
                        span: a.span,
                    });
                }
                ast::ModuleItem::GroundDecl(g) => {
                    module.ground_decls.push(g);
                }
                ast::ModuleItem::EventDecl(e) => {
                    module.events.push(e);
                }
                ast::ModuleItem::ModuleInstantiation(m) => {
                    let attributes = Self::convert_attrs(&m.attrs);
                    for inst in m.instances {
                        module.instances.push(Instance {
                            module_name: m.module_name.0.clone(),
                            instance_name: inst.name.0.clone(),
                            range: inst.range,
                            param_assignments: m.param_assignments.clone(),
                            connections: inst.connections,
                            attributes: attributes.clone(),
                        });
                    }
                }
                ast::ModuleItem::Defparam(d) => {
                    module.defparams.push(d);
                }
                ast::ModuleItem::ContinuousAssign(c) => {
                    module.continuous_assigns.push(c);
                }
                ast::ModuleItem::TaskDecl(t) => {
                    let mut task = Task {
                        name: t.name.0.clone(),
                        automatic: t.automatic,
                        ports: t.ports,
                        variables: Vec::new(),
                        body: *t.body,
                        attributes: Self::convert_attrs(&t.attrs),
                        span: t.span,
                    };
                    for t_item in t.items {
                        match t_item {
                            ast::TaskItem::BlockItem(ast::BlockItem::VarDecl(v)) => {
                                Self::convert_var_decl(&v, &mut task.variables);
                            }
                            ast::TaskItem::Port(p) => {
                                task.ports.push(p);
                            }
                            _ => {}
                        }
                    }
                    module.tasks.push(task);
                }
                ast::ModuleItem::InitialConstruct { .. } |
                ast::ModuleItem::AlwaysConstruct { .. } | ast::ModuleItem::Generate(_) |
                ast::ModuleItem::LoopGenerate(_) | ast::ModuleItem::IfGenerate(_) |
                ast::ModuleItem::CaseGenerate(_) | ast::ModuleItem::Specify(_) |
                ast::ModuleItem::Specparam(_) | ast::ModuleItem::GateInstantiation(_) => {}
            }
        }

        module
    }

    fn convert_discipline(decl: ast::DisciplineDecl) -> Discipline {
        let attributes = decl
            .items
            .into_iter()
            .map(|attr| DisciplineAttr {
                name: Self::path_to_string(&attr.name),
                value: attr.val,
            })
            .collect();
        Discipline { name: decl.name.0, attributes, span: decl.span }
    }

    fn convert_nature(decl: ast::NatureDecl) -> Nature {
        let attributes = decl
            .items
            .into_iter()
            .map(|attr| NatureAttr { name: attr.name.0, value: attr.val })
            .collect();
        Nature {
            name: decl.name.0,
            parent: decl.parent.map(|p| Self::path_to_string(&p)),
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
}
