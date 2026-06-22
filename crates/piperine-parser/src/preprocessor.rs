//! Verilog-A/AMS preprocessor.
//!
//! Operates on the [`Lexed`] token stream produced by [`crate::lexer`]. Handles
//! `` `define `` (object- and function-like), `` `undef ``, `` `include ``,
//! and the `` `ifdef / `ifndef / `else / `endif `` conditional directives, then
//! expands macro uses. The result is a token stream with no directives, macro
//! uses, newlines, or continuation backslashes left — ready for the parser.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::lexer::{tokenize, Lexed, Tok};

#[derive(Debug, Clone)]
struct Macro {
    /// `Some(params)` for function-like macros, `None` for object-like.
    params: Option<Vec<String>>,
    body: Vec<Lexed>,
}

struct CondFrame {
    /// Whether the currently selected branch emits tokens.
    active: bool,
    /// Whether any branch in this `ifdef` chain has been taken yet.
    taken: bool,
    /// Whether the enclosing context was active when this frame opened.
    parent_active: bool,
}

pub struct Preprocessor {
    macros: HashMap<String, Macro>,
    include_dirs: Vec<PathBuf>,
    out: Vec<Lexed>,
    conds: Vec<CondFrame>,
    /// Guards against runaway include recursion.
    include_depth: usize,
}

impl Preprocessor {
    pub fn new(include_dirs: Vec<PathBuf>) -> Self {
        Preprocessor {
            macros: HashMap::new(),
            include_dirs,
            out: Vec::new(),
            conds: Vec::new(),
            include_depth: 0,
        }
    }

    /// Predefine an object-like macro (e.g. `__VAMS_ENABLE__`).
    pub fn define(&mut self, name: &str, body: &str) {
        let body = tokenize(body).unwrap_or_default();
        self.macros.insert(name.to_string(), Macro { params: None, body });
    }

    pub fn run(mut self, tokens: Vec<Lexed>) -> Result<Vec<Lexed>, String> {
        self.process(&tokens)?;
        if !self.conds.is_empty() {
            return Err("unterminated `ifdef/`ifndef (missing `endif)".into());
        }
        Ok(self.out)
    }

    fn active(&self) -> bool {
        self.conds.last().map(|f| f.active).unwrap_or(true)
    }

    fn process(&mut self, tokens: &[Lexed]) -> Result<(), String> {
        let mut i = 0;
        while i < tokens.len() {
            let t = &tokens[i];
            match &t.tok {
                Tok::Newline | Tok::Backslash => {
                    i += 1;
                }
                Tok::Tick(name) => {
                    i = self.directive_or_macro(tokens, i, name.clone())?;
                }
                _ => {
                    if self.active() {
                        self.out.push(t.clone());
                    }
                    i += 1;
                }
            }
        }
        Ok(())
    }

    /// Dispatch a `` `name `` token: directive keyword or macro use. Returns the
    /// index to continue from.
    fn directive_or_macro(
        &mut self,
        tokens: &[Lexed],
        i: usize,
        name: String,
    ) -> Result<usize, String> {
        match name.as_str() {
            "define" => self.dir_define(tokens, i),
            "undef" => self.dir_undef(tokens, i),
            "include" => self.dir_include(tokens, i),
            "ifdef" => self.dir_ifdef(tokens, i, false),
            "ifndef" => self.dir_ifdef(tokens, i, true),
            "else" => self.dir_else(tokens, i),
            "endif" => self.dir_endif(tokens, i),
            // ignored no-op directives
            "resetall" | "celldefine" | "endcelldefine" | "default_nettype"
            | "timescale" | "begin_keywords" | "end_keywords" => {
                Ok(skip_to_newline(tokens, i + 1))
            }
            _ => {
                if self.active() {
                    let (toks, next) = self.expand_use(tokens, i, &name)?;
                    self.out.extend(toks);
                    Ok(next)
                } else {
                    Ok(i + 1)
                }
            }
        }
    }

