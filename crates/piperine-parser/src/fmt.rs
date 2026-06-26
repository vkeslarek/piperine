use crate::lexer::{Lexed, Tok};

pub struct FormatOptions {
    pub indent_string: String,
    pub line_ending: String,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_string: "    ".to_string(),
            line_ending: "\n".to_string(),
        }
    }
}

pub struct FormatState {
    pub indent_level: usize,
    pub paren_depth: usize,
    pub at_line_start: bool,
    pub in_directive_line: bool,
    pub options: FormatOptions,
}

impl FormatState {
    pub fn push_newline(&mut self, output: &mut String) {
        if self.in_directive_line {
            // Do not auto-break lines inside compiler directives (let original newlines dictate)
            return;
        }
        self.force_newline(output);
    }

    pub fn force_newline(&mut self, output: &mut String) {
        output.push_str(&self.options.line_ending);
        self.at_line_start = true;
    }
}

pub trait FormatRule {
    /// Return true to consume the token completely and `continue` to the next token (e.g. original Newline).
    fn consume_token(&mut self, _t: &Lexed, _prev: Option<&Lexed>, _state: &mut FormatState, _output: &mut String) -> bool { false }

    fn before_token(&mut self, _t: &Lexed, _state: &mut FormatState, _output: &mut String) {}
    
    fn space_after(&self, _prev: Option<&Lexed>, _t: &Lexed, _next: Option<&Lexed>, _state: &FormatState) -> Option<bool> { None }
    
    fn after_token(&mut self, _t: &Lexed, _state: &mut FormatState, _output: &mut String) {}
}

// -----------------------------------------------------------------------------
// RULES IMPLEMENTATION
// -----------------------------------------------------------------------------

/// BlankLineRule
/// 
/// Purpose: Preserves intentional visual spacing while ignoring arbitrary whitespace.
/// - In Verilog, visual flow is important (e.g. blank lines separating logical blocks).
/// - By default, this rule discards consecutive newlines to let the formatter take control.
/// - However, if it detects two consecutive newlines in the original source, it preserves
///   exactly ONE blank line (preventing excessive blank lines).
/// - It also gracefully handles the end of compiler directives by inserting a newline.
struct BlankLineRule;
impl FormatRule for BlankLineRule {
    fn consume_token(&mut self, t: &Lexed, prev: Option<&Lexed>, state: &mut FormatState, output: &mut String) -> bool {
        if t.tok == Tok::Newline {
            if state.in_directive_line {
                state.force_newline(output);
                
                // Clear directive mode unless we hit a line continuation `\`
                if prev.map_or(true, |p| p.tok != Tok::Backslash) {
                    state.in_directive_line = false;
                }
                return true;
            }

            if let Some(p) = prev {
                if matches!(p.tok, Tok::LineComment) {
                    state.force_newline(output);
                    return true;
                }
                if matches!(p.tok, Tok::BlockComment) {
                    state.force_newline(output);
                    return true;
                }
                
                if p.tok == Tok::Newline {
                    let double_newline = format!("{}{}", state.options.line_ending, state.options.line_ending);
                    if !output.ends_with(&double_newline) {
                        state.force_newline(output);
                    }
                }
            }
            return true;
        }
        false
    }
}

/// ParenthesisRule
/// 
/// Purpose: Tracks the depth of nested parentheses `()`.
/// - Keeping track of parenthesis depth allows other rules (like SpacingRule) to 
///   behave differently when inside mathematical expressions or `for` loop headers 
///   (where semicolons shouldn't break the line).
struct ParenthesisRule;
impl FormatRule for ParenthesisRule {
    fn before_token(&mut self, t: &Lexed, state: &mut FormatState, _output: &mut String) {
        match t.tok {
            Tok::LParen => state.paren_depth += 1,
            Tok::RParen => state.paren_depth = state.paren_depth.saturating_sub(1),
            _ => {}
        }
    }
}

/// BlockRule
/// 
/// Purpose: Handles block indentation and structural line breaks.
/// - Ensures that keywords like `module`, `analog`, `always`, and variable 
///   declarations start on a fresh line.
/// - Increases the `indent_level` when entering scopes (`begin`, `case`, `module`).
/// - Decreases the `indent_level` and inserts line breaks when exiting scopes (`end`).
struct BlockRule;
impl FormatRule for BlockRule {
    fn before_token(&mut self, t: &Lexed, state: &mut FormatState, output: &mut String) {
        if let Tok::Ident(s) = &t.tok {
            // Line break before certain keywords
            if !state.at_line_start && matches!(s.as_str(), "module" | "connectmodule" | "connectrules" | "electrical" | "logic" | "reg" | "wire" | "parameter" | "localparam" | "input" | "output" | "inout" | "analog" | "always" | "initial" | "assign") {
                state.push_newline(output);
            }
            
            // Reduce indent level before end blocks
            if matches!(s.as_str(), "endmodule" | "endconnectmodule" | "endconnectrules" | "end" | "endtask" | "endfunction" | "endcase" | "endgenerate" | "endnature" | "enddiscipline") {
                state.indent_level = state.indent_level.saturating_sub(1);
            }
        }
    }

