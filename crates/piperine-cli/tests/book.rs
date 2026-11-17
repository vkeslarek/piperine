//! The Book's command line (feature `msl-cli`, T36; CLI-19): every listing of the Book that
//! claims to be what `piperine` prints is held to the real binary, and every file the Book shows
//! of an example project is held to that project. The Book is `docs/book/content.html`, read with
//! plain string functions as `piperine-lang`'s `tests/book.rs` reads it (sections by
//! `<section id="..">`, blocks by `<pre data-lang="..">`, code spans by `<code>`).
//!
//! The Book marks its listings with attributes the guard reads:
//!
//! - `data-cli-output="<command>"` on a `text` block: the block is what `<command>` prints, run in
//!   the project `data-cli-project` names (`rc` is a copy of `examples/rc`, `buck` is
//!   `examples/buck`, any other name is assembled from the Book's own files), or in an empty
//!   directory holding the Book's lone files when there is none;
//! - `data-cli-lines="a-b"` (or `"a"`): only those lines of the output, 1-based, inclusive;
//! - `data-cli-stream="stderr"`: the block is standard error of a command that exits 1;
//! - `data-cli-exit="1"`: the block is standard output of a command that exits 1 (records
//!   printed before a failure stay, CLI-07 AC 5);
//! - `data-cli-written="<file>"`: the block is the file the command writes;
//! - `data-cli-cache="empty"`: the command runs on a cache of its own, empty.
//!
//! A source file a listing needs is marked by an HTML comment right before its block,
//! `<!-- cli-file: <path> -->`: `rc/benches/delayed.msl` is added to the copy of `examples/rc`,
//! `lowpass/src/lib.msl` belongs to the project `lowpass`, `blinky.msl` is a lone file. Blocks
//! marked with one path are joined in order, and a listing sees the file as the Book has written
//! it up to that listing.
//!
//! Every child process gets `PIPERINE_CACHE_DIR` set to a temporary directory shared by this
//! target; no test sets a variable of its own process.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn content() -> String {
    std::fs::read_to_string(repo().join("docs/book/content.html")).expect("docs/book/content.html exists")
}

/// The five entities the Book's source uses, decoded.
fn decode(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
}

/// The text of the chapter `id`, from its `<section>` tag to the first `</section>`.
fn chapter(html: &str, id: &str) -> String {
    let open = format!("<section id=\"{id}\" class=\"chapter\">");
    let at = html.find(&open).unwrap_or_else(|| panic!("the Book has a chapter `{id}`"));
    let rest = &html[at..];
    rest[..rest.find("</section>").expect("a chapter closes")].to_string()
}

/// The decoded text of every `<code>..</code>` span of `html`.
fn code_spans(html: &str) -> Vec<String> {
    html.split("<code>").skip(1).filter_map(|s| s.split_once("</code>")).map(|(span, _)| decode(span)).collect()
}

/// The text of `html` with tags removed and whitespace collapsed.
fn plain(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode(&out).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One `<pre data-lang="..">` block: where it starts in the Book, its language, its other
/// attributes, its decoded text, and the `cli-file` path marked right before it.
struct Block {
    at: usize,
    lang: String,
    attrs: BTreeMap<String, String>,
    text: String,
    file: Option<String>,
}

impl Block {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }
}

/// The attributes `name="value"` of an opening tag's tail.
fn attributes(tail: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut rest = tail;
    while let Some((name, after)) = rest.split_once("=\"") {
        let (value, next) = after.split_once('"').expect("an attribute value closes");
        out.insert(name.trim().to_string(), decode(value));
        rest = next;
    }
    out
}

/// Every block of `html`, in order.
fn blocks(html: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = html[from..].find("<pre data-lang=\"") {
        let at = from + found;
        let after = &html[at + "<pre data-lang=\"".len()..];
        let (lang, rest) = after.split_once('"').expect("the language closes");
        let (tail, body) = rest.split_once('>').expect("a block's opening tag closes");
        let (text, _) = body.split_once("</pre>").expect("a block closes");
        let before = html[..at].trim_end();
        let file = before.strip_suffix("-->").and_then(|b| b.rsplit_once("<!--")).and_then(|(_, comment)| comment.strip_prefix(" cli-file: ")).map(|path| path.trim().to_string());
        out.push(Block { at, lang: lang.to_string(), attrs: attributes(tail), text: decode(text), file });
        from = at + 1;
    }
    out
}

