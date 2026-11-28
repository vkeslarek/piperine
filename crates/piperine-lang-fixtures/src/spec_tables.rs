//! The markdown-table readers of the specification tests: a section by its heading and the body rows of its tables.

/// The sections and tables of the specification's markdown.
pub struct SpecTables;

impl SpecTables {
    fn heading_level(line: &str) -> usize {
        let hashes = line.bytes().take_while(|b| *b == b'#').count();
        if hashes > 0 && line[hashes..].starts_with(' ') { hashes } else { 0 }
    }

    /// The lines from the heading that starts with `heading` (for example
    /// `### B.3.6`) up to the next heading of the same or a higher level.
    /// Lines inside a code fence are never taken for headings.
    pub fn section(text: &str, heading: &str) -> String {
        let mut out = String::new();
        let mut level = 0;
        let mut fenced = false;
        for line in text.lines() {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
            }
            let l = if fenced { 0 } else { SpecTables::heading_level(line) };
            if level == 0 {
                if l > 0 && line.starts_with(heading) {
                    level = l;
                    out.push_str(line);
                    out.push('\n');
                }
                continue;
            }
            if l > 0 && l <= level {
                break;
            }
            out.push_str(line);
            out.push('\n');
        }
        assert!(level > 0, "no heading starts with `{heading}`");
        out
    }

    /// The body rows of every table of `section`: cells trimmed, header and
    /// separator rows left out.
    pub fn table_rows(section: &str) -> Vec<Vec<String>> {
        let mut rows = Vec::new();
        let mut header_seen = false;
        for line in section.lines() {
            let t = line.trim();
            if !t.starts_with('|') {
                header_seen = false;
                continue;
            }
            let cells: Vec<String> = t.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect();
            if cells.iter().all(|c| !c.is_empty() && c.bytes().all(|b| b == b'-' || b == b':')) {
                continue;
            }
            if !header_seen {
                header_seen = true;
                continue;
            }
            rows.push(cells);
        }
        rows
    }
}

