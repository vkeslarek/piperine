//! Scope: the MSL parser (`piperine_lang::msl::parser`) building spanned AST
//! nodes (requirement MSLP-02, MSLP-06). Types, paths, generics and patterns
//! (T5), extended with expressions/statements (T6), items (T7) and the
//! language's own productions (T8-T10).
//! Fixtures are minimal sources; assertions name the AST node and, where the
//! spec says a node's span covers its text, the exact byte range.

use piperine_lang::msl::Span;
use piperine_lang::msl::ast::*;
use piperine_lang::msl::parser::{ExpressionGrammar, SourceParser, GenericsGrammar, PatternGrammar, TypeGrammar};

/// Associated test helpers of `parser`, grouped on one fixture type.
struct ParserFixture;

/// Fixtures of the test `count_stmts`, hoisted out of its body.
struct CountStatementsFixture;

impl CountStatementsFixture {
    fn expr(e: &Expr, acc: &mut (usize, usize)) {
        match &e.kind {
            ExprKind::Block(b) => CountStatementsFixture::blk(b, acc),
            ExprKind::If { then, else_, .. } => {
                CountStatementsFixture::blk(then, acc);
                if let Some(e) = else_ {
                    CountStatementsFixture::expr(e, acc);
                }
            }
            ExprKind::Match { arms, .. } => arms.iter().for_each(|a| CountStatementsFixture::expr(&a.body, acc)),
            ExprKind::For { body, .. } | ExprKind::While { body, .. } | ExprKind::Loop { body } => CountStatementsFixture::blk(body, acc),
            _ => {}
        }
    }

    fn blk(b: &Block, acc: &mut (usize, usize)) {
        for s in &b.stmts {
            match &s.kind {
                StmtKind::Contribution { .. } => acc.0 += 1,
                StmtKind::Commit { .. } => acc.1 += 1,
                StmtKind::Expr { expr: e, .. } => CountStatementsFixture::expr(e, acc),
                _ => {}
            }
        }
    }
}

impl ParserFixture {
    fn ty(src: &str) -> Type {
        let (t, diags) = TypeGrammar::parse(src);
        assert!(diags.is_empty(), "{src:?}: {diags:?}");
        t.unwrap_or_else(|| panic!("{src:?} did not parse"))
    }

    fn pat(src: &str) -> Pat {
        let (p, diags) = PatternGrammar::parse(src);
        assert!(diags.is_empty(), "{src:?}: {diags:?}");
        p.unwrap_or_else(|| panic!("{src:?} did not parse"))
    }

    fn expr(src: &str) -> Expr {
        let (e, diags) = ExpressionGrammar::parse(src);
        assert!(diags.is_empty(), "{src:?}: {diags:?}");
        e.unwrap_or_else(|| panic!("{src:?} did not parse"))
    }

    fn block(src: &str) -> Block {
        let (b, diags) = ExpressionGrammar::parse_block(src);
        assert!(diags.is_empty(), "{src:?}: {diags:?}");
        b.unwrap_or_else(|| panic!("{src:?} did not parse"))
    }

    /// Parse a whole file, expecting no diagnostics.
    fn file(src: &str) -> File {
        let r = SourceParser::parse(src);
        assert!(r.diagnostics.is_empty(), "{src:?}: {:?}", r.diagnostics);
        r.file
    }

    fn only_item(src: &str) -> Item {
        let mut f = ParserFixture::file(src);
        assert_eq!(f.items.len(), 1, "{src:?}");
        f.items.remove(0)
    }

    fn path_names(p: &Path) -> Vec<&str> {
        p.segments.iter().map(|s| s.ident.name.as_str()).collect()
    }

    /// The type is a path whose segment names are `names`.
    fn assert_path_type(t: &Type, names: &[&str]) {
        match &t.kind {
            TypeKind::Path(p) => assert_eq!(ParserFixture::path_names(p), names),
            other => panic!("expected a path type, got {other:?}"),
        }
    }

    fn angle_args(t: &Type) -> &[GenericArg] {
        let TypeKind::Path(p) = &t.kind else { panic!("not a path type: {t:?}") };
        match p.segments.last().and_then(|s| s.args.as_ref()) {
            Some(GenericArgs::Angle { args, .. }) => args,
            other => panic!("expected angle args, got {other:?}"),
        }
    }

    /// The expression is `lhs op rhs`; returns both sides.
    fn binary(e: &Expr, op: BinOp) -> (&Expr, &Expr) {
        match &e.kind {
            ExprKind::Binary { op: o, lhs, rhs } if *o == op => (lhs, rhs),
            other => panic!("expected {op:?}, got {other:?}"),
        }
    }

    fn is_path(e: &Expr, name: &str) -> bool {
        matches!(&e.kind, ExprKind::Path(p) if ParserFixture::path_names(p) == [name])
    }

    fn is_int(e: &Expr, text: &str) -> bool {
        matches!(&e.kind, ExprKind::Lit(Lit { kind: LitKind::Int(t), .. }) if t == text)
    }

    /// Counts of (contributions, commits) in a block, at any nesting.
    fn count_stmts(b: &Block) -> (usize, usize) {
        let mut acc = (0, 0);
        CountStatementsFixture::blk(b, &mut acc);
        acc
    }

    // ---- Appendix B.3 productions (T8 onwards) ----------------------------------------------------