/// The Book's marked files as they stand before the Book offset `upto`, by path.
fn files_before(all: &[Block], upto: usize) -> BTreeMap<String, String> {
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    for block in all.iter().filter(|b| b.at < upto) {
        if let Some(path) = &block.file {
            let text = files.entry(path.clone()).or_default();
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&block.text);
            text.push('\n');
        }
    }
    files
}

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_piperine"))
}

/// One cache directory for every child of this target that does not ask for an empty one.
fn shared_cache() -> &'static Path {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    CACHE.get_or_init(|| tempfile::tempdir().expect("tmp")).path()
}

/// Copies the directory `from` into `to`, leaving out a `target` directory.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a directory");
    for entry in std::fs::read_dir(from).expect("a readable directory") {
        let entry = entry.expect("an entry");
        if entry.file_name() == "target" {
            continue;
        }
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("a copy");
        }
    }
}

/// Writes `text` at `dir/path`, creating its directories.
fn write(dir: &Path, path: &str, text: &str) {
    let file = dir.join(path);
    std::fs::create_dir_all(file.parent().expect("a parent")).expect("a directory");
    std::fs::write(file, text).expect("a file");
}

/// The directory a listing's command runs in, with the temporary directory that holds it.
fn workdir(all: &[Block], block: &Block) -> (Option<tempfile::TempDir>, PathBuf) {
    let files = files_before(all, block.at);
    match block.attr("data-cli-project") {
        Some("buck") => (None, repo().join("examples/buck")),
        project => {
            let tmp = tempfile::tempdir().expect("tmp");
            let dir = match project {
                Some(name) => tmp.path().join(name),
                None => tmp.path().to_path_buf(),
            };
            if project == Some("rc") {
                copy_tree(&repo().join("examples/rc"), &dir);
            }
            for (path, text) in &files {
                match (project, path.split_once('/')) {
                    (Some(name), Some((head, rest))) if head == name => write(&dir, rest, text),
                    (None, None) => write(&dir, path, text),
                    _ => {}
                }
            }
            (Some(tmp), dir)
        }
    }
}

/// Runs a listing's command and returns the text the listing claims, with the command for the
/// failure message.
fn produce(all: &[Block], block: &Block) -> (String, String) {
    let command = block.attr("data-cli-output").expect("a listing names its command").to_string();
    let words: Vec<&str> = command.split_whitespace().collect();
    assert_eq!(words.first(), Some(&"piperine"), "a listing's command starts with `piperine`: {command}");
    let (_tmp, dir) = workdir(all, block);
    let empty = tempfile::tempdir().expect("tmp");
    let cache = if block.attr("data-cli-cache") == Some("empty") { empty.path() } else { shared_cache() };
    let out: Output = Command::new(binary())
        .args(&words[1..])
        .current_dir(&dir)
        .env("PIPERINE_CACHE_DIR", cache)
        .env_remove("PIPERINE_BACKEND")
        .output()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", binary().display()));
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let text = if block.attr("data-cli-stream") == Some("stderr") {
        assert_eq!(out.status.code(), Some(1), "`{command}` is shown refused, so it exits 1; stderr:\n{stderr}");
        stderr
    } else {
        let exit: i32 = block.attr("data-cli-exit").map_or(0, |code| code.parse().expect("an exit code"));
        assert_eq!(out.status.code(), Some(exit), "`{command}` exits {exit}; stderr:\n{stderr}");
        match block.attr("data-cli-written") {
            Some(file) => std::fs::read_to_string(dir.join(file)).unwrap_or_else(|e| panic!("`{command}` wrote {file}: {e}")),
            None => String::from_utf8(out.stdout).expect("standard output is text"),
        }
    };
    (command, select(&text, block.attr("data-cli-lines")))
}

