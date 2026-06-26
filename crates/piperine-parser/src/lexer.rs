//! Hand-written lexer for Verilog-A/AMS.
//!
//! Produces a flat token stream. Newlines are preserved as [`Tok::Newline`]
//! because the preprocessor is line-oriented (directives run to end of line).
//! Comments and other whitespace are discarded. The same token stream feeds
//! both the preprocessor (which expands macros / resolves includes) and the
//! parser.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    Ident(String),
    /// `` `name `` — a directive (`define`, `include`, …) or a macro use.
    Tick(String),
    /// `$name` — a system function/task.
    SysCall(String),
    Int(String),
    Real(String),
    /// String literal, stored *with* surrounding quotes.
    Str(String),

    // Brackets
    LParen,
    RParen,
    LBrack,
    RBrack,
    LBrace,
    RBrace,
    AttrStart, // (*
    AttrEnd,   // *)
    ArrStart,  // '{

    // Punctuation
    Comma,
    Semi,
    Colon,
    Dot,
    At,
    Question,

    // Operators
    Contrib, // <+
    Assign,  // =
    EqEq,    // ==
    NotEq,   // !=
    Not,     // !
    Tilde,   // ~
    Lt,      // <
    Gt,      // >
    Le,      // <=
    Ge,      // >=
    Shl,     // <<
    Shr,     // >>
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Pow,     // **
    PlusPlus,   // ++
    MinusMinus, // --
    PlusEq,     // +=
    MinusEq,    // -=
    StarEq,     // *=
    SlashEq,    // /=
    PercentEq,  // %=
    Amp,     // &
    AmpAmp,  // &&
    Pipe,    // |
    PipePipe,// ||
    Caret,   // ^
    XnorC,   // ^~
    XnorT,   // ~^

    Hash, // # (parameter override)

    Backslash, // a line-continuation backslash (followed by newline)
    Newline,

    LineComment,
    BlockComment,
    
    // Phase 1 Extensions
    CaseEq,    // ===
    CaseNeq,   // !==
    Arrow,     // ->
    PathFull,  // *>
    PlusColon, // +:
    MinusColon,// -:
    ArithShl,  // <<<
    ArithShr,  // >>>
    PartSelectUp,   // +:
    PartSelectDown, // -:
    Nand,      // ~&
    Nor,       // ~|
    SizedLit(String), // e.g. 4'b1010, 'hFF
}

#[derive(Debug, Clone)]
pub struct Lexed {
    pub tok: Tok,
    pub start: usize,
    pub end: usize,
}

pub fn tokenize_with_comments(src: &str) -> Result<Vec<Lexed>, String> {
    Lexer::new(src).run()
}

pub fn tokenize(src: &str) -> Result<Vec<Lexed>, String> {
    let all = tokenize_with_comments(src)?;
    Ok(all.into_iter().filter(|l| !matches!(l.tok, Tok::LineComment | Tok::BlockComment)).collect())
}

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    out: Vec<Lexed>,
}

