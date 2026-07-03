use crate::parse::lexer::{Lexed, Tok};

pub mod blank_line;
pub mod block;
pub mod comment;
pub mod parenthesis;
pub mod spacing;

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
    pub brace_depth: usize,
    pub at_line_start: bool,
    pub options: FormatOptions,
}

impl FormatState {
    pub fn push_newline(&mut self, output: &mut String) {
        self.force_newline(output);
    }

    pub fn force_newline(&mut self, output: &mut String) {
        while output.ends_with(' ') || output.ends_with('\t') {
            output.pop();
        }
        output.push_str(&self.options.line_ending);
        self.at_line_start = true;
    }
}

pub trait FormatRule {
    fn consume_token(&mut self, _t: &Lexed, _prev: Option<&Lexed>, _state: &mut FormatState, _output: &mut String) -> bool { false }
    fn before_token(&mut self, _t: &Lexed, _next: Option<&Lexed>, _state: &mut FormatState, _output: &mut String) {}
    fn space_after(&self, _prev: Option<&Lexed>, _t: &Lexed, _next: Option<&Lexed>, _state: &FormatState) -> Option<bool> { None }
    fn after_token(&mut self, _t: &Lexed, _next: Option<&Lexed>, _state: &mut FormatState, _output: &mut String) {}
}

pub struct TokenFormatter<'a> {
    state: FormatState,
    rules: Vec<Box<dyn FormatRule>>,
    src: &'a str,
    output: String,
}

impl<'a> TokenFormatter<'a> {
    pub fn new(src: &'a str, options: FormatOptions) -> Self {
        let rules: Vec<Box<dyn FormatRule>> = vec![
            Box::new(blank_line::BlankLineRule),
            Box::new(parenthesis::ParenthesisRule),
            Box::new(block::BlockRule),
            Box::new(comment::CommentRule),
            Box::new(spacing::SpacingRule),
        ];

        Self {
            state: FormatState {
                indent_level: 0,
                paren_depth: 0,
                brace_depth: 0,
                at_line_start: true,
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
            
            let mut next_idx = i + 1;
            while next_idx < tokens.len() && tokens[next_idx].tok == Tok::Newline {
                next_idx += 1;
            }
            let next = if next_idx < tokens.len() { Some(&tokens[next_idx]) } else { None };

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

            for rule in &mut self.rules {
                rule.before_token(t, next, &mut self.state, &mut self.output);
            }

            self.push_indent();

            let lexeme = &self.src[t.start..t.end];
            self.output.push_str(lexeme);

            let mut insert_space = true;
            for rule in &self.rules {
                if let Some(decision) = rule.space_after(prev, t, next, &self.state) {
                    insert_space = decision;
                    break;
                }
            }
            if insert_space && next.is_some() {
                self.output.push(' ');
            }

            for rule in &mut self.rules {
                rule.after_token(t, next, &mut self.state, &mut self.output);
            }

            i += 1;
        }
        
        self.output
    }

    pub fn format_source(src: &str, tokens: &[Lexed], options: FormatOptions) -> String {
        TokenFormatter::new(src, options).format(tokens)
    }
}