    /// The source text a node's span covers.
    fn text<'s>(src: &'s str, span: &Loc) -> &'s str {
        &src[span.0.range()]
    }

    /// A module item, expecting no diagnostics.
    fn module_item(src: &str) -> ModuleItem {
        let (it, diags) = piperine_lang::msl::parser::ModuleGrammar::parse(src);
        assert!(diags.is_empty(), "{src:?}: {diags:?}");
        it.unwrap_or_else(|| panic!("{src:?} did not parse"))
    }

    fn natures_signals_and_sigtypes() {
        // ---- nature_decl, nature_field (B.3.2, Part II §4) ----
        let src = "nature Electrical { potential: Voltage { access: v, units: \"V\" }, flow: Current { access: i }, }";
        let item = ParserFixture::only_item(src);
        assert_eq!(ParserFixture::text(src, &item.span), src);
        let ItemKind::Nature(n) = &item.kind else { panic!("{:?}", item.kind) };
        assert_eq!(n.name.name, "Electrical");
        assert_eq!(n.fields.len(), 2);
        let NatureFieldKind::Quantity { which, name, attrs } = &n.fields[0].kind else { panic!() };
        assert_eq!((*which, name.name.as_str()), (Quantity::Potential, "Voltage"));
        assert_eq!(attrs.iter().map(|a| a.name.name.as_str()).collect::<Vec<_>>(), ["access", "units"]);
        assert_eq!(ParserFixture::text(src, &n.fields[0].span), "potential: Voltage { access: v, units: \"V\" }");
        let NatureFieldKind::Quantity { which, attrs, .. } = &n.fields[1].kind else { panic!() };
        assert_eq!((*which, attrs.len()), (Quantity::Flow, 1));

        // Discrete natures: `value`; generics; no trailing comma.
        let src = "nature Bit<T: Repr> { value: T }";
        let item = ParserFixture::only_item(src);
        let ItemKind::Nature(n) = &item.kind else { panic!() };
        assert!(n.generics.is_some());
        assert!(matches!(n.fields[0].kind, NatureFieldKind::Value(_)));
        assert_eq!(ParserFixture::text(src, &n.fields[0].span), "value: T");

        // `pub` and attributes in front (B.3.5 amendment 1).
        let item = ParserFixture::only_item("#[doc = \"x\"] pub nature N { value: bool }");
        assert!(item.public);
        assert_eq!(item.meta.attrs.len(), 1);

        // ---- sigtype (Part II §7): `flip`, arrays of sigtypes ----
        let t = ParserFixture::ty("flip Stream<T>");
        let TypeKind::Flip(inner) = &t.kind else { panic!("{t:?}") };
        ParserFixture::assert_path_type(inner, &["Stream"]);
        let t = ParserFixture::ty("[flip Stream<T>; N]");
        let TypeKind::Array { elem, len } = &t.kind else { panic!("{t:?}") };
        assert!(matches!(elem.kind, TypeKind::Flip(_)));
        assert!(matches!(len.kind, ExprKind::Path(_)));
        let t = ParserFixture::ty("[[Electrical; 2]; 3]");
        assert!(matches!(&t.kind, TypeKind::Array { elem, .. } if matches!(elem.kind, TypeKind::Array { .. })));
        ParserFixture::assert_path_type(&ParserFixture::ty("Option<Electrical>"), &["Option"]);

        // ---- struct_field: carrier fields make a bundle; `dir` per field ----
        let src = "struct Stream<T> { in data: T, out ready: Bit, pub valid: Bit }";
        let item = ParserFixture::only_item(src);
        let ItemKind::Struct(s) = &item.kind else { panic!() };
        let Fields::Named(fields) = &s.fields else { panic!() };
        assert_eq!(fields.iter().map(|f| f.dir).collect::<Vec<_>>(), [Some(Dir::In), Some(Dir::Out), None]);
        assert!(fields[2].public);
        assert_eq!(ParserFixture::text(src, &fields[0].span), "in data: T");
        let src = "struct Pair { a: flip Stream<T>, b: [Electrical; 2] }";
        let ItemKind::Struct(s) = ParserFixture::only_item(src).kind else { panic!() };
        let Fields::Named(fields) = &s.fields else { panic!() };
        assert!(matches!(fields[0].ty.kind, TypeKind::Flip(_)));
        assert!(matches!(fields[1].ty.kind, TypeKind::Array { .. }));

        // ---- signal_decl ----
        let src = "signal x: Bit<u8> = 0;";
        let it = ParserFixture::module_item(src);
        assert_eq!(ParserFixture::text(src, &it.span), src);
        let ModuleItemKind::Signal { name, ty, init } = &it.kind else { panic!("{:?}", it.kind) };
        assert_eq!(name.name, "x");
        ParserFixture::assert_path_type(ty, &["Bit"]);
        assert!(init.is_some());
        let it = ParserFixture::module_item("signal rx: flip Stream<T>;");
        let ModuleItemKind::Signal { ty, init, .. } = &it.kind else { panic!() };
        assert!(matches!(ty.kind, TypeKind::Flip(_)) && init.is_none());
        let it = ParserFixture::module_item("/// doc\n#[a] pub signal s: [Bit; 4];");
        assert!(it.public);
        assert_eq!((it.meta.docs.len(), it.meta.attrs.len()), (1, 1));
    }

    fn modules_ports_items_and_generation() {
        // ---- module_decl, port, dir, param ----
        let src = "module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; tc1: f64 = 0.0; }";
        let item = ParserFixture::only_item(src);
        assert_eq!(ParserFixture::text(src, &item.span), src);
        let ItemKind::Module(m) = &item.kind else { panic!("{:?}", item.kind) };
        assert_eq!(m.name.name, "Resistor");
        assert_eq!(m.ports.len(), 2);
        assert_eq!(ParserFixture::text(src, &m.ports[0].span), "p: Electrical");
        assert!(m.abi.is_none() && m.ret.is_none() && m.where_.is_none() && m.result.is_none());
        assert_eq!(m.body.len(), 2);
        let ModuleItemKind::Param { name, ty, default, where_ } = &m.body[0].kind else { panic!() };
        assert_eq!(name.name, "r");
        ParserFixture::assert_path_type(ty, &["f64"]);
        assert!(default.is_none() && where_.is_some());
        assert_eq!(ParserFixture::text(src, &m.body[0].span), "r: f64 where r > 0.0;");
        let ModuleItemKind::Param { default, where_, .. } = &m.body[1].kind else { panic!() };
        assert!(default.is_some() && where_.is_none());
        let ModuleItemKind::Param { default, where_, .. } =
            &ParserFixture::module_item("tnom: f64 = 300.15 where tnom > 0.0;").kind else { panic!() };
        assert!(default.is_some() && where_.is_some());

        // Directions, defaults, attributes, docs on ports; generics; result type;
        // `where`; `pub extern "abi" module`.
        let src = "pub extern \"osdi\" module Q<N: u32>(/// d\n#[a] in a: Bit, out b: Bit = Bit::Low, c: f64) -> Bit where N > 0 { r: f64; }";
        let item = ParserFixture::only_item(src);
        assert!(item.public);
        let ItemKind::Module(m) = &item.kind else { panic!() };
        assert_eq!(m.abi.as_ref().map(|l| l.kind.clone()), Some(LitKind::Str { text: "osdi".into(), raw_hashes: None }));
        assert!(m.generics.is_some() && m.ret.is_some() && m.where_.is_some());
        assert_eq!(m.ports.iter().map(|p| p.dir).collect::<Vec<_>>(), [Some(Dir::In), Some(Dir::Out), None]);
        assert_eq!((m.ports[0].meta.docs.len(), m.ports[0].meta.attrs.len()), (1, 1));
        assert!(m.ports[1].default.is_some() && m.ports[2].default.is_none());
        // A port may take a sigtype.
        let ItemKind::Module(m) = ParserFixture::only_item("module M(rx: flip Stream<T>, v: [Electrical; 4]) {}").kind else { panic!() };
        assert!(matches!(m.ports[0].ty.kind, TypeKind::Flip(_)));
        assert!(matches!(m.ports[1].ty.kind, TypeKind::Array { .. }));
        // An empty port list and a trailing comma.
        assert!(matches!(ParserFixture::only_item("module M() {}").kind, ItemKind::Module(_)));
        let ItemKind::Module(m) = ParserFixture::only_item("module M(a: Bit,) {}").kind else { panic!() };
        assert_eq!(m.ports.len(), 1);

        // ---- module result expression ----
        let src = "module Id(a: Bit) -> Bit { a }";
        let ItemKind::Module(m) = &ParserFixture::only_item(src).kind else { panic!() };
        assert_eq!(ParserFixture::text(src, &m.result.as_ref().unwrap().span), "a");
        let src = "module Sum(a: Bit) -> Bit { let t = a + 1; t * 2 }";
        let ItemKind::Module(m) = &ParserFixture::only_item(src).kind else { panic!() };
        assert_eq!(m.body.len(), 1);
        assert_eq!(ParserFixture::text(src, &m.result.as_ref().unwrap().span), "t * 2");

        // ---- `let`, definitions, `after`, places ----
        let src = "let x = a + b;";
        let it = ParserFixture::module_item(src);
        assert_eq!(ParserFixture::text(src, &it.span), src);
        assert!(matches!(&it.kind, ModuleItemKind::Let { pat, .. } if matches!(pat.kind, PatKind::Ident { .. })));
        let src = "y = a after 1n;";
        let it = ParserFixture::module_item(src);
        assert_eq!(ParserFixture::text(src, &it.span), src);
        let ModuleItemKind::Def { place, value, after } = &it.kind else { panic!("{:?}", it.kind) };
        assert!(matches!(place.kind, ExprKind::Path(_)) && matches!(value.kind, ExprKind::Path(_)));
        assert_eq!(ParserFixture::text(src, &after.as_ref().unwrap().span), "1n");
        let ModuleItemKind::Def { place, after, .. } = &ParserFixture::module_item("st.n = x;").kind else { panic!() };
        assert!(matches!(&place.kind, ExprKind::Field { name, .. } if name.name == "n") && after.is_none());
        let ModuleItemKind::Def { place, .. } = &ParserFixture::module_item("sh[3] = y;").kind else { panic!() };
        assert!(matches!(place.kind, ExprKind::Index { .. }));
        let ModuleItemKind::Def { place, .. } = &ParserFixture::module_item("bus[i].data = y;").kind else { panic!() };
        assert!(matches!(&place.kind, ExprKind::Field { expr, .. } if matches!(expr.kind, ExprKind::Index { .. })));

        // ---- instance: positional, named, param block, callees ----
        let ItemKind::Module(m) = ParserFixture::only_item(
            "module T(a: Electrical) { let u = Resistor(a, gnd) { r: 1k }; let v = Nch(d, g, s, b, w: 1u); }",
        )
        .kind
        else {
            panic!()
        };
        let ModuleItemKind::Let { value, .. } = &m.body[0].kind else { panic!() };
        let ExprKind::Call { callee, args, params } = &value.kind else { panic!("{:?}", value.kind) };
        assert!(matches!(callee.kind, ExprKind::Path(_)));
        assert_eq!(args.len(), 2);
        assert_eq!(params.as_ref().map(Vec::len), Some(1));
        let ModuleItemKind::Let { value, .. } = &m.body[1].kind else { panic!() };
        let ExprKind::Call { args, params, .. } = &value.kind else { panic!() };
        assert_eq!((args.len(), args[4].name.as_ref().map(|n| n.name.as_str()), params.is_none()), (5, Some("w"), true));
        // The callee is a call or a parenthesized expression (B.3.5 amendment 6).
        let ExprKind::Call { callee, args, .. } = &ParserFixture::expr("nch(0.13u, 2u)(d, g, s, b)").kind else { panic!() };
        assert!(matches!(callee.kind, ExprKind::Call { .. }) && args.len() == 4);
        let ExprKind::Call { callee, .. } = &ParserFixture::expr("(pick)(x)").kind else { panic!() };
        assert!(matches!(callee.kind, ExprKind::Paren(_)));
        // ---- template: `path generic_args? { fields }` ----
        let src = "Resistor { r: 1k, tc1: 0.1 }";
        let e = ParserFixture::expr(src);
        let ExprKind::Struct { path, fields } = &e.kind else { panic!("{:?}", e.kind) };
        assert_eq!((ParserFixture::path_names(path), fields.len()), (vec!["Resistor"], 2));
        assert_eq!(ParserFixture::text(src, &e.span), src);
        let ExprKind::Struct { path, .. } = &ParserFixture::expr("Nch::<2> { w: 1u }").kind else { panic!() };
        assert!(path.segments[0].args.is_some());

        // ---- gen<module_item>: for, if / else if / else ----
        let src = "for i in 0..N { q[i] = d[i]; r: f64; }";
        let it = ParserFixture::module_item(src);
        assert_eq!(ParserFixture::text(src, &it.span), src);
        let ModuleItemKind::Gen(g) = &it.kind else { panic!("{:?}", it.kind) };
        assert_eq!(ParserFixture::text(src, &g.span), src);
        let GenKind::For { pat, iter, body } = &g.kind else { panic!() };
        assert!(matches!(pat.kind, PatKind::Ident { .. }) && matches!(iter.kind, ExprKind::Range { .. }));
        assert_eq!(body.len(), 2);
        let src = "if A { x = 1; } else if B { x = 2; } else { x = 3; }";
        let ModuleItemKind::Gen(g) = &ParserFixture::module_item(src).kind else { panic!() };
        let GenKind::If { then, else_, .. } = &g.kind else { panic!() };
        assert_eq!(then.len(), 1);
        let GenElse::If(second) = &**else_.as_ref().unwrap() else { panic!() };
        let GenKind::If { else_, .. } = &second.kind else { panic!() };
        assert!(matches!(&**else_.as_ref().unwrap(), GenElse::Block(b) if b.len() == 1));
        // Nested generation, and `else for` (B.3.5 amendment 7 reads `else gen<X>`).
        let ModuleItemKind::Gen(g) = &ParserFixture::module_item("for i in 0..2 { for j in 0..2 { a[i][j] = 0; } }").kind else { panic!() };
        let GenKind::For { body, .. } = &g.kind else { panic!() };
        assert!(matches!(body[0].kind, ModuleItemKind::Gen(_)));
    }

    fn behavior_blocks_and_builtin_forms() {
        // ---- behavior_block: analog ----
        let src = "analog for Resistor { i(p, n) +> v(p, n) / r; }";
        let item = ParserFixture::only_item(src);
        assert_eq!(ParserFixture::text(src, &item.span), src);
        let ItemKind::Behavior(b) = &item.kind else { panic!("{:?}", item.kind) };
        assert_eq!(b.kind, BehaviorKind::Analog);
        assert!(b.names.is_empty());
        assert_eq!(ParserFixture::path_names(&b.target), ["Resistor"]);
        let BehaviorBody::Analog(stmts) = &b.body else { panic!() };
        assert_eq!(stmts.len(), 1);
        assert!(matches!(stmts[0].kind, StmtKind::Contribution { .. }));
        assert_eq!(ParserFixture::text(src, &stmts[0].span), "i(p, n) +> v(p, n) / r;");

        // `names`: the generic names of the module, and a target with arguments.
        let src = "analog<N, M> for Urc<N, M> { let k = N * M; i(a, b) +> k; }";
        let ItemKind::Behavior(b) = &ParserFixture::only_item(src).kind else { panic!() };
        assert_eq!(b.names.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["N", "M"]);
        assert!(b.target.segments[0].args.is_some());
        let BehaviorBody::Analog(stmts) = &b.body else { panic!() };
        assert_eq!(stmts.len(), 2);
        // `contribution` at any depth of an analog block (B.3.4 rule 1).
        let ItemKind::Behavior(b) =
            &ParserFixture::only_item("analog for M { if a { for k in 0..3 { match x { _ => { i(p, n) +> 1.0; } } } } }").kind
        else {
            panic!()
        };
        assert!(matches!(b.body, BehaviorBody::Analog(_)));

        // ---- behavior_block: digital, reaction, commit ----
        let src = "digital for Reg { on clk.rise() { q <- d after 1n; } }";
        let item = ParserFixture::only_item(src);
        assert_eq!(ParserFixture::text(src, &item.span), src);
        let ItemKind::Behavior(b) = &item.kind else { panic!() };
        assert_eq!(b.kind, BehaviorKind::Digital);
        let BehaviorBody::Digital(items) = &b.body else { panic!() };
        let DigitalItemKind::Reaction(r) = &items[0].kind else { panic!() };
        assert_eq!(ParserFixture::text(src, &r.event.span), "clk.rise()");
        assert_eq!(ParserFixture::text(src, &items[0].span), "on clk.rise() { q <- d after 1n; }");
        assert!(matches!(&r.body.stmts[0].kind, StmtKind::Commit { after: Some(_), .. }));
        let ItemKind::Behavior(b) = &ParserFixture::only_item("digital<W> for Pipe<W> { on a { x <- 1; } on b { y[2] <- 0; } }").kind else {
            panic!()
        };
        assert_eq!(b.names.len(), 1);
        let BehaviorBody::Digital(items) = &b.body else { panic!() };
        assert_eq!(items.len(), 2);

        // gen<digital_item>: for / if over reactions.
        let src = "digital for P { for i in 0..N { on e(i) { q[i] <- d[i]; } } if C { on a { x <- 1; } } else { on b { x <- 0; } } }";
        let ItemKind::Behavior(b) = &ParserFixture::only_item(src).kind else { panic!() };
        let BehaviorBody::Digital(items) = &b.body else { panic!() };
        assert!(matches!(&items[0].kind, DigitalItemKind::Gen(g) if matches!(g.kind, GenKind::For { .. })));
        let DigitalItemKind::Gen(g) = &items[1].kind else { panic!() };
        let GenKind::If { then, else_, .. } = &g.kind else { panic!() };
        assert!(matches!(then[0].kind, DigitalItemKind::Reaction(_)));
        assert!(matches!(&**else_.as_ref().unwrap(), GenElse::Block(_)));

        // A commit at any depth of a reaction (B.3.4 rule 2).
        let ItemKind::Behavior(_) =
            &ParserFixture::only_item("digital for M { on e { if a { match x { _ => { q <- 1; } } } } }").kind
        else {
            panic!()
        };

        // ---- attr_derivative, attr_derive, attr_link; call and method_call with named arguments ----
        let item = ParserFixture::only_item("#[derivative(df_dx, df_dy)] extern \"C\" fn f(x: f64, y: f64) -> f64;");
        let AttrInput::List(args) = &item.meta.attrs[0].input else { panic!() };
        assert_eq!(args.len(), 2);
        assert!(matches!(&item.kind, ItemKind::Fn(f) if f.abi.is_some() && f.body.is_none()));
        let item = ParserFixture::only_item("#[derive(Repr, Clone)] struct S;");
        assert!(matches!(&item.meta.attrs[0].input, AttrInput::List(a) if a.len() == 2));
        let item = ParserFixture::only_item("#[link(\"x.osdi\", model = \"bsim4\")] extern \"osdi\" module B() { w: f64; }");
        let AttrInput::List(args) = &item.meta.attrs[0].input else { panic!() };
        assert!(matches!(&args[0], AttrArg::Lit(_)) && matches!(&args[1], AttrArg::NameValue(n, _) if n.name == "model"));
        let ExprKind::Call { args, .. } = &ParserFixture::expr("f(1, 2, w: 3, l: 4)").kind else { panic!() };
        assert_eq!(args.iter().filter(|a| a.name.is_some()).count(), 2);
        let ExprKind::MethodCall { method, args, .. } = &ParserFixture::expr("x.scale(2, by: 3)").kind else { panic!() };
        assert_eq!((method.name.as_str(), args.len(), args[1].name.is_some()), ("scale", 2, true));

        // ---- emit, include_str (B.3.2 built-in `!` forms) ----
        let src = "emit!(\"sim\", finish = true, code = 3)";
        let e = ParserFixture::expr(src);
        let ExprKind::Emit { stream, fields } = &e.kind else { panic!("{:?}", e.kind) };
        assert_eq!(stream.kind, LitKind::Str { text: "sim".into(), raw_hashes: None });
        assert_eq!(fields.iter().map(|f| f.name.name.as_str()).collect::<Vec<_>>(), ["finish", "code"]);
        assert_eq!(ParserFixture::text(src, &e.span), src);
        assert!(matches!(ParserFixture::expr("emit!(\"sim\")").kind, ExprKind::Emit { .. }));
        assert!(matches!(ParserFixture::expr("emit!(\"sim\", a = 1,)").kind, ExprKind::Emit { .. }));
        // An `emit` statement inside a behavior block.
        let ItemKind::Behavior(b) = &ParserFixture::only_item("analog for M { emit!(\"x\", a = 1); }").kind else { panic!() };
        let BehaviorBody::Analog(stmts) = &b.body else { panic!() };
        assert!(matches!(&stmts[0].kind, StmtKind::Expr { expr: Expr { kind: ExprKind::Emit { .. }, .. }, semi: true }));
        let src = "const A: str = include_str!(\"a.txt\");";
        let ItemKind::Const(c) = &ParserFixture::only_item(src).kind else { panic!() };
        let value = c.value.as_ref().unwrap();
        assert!(matches!(&value.kind, ExprKind::IncludeStr(l) if l.kind == LitKind::Str { text: "a.txt".into(), raw_hashes: None }));
        assert_eq!(ParserFixture::text(src, &value.span), "include_str!(\"a.txt\")");
    }

    /// B.3's remaining productions: each node's span covers exactly its text.
    fn spans_of_instances_calls_places_names_and_attributes() {
        // ---- sigtype ----
        for src in ["flip Stream<T>", "[flip Stream<T>; N]", "[[Electrical; 2]; 3]", "Option<Electrical>"] {
            assert_eq!(ParserFixture::text(src, &ParserFixture::ty(src).span), src);
        }
        let src = "[flip Stream<T>; N]";
        let t = ParserFixture::ty(src);
        let TypeKind::Array { elem, len } = &t.kind else { panic!() };
        assert_eq!((ParserFixture::text(src, &elem.span), ParserFixture::text(src, &len.span)), ("flip Stream<T>", "N"));

        // ---- instance: call with a param block, callee, call, arguments ----
        let src = "let u = Resistor(a, gnd) { r: 1k, tc1: 0.1 };";
        let ModuleItemKind::Let { value, .. } = &ParserFixture::module_item(src).kind else { panic!() };
        assert_eq!(ParserFixture::text(src, &value.span), "Resistor(a, gnd) { r: 1k, tc1: 0.1 }");
        let ExprKind::Call { callee, args, params } = &value.kind else { panic!() };
        assert_eq!(ParserFixture::text(src, &callee.span), "Resistor");
        assert_eq!(args.iter().map(|a| ParserFixture::text(src, &a.span)).collect::<Vec<_>>(), ["a", "gnd"]);
        let fields = params.as_ref().unwrap();
        assert_eq!(fields.iter().map(|f| ParserFixture::text(src, &f.span)).collect::<Vec<_>>(), ["r: 1k", "tc1: 0.1"]);
        assert_eq!(fields.iter().map(|f| ParserFixture::text(src, &f.name.span)).collect::<Vec<_>>(), ["r", "tc1"]);
        let src = "Nch::<2>(d, g, s, b, w: 1u)";
        let e = ParserFixture::expr(src);
        assert_eq!(ParserFixture::text(src, &e.span), src);
        let ExprKind::Call { callee, args, .. } = &e.kind else { panic!() };
        assert_eq!(ParserFixture::text(src, &callee.span), "Nch::<2>");
        assert_eq!(ParserFixture::text(src, &args[4].span), "w: 1u");
        assert_eq!(ParserFixture::text(src, &args[4].name.as_ref().unwrap().span), "w");
        assert_eq!(ParserFixture::text(src, &args[4].value.span), "1u");
        // callee: a call, a parenthesized expression.
        let src = "nch(0.13u, 2u)(d, g, s, b)";
        let e = ParserFixture::expr(src);
        assert_eq!(ParserFixture::text(src, &e.span), src);
        let ExprKind::Call { callee, .. } = &e.kind else { panic!() };
        assert_eq!(ParserFixture::text(src, &callee.span), "nch(0.13u, 2u)");
        let src = "(pick)(x)";
        let ExprKind::Call { callee, args, .. } = &ParserFixture::expr(src).kind else { panic!() };
        assert_eq!((ParserFixture::text(src, &callee.span), ParserFixture::text(src, &args[0].span)), ("(pick)", "x"));

        // ---- method_call ----
        let src = "x.scale(2, by: 3)";
        let e = ParserFixture::expr(src);
        assert_eq!(ParserFixture::text(src, &e.span), src);
        let ExprKind::MethodCall { recv, method, args, .. } = &e.kind else { panic!() };
        assert_eq!(ParserFixture::text(src, &recv.span), "x");
        assert_eq!(ParserFixture::text(src, &method.span), "scale");
        assert_eq!(args.iter().map(|a| ParserFixture::text(src, &a.span)).collect::<Vec<_>>(), ["2", "by: 3"]);
        let src = "a.b().c::<T>(1)";
        let ExprKind::MethodCall { recv, generics, .. } = &ParserFixture::expr(src).kind else { panic!() };
        assert_eq!(ParserFixture::text(src, &recv.span), "a.b()");
        assert!(generics.is_some());

        // ---- names: `analog<N, M> for ...` ----
        let src = "analog<N, M> for Urc<N, M> { i(a, b) +> 1.0; }";
        let ItemKind::Behavior(b) = &ParserFixture::only_item(src).kind else { panic!() };
        assert_eq!(b.names.iter().map(|n| ParserFixture::text(src, &n.span)).collect::<Vec<_>>(), ["N", "M"]);
        assert_eq!(ParserFixture::text(src, &b.target.span), "Urc<N, M>");

        // ---- place: path, index and field steps ----
        for place_src in ["st", "st.n", "sh[3]", "bus[i].data", "a::b[0][1].c"] {
            let src = format!("{place_src} = y;");
            let ModuleItemKind::Def { place, .. } = &ParserFixture::module_item(&src).kind else { panic!() };
            assert_eq!(ParserFixture::text(&src, &place.span), place_src);
        }
        let src = "bus[i].data = y;";
        let ModuleItemKind::Def { place, .. } = &ParserFixture::module_item(src).kind else { panic!() };
        let ExprKind::Field { expr: base, name } = &place.kind else { panic!() };
        assert_eq!((ParserFixture::text(src, &base.span), ParserFixture::text(src, &name.span)), ("bus[i]", "data"));

        // ---- attr_derivative, attr_derive, attr_link ----
        for (src, attr_text, args) in [
            ("#[derivative(df_dx, df_dy)] extern \"C\" fn f(x: f64, y: f64) -> f64;", "#[derivative(df_dx, df_dy)]", vec!["df_dx", "df_dy"]),
            ("#[derive(Repr, Clone)] struct S;", "#[derive(Repr, Clone)]", vec!["Repr", "Clone"]),
            (
                "#[link(\"x.osdi\", model = \"bsim4\")] extern \"osdi\" module B() { w: f64; }",
                "#[link(\"x.osdi\", model = \"bsim4\")]",
                vec!["\"x.osdi\"", "model = \"bsim4\""],
            ),
        ] {
            let item = ParserFixture::only_item(src);
            let attr = &item.meta.attrs[0];
            assert_eq!(ParserFixture::text(src, &attr.span), attr_text);
            assert_eq!(ParserFixture::text(src, &attr.path.span), &attr_text[2..attr_text.find('(').unwrap()]);
            let AttrInput::List(list) = &attr.input else { panic!() };
            let shown: Vec<&str> = list
                .iter()
                .map(|a| match a {
                    AttrArg::Path(p) => ParserFixture::text(src, &p.span),
                    AttrArg::Lit(l) => ParserFixture::text(src, &l.span),
                    AttrArg::NameValue(n, l) => &src[n.span.0.start..l.span.0.end],
                })
                .collect();
            assert_eq!(shown, args, "{src}");
        }
        let src = "#[doc = \"x\"] pub nature N { value: bool }";
        let item = ParserFixture::only_item(src);
        assert_eq!(ParserFixture::text(src, &item.meta.attrs[0].span), "#[doc = \"x\"]");
    }

    /// The field list of the `Call`'s param block of a module-body `let`.
    fn let_param_fields(src: &str) -> Vec<FieldInit> {
        let f = ParserFixture::file(src);
        let ItemKind::Module(m) = &f.items[0].kind else { panic!("not a module") };
        for item in &m.body {
            if let ModuleItemKind::Let { value, .. } = &item.kind {
                if let ExprKind::Call { params: Some(p), .. } = &value.kind {
                    return p.clone();
                }
            }
        }
        panic!("no let with a param block in {src}")
    }

    /// The statements of a `fn` body source.
    fn fn_stmts(src: &str) -> Vec<Stmt> {
        let ItemKind::Fn(f) = ParserFixture::only_item(src).kind else { panic!("not a fn") };
        f.body.expect("a body").stmts
    }

    fn doc_texts(m: &Meta) -> Vec<String> {
        m.docs.iter().map(|d| d.text.trim().to_string()).collect()
    }

    fn attr_names(m: &Meta) -> Vec<String> {
        m.attrs.iter().map(|a| ParserFixture::path_names(&a.path).join("::")).collect()
    }
}