    fn after_token(&mut self, t: &Lexed, state: &mut FormatState, output: &mut String) {
        if let Tok::Ident(s) = &t.tok {
            match s.as_str() {
                "begin" | "case" | "generate" => {
                    state.indent_level += 1;
                    state.push_newline(output);
                }
                "module" | "connectmodule" | "connectrules" | "task" | "function" | "nature" | "discipline" => {
                    state.indent_level += 1;
                }
                "endmodule" | "endconnectmodule" | "endconnectrules" | "end" | "endtask" | "endfunction" | "endcase" | "endgenerate" | "endnature" | "enddiscipline" => {
                    state.push_newline(output);
                }
                _ => {}
            }
        }
    }
}

/// DirectiveRule
/// 
/// Purpose: Formats C-style preprocessor directives (`ifdef`, `define`, `timescale`).
/// - Directives are line-oriented, so this rule ensures they always start on a new line.
/// - Increases indentation inside `ifdef` / `ifndef` blocks to visually nest code.
/// - Sets `in_directive_line = true` so the `BlankLineRule` can properly terminate the 
///   directive when it spots the original end-of-line.
struct DirectiveRule;
impl FormatRule for DirectiveRule {
    fn before_token(&mut self, t: &Lexed, state: &mut FormatState, output: &mut String) {
        if let Tok::Tick(s) = &t.tok {
            // Line break before directive
            if !state.at_line_start && matches!(s.as_str(), "ifdef" | "ifndef" | "else" | "elsif" | "endif" | "define" | "undef" | "include" | "timescale" | "default_nettype") {
                state.push_newline(output);
            }
            // Reduce indent level before else/endif
            if matches!(s.as_str(), "else" | "elsif" | "endif") {
                state.indent_level = state.indent_level.saturating_sub(1);
            }
        }
    }
    
    fn after_token(&mut self, t: &Lexed, state: &mut FormatState, _output: &mut String) {
        if let Tok::Tick(s) = &t.tok {
            state.in_directive_line = true;
            if matches!(s.as_str(), "ifdef" | "ifndef" | "else" | "elsif") {
                state.indent_level += 1;
            }
        }
    }
}