    // ── directives ──────────────────────────────────────────────────────

    fn dir_define(&mut self, tokens: &[Lexed], i: usize) -> Result<usize, String> {
        // i points at `define
        let mut j = i + 1;
        let (name, name_end) = expect_ident(tokens, j)?;
        j += 1;
        // function-like only when '(' is adjacent to the macro name
        let mut params = None;
        if j < tokens.len()
            && tokens[j].tok == Tok::LParen
            && tokens[j].start == name_end
        {
            j += 1;
            let mut ps = Vec::new();
            loop {
                let (p, _) = expect_ident(tokens, j)?;
                ps.push(p);
                j += 1;
                match tokens.get(j).map(|t| &t.tok) {
                    Some(Tok::Comma) => j += 1,
                    Some(Tok::RParen) => {
                        j += 1;
                        break;
                    }
                    _ => return Err("malformed macro parameter list".into()),
                }
            }
            params = Some(ps);
        }
        // body: until a non-continued newline
        let mut body = Vec::new();
        while j < tokens.len() {
            match &tokens[j].tok {
                Tok::Backslash => {
                    // continuation: skip backslash and following newline
                    j += 1;
                    if matches!(tokens.get(j).map(|t| &t.tok), Some(Tok::Newline)) {
                        j += 1;
                    }
                }
                Tok::Newline => {
                    j += 1;
                    break;
                }
                _ => {
                    body.push(tokens[j].clone());
                    j += 1;
                }
            }
        }
        if self.active() {
            self.macros.insert(name, Macro { params, body });
        }
        Ok(j)
    }

    fn dir_undef(&mut self, tokens: &[Lexed], i: usize) -> Result<usize, String> {
        let (name, _) = expect_ident(tokens, i + 1)?;
        if self.active() {
            self.macros.remove(&name);
        }
        Ok(skip_to_newline(tokens, i + 1))
    }

    fn dir_include(&mut self, tokens: &[Lexed], i: usize) -> Result<usize, String> {
        let next = skip_to_newline(tokens, i + 1);
        if !self.active() {
            return Ok(next);
        }
        let path_tok = tokens.get(i + 1).ok_or("`include missing path")?;
        let raw = match &path_tok.tok {
            Tok::Str(s) => s.trim_matches('"').to_string(),
            _ => return Err("`include expects a quoted path".into()),
        };
        self.include_file(&raw)?;
        Ok(next)
    }

    fn include_file(&mut self, rel: &str) -> Result<(), String> {
        if self.include_depth > 64 {
            return Err("`include nested too deep".into());
        }
        // C-style standard headers map onto the bundled Verilog-AMS headers.
        let base = Path::new(rel).file_name().and_then(|s| s.to_str()).unwrap_or(rel);
        let candidates: &[&str] = match base {
            "discipline.h" | "disciplines.h" => &["disciplines.vams"],
            "constants.h" => &["constants.vams"],
            _ => &[],
        };
        let resolved = self
            .include_dirs
            .iter()
            .map(|d| d.join(rel))
            .find(|p| p.is_file())
            .or_else(|| {
                candidates.iter().find_map(|c| {
                    self.include_dirs.iter().map(|d| d.join(c)).find(|p| p.is_file())
                })
            })
            .ok_or_else(|| format!("`include: cannot find \"{rel}\""))?;
        let text = std::fs::read_to_string(&resolved)
            .map_err(|e| format!("`include {}: {e}", resolved.display()))?;
        let toks = tokenize(&text).map_err(|e| format!("{}: {e}", resolved.display()))?;
        // Search the included file's own directory first for nested includes.
        let dir = resolved.parent().map(Path::to_path_buf);
        let pushed = match dir {
            Some(d) if !self.include_dirs.contains(&d) => {
                self.include_dirs.insert(0, d);
                true
            }
            _ => false,
        };
        self.include_depth += 1;
        let res = self.process(&toks);
        self.include_depth -= 1;
        if pushed {
            self.include_dirs.remove(0);
        }
        res
    }

