//! Preprocessor for LRM chapter 10 (compiler directives). Written from
//! `VAMS-LRM-2-4.md` §10 only (no third-party compiler consulted —
//! `tests/purity.rs` R1 gate). Runs on raw source text, before the lexer
//! (design.md §4.1's `Preprocessor -> Lexer -> Grammar` pipeline order):
//! macro definition/expansion and conditional compilation are textual, not
//! part of the token grammar.
//!
//! Scope (tasks.md T5's stated set): `` `define ``/`` `undef `` (incl.
//! macro arguments and CPP-style rescan expansion), the `` `ifdef ``/
//! `` `ifndef ``/`` `else ``/`` `elsif ``/`` `endif `` conditional family,
//! `` `include ``, `` `timescale ``, `` `resetall ``. Directives outside
//! this set (`` `default_discipline ``, `` `celldefine ``, `` `pragma ``,
//! ...) are language-semantic, not textual-substitution, directives — they
//! pass through untouched for the grammar layer (T9+) to handle.
//!
//! This crate performs no filesystem I/O (purity contract): `` `include ``
//! resolution is delegated to a caller-supplied [`IncludeResolver`].

use std::collections::HashMap;

use crate::error::{ParseError, ParseErrorKind, Span};

/// Resolves an `` `include "path" `` request to file contents. Returning
/// `None` becomes a typed [`ParseErrorKind::IncludeNotFound`] naming the
/// path and the including span — this crate never touches the filesystem
/// itself.
pub trait IncludeResolver {
    fn resolve(&self, path: &str) -> Option<String>;
}

/// An [`IncludeResolver`] that resolves nothing. The default for a
/// preprocessor run over a single self-contained source string.
pub struct NoIncludes;

impl IncludeResolver for NoIncludes {
    fn resolve(&self, _path: &str) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone)]
struct MacroDef {
    /// `None` for an object-like macro (`` `define WORDSIZE 8 ``); `Some`
    /// for a function-like macro (`` `define ADD(a,b) (a+b) ``) — LRM
    /// distinguishes the two by whether `(` immediately follows the name
    /// with no intervening white space (§10.4/1364-2005).
    params: Option<Vec<String>>,
    body: String,
}

/// One open `` `ifdef ``/`` `ifndef `` frame.
struct CondFrame {
    /// The directive that opened this frame (`"ifdef"`/`"ifndef"`),
    /// carried for the "never closed" diagnostic.
    opener: String,
    /// Whether the *current* branch of this frame is the one being taken.
    branch_active: bool,
    /// Whether some branch of this frame has already matched — a second
    /// `` `elsif `` after a match takes the "already satisfied" path
    /// (deactivated) rather than re-testing its condition.
    already_taken: bool,
}

pub struct Preprocessor {
    resolver: Box<dyn IncludeResolver>,
    macros: HashMap<String, MacroDef>,
}

impl Default for Preprocessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Preprocessor {
    pub fn new() -> Self {
        Self { resolver: Box::new(NoIncludes), macros: HashMap::new() }
    }

    pub fn with_include_resolver(resolver: impl IncludeResolver + 'static) -> Self {
        Self { resolver: Box::new(resolver), macros: HashMap::new() }
    }

    /// Runs the full preprocessing pass over `source`, returning the
    /// expanded, directive-free (for the directives in scope) text ready
    /// for the lexer.
    pub fn run(&mut self, source: &str) -> Result<String, ParseError> {
        let mut out = String::new();
        let mut cond_stack: Vec<CondFrame> = Vec::new();
        let mut include_stack: Vec<String> = Vec::new();
        self.process(source, &mut out, &mut cond_stack, &mut include_stack, &mut Vec::new())?;
        if let Some(frame) = cond_stack.pop() {
            return Err(ParseError::new(
                ParseErrorKind::UnterminatedConditional { directive: frame.opener },
                Span::new(source.len(), source.len()),
            ));
        }
        Ok(out)
    }