/// SpacingRule
/// 
/// Purpose: Fine-tunes micro-spacing between individual tokens.
/// - Determines whether a space should be placed around operators (`+`, `-`, `=`, etc.).
/// - Removes spaces around punctuation (dots, commas, closing parenthesis).
/// - Eliminates spaces before `(` in function calls (e.g. `V(xt1)`), while maintaining 
///   spaces for control structures like `if (cond)`.
/// - Differentiates between unary operators (e.g. `-psi`) and binary operators, removing 
///   the trailing space for unary variants.
/// - Handles line-breaks after semicolons `;` (except when inside a `for` loop).
struct SpacingRule;
impl FormatRule for SpacingRule {
    fn space_after(&self, prev: Option<&Lexed>, t: &Lexed, next: Option<&Lexed>, _state: &FormatState) -> Option<bool> {
        if let Some(nxt) = next {
            match nxt.tok {
                Tok::Semi | Tok::Comma | Tok::RParen | Tok::Dot | Tok::Newline => return Some(false),
                Tok::LParen => {
                    if let Tok::Ident(s) = &t.tok {
                        // Do not put space before parenthesis for functions/instances, but keep it for if, for, etc.
                        if !matches!(s.as_str(), "if" | "for" | "while" | "case" | "casez" | "casex") {
                            return Some(false);
                        }
                    } else if matches!(t.tok, Tok::At | Tok::SysCall(_) | Tok::Tick(_)) {
                        return Some(false);
                    }
                }
                _ => {}
            }
            match t.tok {
                Tok::LParen | Tok::Dot | Tok::At | Tok::Tilde | Tok::Not => return Some(false),
                Tok::Minus | Tok::Plus => {
                    // Check if it's unary
                    let is_unary = match prev {
                        None => true,
                        Some(p) => {
                            if matches!(p.tok, Tok::LParen | Tok::Comma | Tok::Assign | Tok::EqEq | Tok::NotEq | Tok::Lt | Tok::Gt | Tok::Le | Tok::Ge | Tok::Plus | Tok::Minus | Tok::Star | Tok::Slash | Tok::Percent | Tok::Amp | Tok::AmpAmp | Tok::Pipe | Tok::PipePipe | Tok::Caret | Tok::Colon | Tok::Question | Tok::Contrib) {
                                true
                            } else if let Tok::Ident(s) = &p.tok {
                                matches!(s.as_str(), "return" | "assign" | "if" | "while" | "case" | "casez" | "casex")
                            } else {
                                false
                            }
                        }
                    };
                    if is_unary {
                        return Some(false);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn after_token(&mut self, t: &Lexed, state: &mut FormatState, output: &mut String) {
        if let Tok::Semi = &t.tok {
            if state.paren_depth == 0 {
                state.push_newline(output);
            } else {
                output.push(' ');
            }
        }
    }
}

/// CommentRule
/// 
/// Purpose: Ensures that comments (LineComment and BlockComment) don't break spacing or indentation.
/// - Line comments should probably have a space before them if they are on the same line as code.
/// - Block comments don't need any special formatting since they print their exact content.
struct CommentRule;
impl FormatRule for CommentRule {
    fn before_token(&mut self, t: &Lexed, state: &mut FormatState, output: &mut String) {
        if let Tok::LineComment = &t.tok {
            if !state.at_line_start {
                output.push(' ');
            }
        }
    }
    
    fn space_after(&self, _prev: Option<&Lexed>, t: &Lexed, _next: Option<&Lexed>, _state: &FormatState) -> Option<bool> {
        if matches!(t.tok, Tok::LineComment | Tok::BlockComment) {
            return Some(false); // Newline handles the spacing after a line comment
        }
        None
    }
}

// -----------------------------------------------------------------------------
// ENGINE
// -----------------------------------------------------------------------------

pub struct TokenFormatter<'a> {
    state: FormatState,
    rules: Vec<Box<dyn FormatRule>>,
    src: &'a str,
    output: String,
}

impl<'a> TokenFormatter<'a> {
    pub fn new(src: &'a str, options: FormatOptions) -> Self {
        let rules: Vec<Box<dyn FormatRule>> = vec![
            Box::new(BlankLineRule),
            Box::new(ParenthesisRule),
            Box::new(BlockRule),
            Box::new(DirectiveRule),
            Box::new(CommentRule),
            Box::new(SpacingRule),
        ];

        Self {
            state: FormatState {
                indent_level: 0,
                paren_depth: 0,
                at_line_start: true,
                in_directive_line: false,
                options,
            },
            rules,
            src,
            output: String::new(),
        }
    }

    fn push_indent(&mut self) {
        if self.state.at_line_start {
            self.output.push_str(&self.state.options.indent_string.repeat(self.state.indent_level));
            self.state.at_line_start = false;
        }
    }

    pub fn format(mut self, tokens: &[Lexed]) -> String {
        let mut i = 0;
        
        while i < tokens.len() {
            let t = &tokens[i];
            let prev = if i > 0 { Some(&tokens[i-1]) } else { None };
            let next = if i + 1 < tokens.len() { Some(&tokens[i+1]) } else { None };

            // Phase 1: Consume entirely (e.g. Newline)
            let mut consumed = false;
            for rule in &mut self.rules {
                if rule.consume_token(t, prev, &mut self.state, &mut self.output) {
                    consumed = true;
                    break;
                }
            }
            if consumed {
                i += 1;
                continue;
            }

            // Phase 2: Before print mutations
            for rule in &mut self.rules {
                rule.before_token(t, &mut self.state, &mut self.output);
            }

            // Print Indent if needed
            self.push_indent();

            // Print the actual token string
            let lexeme = &self.src[t.start..t.end];
            self.output.push_str(lexeme);

            // Phase 3: Spacing
            let mut insert_space = true;
            for rule in &self.rules {
                if let Some(decision) = rule.space_after(prev, t, next, &self.state) {
                    insert_space = decision;
                    break; // first rule to decide wins
                }
            }
            if insert_space && next.is_some() {
                self.output.push(' ');
            }

            // Phase 4: After print mutations
            for rule in &mut self.rules {
                rule.after_token(t, &mut self.state, &mut self.output);
            }

            i += 1;
        }
        
        self.output
    }
}

pub fn format_source(src: &str, tokens: &[Lexed], options: FormatOptions) -> String {
    let formatter = TokenFormatter::new(src, options);
    formatter.format(tokens)
}