#[test]
fn inherited_rust_forms_parse() {
    // ---- types ----
    // Paths and generic arguments (types, literals, braced const blocks).
    ParserFixture::assert_path_type(&ParserFixture::ty("f64"), &["f64"]);
    ParserFixture::assert_path_type(&ParserFixture::ty("Self::WIDTH"), &["Self", "WIDTH"]);
    ParserFixture::assert_path_type(&ParserFixture::ty("crate::a::B"), &["crate", "a", "B"]);
    let bit = ParserFixture::ty("Bit<u8>");
    ParserFixture::assert_path_type(&bit, &["Bit"]);
    assert!(matches!(&ParserFixture::angle_args(&bit)[0], GenericArg::Type(t) if matches!(&t.kind, TypeKind::Path(_))));
    let uint = ParserFixture::ty("UInt<{N}>");
    assert!(matches!(&ParserFixture::angle_args(&uint)[0], GenericArg::Const(Expr { kind: ExprKind::Block(_), .. })));
    let lit_arg = ParserFixture::ty("Foo<3, true>");
    assert!(matches!(&ParserFixture::angle_args(&lit_arg)[0], GenericArg::Const(Expr { kind: ExprKind::Lit(_), .. })));
    assert_eq!(ParserFixture::angle_args(&lit_arg).len(), 2);
    // Nested generics close with two separate `>` tokens.
    let nested = ParserFixture::ty("Vec<Vec<T>>");
    let GenericArg::Type(inner) = &ParserFixture::angle_args(&nested)[0] else { panic!() };
    ParserFixture::assert_path_type(inner, &["Vec"]);
    // Trailing comma in generic arguments.
    assert_eq!(ParserFixture::angle_args(&ParserFixture::ty("Map<K, V,>")).len(), 2);

    // Arrays, slices, tuples, unit, parenthesized, references, `_`.
    assert!(matches!(ParserFixture::ty("[Bit; 8]").kind, TypeKind::Array { .. }));
    assert!(matches!(ParserFixture::ty("[(f64, f64); N]").kind, TypeKind::Array { .. }));
    assert!(matches!(ParserFixture::ty("[bool; Self::WIDTH]").kind, TypeKind::Array { .. }));
    assert!(matches!(&ParserFixture::ty("(f64, f64)").kind, TypeKind::Tuple(v) if v.len() == 2));
    assert!(matches!(&ParserFixture::ty("(T,)").kind, TypeKind::Tuple(v) if v.len() == 1));
    assert!(matches!(&ParserFixture::ty("()").kind, TypeKind::Tuple(v) if v.is_empty()));
    ParserFixture::assert_path_type(&ParserFixture::ty("(T)"), &["T"]);
    assert!(matches!(ParserFixture::ty("_").kind, TypeKind::Infer));

    // `flip` reverses a bundle type; it nests inside arrays (`sigtype`).
    assert!(matches!(&ParserFixture::ty("flip Stream<T>").kind, TypeKind::Flip(t) if matches!(t.kind, TypeKind::Path(_))));
    let flipped_array = ParserFixture::ty("[flip Stream<T>; N]");
    let TypeKind::Array { elem, .. } = &flipped_array.kind else { panic!() };
    assert!(matches!(elem.kind, TypeKind::Flip(_)));

    // Spans cover the text exactly.
    assert_eq!(ParserFixture::ty("Bit<u8>").span.0, Span::new(0, 7));
    assert_eq!(ParserFixture::ty("[Bit; 8]").span.0, Span::new(0, 8));
    assert_eq!(ParserFixture::ty("flip Stream<T>").span.0, Span::new(0, 14));

    // ---- generics and bounds ----
    let (g, diags) = GenericsGrammar::parse("<T: Repr = bool, const N: u32, C: Fn(Cell), U: A + B>");
    assert!(diags.is_empty(), "{diags:?}");
    let g = g.unwrap();
    assert_eq!(g.params.len(), 4);
    assert!(matches!(&g.params[0].kind, GenericParamKind::Type { bounds, default: Some(_) } if bounds.len() == 1));
    assert!(matches!(&g.params[1].kind, GenericParamKind::Const { default: None, .. }));
    let GenericParamKind::Type { bounds, .. } = &g.params[2].kind else { panic!() };
    assert!(matches!(
        bounds[0].segments[0].args,
        Some(GenericArgs::Paren { ref inputs, output: None, .. }) if inputs.len() == 1
    ));
    assert!(matches!(&g.params[3].kind, GenericParamKind::Type { bounds, default: None } if bounds.len() == 2));
    assert_eq!(g.span.0, Span::new(0, 53));

    // ---- patterns ----
    assert!(matches!(ParserFixture::pat("_").kind, PatKind::Wild));
    assert!(matches!(&ParserFixture::pat("x").kind, PatKind::Ident { sub: None, .. }));
    assert!(matches!(&ParserFixture::pat("x @ 1..=5").kind, PatKind::Ident { sub: Some(_), .. }));
    assert!(matches!(&ParserFixture::pat("Phase::Liquid").kind, PatKind::Path(p) if ParserFixture::path_names(p) == ["Phase", "Liquid"]));
    assert!(matches!(&ParserFixture::pat("Some(x)").kind, PatKind::TupleStruct { pats, .. } if pats.len() == 1));
    assert!(matches!(&ParserFixture::pat("Data { n }").kind, PatKind::Struct { fields, rest: false, .. } if fields.len() == 1 && fields[0].pat.is_none()));
    assert!(matches!(&ParserFixture::pat("P { x: 0, .. }").kind, PatKind::Struct { fields, rest: true, .. } if fields.len() == 1 && fields[0].pat.is_some()));
    assert!(matches!(&ParserFixture::pat("(a, b)").kind, PatKind::Tuple(v) if v.len() == 2));
    assert!(matches!(&ParserFixture::pat("()").kind, PatKind::Tuple(v) if v.is_empty()));
    assert!(matches!(ParserFixture::pat("(a)").kind, PatKind::Ident { .. }));
    assert!(matches!(&ParserFixture::pat("[a, b, ..]").kind, PatKind::Array(v) if v.len() == 3 && matches!(v[2].kind, PatKind::Rest)));
    // Alternatives, including nested in a tuple (spec: `(None, y) | (y, None)`).
    assert!(matches!(&ParserFixture::pat("(None, y) | (y, None)").kind, PatKind::Or(v) if v.len() == 2));
    // Literals: integers, negative, strings, bools.
    assert!(matches!(&ParserFixture::pat("0").kind, PatKind::Lit(_)));
    assert!(matches!(&ParserFixture::pat("-1").kind, PatKind::Lit(e) if matches!(e.kind, ExprKind::Unary { op: UnOp::Neg, .. })));
    assert!(matches!(&ParserFixture::pat("\"s\"").kind, PatKind::Lit(_)));
    assert!(matches!(&ParserFixture::pat("true").kind, PatKind::Lit(e) if matches!(&e.kind, ExprKind::Lit(Lit { kind: LitKind::Bool(true), .. }))));
    // Ranges.
    assert!(matches!(&ParserFixture::pat("0..=9").kind, PatKind::Range { inclusive: true, lo: Some(_), hi: Some(_) }));
    assert!(matches!(&ParserFixture::pat("0b00..=0b11").kind, PatKind::Range { inclusive: true, .. }));
    assert!(matches!(&ParserFixture::pat("..=9").kind, PatKind::Range { lo: None, hi: Some(_), inclusive: true }));
    assert!(matches!(&ParserFixture::pat("0..").kind, PatKind::Range { lo: Some(_), hi: None, inclusive: false }));
    assert!(matches!(&ParserFixture::pat("A..=B").kind, PatKind::Range { .. }));
    // Spans.
    assert_eq!(ParserFixture::pat("Some(x)").span.0, Span::new(0, 7));
    assert_eq!(ParserFixture::pat("(None, y) | (y, None)").span.0, Span::new(0, 21));

    // ---- inherited lexical forms ----
    // Raw strings (`r"..."`, `r#"..."#`) are string literals; the AST keeps the
    // value, a backslash or quote inside it escaped.
    let raw = ParserFixture::expr(r##"r#"a "b" \c"#"##);
    assert!(matches!(&raw.kind, ExprKind::Lit(Lit { kind: LitKind::Str { text: s, raw_hashes: Some(1) }, .. }) if s == r#"a \"b\" \\c"#), "{raw:?}");
    assert_eq!(raw.span.0, Span::new(0, 13));
    assert!(matches!(&ParserFixture::expr(r#"r"x""#).kind, ExprKind::Lit(Lit { kind: LitKind::Str { text: s, raw_hashes: Some(0) }, .. }) if s == "x"));
    // `1.` is a float; `1..2`, `1.max(2)` and `t.0.1` keep their meaning.
    assert!(matches!(&ParserFixture::expr("1.").kind, ExprKind::Lit(Lit { kind: LitKind::Float { raw, .. }, .. }) if raw == "1."));
    assert!(matches!(&ParserFixture::expr("1. + 2.5").kind, ExprKind::Binary { .. }));
    assert!(matches!(&ParserFixture::expr("1..2").kind, ExprKind::Range { .. }));
    assert!(matches!(&ParserFixture::expr("1.max(2)").kind, ExprKind::MethodCall { .. }));
    assert!(matches!(&ParserFixture::expr("t.0.1").kind, ExprKind::Field { .. }));
    // Non-ASCII identifiers (Rust's `XID_Start` / `XID_Continue`).
    let f = ParserFixture::file("fn café(µ: f64) -> f64 { let π = µ; π }");
    let ItemKind::Fn(decl) = &f.items[0].kind else { panic!() };
    assert_eq!(decl.name.name, "café");
    // A leading byte order mark is skipped; spans stay in source bytes.
    let f = ParserFixture::file("\u{FEFF}fn f() {}");
    assert_eq!(f.items[0].span.0, Span::new(3, 12));
    // `let ... else`.
    let b = ParserFixture::block("{ let Some(x) = o else { return 0; }; x }");
    let StmtKind::Let { else_: Some(diverge), init: Some(_), .. } = &b.stmts[0].kind else { panic!("{:?}", b.stmts[0]) };
    assert_eq!(diverge.stmts.len(), 1);
    assert!(matches!(&ParserFixture::block("{ let x = 1; }").stmts[0].kind, StmtKind::Let { else_: None, .. }));
}

#[test]
fn inherited_rust_forms_parse_expressions() {
    // ---- precedence, Rust's ----
    let e = ParserFixture::expr("1 + 2 * 3");
    let (l, r) = ParserFixture::binary(&e, BinOp::Add);
    assert!(ParserFixture::is_int(l, "1"));
    ParserFixture::binary(r, BinOp::Mul);
    assert_eq!(e.span.0, Span::new(0, 9));
    // `&` binds tighter than `==`; `as` tighter than `*`; `||` loosest.
    let e = ParserFixture::expr("(byte >> 7) & 1 == 1");
    let (l, _) = ParserFixture::binary(&e, BinOp::Eq);
    ParserFixture::binary(l, BinOp::BitAnd);
    let e = ParserFixture::expr("k as f64 * half");
    let (l, _) = ParserFixture::binary(&e, BinOp::Mul);
    assert!(matches!(l.kind, ExprKind::Cast { .. }));
    let e = ParserFixture::expr("a || b && c == d");
    let (_, r) = ParserFixture::binary(&e, BinOp::Or);
    let (_, r) = ParserFixture::binary(r, BinOp::And);
    ParserFixture::binary(r, BinOp::Eq);
    // Unary binds tighter than a cast: `-x as f64` is `(-x) as f64`.
    let e = ParserFixture::expr("-x as f64");
    let ExprKind::Cast { expr: inner, .. } = &e.kind else { panic!("{e:?}") };
    assert!(matches!(inner.kind, ExprKind::Unary { op: UnOp::Neg, .. }));
    assert!(matches!(ParserFixture::expr("!clk").kind, ExprKind::Unary { op: UnOp::Not, .. }));
    // The `>` family: `>>`, `>=`, `>`, `>>=` from touching `>` tokens.
    ParserFixture::binary(&ParserFixture::expr("a >> b"), BinOp::Shr);
    ParserFixture::binary(&ParserFixture::expr("a >= b"), BinOp::Ge);
    ParserFixture::binary(&ParserFixture::expr("a > b"), BinOp::Gt);
    ParserFixture::binary(&ParserFixture::expr("a <= b"), BinOp::Le);
    ParserFixture::binary(&ParserFixture::expr("a << b"), BinOp::Shl);
    assert!(matches!(ParserFixture::expr("a >>= 1").kind, ExprKind::Assign { op: Some(BinOp::Shr), .. }));
    // Assignment, compound assignment, right associative.
    assert!(matches!(ParserFixture::expr("a = b").kind, ExprKind::Assign { op: None, .. }));
    assert!(matches!(ParserFixture::expr("s += 1").kind, ExprKind::Assign { op: Some(BinOp::Add), .. }));
    let ExprKind::Assign { rhs, .. } = &ParserFixture::expr("a = b = c").kind else { panic!() };
    assert!(matches!(rhs.kind, ExprKind::Assign { .. }));
    // Ranges.
    assert!(matches!(ParserFixture::expr("0..7").kind, ExprKind::Range { lo: Some(_), hi: Some(_), inclusive: false }));
    assert!(matches!(ParserFixture::expr("0..=ROWS").kind, ExprKind::Range { inclusive: true, .. }));
    assert!(matches!(ParserFixture::expr("..4").kind, ExprKind::Range { lo: None, hi: Some(_), .. }));

    // ---- literals, paths, tuples, arrays ----
    assert!(matches!(ParserFixture::expr("true").kind, ExprKind::Lit(Lit { kind: LitKind::Bool(true), .. })));
    assert!(matches!(ParserFixture::expr("2.2u").kind, ExprKind::Lit(Lit { kind: LitKind::Float { raw, .. }, .. }) if raw == "2.2u"));
    assert!(matches!(ParserFixture::expr("Phase::Liquid").kind, ExprKind::Path(_)));
    assert!(matches!(ParserFixture::expr("(a)").kind, ExprKind::Paren(_)));
    assert!(matches!(&ParserFixture::expr("(a, b)").kind, ExprKind::Tuple(v) if v.len() == 2));
    assert!(matches!(&ParserFixture::expr("()").kind, ExprKind::Tuple(v) if v.is_empty()));
    assert!(matches!(&ParserFixture::expr("[a, b]").kind, ExprKind::Array(v) if v.len() == 2));
    assert!(matches!(&ParserFixture::expr("[]").kind, ExprKind::Array(v) if v.is_empty()));
    assert!(matches!(ParserFixture::expr("[0; 4]").kind, ExprKind::ArrayRepeat { .. }));

    // ---- calls: positional, named, turbofish, instances, templates ----
    let ExprKind::Call { args, params: None, .. } = &ParserFixture::expr("idt(x, 1.0, reset: rst)").kind else { panic!() };
    assert_eq!(args.len(), 3);
    assert!(args[0].name.is_none() && args[1].name.is_none());
    assert_eq!(args[2].name.as_ref().map(|n| n.name.as_str()), Some("reset"));
    let ExprKind::Call { callee, .. } = &ParserFixture::expr("idt::<Heat>(pwr(t), 0.0, reset: ph != Phase::Boiling)").kind else { panic!() };
    let ExprKind::Path(p) = &callee.kind else { panic!() };
    assert!(matches!(&p.segments[0].args, Some(GenericArgs::Angle { turbofish: true, .. })));
    // An instance: a call followed by a param block; the callee may be a call.
    let ExprKind::Call { params: Some(params), .. } =
        &ParserFixture::expr("MemoryBank::<_, 64, 32>(wl, bl) { cell: Dram1T1C { cs: 30f } }").kind
    else {
        panic!()
    };
    assert_eq!(params.len(), 1);
    assert!(matches!(params[0].value.kind, ExprKind::Struct { .. }));
    let ExprKind::Call { callee, .. } = &ParserFixture::expr("nch(0.13u, 2u)(d, g, s, b)").kind else { panic!() };
    assert!(matches!(callee.kind, ExprKind::Call { .. }));
    // A struct expression or template, with and without fields.
    assert!(matches!(&ParserFixture::expr("Cell { wl: wl[r], bl: bl[c] }").kind, ExprKind::Struct { fields, .. } if fields.len() == 2));
    assert!(matches!(&ParserFixture::expr("Dram1T1C { cs: 30f }").kind, ExprKind::Struct { .. }));
    // Method calls, fields, tuple indexes, indexing, `?`.
    assert!(matches!(&ParserFixture::expr("x.known().unwrap_or(d)").kind, ExprKind::MethodCall { method, .. } if method.name == "unwrap_or"));
    assert!(matches!(&ParserFixture::expr("self.p").kind, ExprKind::Field { name, .. } if name.name == "p"));
    assert!(matches!(&ParserFixture::expr("t.0").kind, ExprKind::Field { name, .. } if name.name == "0"));
    assert!(matches!(ParserFixture::expr("sh[k + 1]").kind, ExprKind::Index { .. }));
    assert!(matches!(ParserFixture::expr("self.level(k)?").kind, ExprKind::Try(_)));
    let ExprKind::MethodCall { generics: Some(_), .. } = &ParserFixture::expr("it.collect::<Vec<_>>()").kind else { panic!() };

    // ---- closures ----
    let ExprKind::Closure { params, .. } = &ParserFixture::expr("|new, old| deltalim(new, old, 1.0)").kind else { panic!() };
    assert_eq!(params.len(), 2);
    assert!(matches!(&ParserFixture::expr("|x: f64| -> f64 { x }").kind, ExprKind::Closure { ret: Some(_), .. }));
    assert!(matches!(&ParserFixture::expr("|| 0").kind, ExprKind::Closure { params, .. } if params.is_empty()));
    // A closure passed to a method, holding a `match` with or-patterns.
    ParserFixture::expr("d.iter().fold(None, |a, x| match (a, x) { (None, y) | (y, None) => y, _ => Some(0) })");

    // ---- control flow ----
    let e = ParserFixture::expr("if l < 0.18u { 0.42 } else if l < 1u { 0.45 } else { 0.47 }");
    let ExprKind::If { else_: Some(e1), .. } = &e.kind else { panic!() };
    let ExprKind::If { else_: Some(e2), .. } = &e1.kind else { panic!() };
    assert!(matches!(e2.kind, ExprKind::Block(_)));
    assert!(matches!(ParserFixture::expr("if let Some(x) = y { x } else { 0 }").kind, ExprKind::If { .. }));
    let e = ParserFixture::expr("match k { 0 => { a } 16 => b, _ if k % 2 == 1 => c, _ => d }");
    let ExprKind::Match { arms, .. } = &e.kind else { panic!() };
    assert_eq!(arms.len(), 4);
    assert!(arms[2].guard.is_some() && arms[0].guard.is_none());
    assert!(matches!(ParserFixture::expr("while i < 3 { i += 1; }").kind, ExprKind::While { .. }));
    assert!(matches!(ParserFixture::expr("loop { break; }").kind, ExprKind::Loop { .. }));
    assert!(matches!(ParserFixture::expr("for r in 0..ROWS { f(r); }").kind, ExprKind::For { .. }));
    assert!(matches!(ParserFixture::expr("{ let x = 1; x + 1 }").kind, ExprKind::Block(_)));
    assert!(matches!(ParserFixture::expr("return x").kind, ExprKind::Return(Some(_))));
    // A condition does not take a struct literal: `x { .. }` is the block.
    let ExprKind::If { cond, then, .. } = &ParserFixture::expr("if flag { 1 } else { 2 }").kind else { panic!() };
    assert!(ParserFixture::is_path(cond, "flag"));
    assert_eq!(then.stmts.len(), 1);

    // ---- built-in `!` forms ----
    let ExprKind::Emit { stream, fields } = &ParserFixture::expr("emit!(\"sim\", finish = true, msg = msg)").kind else { panic!() };
    assert!(matches!(&stream.kind, LitKind::Str { text: s, .. } if s == "sim"));
    assert_eq!(fields.len(), 2);
    assert!(matches!(&ParserFixture::expr("include_str!(\"diode_iv.tbl\")").kind, ExprKind::IncludeStr(_)));

    // ---- statements ----
    let b = ParserFixture::block("{ let (a, b) = (1, 2); let s: i2 = state; s += 1; bound_step(t / 50.0); s }");
    assert_eq!(b.stmts.len(), 5);
    assert!(matches!(&b.stmts[0].kind, StmtKind::Let { init: Some(_), ty: None, .. }));
    assert!(matches!(&b.stmts[1].kind, StmtKind::Let { ty: Some(_), .. }));
    assert!(matches!(&b.stmts[2].kind, StmtKind::Expr { semi: true, .. }));
    assert!(matches!(&b.stmts[4].kind, StmtKind::Expr { semi: false, .. }));
    // A block-like statement needs no `;` and does not swallow what follows.
    let b = ParserFixture::block("{ if a { f(); } g(); }");
    assert_eq!(b.stmts.len(), 2);
    assert!(matches!(&b.stmts[0].kind, StmtKind::Expr { semi: false, .. }));
    assert_eq!(ParserFixture::block("{ ; }").stmts.len(), 1);
    assert_eq!(ParserFixture::block("{}").span.0, Span::new(0, 2));
    // Spans: a statement covers its text including the `;`.
    let b = ParserFixture::block("{ let x = 1; }");
    assert_eq!(b.stmts[0].span.0, Span::new(2, 12));
    assert_eq!(b.span.0, Span::new(0, 14));

    // ---- types holding expressions ----
    let t = ParserFixture::ty("UInt<{N + 1}>");
    assert!(matches!(&ParserFixture::angle_args(&t)[0], GenericArg::Const(Expr { kind: ExprKind::Block(_), .. })));
    assert!(matches!(ParserFixture::ty("[Bit; N + 1]").kind, TypeKind::Array { .. }));
}

#[test]
fn contribution_commit_at_any_depth() {
    // Top level.
    let b = ParserFixture::block("{ i(p, n) +> v(p, n) / r; }");
    assert!(matches!(&b.stmts[0].kind, StmtKind::Contribution { .. }));
    assert_eq!(b.stmts[0].span.0, Span::new(2, 25));
    let b = ParserFixture::block("{ q <- d; }");
    assert!(matches!(&b.stmts[0].kind, StmtKind::Commit { after: None, .. }));
    // `after` belongs to the commit; the expression stops in front of it.
    let b = ParserFixture::block("{ clk <- !clk after period / 2.0 + j; }");
    let StmtKind::Commit { value, after: Some(after), .. } = &b.stmts[0].kind else { panic!() };
    assert!(matches!(value.kind, ExprKind::Unary { op: UnOp::Not, .. }));
    ParserFixture::binary(after, BinOp::Add);
    // Places: a carrier, an element, a field.
    let b = ParserFixture::block("{ st.n <- 1; sh[3] <- x; sh[7] <- rx; }");
    assert_eq!(ParserFixture::count_stmts(&b), (0, 3));
    let StmtKind::Commit { place, .. } = &b.stmts[0].kind else { panic!() };
    assert!(matches!(place.kind, ExprKind::Field { .. }));
    let StmtKind::Commit { place, .. } = &b.stmts[1].kind else { panic!() };
    assert!(matches!(place.kind, ExprKind::Index { .. }));
    // Under `if` / `else`.
    let b = ParserFixture::block("{ if ctl.high() { v(a, b) +> 0.0; } else { i(a, b) +> 0.0; } }");
    assert_eq!(ParserFixture::count_stmts(&b), (2, 0));
    // Under `match` arms with a block body.
    let b = ParserFixture::block(
        "{ match ph { Phase::Liquid => { pwr(t) +> ddt(m * cl * temp(t)); } \
           Phase::Boiling => { temp(t) +> tb; } \
           Phase::Vapor => { pwr(t) +> ddt(m * cv * temp(t)); } } }",
    );
    assert_eq!(ParserFixture::count_stmts(&b), (3, 0));
    // Under `for`, and nested several levels deep, commits as well.
    let b = ParserFixture::block("{ for k in 0..7 { sh[k] <- sh[k + 1]; } }");
    assert_eq!(ParserFixture::count_stmts(&b), (0, 1));
    let b = ParserFixture::block("{ if a { for k in 0..3 { match k { 0 => { x +> 1.0; } _ => { y <- 2; } } } } }");
    assert_eq!(ParserFixture::count_stmts(&b), (1, 1));
    // `while`, `loop` and a plain nested block.
    let b = ParserFixture::block("{ while c { x +> 1.0; } loop { y <- 1; } { z +> 2.0; } }");
    assert_eq!(ParserFixture::count_stmts(&b), (2, 1));
}

#[test]
fn unknown_is_an_ordinary_name() {
    // explicit-unknowns EU-02: `Unknown` is no literal and no reserved name; the parser takes it wherever an
    // identifier goes (the checker finds nothing by that name: `E0161`), and raises no `E0908`.
    assert!(matches!(&ParserFixture::expr("Unknown").kind, ExprKind::Path(_)));
    for src in [
        "fn Unknown() {}",
        "struct Unknown;",
        "enum E { Unknown }",
        "struct S { Unknown: u8 }",
        "const Unknown: u8 = 1;",
        "fn f(Unknown: u8) {}",
        "fn f() { let Unknown = 1; }",
        "fn f() { let (a, Unknown) = t; }",
        "fn f() { for Unknown in 0..3 {} }",
        "fn f() { let g = |Unknown| 1; }",
        "fn f() { let x = Unknown; g(Unknown, 1); }",
    ] {
        let r = SourceParser::parse(src);
        assert!(r.diagnostics.is_empty(), "{src:?}: {:?}", r.diagnostics);
    }
}

#[test]
fn a_struct_field_takes_an_initial_value_after_its_type() {
    // §7 rule 10: `out q: Bit = false` is the field default, an expression after the type; a field without one has none
    let ItemKind::Struct(s) = ParserFixture::only_item("struct Latch { out q: Bit<u8> = 1 + 2, in d: Bit }").kind else { panic!() };
    let Fields::Named(fields) = s.fields else { panic!("named fields") };
    assert!(matches!(&fields[0].init, Some(Expr { kind: ExprKind::Binary { .. }, .. })), "{:?}", fields[0].init);
    assert!(fields[1].init.is_none());
    // an `in` field takes one the same way (the consumer's start)
    let ItemKind::Struct(s) = ParserFixture::only_item("struct Hand { in ready: Bit = true }").kind else { panic!() };
    let Fields::Named(fields) = s.fields else { panic!("named fields") };
    assert!(matches!(&fields[0].init, Some(Expr { kind: ExprKind::Lit(_), .. })), "{:?}", fields[0].init);
    assert!(piperine_lang::msl::print::Printer::print(&SourceParser::parse("struct Hand { in ready: Bit = true }").file).contains("in ready: Bit = true"));
    // the printer writes it back, and reading that gives the same tree
    let printed = piperine_lang::msl::print::Printer::print(&SourceParser::parse("struct Latch { out q: Bit<u8> = 1 + 2, in d: Bit }").file);
    assert!(printed.contains("out q: Bit<u8> = 1 + 2"), "{printed}");
    assert!(SourceParser::parse(&printed).diagnostics.is_empty(), "{printed}");
}

#[test]
fn inherited_rust_forms_parse_items() {
    // ---- fn ----
    let ItemKind::Fn(f) = ParserFixture::only_item("fn windup(u: f64, lo: f64, hi: f64) -> f64 { u.clamp(lo, hi) - u }").kind else { panic!() };
    assert_eq!(f.name.name, "windup");
    assert_eq!(f.params.len(), 3);
    assert!(f.ret.is_some() && f.body.is_some() && !f.is_const && f.abi.is_none());
    let item = ParserFixture::only_item("pub fn handshake(req: Bit, ack: Bit) -> Event { req.rise() && ack }");
    assert!(item.public);
    // A declaration without a body, `extern "C"`, `const fn`, generics, `where`, `self`.
    let ItemKind::Fn(f) = ParserFixture::only_item("pub fn pnjlim(new: f64, old: f64) -> f64;").kind else { panic!() };
    assert!(f.body.is_none());
    let ItemKind::Fn(f) = ParserFixture::only_item("extern \"C\" fn erfcx(x: f64) -> f64;").kind else { panic!() };
    assert!(matches!(&f.abi, Some(Lit { kind: LitKind::Str { text: s, .. }, .. }) if s == "C"));
    let ItemKind::Fn(f) = ParserFixture::only_item("const fn parse(s: str) -> u32 { 0 }").kind else { panic!() };
    assert!(f.is_const);
    let ItemKind::Fn(f) = ParserFixture::only_item("fn f<const N: u32>(x: f64) -> f64 where N >= 1 { x }").kind else { panic!() };
    assert!(f.generics.is_some());
    let Some(Expr { kind: ExprKind::Binary { op: BinOp::Ge, .. }, .. }) = &f.where_ else { panic!("{:?}", f.where_) };
    let ItemKind::Fn(f) = ParserFixture::only_item("fn celsius(self) -> f64 { 1.0 }").kind else { panic!() };
    assert!(matches!(f.params[0], FnParam::SelfValue(_)));
    // No trailing-comma requirement, and a trailing comma is fine.
    ParserFixture::file("fn f(a: u8,) {}");
    // ---- struct ----
    let ItemKind::Struct(s) = ParserFixture::only_item("struct Stream<T: Repr> { out valid: Bit, in ready: Bit, out data: Bit<T> }").kind else { panic!() };
    let Fields::Named(fields) = &s.fields else { panic!() };
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].dir, Some(Dir::Out));
    assert_eq!(fields[1].dir, Some(Dir::In));
    assert!(matches!(ParserFixture::only_item("struct Wrap(u8, f64);").kind, ItemKind::Struct(StructDecl { fields: Fields::Tuple(v), .. }) if v.len() == 2));
    assert!(matches!(ParserFixture::only_item("struct U;").kind, ItemKind::Struct(StructDecl { fields: Fields::Unit, .. })));
    assert!(matches!(ParserFixture::only_item("struct Diff { p: Electrical, n: Electrical }").kind, ItemKind::Struct(_)));
    // ---- enum ----
    let ItemKind::Enum(e) = ParserFixture::only_item("enum Tx { Idle, Start, Data { n: u3 }, Stop }").kind else { panic!() };
    assert_eq!(e.variants.len(), 4);
    assert!(matches!(e.variants[2].fields, Fields::Named(_)));
    let ItemKind::Enum(e) = ParserFixture::only_item("enum Op { Add = 0b00, Sub = 0b01, }").kind else { panic!() };
    assert!(e.variants[0].discriminant.is_some());
    assert!(matches!(ParserFixture::only_item("enum Opt<T> { No, Yes(T) }").kind, ItemKind::Enum(EnumDecl { variants, .. }) if matches!(variants[1].fields, Fields::Tuple(_))));
    // ---- impl, trait ----
    let ItemKind::Impl(i) = ParserFixture::only_item("impl<T> Stream<T> { pub fn beat(self, clk: Bit) -> Event { clk.rise() && self.valid } }").kind else { panic!() };
    assert!(i.trait_.is_none() && i.generics.is_some() && i.items.len() == 1);
    assert!(i.items[0].public);
    let ItemKind::Impl(i) = ParserFixture::only_item("impl<T: Repr> Known for T {}").kind else { panic!() };
    assert_eq!(ParserFixture::path_names(i.trait_.as_ref().unwrap()), ["Known"]);
    // An `impl` may hold `const` items (B.3.5 amendment 4).
    let ItemKind::Impl(i) = ParserFixture::only_item("impl Logic { pub const Z: Logic = Logic::z(); }").kind else { panic!() };
    assert!(matches!(i.items[0].kind, ImplItemKind::Const(_)));
    let ItemKind::Trait(t) = ParserFixture::only_item(
        "pub trait Repr: Copy { const WIDTH: u32; fn encode(self) -> [bool; Self::WIDTH]; fn known(self) -> Option<Self> { Some(self) } }",
    )
    .kind
    else {
        panic!()
    };
    assert_eq!(t.supertraits.len(), 1);
    assert_eq!(t.items.len(), 3);
    assert!(matches!(&t.items[0].kind, ImplItemKind::Const(ConstItem { value: None, .. })));
    // ---- const, use, mod ----
    let ItemKind::Const(c) = ParserFixture::only_item("const IV: [(f64, f64); 64] = parse_table(include_str!(\"diode_iv.tbl\"));").kind else { panic!() };
    assert_eq!(c.name.name, "IV");
    let ItemKind::Use(u) = ParserFixture::only_item("use a::b::{c, d as e, f::*};").kind else { panic!() };
    assert_eq!(u.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    let UseKind::Group(g) = &u.kind else { panic!() };
    assert_eq!(g.len(), 3);
    assert!(matches!(&g[1].kind, UseKind::Simple { alias: Some(a) } if a.name == "e"));
    assert!(matches!(g[2].kind, UseKind::Glob));
    assert!(matches!(ParserFixture::only_item("use crate::prelude::*;").kind, ItemKind::Use(UseTree { kind: UseKind::Glob, .. })));
    let ItemKind::Mod(m) = ParserFixture::only_item("mod m { fn f() {} const X: u8 = 1; }").kind else { panic!() };
    assert_eq!(m.items.as_ref().map(Vec::len), Some(2));
    assert!(matches!(ParserFixture::only_item("mod ext;").kind, ItemKind::Mod(ModItem { items: None, .. })));
    // Items inside a block.
    let b = ParserFixture::block("{ const K: u8 = 1; fn g() {} let x = K; x }");
    assert!(matches!(b.stmts[0].kind, StmtKind::Item(_)) && matches!(b.stmts[1].kind, StmtKind::Item(_)));
    // Several items, in order; an empty file has none.
    assert_eq!(ParserFixture::file("fn a() {} fn b() {} struct C;").items.len(), 3);
    assert!(ParserFixture::file("").items.is_empty());
    assert!(ParserFixture::file("  // just a comment\n").items.is_empty());
    // The spec's closure-in-fn example, whole.
    ParserFixture::file("pub fn tri<T: Repr, const N: usize>(d: [Option<T>; N]) -> Option<T> {\n    d.iter().fold(None, |a, x| match (a, x) { (None, y) | (y, None) => y, _ => Some(0) })\n}");
    ParserFixture::file("pub fn finish() { emit!(\"sim\", finish = true); }");
}

#[test]
fn doc_comments_and_attrs_attach() {
    let src = "/// Ideal resistor.\n/// Two lines.\n#[derive(Repr, Clone)]\n#[repr(u2)]\npub enum Op {\n    /// the add\n    Add = 0,\n    #[doc = \"sub\"] Sub = 1,\n}\n";
    let item = ParserFixture::only_item(src);
    // Docs keep their text verbatim and attach to the next item.
    let texts: Vec<_> = item.meta.docs.iter().map(|d| d.text.as_str()).collect();
    assert_eq!(texts, [" Ideal resistor.", " Two lines."]);
    assert_eq!(item.meta.attrs.len(), 2);
    assert_eq!(ParserFixture::path_names(&item.meta.attrs[0].path), ["derive"]);
    let AttrInput::List(args) = &item.meta.attrs[0].input else { panic!() };
    assert_eq!(args.len(), 2);
    assert!(matches!(&args[0], AttrArg::Path(p) if ParserFixture::path_names(p) == ["Repr"]));
    assert_eq!(ParserFixture::path_names(&item.meta.attrs[1].path), ["repr"]);
    // The item's span starts at its first doc comment.
    assert_eq!(item.span.0.start, 0);
    let ItemKind::Enum(e) = &item.kind else { panic!() };
    // Variants take docs and attributes too.
    assert_eq!(e.variants[0].meta.docs[0].text, " the add");
    assert!(matches!(&e.variants[1].meta.attrs[0].input, AttrInput::Eq(_)));
    assert_eq!(ParserFixture::path_names(&e.variants[1].meta.attrs[0].path), ["doc"]);

    // Struct fields, impl items.
    let item = ParserFixture::only_item("struct S {\n    /// a doc\n    #[doc = \"x\"]\n    pub a: Bit,\n    b: Bit,\n}");
    let ItemKind::Struct(StructDecl { fields: Fields::Named(f), .. }) = &item.kind else { panic!() };
    assert_eq!(f[0].meta.docs.len(), 1);
    assert_eq!(f[0].meta.attrs.len(), 1);
    assert!(f[0].public);
    assert!(f[1].meta.docs.is_empty() && f[1].meta.attrs.is_empty());
    let item = ParserFixture::only_item("impl X {\n    /// method doc\n    pub fn f(self) {}\n    /// const doc\n    const C: u8 = 1;\n}");
    let ItemKind::Impl(i) = &item.kind else { panic!() };
    assert_eq!(i.items[0].meta.docs[0].text, " method doc");
    assert_eq!(i.items[1].meta.docs[0].text, " const doc");

    // `#[link(..)]` with a string and a name-value argument.
    let item = ParserFixture::only_item("#[link(\"bsim4.osdi\", model = \"bsim4\")] fn f() {}");
    let AttrInput::List(args) = &item.meta.attrs[0].input else { panic!() };
    assert!(matches!(&args[0], AttrArg::Lit(Lit { kind: LitKind::Str { text: s, .. }, .. }) if s == "bsim4.osdi"));
    assert!(matches!(&args[1], AttrArg::NameValue(n, _) if n.name == "model"));

    // Ordinary comments are discarded; `//!` is the file's inner doc.
    let f = ParserFixture::file("//! crate doc\n// plain\n/* block */\nfn f() {}");
    assert_eq!(f.inner_docs.len(), 1);
    assert_eq!(f.inner_docs[0].text, " crate doc");
    assert!(f.items[0].meta.docs.is_empty());
}

#[test]
fn every_b3_production_parses_with_span() {
    ParserFixture::natures_signals_and_sigtypes();
    ParserFixture::modules_ports_items_and_generation();
    ParserFixture::behavior_blocks_and_builtin_forms();
    ParserFixture::spans_of_instances_calls_places_names_and_attributes();
}

#[test]
fn contextual_keywords_are_identifiers_outside_nature() {
    // MSLP-06: `potential`, `flow`, `value`, `resolve`, `analog`, `digital`
    // are identifiers everywhere except at the position their production names.
    let f = ParserFixture::file(
        "fn analog(digital: f64) -> f64 { digital }
         fn flow(potential: f64, value: f64, resolve: f64) -> f64 { potential + value * resolve }
         struct S { value: f64, flow: f64, potential: f64 }
         const resolve: f64 = 1.0;
         module digital(analog: Electrical, flow: Electrical) { potential: f64; value = analog; }
         analog for digital { let potential = 1.0; let flow = potential; flow +> analog; }
         digital for analog { on potential { value <- resolve; } }",
    );
    assert_eq!(f.items.len(), 7);
    assert!(matches!(f.items[4].kind, ItemKind::Module(_)));
    assert!(matches!(f.items[5].kind, ItemKind::Behavior(_)));
    // Calls, fields, paths and patterns.
    for src in ["analog(1.0)", "digital.flow", "value::resolve", "potential + flow", "x.analog().digital"] {
        ParserFixture::expr(src);
    }
    ParserFixture::pat("analog");
    ParserFixture::pat("value");
    // Inside a nature the words keep their meaning; elsewhere in the same
    // file they stay names.
    let f = ParserFixture::file("nature N { potential: P, flow: F } fn value() { let potential = 1; potential }");
    assert_eq!(f.items.len(), 2);
    // `analog` not followed by `for` or `<` is not a behavior block.
    assert!(!SourceParser::parse("analog { }").diagnostics.is_empty());
    assert!(!SourceParser::parse("analog Foo { }").diagnostics.is_empty());
}

#[test]
fn struct_expression_field_shorthand() {
    // Rust 2024 value grammar: `Logic { v, s: Strength::Strong }` is
    // `Logic { v: v, s: .. }`; the shorthand field's value is the name itself.
    let ExprKind::Struct { fields, .. } = &ParserFixture::expr("Logic { v, s: Strength::Strong }").kind else { panic!() };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name.name, "v");
    assert!(matches!(&fields[0].value.kind, ExprKind::Path(p) if ParserFixture::path_names(p) == ["v"]));
    assert!(matches!(&fields[1].value.kind, ExprKind::Path(p) if ParserFixture::path_names(p) == ["Strength", "Strong"]));
    assert!(fields[0].shorthand && !fields[1].shorthand);
}

#[test]
fn nested_tuple_index() {
    // `x.0.1` is two tuple indexes, though the lexer reads `0.1` as a float.
    let src = "x.0.1";
    let e = ParserFixture::expr(src);
    let ExprKind::Field { expr: inner, name } = &e.kind else { panic!("{:?}", e.kind) };
    assert_eq!(name.name, "1");
    let ExprKind::Field { expr: base, name } = &inner.kind else { panic!() };
    assert_eq!(name.name, "0");
    assert!(matches!(&base.kind, ExprKind::Path(p) if ParserFixture::path_names(p) == ["x"]));
    assert_eq!(ParserFixture::text(src, &e.span), "x.0.1");
    assert_eq!(ParserFixture::text(src, &inner.span), "x.0");
    // Three levels, and an ordinary float field is not one.
    assert!(matches!(&ParserFixture::expr("a.0.1.2").kind, ExprKind::Field { name, .. } if name.name == "2"));
    assert!(!ExpressionGrammar::parse("x.1.5e3").1.is_empty());
}

#[test]
fn negative_literal_is_a_const_generic_argument() {
    // POL-09 AC 1: `Int< -1 >` and `Int<{ -1 }>` parse clean; the argument is
    // the const expression `-1`; print gives each spelling back.
    let bare = ParserFixture::ty("Int< -1 >");
    let GenericArg::Const(e) = &ParserFixture::angle_args(&bare)[0] else { panic!("not a const argument") };
    let ExprKind::Unary { op: UnOp::Neg, expr: inner } = &e.kind else { panic!("not -1: {e:?}") };
    assert!(ParserFixture::is_int(inner, "1"));
    // The braced spelling holds the same `Unary(Neg, Lit)` as its block tail.
    let braced = ParserFixture::ty("Int<{ -1 }>");
    let GenericArg::Const(Expr { kind: ExprKind::Block(b), .. }) = &ParserFixture::angle_args(&braced)[0] else { panic!() };
    let StmtKind::Expr { expr: tail, semi: false } = &b.stmts[0].kind else { panic!("{b:?}") };
    assert!(matches!(&tail.kind, ExprKind::Unary { op: UnOp::Neg, .. }), "{tail:?}");
    // Both spellings print and reparse to the same tree.
    for src in ["module M(a: Bit<Int< -1 >>) {}", "module M(a: Bit<Int<{ -1 }>>) {}"] {
        let first = ParserFixture::file(src);
        let text = piperine_lang::msl::print::Printer::print(&first);
        let again = ParserFixture::file(&text);
        assert!(first == again, "{src}: printed as {text}");
        assert!(text.contains("-1"), "{text}");
    }
    // A float literal too.
    let f = ParserFixture::ty("Foo< -1.5, 2 >");
    assert_eq!(ParserFixture::angle_args(&f).len(), 2);
}

#[test]
fn commit_token_in_generic_arguments_is_still_an_error() {
    // POL-09 AC 2: `Int<-1>` lexes `<-` (the commit operator): a diagnostic at it.
    let src = "module M(a: Bit<Int<-1>>) {}";
    let r = SourceParser::parse(src);
    assert!(!r.diagnostics.is_empty(), "no diagnostic");
    let at = src.find("<-").unwrap();
    assert!(
        r.diagnostics.iter().any(|d| d.span().start == at && d.span().end == at + 2),
        "{:?}",
        r.diagnostics
    );
}

#[test]
fn function_pointer_types_parse() {
    // POL-09 AC 3: `fn(f64) -> f64`, `fn([T; N]) -> T` and `fn()` in a fn
    // parameter, a struct field, a `const` and a `let` annotation.
    let f = ParserFixture::ty("fn(f64) -> f64");
    let TypeKind::FnPtr { params, ret } = &f.kind else { panic!("{f:?}") };
    assert_eq!(params.len(), 1);
    ParserFixture::assert_path_type(&params[0], &["f64"]);
    ParserFixture::assert_path_type(ret.as_deref().expect("a return type"), &["f64"]);

    let g = ParserFixture::ty("fn([T; 2]) -> T");
    let TypeKind::FnPtr { params, ret } = &g.kind else { panic!("{g:?}") };
    assert!(matches!(&params[0].kind, TypeKind::Array { .. }));
    assert!(ret.is_some());

    let h = ParserFixture::ty("fn()");
    assert!(matches!(&h.kind, TypeKind::FnPtr { params, ret } if params.is_empty() && ret.is_none()));

    let sources = [
        "fn apply(f: fn(f64) -> f64, x: f64) -> f64 { f(x) }",
        "struct S { f: fn([T; 2]) -> T }",
        "const F: fn() = g;",
        "fn k() { let f: fn(f64) -> f64 = g; }",
    ];
    for src in sources {
        let first = ParserFixture::file(src);
        let text = piperine_lang::msl::print::Printer::print(&first);
        assert!(text.contains("fn("), "{src}: printed as {text}");
        assert!(first == ParserFixture::file(&text), "{src}: printed as {text}");
    }
    // `fn` at item position is still a function item.
    assert!(matches!(ParserFixture::only_item("fn g() {}").kind, ItemKind::Fn(_)));
}

#[test]
fn param_block_and_template_shorthand_keep_their_form() {
    // POL-11 AC 1: `{ r }` holds `shorthand == true` with the path `r` as its
    // value; `{ r: r }` holds `false`; both in a param block and a template.
    let fields = ParserFixture::let_param_fields("module T() { let x = R(a, b) { r, c: 1p, d: d }; }");
    assert_eq!(fields.len(), 3);
    assert!(fields[0].shorthand);
    assert!(matches!(&fields[0].value.kind, ExprKind::Path(p) if ParserFixture::path_names(p) == ["r"]));
    assert!(!fields[1].shorthand);
    assert!(!fields[2].shorthand, "`d: d` is the long form");
    // A template: `R { r, c: 1p }`.
    let ExprKind::Struct { fields, .. } = &ParserFixture::expr("R { r, c: 1p }").kind else { panic!() };
    assert!(fields[0].shorthand && !fields[1].shorthand);
    let ExprKind::Struct { fields, .. } = &ParserFixture::expr("R { r: r }").kind else { panic!() };
    assert!(!fields[0].shorthand);
    // `emit!` arguments are never shorthand.
    let ExprKind::Emit { fields, .. } = &ParserFixture::expr("emit!(\"s\", a = 1)").kind else { panic!() };
    assert!(fields.iter().all(|f| !f.shorthand));
}

#[test]
fn docs_and_attrs_attach_to_statements() {
    // POL-12 AC 1: docs and attributes before a `let`, an expression
    // statement, a contribution and a commit are held, in order, in the
    // metadata of that statement.
    let stmts = ParserFixture::fn_stmts(
        "fn f() {\n    /// first\n    /// second\n    #[a]\n    #[b = 1]\n    let y = 1;\n    /// call\n    g();\n    /// tail\n    #[c]\n    y\n}",
    );
    assert_eq!(stmts.len(), 3);
    let docs = |s: &Stmt| s.meta.docs.iter().map(|d| d.text.trim().to_string()).collect::<Vec<_>>();
    let attrs = |s: &Stmt| s.meta.attrs.iter().map(|a| ParserFixture::path_names(&a.path).join("::")).collect::<Vec<_>>();
    assert!(matches!(stmts[0].kind, StmtKind::Let { .. }));
    assert_eq!(docs(&stmts[0]), ["first", "second"]);
    assert_eq!(attrs(&stmts[0]), ["a", "b"]);
    assert_eq!(docs(&stmts[1]), ["call"]);
    assert!(attrs(&stmts[1]).is_empty());
    assert_eq!((docs(&stmts[2]), attrs(&stmts[2])), (vec!["tail".to_string()], vec!["c".to_string()]));

    // A contribution in an `analog` block, a commit in a reaction, and a
    // block-like statement.
    let f = ParserFixture::file(
        "analog for M {\n    /// gate current\n    #[probe]\n    i(a) +> 1.0;\n}\ndigital for M {\n    on e {\n        /// drive\n        q <- 1;\n        /// loop\n        if a { }\n    }\n}",
    );
    let ItemKind::Behavior(BehaviorBlock { body: BehaviorBody::Analog(a), .. }) = &f.items[0].kind else { panic!() };
    assert!(matches!(a[0].kind, StmtKind::Contribution { .. }));
    assert_eq!((docs(&a[0]), attrs(&a[0])), (vec!["gate current".to_string()], vec!["probe".to_string()]));
    let ItemKind::Behavior(BehaviorBlock { body: BehaviorBody::Digital(d), .. }) = &f.items[1].kind else { panic!() };
    let DigitalItemKind::Reaction(r) = &d[0].kind else { panic!("{:?}", d[0].kind) };
    assert!(matches!(r.body.stmts[0].kind, StmtKind::Commit { .. }));
    assert_eq!(docs(&r.body.stmts[0]), ["drive"]);
    assert_eq!(docs(&r.body.stmts[1]), ["loop"]);

    // A doc before an item keyword stays the item's.
    let stmts = ParserFixture::fn_stmts("fn f() { /// the const\n const X: u8 = 1; }");
    let StmtKind::Item(item) = &stmts[0].kind else { panic!("{:?}", stmts[0].kind) };
    assert_eq!(item.meta.docs.len(), 1);
    assert!(stmts[0].meta.docs.is_empty());
}

#[test]
fn docs_and_attrs_attach_to_hardware_items() {
    // POL-12 AC 3 and the six misleading diagnostics of A-07: each source
    // parses with no diagnostic, and the docs and attributes are held, in
    // order, in the metadata of the node they precede.
    let sources = [
        "analog for M {\n    /// gate current\n    i(a) +> 1.0;\n}",
        "fn f() {\n    /// x\n    let y = 1;\n}",
        "digital for M {\n    /// clock edge\n    on e { }\n}",
        "module M() {\n    /// drive\n    s = x;\n    for i in 0..2 { }\n}",
        "nature N {\n    /// level\n    value: bool\n}",
        "module M() {\n    /// loop\n    for i in 0..2 { }\n}",
    ];
    for src in sources {
        let r = SourceParser::parse(src);
        assert!(r.diagnostics.is_empty(), "{src}: {:?}", r.diagnostics);
    }

    // A reaction and a digital generation (`for` and `if`).
    let f = ParserFixture::file(
        "digital for M {\n    /// clock edge\n    /// second\n    #[a]\n    on e { }\n    /// gen for\n    for i in 0..2 { /// inner\n        on x { } }\n    #[g]\n    /// gen if\n    if true { on y { } } else { on z { } }\n}",
    );
    let ItemKind::Behavior(BehaviorBlock { body: BehaviorBody::Digital(d), .. }) = &f.items[0].kind else { panic!() };
    assert_eq!(ParserFixture::doc_texts(&d[0].meta), ["clock edge", "second"]);
    assert_eq!(ParserFixture::attr_names(&d[0].meta), ["a"]);
    assert!(matches!(d[1].kind, DigitalItemKind::Gen(_)));
    assert_eq!(ParserFixture::doc_texts(&d[1].meta), ["gen for"]);
    let DigitalItemKind::Gen(Gen { kind: GenKind::For { body, .. }, .. }) = &d[1].kind else { panic!() };
    assert_eq!(ParserFixture::doc_texts(&body[0].meta), ["inner"]);
    assert_eq!((ParserFixture::doc_texts(&d[2].meta), ParserFixture::attr_names(&d[2].meta)), (vec!["gen if".to_string()], vec!["g".to_string()]));

    // A definition and a module generation.
    let f = ParserFixture::file("module M() {\n    /// drive\n    #[d]\n    s = x after 1;\n    #[e]\n    /// gen\n    if a { } else { }\n}");
    let ItemKind::Module(m) = &f.items[0].kind else { panic!() };
    assert!(matches!(m.body[0].kind, ModuleItemKind::Def { .. }));
    assert_eq!((ParserFixture::doc_texts(&m.body[0].meta), ParserFixture::attr_names(&m.body[0].meta)), (vec!["drive".to_string()], vec!["d".to_string()]));
    assert!(matches!(m.body[1].kind, ModuleItemKind::Gen(_)));
    assert_eq!((ParserFixture::doc_texts(&m.body[1].meta), ParserFixture::attr_names(&m.body[1].meta)), (vec!["gen".to_string()], vec!["e".to_string()]));

    // Nature fields: a quantity, a value.
    let f = ParserFixture::file(
        "nature N {\n    /// pot\n    potential: V { access: v },\n    #[f]\n    flow: I { access: i },\n}\nnature L {\n    /// level\n    value: bool,\n}",
    );
    let ItemKind::Nature(n) = &f.items[0].kind else { panic!() };
    assert_eq!(ParserFixture::doc_texts(&n.fields[0].meta), ["pot"]);
    assert_eq!(ParserFixture::attr_names(&n.fields[1].meta), ["f"]);
    let ItemKind::Nature(l) = &f.items[1].kind else { panic!() };
    assert_eq!(ParserFixture::doc_texts(&l.fields[0].meta), ["level"]);
}