    /// Scans `source`, appending emitted text to `out`. `expanding` tracks
    /// macro names currently being expanded on the call stack (rescan
    /// self-reference guard): a macro that invokes itself, directly or
    /// indirectly, is emitted literally at the point of self-reference
    /// instead of expanding forever.
    #[allow(clippy::too_many_arguments)]
    fn process(
        &mut self,
        source: &str,
        out: &mut String,
        cond_stack: &mut Vec<CondFrame>,
        include_stack: &mut Vec<String>,
        expanding: &mut Vec<String>,
    ) -> Result<(), ParseError> {
        let bytes = source.as_bytes();
        let mut pos = 0usize;
        let active = |stack: &[CondFrame]| stack.iter().all(|f| f.branch_active);

        while pos < bytes.len() {
            match bytes[pos] {
                b'"' => {
                    let end = scan_string(source, pos);
                    if active(cond_stack) {
                        out.push_str(&source[pos..end]);
                    }
                    pos = end;
                }
                b'/' if bytes.get(pos + 1) == Some(&b'/') => {
                    let end = scan_line_comment(source, pos);
                    if active(cond_stack) {
                        out.push_str(&source[pos..end]);
                    }
                    pos = end;
                }
                b'/' if bytes.get(pos + 1) == Some(&b'*') => {
                    let end = scan_block_comment(source, pos);
                    if active(cond_stack) {
                        out.push_str(&source[pos..end]);
                    }
                    pos = end;
                }
                b'`' => {
                    let (name, name_end) = scan_directive_name(source, pos);
                    if name.is_empty() {
                        // A lone backtick with no identifier following is
                        // not this module's concern (a lexical error the
                        // lexer will raise); pass it through.
                        if active(cond_stack) {
                            out.push('`');
                        }
                        pos += 1;
                        continue;
                    }
                    pos = self.handle_directive_or_macro(
                        source,
                        pos,
                        name_end,
                        &name,
                        out,
                        cond_stack,
                        include_stack,
                        expanding,
                    )?;
                }
                _ => {
                    let ch_len = source[pos..].chars().next().map(char::len_utf8).unwrap_or(1);
                    if active(cond_stack) {
                        out.push_str(&source[pos..pos + ch_len]);
                    }
                    pos += ch_len;
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_directive_or_macro(
        &mut self,
        source: &str,
        directive_start: usize,
        name_end: usize,
        name: &str,
        out: &mut String,
        cond_stack: &mut Vec<CondFrame>,
        include_stack: &mut Vec<String>,
        expanding: &mut Vec<String>,
    ) -> Result<usize, ParseError> {
        let active_now = cond_stack.iter().all(|f| f.branch_active);

        match name {
            "ifdef" | "ifndef" => {
                let (arg, after) = scan_identifier_after_ws(source, name_end);
                let cond = self.macros.contains_key(&arg) == (name == "ifdef");
                // Only meaningful when the enclosing scope is active — a
                // nested `ifdef` inside a skipped branch still needs to be
                // pushed (to balance nesting) but never becomes active.
                let branch_active = active_now && cond;
                cond_stack.push(CondFrame {
                    opener: name.to_string(),
                    branch_active,
                    already_taken: branch_active,
                });
                Ok(after)
            }
            "elsif" => {
                let (arg, after) = scan_identifier_after_ws(source, name_end);
                if cond_stack.is_empty() {
                    return Err(ParseError::new(
                        ParseErrorKind::UnmatchedConditionalDirective { directive: "elsif".to_string() },
                        Span::new(directive_start, name_end),
                    ));
                }
                let last = cond_stack.len() - 1;
                let parent_active = cond_stack[..last].iter().all(|f| f.branch_active);
                let frame = &mut cond_stack[last];
                if frame.already_taken {
                    frame.branch_active = false;
                } else {
                    let cond = parent_active && self.macros.contains_key(&arg);
                    frame.branch_active = cond;
                    frame.already_taken = cond;
                }
                Ok(after)
            }
            "else" => {
                let Some(frame) = cond_stack.last_mut() else {
                    return Err(ParseError::new(
                        ParseErrorKind::UnmatchedConditionalDirective { directive: "else".to_string() },
                        Span::new(directive_start, name_end),
                    ));
                };
                frame.branch_active = !frame.already_taken;
                frame.already_taken = true;
                Ok(name_end)
            }
            "endif" => {
                if cond_stack.pop().is_none() {
                    return Err(ParseError::new(
                        ParseErrorKind::UnmatchedConditionalDirective { directive: "endif".to_string() },
                        Span::new(directive_start, name_end),
                    ));
                }
                Ok(name_end)
            }
            "define" if active_now => self.handle_define(source, name_end),
            "define" => Ok(skip_to_end_of_logical_line(source, name_end)),
            "undef" if active_now => {
                let (arg, after) = scan_identifier_after_ws(source, name_end);
                self.macros.remove(&arg);
                Ok(after)
            }
            "undef" => Ok(skip_to_end_of_logical_line(source, name_end)),
            "include" if active_now => {
                self.handle_include(source, directive_start, name_end, out, include_stack, expanding)
            }
            "include" => Ok(skip_to_end_of_logical_line(source, name_end)),
            "timescale" | "resetall" if active_now => {
                // Textually inert for this preprocessor's purposes — LRM
                // §10 places these among the 1364-2005-inherited
                // directives with no macro-substitution role. They pass
                // through as directive text for the grammar layer.
                out.push_str(&source[directive_start..name_end]);
                Ok(name_end)
            }
            "timescale" | "resetall" => Ok(name_end),
            _ if active_now => {
                self.handle_macro_invocation(source, directive_start, name_end, name, out, expanding)
            }
            _ => Ok(skip_macro_invocation_args(source, name_end)),
        }
    }

    /// `` `define NAME body `` or `` `define NAME(p1,p2) body ``. The body
    /// runs to end of logical line (a trailing `\` continues onto the next
    /// physical line).
    fn handle_define(&mut self, source: &str, after_define: usize) -> Result<usize, ParseError> {
        let (name, after_name) = scan_identifier_after_ws(source, after_define);
        let bytes = source.as_bytes();
        let (params, after_params) = if bytes.get(after_name) == Some(&b'(') {
            scan_param_list(source, after_name)
        } else {
            (None, after_name)
        };
        let (body, after_body) = scan_macro_body(source, after_params);
        self.macros.insert(name, MacroDef { params, body });
        Ok(after_body)
    }

    /// LRM §10.1: a compiler directive's scope extends "across all files
    /// processed". This implementation scopes `` `ifdef ``/`` `endif ``
    /// nesting to the file/macro-expansion unit it opened in (each
    /// `` `include `` gets its own conditional stack) — a deliberate,
    /// documented simplification: conditionals spanning an include
    /// boundary are out of this task's scope (not required by tasks.md
    /// T5's "Done when" list). `` `define ``/`` `undef `` state (the
    /// `macros` map) *does* cross file boundaries correctly, since it is
    /// `&mut self` state shared across the whole run.
    fn handle_include(
        &mut self,
        source: &str,
        directive_start: usize,
        after_include: usize,
        out: &mut String,
        include_stack: &mut Vec<String>,
        expanding: &mut Vec<String>,
    ) -> Result<usize, ParseError> {
        let (path, after_path) = scan_quoted_path(source, after_include);
        if include_stack.iter().any(|p| p == &path) {
            return Err(ParseError::new(
                ParseErrorKind::RecursiveInclude { path },
                Span::new(directive_start, after_path),
            ));
        }
        let Some(contents) = self.resolver.resolve(&path) else {
            return Err(ParseError::new(
                ParseErrorKind::IncludeNotFound { path },
                Span::new(directive_start, after_path),
            ));
        };
        include_stack.push(path);
        let mut cond_stack = Vec::new();
        self.process(&contents, out, &mut cond_stack, include_stack, expanding)?;
        if let Some(frame) = cond_stack.pop() {
            include_stack.pop();
            return Err(ParseError::new(
                ParseErrorKind::UnterminatedConditional { directive: frame.opener },
                Span::new(after_path, after_path),
            ));
        }
        include_stack.pop();
        Ok(after_path)
    }

    /// Expands a `` `NAME `` or `` `NAME(args) `` macro invocation,
    /// rescanning the expansion (CPP-style) so nested macro references
    /// inside the body are themselves expanded.
    fn handle_macro_invocation(
        &mut self,
        source: &str,
        directive_start: usize,
        name_end: usize,
        name: &str,
        out: &mut String,
        expanding: &mut Vec<String>,
    ) -> Result<usize, ParseError> {
        let Some(def) = self.macros.get(name).cloned() else {
            return Err(ParseError::new(
                ParseErrorKind::UndefinedMacro { name: name.to_string() },
                Span::new(directive_start, name_end),
            ));
        };

        let (args, after) = match &def.params {
            None => (Vec::new(), name_end),
            Some(_) => scan_call_args(source, name_end),
        };

        let expanded = if let Some(params) = &def.params {
            if args.len() != params.len() {
                return Err(ParseError::new(
                    ParseErrorKind::MacroArityMismatch {
                        name: name.to_string(),
                        expected: params.len(),
                        found: args.len(),
                    },
                    Span::new(directive_start, after),
                ));
            }
            substitute_params(&def.body, params, &args)
        } else {
            def.body.clone()
        };

        if expanding.contains(&name.to_string()) {
            // Self-reference during expansion: emit literally rather than
            // recursing forever. This bounds `define` recursion; the
            // recursive-*include* case (a distinct, spec-required error)
            // is handled separately in `handle_include`.
            out.push_str(&expanded);
            return Ok(after);
        }

        expanding.push(name.to_string());
        let mut cond_stack = Vec::new();
        let mut include_stack = Vec::new();
        self.process(&expanded, out, &mut cond_stack, &mut include_stack, expanding)?;
        expanding.pop();
        Ok(after)
    }
}

// ── Character-level scanning helpers ────────────────────────────────────
// Free functions (not methods): each is a pure function of a source slice
// and a starting position, with no `Preprocessor` state to operate on.

fn scan_string(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut pos = start + 1;
    while pos < bytes.len() {
        match bytes[pos] {
            b'\\' => pos += 2,
            b'"' => return pos + 1,
            b'\n' => return pos,
            _ => pos += 1,
        }
    }
    pos
}

fn scan_line_comment(source: &str, start: usize) -> usize {
    source[start..].find('\n').map(|i| start + i).unwrap_or(source.len())
}

fn scan_block_comment(source: &str, start: usize) -> usize {
    source[start + 2..]
        .find("*/")
        .map(|i| start + 2 + i + 2)
        .unwrap_or(source.len())
}

/// Reads the identifier immediately following a `` ` `` at `backtick_pos`.
/// Returns `("", backtick_pos + 1)` if no identifier character follows.
fn scan_directive_name(source: &str, backtick_pos: usize) -> (String, usize) {
    scan_bare_identifier(source, backtick_pos + 1)
}

/// Skips leading spaces/tabs (not newlines — a directive's argument must
/// stay on the same physical line).
fn skip_inline_ws(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut pos = start;
    while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t') {
        pos += 1;
    }
    pos
}

/// Skips leading white space, then reads an identifier — used for a
/// directive's argument (`` `ifdef NAME``, `` `define NAME``), where LRM
/// syntax requires white space between the directive keyword and its
/// argument. Contrast with [`scan_bare_identifier`], used where no
/// preceding white space is expected (immediately after `` ` ``).
fn scan_identifier_after_ws(source: &str, start: usize) -> (String, usize) {
    scan_bare_identifier(source, skip_inline_ws(source, start))
}

fn scan_bare_identifier(source: &str, start: usize) -> (String, usize) {
    let bytes = source.as_bytes();
    let mut pos = start;
    while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_' || bytes[pos] == b'$') {
        pos += 1;
    }
    (source[start..pos].to_string(), pos)
}

/// After `` `define NAME``, with no space before `(` — a function-like
/// macro's parameter list.
fn scan_param_list(source: &str, open_paren: usize) -> (Option<Vec<String>>, usize) {
    let bytes = source.as_bytes();
    let mut pos = open_paren + 1;
    let mut params = Vec::new();
    loop {
        while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t') {
            pos += 1;
        }
        if bytes.get(pos) == Some(&b')') {
            pos += 1;
            break;
        }
        let (name, after) = scan_bare_identifier(source, pos);
        if !name.is_empty() {
            params.push(name);
        }
        pos = after;
        while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t') {
            pos += 1;
        }
        match bytes.get(pos) {
            Some(b',') => pos += 1,
            Some(b')') => {
                pos += 1;
                break;
            }
            _ => break,
        }
    }
    (Some(params), pos)
}

