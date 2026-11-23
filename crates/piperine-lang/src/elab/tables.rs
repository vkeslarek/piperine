//! `TableText` and `HexText`: the text `parse_table` and `parse_hex` read at elaboration (§22.6.4, PI-09).
//! A text that fits the format and not the `const`'s type is `E0614` (§20 rule 5), naming the source and the
//! row; a text that is not in the format at all is `E0706` naming the source, the row and the token.

/// Why a text is not the array its `const` asks for: the code and the message that names the source, the
/// row and, for `E0706`, the token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableFault {
    /// `E0614` (the text does not fit the type) or `E0706` (the text is not in the format).
    pub code: &'static str,
    /// What the diagnostic says.
    pub message: String,
}

impl TableFault {
    /// `E0614`: the text does not fit the `const`'s type (§22.6.4 rule 5).
    fn misfit(source: &str, row: usize, why: String) -> TableFault {
        TableFault { code: "E0614", message: format!("the text of {source} does not fit the `const`'s type at row {row}: {why}") }
    }

    /// `E0706`: a token that is not in the format of §22.6.4 (rule 6).
    fn format(source: &str, row: usize, token: &str, why: &str) -> TableFault {
        TableFault { code: "E0706", message: format!("the text of {source} is not in the format of §22.6.4 at row {row}: `{token}` {why}") }
    }
}

/// The text of a `parse_table`: one row per line, columns separated by whitespace, blank lines skipped, `#`
/// starting a comment that runs to the end of the line, each column a real literal with its SI suffix.
pub struct TableText<'a> {
    /// How the diagnostics name the text: a resolved path, or the text itself.
    source: &'a str,
    /// The text.
    text: &'a str,
}

impl<'a> TableText<'a> {
    /// The table `text`, known in diagnostics as `source`.
    pub fn new(source: &'a str, text: &'a str) -> TableText<'a> {
        TableText { source, text }
    }

    /// The rows of the table, each of `columns` numbers: `count` rows are wanted, no more and no fewer
    /// (`E0614`); a token that is no real literal, or one whose nearest `f64` is infinite, is `E0706`.
    pub fn rows(&self, columns: usize, count: usize) -> Result<Vec<Vec<f64>>, TableFault> {
        let mut rows = Vec::new();
        for (index, line) in self.text.lines().enumerate() {
            let number = index + 1;
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let tokens: Vec<&str> = line.split_whitespace().collect();
            let mut row = Vec::with_capacity(tokens.len());
            for token in &tokens {
                row.push(self.real(number, token)?);
            }
            if row.len() != columns {
                return Err(TableFault::misfit(self.source, number, format!("it has {} columns, the type wants {columns}", row.len())));
            }
            rows.push(row);
        }
        if rows.len() != count {
            return Err(TableFault::misfit(self.source, self.text.lines().count().max(1), format!("the text has {} rows, the type wants {count}", rows.len())));
        }
        Ok(rows)
    }

    /// The real literal `token`, with an SI suffix.
    fn real(&self, row: usize, token: &str) -> Result<f64, TableFault> {
        let (digits, scale) = match token.chars().last() {
            Some(suffix) if Self::scale(suffix).is_some() && token.len() > 1 => (&token[..token.len() - suffix.len_utf8()], Self::scale(suffix).unwrap_or(1.0)),
            _ => (token, 1.0),
        };
        if scale != 1.0 && digits.contains(['e', 'E']) {
            return Err(TableFault::format(self.source, row, token, "has an exponent and an SI suffix: Appendix A.2 rule 4 allows one or the other"));
        }
        let literal = digits.strip_prefix(['+', '-']).unwrap_or(digits);
        let shaped = literal.chars().next().is_some_and(|c| c.is_ascii_digit() || c == '.') && literal.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-' | '_'));
        let value = shaped.then(|| digits.replace('_', "").parse::<f64>().ok()).flatten().ok_or_else(|| TableFault::format(self.source, row, token, "is not a real literal"))?;
        let scaled = value * scale;
        if scaled.is_finite() { Ok(scaled) } else { Err(TableFault::format(self.source, row, token, "is infinite: its nearest `f64` is not a number")) }
    }

    /// The factor of an SI suffix.
    fn scale(suffix: char) -> Option<f64> {
        Some(match suffix {
            'T' => 1e12,
            'G' => 1e9,
            'M' => 1e6,
            'k' => 1e3,
            'm' => 1e-3,
            'u' => 1e-6,
            'n' => 1e-9,
            'p' => 1e-12,
            'f' => 1e-15,
            'a' => 1e-18,
            _ => return None,
        })
    }
}