fn is_ident_start(c: u8) -> bool {
    c == b'_' || c.is_ascii_alphabetic()
}
fn is_ident_continue(c: u8) -> bool {
    c == b'_' || c == b'$' || c.is_ascii_alphanumeric()
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Lexer { src: src.as_bytes(), pos: 0, out: Vec::new() }
    }

    fn peek(&self, off: usize) -> u8 {
        self.src.get(self.pos + off).copied().unwrap_or(0)
    }

    fn push(&mut self, tok: Tok, start: usize) {
        self.out.push(Lexed { tok, start, end: self.pos });
    }

    fn run(mut self) -> Result<Vec<Lexed>, String> {
        while self.pos < self.src.len() {
            let c = self.peek(0);
            let start = self.pos;
            match c {
                b'\n' => {
                    self.pos += 1;
                    self.push(Tok::Newline, start);
                }
                b' ' | b'\t' | b'\r' | 0x0c => {
                    self.pos += 1;
                }
                b'/' if self.peek(1) == b'/' => self.skip_line_comment(),
                b'/' if self.peek(1) == b'*' => self.skip_block_comment()?,
                b'"' => self.string(start)?,
                b'`' => self.tick(start)?,
                b'$' => self.syscall(start),
                b'\\' => self.backslash_or_escaped_ident(start),
                b'\'' if self.peek(1) != b'{' => self.unsized_base(start)?,
                _ if is_ident_start(c) => self.ident(start),
                _ if c.is_ascii_digit() => self.number(start)?,
                b'.' if self.peek(1).is_ascii_digit() => self.number(start)?,
                _ => self.punct(start)?,
            }
        }
        Ok(self.out)
    }

    fn skip_line_comment(&mut self) {
        let start = self.pos;
        while self.pos < self.src.len() && self.peek(0) != b'\n' {
            self.pos += 1;
        }
        self.push(Tok::LineComment, start);
    }

    fn skip_block_comment(&mut self) -> Result<(), String> {
        let mut start = self.pos;
        self.pos += 2;
        loop {
            if self.pos >= self.src.len() {
                return Err("unterminated block comment".into());
            }
            if self.peek(0) == b'*' && self.peek(1) == b'/' {
                self.pos += 2;
                if start < self.pos {
                    self.push(Tok::BlockComment, start);
                }
                return Ok(());
            }
            // Emit newlines inside block comments so directive line-tracking
            // upstream stays consistent.
            if self.peek(0) == b'\n' {
                if start < self.pos {
                    self.push(Tok::BlockComment, start);
                }
                let s = self.pos;
                self.pos += 1;
                self.push(Tok::Newline, s);
                start = self.pos;
            } else {
                self.pos += 1;
            }
        }
    }

    fn string(&mut self, start: usize) -> Result<(), String> {
        self.pos += 1; // opening quote
        while self.pos < self.src.len() {
            match self.peek(0) {
                b'\\' => self.pos += 2,
                b'"' => {
                    self.pos += 1;
                    let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap().to_string();
                    self.push(Tok::Str(s), start);
                    return Ok(());
                }
                b'\n' => return Err("unterminated string literal".into()),
                _ => self.pos += 1,
            }
        }
        Err("unterminated string literal".into())
    }

    fn tick(&mut self, start: usize) -> Result<(), String> {
        self.pos += 1; // backtick
        if !is_ident_start(self.peek(0)) {
            return Err(format!("expected identifier after '`' at byte {start}"));
        }
        let name_start = self.pos;
        while self.pos < self.src.len() && is_ident_continue(self.peek(0)) {
            self.pos += 1;
        }
        let name = std::str::from_utf8(&self.src[name_start..self.pos]).unwrap().to_string();
        self.push(Tok::Tick(name), start);
        Ok(())
    }

    fn syscall(&mut self, start: usize) {
        self.pos += 1; // $
        let name_start = self.pos;
        while self.pos < self.src.len() && is_ident_continue(self.peek(0)) {
            self.pos += 1;
        }
        let name = std::str::from_utf8(&self.src[name_start..self.pos]).unwrap().to_string();
        self.push(Tok::SysCall(name), start);
    }

    fn backslash_or_escaped_ident(&mut self, start: usize) {
        // Look ahead to see if it's just whitespace until newline
        let mut temp_pos = self.pos + 1;
        while temp_pos < self.src.len() && (self.src[temp_pos] == b' ' || self.src[temp_pos] == b'\t' || self.src[temp_pos] == b'\r') {
            temp_pos += 1;
        }
        
        if temp_pos >= self.src.len() || self.src[temp_pos] == b'\n' {
            // It's a line continuation (possibly with trailing spaces)
            self.pos += 1;
            self.push(Tok::Backslash, start);
            return;
        }

        // escaped identifier: `\` then non-whitespace run, terminated by whitespace
        self.pos += 1;
        let name_start = self.pos;
        while self.pos < self.src.len() && !self.peek(0).is_ascii_whitespace() {
            self.pos += 1;
        }
        let name = std::str::from_utf8(&self.src[name_start..self.pos]).unwrap().to_string();
        if self.pos < self.src.len() && (self.peek(0) == b' ' || self.peek(0) == b'\t') {
            self.pos += 1; // skip trailing space but not newline
        }
        self.push(Tok::Ident(name), start);
    }

    fn ident(&mut self, start: usize) {
        while self.pos < self.src.len() && is_ident_continue(self.peek(0)) {
            self.pos += 1;
        }
        let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap().to_string();
        self.push(Tok::Ident(s), start);
    }

    fn unsized_base(&mut self, start: usize) -> Result<(), String> {
        // e.g. 'b0, 'hFF
        self.pos += 1; // skip '
        self.eat_base_and_value()?;
        let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap().to_string();
        self.push(Tok::SizedLit(s), start);
        Ok(())
    }

    fn eat_base_and_value(&mut self) -> Result<(), String> {
        if matches!(self.peek(0), b's' | b'S') {
            self.pos += 1;
        }
        if matches!(self.peek(0), b'b' | b'B' | b'o' | b'O' | b'd' | b'D' | b'h' | b'H') {
            self.pos += 1;
            // eat whitespace
            while self.pos < self.src.len() && self.peek(0).is_ascii_whitespace() && self.peek(0) != b'\n' {
                self.pos += 1;
            }
            // eat value digits (hex can include a-f, x, z, ?)
            while self.pos < self.src.len() {
                let c = self.peek(0);
                if c.is_ascii_hexdigit() || matches!(c, b'x' | b'X' | b'z' | b'Z' | b'?' | b'_') {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            Ok(())
        } else {
            Err("expected base specifier after ' (b, o, d, h)".to_string())
        }
    }

    fn number(&mut self, start: usize) -> Result<(), String> {
        self.eat_decimal_digits();
        
        // If it's a sized literal like `4'b1010`
        if self.peek(0) == b'\'' && self.peek(1) != b'{' {
            self.pos += 1; // skip '
            self.eat_base_and_value()?;
            let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap().to_string();
            self.push(Tok::SizedLit(s), start);
            return Ok(());
        }

        let mut is_real = false;
        let mut had_exp = false;
        if self.peek(0) == b'.' {
            self.pos += 1;
            is_real = true;
            if self.peek(0).is_ascii_digit() {
                self.eat_decimal_digits();
            }
        }
        if matches!(self.peek(0), b'e' | b'E') {
            self.pos += 1;
            self.eat_float_exponent();
            is_real = true;
            had_exp = true;
        }
        if !had_exp {
            if self.eat_time_unit() {
                is_real = true;
            } else if b"TGMKkmunpfa".contains(&self.peek(0)) {
                self.pos += 1;
                is_real = true;
            }
        }
        let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap().to_string();
        if is_real {
            self.push(Tok::Real(s), start);
        } else {
            self.push(Tok::Int(s), start);
        }
        Ok(())
    }

    /// Consume a SystemVerilog time unit (`s`, `ms`, `us`, `ns`, `ps`, `fs`) if
    /// one appears at a word boundary. Returns whether one was consumed.
    fn eat_time_unit(&mut self) -> bool {
        for unit in [b"fs" as &[u8], b"ps", b"ns", b"us", b"ms"] {
            if self.peek(0) == unit[0]
                && self.peek(1) == unit[1]
                && !is_ident_continue(self.peek(2))
            {
                self.pos += 2;
                return true;
            }
        }
        if self.peek(0) == b's' && !is_ident_continue(self.peek(1)) {
            self.pos += 1;
            return true;
        }
        false
    }

    fn eat_decimal_digits(&mut self) {
        while self.peek(0).is_ascii_digit() || self.peek(0) == b'_' {
            self.pos += 1;
        }
    }

    fn eat_float_exponent(&mut self) {
        if matches!(self.peek(0), b'+' | b'-') {
            self.pos += 1;
        }
        self.eat_decimal_digits();
    }

    fn punct(&mut self, start: usize) -> Result<(), String> {
        let c = self.peek(0);
        let c1 = self.peek(1);
        let c2 = self.peek(2);
        macro_rules! one {
            ($t:expr) => {{ self.pos += 1; self.push($t, start); return Ok(()); }};
        }
        macro_rules! two {
            ($t:expr) => {{ self.pos += 2; self.push($t, start); return Ok(()); }};
        }
        macro_rules! three {
            ($t:expr) => {{ self.pos += 3; self.push($t, start); return Ok(()); }};
        }
        // 3-char operators
        match (c, c1, c2) {
            (b'=', b'=', b'=') => three!(Tok::CaseEq),
            (b'!', b'=', b'=') => three!(Tok::CaseNeq),
            (b'<', b'<', b'<') => three!(Tok::ArithShl),
            (b'>', b'>', b'>') => three!(Tok::ArithShr),
            _ => {}
        }
        match (c, c1) {
            (b'-', b'>') => two!(Tok::Arrow),
            (b'*', b'>') => two!(Tok::PathFull),
            (b'+', b':') => two!(Tok::PartSelectUp),
            (b'-', b':') => two!(Tok::PartSelectDown),
            (b'~', b'&') => two!(Tok::Nand),
            (b'~', b'|') => two!(Tok::Nor),
            (b'(', b'*') => two!(Tok::AttrStart),
            (b'*', b')') => two!(Tok::AttrEnd),
            (b'\'', b'{') => two!(Tok::ArrStart),
            (b'<', b'+') => two!(Tok::Contrib),
            (b'<', b'<') => two!(Tok::Shl),
            (b'>', b'>') => two!(Tok::Shr),
            (b'<', b'=') => two!(Tok::Le),
            (b'>', b'=') => two!(Tok::Ge),
            (b'=', b'=') => two!(Tok::EqEq),
            (b'!', b'=') => two!(Tok::NotEq),
            (b'&', b'&') => two!(Tok::AmpAmp),
            (b'|', b'|') => two!(Tok::PipePipe),
            (b'*', b'*') => two!(Tok::Pow),
            (b'^', b'~') => two!(Tok::XnorC),
            (b'~', b'^') => two!(Tok::XnorT),
            (b'+', b'+') => two!(Tok::PlusPlus),
            (b'-', b'-') => two!(Tok::MinusMinus),
            (b'+', b'=') => two!(Tok::PlusEq),
            (b'-', b'=') => two!(Tok::MinusEq),
            (b'*', b'=') => two!(Tok::StarEq),
            (b'/', b'=') => two!(Tok::SlashEq),
            (b'%', b'=') => two!(Tok::PercentEq),
            (b'(', _) => one!(Tok::LParen),
            (b')', _) => one!(Tok::RParen),
            (b'[', _) => one!(Tok::LBrack),
            (b']', _) => one!(Tok::RBrack),
            (b'{', _) => one!(Tok::LBrace),
            (b'}', _) => one!(Tok::RBrace),
            (b',', _) => one!(Tok::Comma),
            (b';', _) => one!(Tok::Semi),
            (b':', _) => one!(Tok::Colon),
            (b'.', _) => one!(Tok::Dot),
            (b'@', _) => one!(Tok::At),
            (b'?', _) => one!(Tok::Question),
            (b'#', _) => one!(Tok::Hash),
            (b'=', _) => one!(Tok::Assign),
            (b'<', _) => one!(Tok::Lt),
            (b'>', _) => one!(Tok::Gt),
            (b'!', _) => one!(Tok::Not),
            (b'~', _) => one!(Tok::Tilde),
            (b'+', _) => one!(Tok::Plus),
            (b'-', _) => one!(Tok::Minus),
            (b'*', _) => one!(Tok::Star),
            (b'/', _) => one!(Tok::Slash),
            (b'%', _) => one!(Tok::Percent),
            (b'&', _) => one!(Tok::Amp),
            (b'|', _) => one!(Tok::Pipe),
            (b'^', _) => one!(Tok::Caret),
            _ => Err(format!("unexpected character {:?} at byte {start}", c as char)),
        }
    }
}