    fn dir_ifdef(&mut self, tokens: &[Lexed], i: usize, negate: bool) -> Result<usize, String> {
        let (name, _) = expect_ident(tokens, i + 1)?;
        let parent_active = self.active();
        let mut cond = self.macros.contains_key(&name);
        if negate {
            cond = !cond;
        }
        let active = parent_active && cond;
        self.conds.push(CondFrame { active, taken: active, parent_active });
        Ok(skip_to_newline(tokens, i + 1))
    }

    fn dir_else(&mut self, tokens: &[Lexed], i: usize) -> Result<usize, String> {
        let frame = self.conds.last_mut().ok_or("`else without `ifdef")?;
        frame.active = frame.parent_active && !frame.taken;
        frame.taken = true;
        Ok(skip_to_newline(tokens, i + 1))
    }

    fn dir_endif(&mut self, tokens: &[Lexed], i: usize) -> Result<usize, String> {
        self.conds.pop().ok_or("`endif without `ifdef")?;
        Ok(skip_to_newline(tokens, i + 1))
    }

    // ── macro expansion ─────────────────────────────────────────────────

    fn expand_use(
        &self,
        tokens: &[Lexed],
        i: usize,
        name: &str,
    ) -> Result<(Vec<Lexed>, usize), String> {
        let mac = self
            .macros
            .get(name)
            .ok_or_else(|| format!("undefined macro `{name}"))?;
        let mut next = i + 1;

        let args: Vec<Vec<Lexed>> = if mac.params.is_some() {
            // consume adjacent argument list, skipping newlines
            let mut k = next;
            while matches!(tokens.get(k).map(|t| &t.tok), Some(Tok::Newline | Tok::Backslash)) {
                k += 1;
            }
            if !matches!(tokens.get(k).map(|t| &t.tok), Some(Tok::LParen)) {
                return Err(format!("macro `{name} used without arguments"));
            }
            let (args, end) = parse_args(tokens, k)?;
            next = end;
            args
        } else {
            Vec::new()
        };

        let mut active = HashSet::new();
        let expanded = self.expand_macro(name, mac, &args, &tokens[i], &mut active, 0)?;
        Ok((expanded, next))
    }

    /// Expand a single macro invocation. Arguments are substituted into the body
    /// raw, then the whole result is rescanned (CPP-style), so macro uses that
    /// arrive via arguments — `` `M(`other) `` — are expanded too.
    fn expand_macro(
        &self,
        name: &str,
        mac: &Macro,
        args: &[Vec<Lexed>],
        site: &Lexed,
        active: &mut HashSet<String>,
        depth: usize,
    ) -> Result<Vec<Lexed>, String> {
        if depth > 200 {
            return Err(format!("macro expansion of `{name} too deep"));
        }
        let mut subst: HashMap<&str, &Vec<Lexed>> = HashMap::new();
        if let Some(params) = &mac.params {
            if params.len() != args.len() {
                return Err(format!(
                    "macro `{name} expects {} args, got {}",
                    params.len(),
                    args.len()
                ));
            }
            for (p, a) in params.iter().zip(args) {
                subst.insert(p.as_str(), a);
            }
        }

        // substitute params into the body
        let mut sub = Vec::with_capacity(mac.body.len());
        for bt in &mac.body {
            match &bt.tok {
                Tok::Ident(id) if subst.contains_key(id.as_str()) => {
                    for a in subst[id.as_str()] {
                        sub.push(respan(a, site));
                    }
                }
                _ => sub.push(respan(bt, site)),
            }
        }

        active.insert(name.to_string());
        let out = self.expand_seq(&sub, active, depth + 1);
        active.remove(name);
        out
    }