/// Skips leading white space (not newlines) after a directive/macro name,
/// then reads to end of logical line — a trailing `\` before the newline
/// continues the body onto the next physical line (line continuation).
fn scan_macro_body(source: &str, start: usize) -> (String, usize) {
    let bytes = source.as_bytes();
    let mut pos = start;
    while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t') {
        pos += 1;
    }
    let mut body = String::new();
    while pos < bytes.len() {
        match bytes[pos] {
            b'\\' if bytes.get(pos + 1) == Some(&b'\n') => {
                body.push('\n');
                pos += 2;
            }
            b'\n' => break,
            _ => {
                let ch_len = source[pos..].chars().next().map(char::len_utf8).unwrap_or(1);
                body.push_str(&source[pos..pos + ch_len]);
                pos += ch_len;
            }
        }
    }
    (body.trim().to_string(), pos)
}

fn scan_quoted_path(source: &str, start: usize) -> (String, usize) {
    let bytes = source.as_bytes();
    let mut pos = start;
    while pos < bytes.len() && bytes[pos] != b'"' {
        pos += 1;
    }
    if pos >= bytes.len() {
        return (String::new(), pos);
    }
    let quote_start = pos + 1;
    pos += 1;
    while pos < bytes.len() && bytes[pos] != b'"' {
        pos += 1;
    }
    let path = source[quote_start..pos].to_string();
    (path, (pos + 1).min(source.len()))
}