/// The text of a `parse_hex`: the `$readmemh` format, hexadecimal words separated by whitespace, `_` inside a
/// word, `//` and `/* */` comments and `@address` records that move where the next word goes.
pub struct HexText<'a> {
    /// How the diagnostics name the text.
    source: &'a str,
    /// The text.
    text: &'a str,
}

impl<'a> HexText<'a> {
    /// The hex image `text`, known in diagnostics as `source`.
    pub fn new(source: &'a str, text: &'a str) -> HexText<'a> {
        HexText { source, text }
    }

    /// The `count` words of the image as integers of `bits` bits (`signed` or not): a word that is not
    /// hexadecimal, holds an `x` or `z` digit (integers hold no unknown, §22.6.4) or does not fit is `E0706`;
    /// an address past the array, or an index the text never writes, is `E0614`.
    pub fn words(&self, count: usize, bits: u32, signed: bool) -> Result<Vec<i128>, TableFault> {
        let mut words: Vec<Option<i128>> = vec![None; count];
        let mut at = 0usize;
        let mut row = 1usize;
        for token in self.tokens(&mut row) {
            let (number, token) = token;
            if let Some(address) = token.strip_prefix('@') {
                at = usize::from_str_radix(&address.replace('_', ""), 16).map_err(|_| TableFault::format(self.source, number, &token, "is not a hexadecimal address"))?;
                continue;
            }
            let digits = token.replace('_', "");
            if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
                let why = if digits.chars().any(|c| matches!(c, 'x' | 'X' | 'z' | 'Z')) { "holds an unknown digit, which an integer cannot hold" } else { "is not a hexadecimal word" };
                return Err(TableFault::format(self.source, number, &token, why));
            }
            let value = u128::from_str_radix(&digits, 16).ok().filter(|v| bits >= 128 || *v < (1u128 << bits)).ok_or_else(|| TableFault::format(self.source, number, &token, &format!("does not fit {bits} bits")))?;
            let value = if signed && bits < 128 && value >= (1u128 << (bits - 1)) { value as i128 - (1i128 << bits) } else { value as i128 };
            let Some(slot) = words.get_mut(at) else {
                return Err(TableFault::misfit(self.source, number, format!("the word is at address {at}, past the {count} elements of the type")));
            };
            *slot = Some(value);
            at += 1;
        }
        words.into_iter().enumerate().map(|(address, word)| word.ok_or_else(|| TableFault::misfit(self.source, row, format!("the text never writes address {address}")))).collect()
    }

    /// The tokens of the text, comments removed, each with its line; `last` ends as the last line.
    fn tokens(&self, last: &mut usize) -> Vec<(usize, String)> {
        let (mut tokens, mut line, mut current) = (Vec::new(), 1usize, String::new());
        let (mut in_block, mut chars) = (false, self.text.chars().peekable());
        let flush = |current: &mut String, line: usize, tokens: &mut Vec<(usize, String)>| {
            if !current.is_empty() {
                tokens.push((line, std::mem::take(current)));
            }
        };
        while let Some(c) = chars.next() {
            if c == '\n' {
                flush(&mut current, line, &mut tokens);
                line += 1;
            } else if in_block {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    in_block = false;
                }
            } else if c == '/' && chars.peek() == Some(&'/') {
                flush(&mut current, line, &mut tokens);
                for skipped in chars.by_ref() {
                    if skipped == '\n' {
                        line += 1;
                        break;
                    }
                }
            } else if c == '/' && chars.peek() == Some(&'*') {
                flush(&mut current, line, &mut tokens);
                chars.next();
                in_block = true;
            } else if c.is_whitespace() {
                flush(&mut current, line, &mut tokens);
            } else {
                current.push(c);
            }
        }
        flush(&mut current, line, &mut tokens);
        *last = line;
        tokens
    }
}