    /// Scan a token sequence and expand every macro use it contains.
    fn expand_seq(
        &self,
        toks: &[Lexed],
        active: &mut HashSet<String>,
        depth: usize,
    ) -> Result<Vec<Lexed>, String> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < toks.len() {
            match &toks[i].tok {
                Tok::Newline | Tok::Backslash => i += 1,
                Tok::Tick(name) if active.contains(name) => {
                    // self-referential use: emit literally to break the cycle
                    out.push(Lexed {
                        tok: Tok::Ident(name.clone()),
                        start: toks[i].start,
                        end: toks[i].end,
                    });
                    i += 1;
                }
                Tok::Tick(name) => {
                    let mac = self
                        .macros
                        .get(name)
                        .ok_or_else(|| format!("undefined macro `{name}"))?;
                    let (args, next) = if mac.params.is_some() {
                        let mut k = i + 1;
                        while matches!(
                            toks.get(k).map(|t| &t.tok),
                            Some(Tok::Newline | Tok::Backslash)
                        ) {
                            k += 1;
                        }
                        if matches!(toks.get(k).map(|t| &t.tok), Some(Tok::LParen)) {
                            let (a, e) = parse_args(toks, k)?;
                            (a, e)
                        } else {
                            (Vec::new(), i + 1)
                        }
                    } else {
                        (Vec::new(), i + 1)
                    };
                    let exp = self.expand_macro(name, mac, &args, &toks[i], active, depth)?;
                    out.extend(exp);
                    i = next;
                }
                _ => {
                    out.push(toks[i].clone());
                    i += 1;
                }
            }
        }
        Ok(out)
    }
}

// ── helpers ─────────────────────────────────────────────────────────────

fn respan(t: &Lexed, site: &Lexed) -> Lexed {
    Lexed { tok: t.tok.clone(), start: site.start, end: site.end }
}

fn expect_ident(tokens: &[Lexed], i: usize) -> Result<(String, usize), String> {
    match tokens.get(i).map(|t| (&t.tok, t.end)) {
        Some((Tok::Ident(s), end)) => Ok((s.clone(), end)),
        _ => Err("expected identifier".into()),
    }
}

fn skip_to_newline(tokens: &[Lexed], mut i: usize) -> usize {
    while i < tokens.len() {
        match &tokens[i].tok {
            Tok::Backslash => {
                i += 1;
                if matches!(tokens.get(i).map(|t| &t.tok), Some(Tok::Newline)) {
                    i += 1;
                }
            }
            Tok::Newline => return i + 1,
            _ => i += 1,
        }
    }
    i
}

/// Parse a parenthesised, comma-separated argument list. `i` points at `(`.
/// Returns the argument token lists and the index just past the closing `)`.
fn parse_args(tokens: &[Lexed], i: usize) -> Result<(Vec<Vec<Lexed>>, usize), String> {
    debug_assert_eq!(tokens[i].tok, Tok::LParen);
    let mut depth = 0i32;
    let mut args: Vec<Vec<Lexed>> = Vec::new();
    let mut cur: Vec<Lexed> = Vec::new();
    let mut j = i;
    while j < tokens.len() {
        let t = &tokens[j];
        match &t.tok {
            Tok::LParen | Tok::LBrack | Tok::LBrace => {
                depth += 1;
                if depth > 1 {
                    cur.push(t.clone());
                }
            }
            Tok::RParen | Tok::RBrack | Tok::RBrace => {
                depth -= 1;
                if depth == 0 {
                    if !cur.is_empty() || !args.is_empty() {
                        args.push(std::mem::take(&mut cur));
                    }
                    return Ok((args, j + 1));
                }
                cur.push(t.clone());
            }
            Tok::Comma if depth == 1 => {
                args.push(std::mem::take(&mut cur));
            }
            Tok::Newline | Tok::Backslash => {}
            _ => cur.push(t.clone()),
        }
        j += 1;
    }
    Err("unterminated macro argument list".into())
}