/// Parses a balanced-parens, comma-separated argument list for a
/// function-like macro invocation `` `NAME(a, b) ``.
fn scan_call_args(source: &str, name_end: usize) -> (Vec<String>, usize) {
    let bytes = source.as_bytes();
    let mut pos = name_end;
    while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t') {
        pos += 1;
    }
    if bytes.get(pos) != Some(&b'(') {
        return (Vec::new(), name_end);
    }
    pos += 1;
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    loop {
        match bytes.get(pos) {
            None => break,
            Some(b')') if depth == 0 => {
                pos += 1;
                break;
            }
            Some(b',') if depth == 0 => {
                args.push(current.trim().to_string());
                current.clear();
                pos += 1;
            }
            Some(b'(') => {
                depth += 1;
                current.push('(');
                pos += 1;
            }
            Some(b')') => {
                depth -= 1;
                current.push(')');
                pos += 1;
            }
            Some(_) => {
                let ch_len = source[pos..].chars().next().map(char::len_utf8).unwrap_or(1);
                current.push_str(&source[pos..pos + ch_len]);
                pos += ch_len;
            }
        }
    }
    if !current.trim().is_empty() || !args.is_empty() {
        args.push(current.trim().to_string());
    }
    (args, pos)
}

/// Used when a macro invocation appears inside a skipped (inactive)
/// conditional branch: its arguments still need to be consumed so scanning
/// resumes at the right position, without expanding or validating them.
fn skip_macro_invocation_args(source: &str, name_end: usize) -> usize {
    scan_call_args(source, name_end).1
}

fn skip_to_end_of_logical_line(source: &str, start: usize) -> usize {
    scan_macro_body(source, start).1
}

/// Textual substitution of `params[i]` with `args[i]` inside `body` —
/// whole-identifier matches only (a parameter name that is a substring of
/// a longer identifier in the body is never substituted).
fn substitute_params(body: &str, params: &[String], args: &[String]) -> String {
    let mut out = String::new();
    let bytes = body.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() {
        if bytes[pos].is_ascii_alphabetic() || bytes[pos] == b'_' {
            let (word, after) = scan_bare_identifier(body, pos);
            if let Some(i) = params.iter().position(|p| p == &word) {
                out.push_str(&args[i]);
            } else {
                out.push_str(&word);
            }
            pos = after;
        } else {
            let ch_len = body[pos..].chars().next().map(char::len_utf8).unwrap_or(1);
            out.push_str(&body[pos..pos + ch_len]);
            pos += ch_len;
        }
    }
    out
}