/// The lines `a-b` (or the line `a`), 1-based and inclusive, of `text`; all of it without a range.
fn select(text: &str, lines: Option<&str>) -> String {
    let Some(range) = lines else {
        return text.trim_end_matches('\n').to_string();
    };
    let (a, b) = range.split_once('-').unwrap_or((range, range));
    let (a, b): (usize, usize) = (a.parse().expect("a line number"), b.parse().expect("a line number"));
    let all: Vec<&str> = text.lines().collect();
    assert!(b <= all.len(), "the output has {} lines, the listing asks for {range}:\n{text}", all.len());
    all[a - 1..b].join("\n")
}

/// The listings of `html` (whole Book) that lie inside `[from, to)` and satisfy `keep`, each
/// checked against the binary; returns the commands checked.
fn check_listings(html: &str, from: usize, to: usize, keep: impl Fn(&Block) -> bool) -> Vec<String> {
    let all = blocks(html);
    let mut failures = Vec::new();
    let mut commands = Vec::new();
    for block in all.iter().filter(|b| b.at >= from && b.at < to && b.attr("data-cli-output").is_some() && keep(b)) {
        let (command, got) = produce(&all, block);
        if got != block.text {
            failures.push(format!("`{command}`:\n--- the Book\n{}\n--- the binary\n{got}", block.text));
        }
        commands.push(command);
    }
    assert!(failures.is_empty(), "{} listings differ from the binary:\n{}", failures.len(), failures.join("\n\n"));
    commands
}

/// The offsets of the chapter `id` in the whole Book.
fn span_of(html: &str, id: &str) -> (usize, usize) {
    let open = format!("<section id=\"{id}\" class=\"chapter\">");
    let at = html.find(&open).unwrap_or_else(|| panic!("the Book has a chapter `{id}`"));
    (at, at + html[at..].find("</section>").expect("a chapter closes"))
}

/// The text of a file of an example project, without its last line break.
fn example(path: &str) -> String {
    std::fs::read_to_string(repo().join("examples").join(path)).unwrap_or_else(|e| panic!("examples/{path}: {e}")).trim_end_matches('\n').to_string()
}

/// Does `html` hold a block of `lang` whose text is `text`?
fn holds(html: &str, lang: &str, text: &str) -> bool {
    blocks(html).iter().any(|b| b.lang == lang && b.text == text)
}

#[test]
fn the_plot_in_the_chapter_is_the_file_the_binary_writes() {
    // CLI-19 AC 11: the SVG the chapter embeds is the file `piperine run step --plot` writes in
    // `examples/rc`
    let html = chapter(&content(), "cli");
    let at = html.find("<svg").expect("the chapter embeds an SVG");
    let end = html[at..].find("</svg>").expect("the SVG closes") + at + "</svg>".len();
    let tmp = tempfile::tempdir().expect("tmp");
    let dir = tmp.path().join("rc");
    copy_tree(&repo().join("examples/rc"), &dir);
    let out = Command::new(binary()).args(["run", "step", "--plot", "step.svg"]).current_dir(&dir).env("PIPERINE_CACHE_DIR", shared_cache()).env_remove("PIPERINE_BACKEND").output().expect("run piperine");
    assert_eq!(out.status.code(), Some(0), "stderr:\n{}", String::from_utf8_lossy(&out.stderr));
    let svg = std::fs::read_to_string(dir.join("step.svg")).expect("the plot was written");
    assert_eq!(&html[at..end], svg.trim_end_matches('\n'), "the embedded SVG is the file the binary wrote");
}

#[test]
fn the_chapter_shows_fmt_and_says_comments_stay() {
    // CLI-19 AC 12: `piperine fmt` and `piperine fmt --check` in code spans, and a sentence
    // saying comments are kept
    let html = chapter(&content(), "cli");
    let spans = code_spans(&html);
    for name in ["piperine fmt", "piperine fmt --check"] {
        assert!(spans.iter().any(|s| s == name), "the chapter names `{name}` in a code span");
    }
    let text = plain(&html);
    assert!(text.split(". ").any(|s| s.contains("comment") && s.contains("kept")), "a sentence says comments are kept");
    // and the files the chapter shows are formatted: `fmt --check` passes on the RC project
    let out = Command::new(binary()).args(["fmt", "--check"]).current_dir(repo().join("examples/rc")).output().expect("run piperine");
    assert_eq!(out.status.code(), Some(0), "stdout:\n{}", String::from_utf8_lossy(&out.stdout));
}

#[test]
fn the_chapter_shows_the_rc_project_file_for_file() {
    // CLI-19 AC 2: the tree of the RC project and the full text of `Piperine.toml`, `src/lib.msl`
    // and `benches/step.msl`, each equal to the file of `examples/rc`
    let html = chapter(&content(), "cli");
    assert!(holds(&html, "toml", &example("rc/Piperine.toml")), "the chapter shows examples/rc/Piperine.toml as it is");
    assert!(holds(&html, "msl", &example("rc/src/lib.msl")), "the chapter shows examples/rc/src/lib.msl as it is");
    assert!(holds(&html, "msl", &example("rc/benches/step.msl")), "the chapter shows examples/rc/benches/step.msl as it is");
    let tree = blocks(&html).into_iter().find(|b| b.lang == "text" && b.text.starts_with("rc/")).expect("a tree of the project");
    for name in ["Piperine.toml", "src/", "lib.msl", "benches/", "step.msl"] {
        assert!(tree.text.contains(name), "the tree names `{name}`:\n{}", tree.text);
    }
}

#[test]
fn the_welcome_chapter_shows_the_rc_project_file_for_file() {
    // the Book's first chapter prints the same project (the Book's handoff: every file block of
    // the RC project is held to `examples/rc`, the chapter `welcome` included)
    let html = chapter(&content(), "welcome");
    for (lang, path) in [("toml", "rc/Piperine.toml"), ("msl", "rc/src/lib.msl"), ("msl", "rc/benches/step.msl")] {
        assert!(holds(&html, lang, &example(path)), "chapter 1 shows examples/{path} as it is");
    }
}

#[test]
fn the_rc_output_blocks_are_what_the_binary_prints() {
    // CLI-19 AC 3: the standard output of `check`, `run op`, `run step` and `test` in
    // `examples/rc`, and every other RC listing of the chapter, equal to what the binary prints
    let html = content();
    let (from, to) = span_of(&html, "cli");
    let commands = check_listings(&html, from, to, |b| b.attr("data-cli-project") == Some("rc"));
    for asked in ["piperine check", "piperine run op", "piperine run step", "piperine test"] {
        assert!(commands.iter().any(|c| c == asked), "the chapter shows the output of `{asked}`: {commands:?}");
    }
}

#[test]
fn every_cli_listing_of_the_book_is_what_the_binary_prints() {
    // the Book's handoff: every listing outside the chapter `cli` that claims to be the binary's
    // output (chapter 1 and the "Run it" boxes of the language chapters) is what it prints
    let html = content();
    let (from, to) = span_of(&html, "cli");
    let commands = check_listings(&html, 0, html.len(), |b| b.at < from || b.at >= to);
    assert!(commands.len() >= 15, "the Book outside chapter `cli` shows {} listings: {commands:?}", commands.len());
}

#[test]
fn the_buck_block_is_what_the_binary_prints() {
    // CLI-19 AC 4: the buck's bench and its `startup` table equal to the files, and the standard
    // output of `piperine run startup` in `examples/buck` equal to what the binary prints
    let html = content();
    let cli = chapter(&html, "cli");
    assert!(holds(&cli, "msl", &example("buck/benches/startup.msl")), "the chapter shows examples/buck/benches/startup.msl as it is");
    let manifest = example("buck/Piperine.toml");
    let table = blocks(&cli).into_iter().find(|b| b.lang == "toml" && b.text.contains("[run.startup]")).expect("the chapter shows the run table `startup`");
    assert!(manifest.ends_with(&table.text), "the `startup` table is the end of examples/buck/Piperine.toml, as it is:\n{}", table.text);
    assert!(holds(&cli, "sh", "piperine run startup"), "the chapter gives the command in a shell block");
    let (from, to) = span_of(&html, "cli");
    let commands = check_listings(&html, from, to, |b| b.attr("data-cli-project") == Some("buck"));
    assert_eq!(commands, ["piperine run startup"]);
}

#[test]
fn the_chapter_explains_emit_as_print() {
    // CLI-19 AC 5: a part whose heading names `emit!`, an `msl` block with an `emit!` statement,
    // the record line it gives, and `--stream`
    let html = chapter(&content(), "cli");
    let part = html
        .split("<h3>")
        .skip(1)
        .find(|p| p.split_once("</h3>").is_some_and(|(h, _)| h.contains("emit!")))
        .expect("a heading that names `emit!`")
        .to_string();
    let parts = blocks(&part);
    let site = parts.iter().find(|b| b.lang == "msl" && b.text.contains("emit!(\"trip\"")).expect("an msl block with an `emit!` statement");
    let stream = site.text.split("emit!(\"").nth(1).and_then(|s| s.split_once('"')).map(|(s, _)| s).expect("a stream name");
    let record = parts.iter().filter(|b| b.attr("data-cli-output").is_some()).flat_map(|b| b.text.lines()).find(|l| l.starts_with('@') && l.split_whitespace().nth(2) == Some(stream));
    assert!(record.is_some(), "the part shows a record line of the stream `{stream}`");
    assert!(code_spans(&part).iter().any(|s| s.starts_with("--stream ")), "the part shows `--stream`");
    assert!(code_spans(&part).iter().any(|s| s == "--all-checks"), "the part shows `--all-checks`");
}

#[test]
fn the_chapter_lists_commands_keys_flags_codes() {
    // CLI-19 AC 6: every command, every key of a run table with its flag (CLI-03 AC 1 and 2, and
    // `all_checks` of CLI-06 AC 9), the exit codes, the backend choice, `P0010` and `P0011`
    let html = chapter(&content(), "cli");
    let spans = code_spans(&html);
    let has = |s: &str| spans.iter().any(|span| span == s || span.starts_with(&format!("{s} ")));
    for command in ["new", "check", "run", "test", "build", "fmt", "clean"] {
        assert!(has(&format!("piperine {command}")), "the chapter names `piperine {command}`");
    }
    let keys = [
        ("bench", "--bench"),
        ("top", "--top"),
        ("analysis", "--analysis"),
        ("stop", "--stop"),
        ("max_step", "--max-step"),
        ("print_step", "--print-step"),
        ("sweep", "--sweep"),
        ("from", "--from"),
        ("to", "--to"),
        ("step", "--step"),
        ("per_decade", "--per-decade"),
        ("points", "--points"),
        ("print", "--print"),
        ("streams", "--stream"),
        ("params", "--param"),
        ("backend", "--backend"),
        ("csv", "--csv"),
        ("records", "--records"),
        ("plot", "--plot"),
        ("all_checks", "--all-checks"),
    ];
    for (key, flag) in keys {
        assert!(has(key), "the chapter names the key `{key}` in a code span");
        assert!(has(flag), "the chapter names the flag `{flag}` in a code span");
    }
    assert!(has("PIPERINE_BACKEND"), "the chapter names `PIPERINE_BACKEND`");
    for code in ["0", "1", "2"] {
        assert!(html.contains(&format!("<tr><td>{code}</td>")), "the table of exit codes has the code {code}");
    }
    let text = plain(&html);
    for code in ["P0010", "P0011"] {
        assert!(text.contains(code), "the chapter names {code}");
    }
}

#[test]
fn the_chapter_stands_alone() {
    // CLI-19 AC 7: no `§`, no path into the repository's `crates/`, `docs/`, `.specs/` or
    // `tests/`, no link out of the Book
    let html = chapter(&content(), "cli");
    assert!(!html.contains('§'), "the chapter holds a §");
    for path in ["crates/", "docs/", ".specs/", "tests/"] {
        assert!(!html.contains(path), "the chapter holds `{path}`");
    }
    for href in html.split("href=\"").skip(1) {
        assert!(href.starts_with('#'), "a link out of the Book: href=\"{}", href.split('"').next().unwrap_or(""));
    }
}
