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
//! - `data-cli-lines="a-b"` (or `"a"`): the page shows only those lines of the block, 1-based,
//!   inclusive (`build.py` selects them); the block itself holds the whole output and the guard
//!   holds all of it to the binary, so a line the page leaves out cannot hide a wrong one;
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

/// One listing of the Book run against the binary: where it is, its project, its command, the
/// text the Book holds and the text the binary gave (or why the run is not what the listing
/// claims).
struct Checked {
    at: usize,
    project: Option<String>,
    command: String,
    book: String,
    got: Result<String, String>,
}

/// Runs a listing's command; the whole text the listing claims (standard output, standard error
/// or the file written), or why the run is not what the listing claims.
fn produce(all: &[Block], block: &Block) -> Result<String, String> {
    let command = block.attr("data-cli-output").expect("a listing names its command");
    let words: Vec<&str> = command.split_whitespace().collect();
    if words.first() != Some(&"piperine") {
        return Err("a listing's command starts with `piperine`".into());
    }
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
    let refused = block.attr("data-cli-stream") == Some("stderr");
    let exit: i32 = match block.attr("data-cli-exit") {
        Some(code) => code.parse().map_err(|_| format!("`data-cli-exit=\"{code}\"` is no exit code"))?,
        None if refused => 1,
        None => 0,
    };
    if out.status.code() != Some(exit) {
        return Err(format!("exits {:?}, the listing claims {exit}; stderr:\n{stderr}", out.status.code()));
    }
    let text = match (refused, block.attr("data-cli-written")) {
        (true, _) => stderr,
        (false, Some(file)) => std::fs::read_to_string(dir.join(file)).map_err(|e| format!("wrote no {file}: {e}"))?,
        (false, None) => String::from_utf8(out.stdout).map_err(|_| "standard output is not text".to_string())?,
    };
    Ok(text.trim_end_matches('\n').to_string())
}

/// `data-cli-lines="a-b"` (or `"a"`) of a listing: a range of its own lines, 1-based and
/// inclusive, that the rendered page shows. It only selects: the block holds the command's whole
/// output, so a line it leaves out is still held to the binary.
fn selection(block: &Block) -> Result<(), String> {
    let Some(range) = block.attr("data-cli-lines") else { return Ok(()) };
    let (a, b) = range.split_once('-').unwrap_or((range, range));
    let (a, b): (usize, usize) = match (a.parse(), b.parse()) {
        (Ok(a), Ok(b)) => (a, b),
        _ => return Err(format!("`data-cli-lines=\"{range}\"` is no line range")),
    };
    let lines = block.text.lines().count();
    if a == 0 || a > b || b > lines {
        return Err(format!("`data-cli-lines=\"{range}\"` selects outside the block's {lines} lines"));
    }
    Ok(())
}

/// Every listing of the whole Book, any chapter and any project, run once per test binary
/// and shared by the tests that read it (the buck alone takes minutes).
fn checked() -> &'static [Checked] {
    static CHECKED: OnceLock<Vec<Checked>> = OnceLock::new();
    CHECKED.get_or_init(|| {
        let html = content();
        let all = blocks(&html);
        all.iter()
            .filter(|b| b.attr("data-cli-output").is_some())
            .map(|block| Checked {
                at: block.at,
                project: block.attr("data-cli-project").map(str::to_string),
                command: block.attr("data-cli-output").expect("a listing").to_string(),
                book: block.text.clone(),
                got: selection(block).and_then(|()| produce(&all, block)),
            })
            .collect()
    })
}

/// Asserts that each listing of `listings` is what the binary prints, whole; returns their
/// commands.
fn hold(listings: &[&Checked]) -> Vec<String> {
    let mut failures = Vec::new();
    for c in listings {
        match &c.got {
            Ok(got) if *got == c.book => {}
            Ok(got) => failures.push(format!("`{}`:\n--- the Book\n{}\n--- the binary\n{got}", c.command, c.book)),
            Err(why) => failures.push(format!("`{}`: {why}", c.command)),
        }
    }
    assert!(failures.is_empty(), "{} listings differ from the binary:\n{}", failures.len(), failures.join("\n\n"));
    listings.iter().map(|c| c.command.clone()).collect()
}

/// The listings inside `[from, to)` of the Book whose project is `project`.
fn listings_of(from: usize, to: usize, project: &str) -> Vec<&'static Checked> {
    checked().iter().filter(|c| c.at >= from && c.at < to && c.project.as_deref() == Some(project)).collect()
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
    // Book claims held here: cli~a717f995
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
    let commands = hold(&listings_of(from, to, "rc"));
    for asked in ["piperine check", "piperine run op", "piperine run step", "piperine test"] {
        assert!(commands.iter().any(|c| c == asked), "the chapter shows the output of `{asked}`: {commands:?}");
    }
}

#[test]
fn every_cli_listing_of_the_book_is_what_the_binary_prints() {
    // Book claims held here: welcome~72129ad3, welcome~a4895da4, welcome~df325c6f, welcome~6e662db9, from-rust~4938c15f, values~2931bd2e, values~9a0ece05, cli~54e71d59
    // verification F6: every listing of the whole Book that claims to be the binary's output, in
    // every chapter and of any project, is the binary's whole output; `data-cli-lines` only
    // selects the lines the page shows, so a line it leaves out is held all the same
    let all: Vec<&Checked> = checked().iter().collect();
    let commands = hold(&all);
    assert!(commands.len() >= 29, "the Book shows {} listings: {commands:?}", commands.len());
    let html = content();
    let (from, to) = span_of(&html, "cli");
    assert!(all.iter().any(|c| c.at >= from && c.at < to) && all.iter().any(|c| c.at < from), "listings of chapter `cli` and of the other chapters are held");
}

#[test]
fn the_buck_block_is_what_the_binary_prints() {
    // Book claims held here: cli~9c4b6fcf
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
    let commands = hold(&listings_of(from, to, "buck"));
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

/// The Book's listing whose command starts with `prefix`, run on the binary.
fn run_listing(prefix: &str) -> String {
    let html = content();
    let all = blocks(&html);
    let block = all
        .iter()
        .find(|b| b.attr("data-cli-output").is_some_and(|c| c.starts_with(prefix)))
        .unwrap_or_else(|| panic!("the Book has a listing of `{prefix}`"));
    produce(&all, block).unwrap_or_else(|why| panic!("`{prefix}`: {why}"))
}

#[test]
fn the_cycle_counter_prints_three_pulses() {
    // CAT-05 AC 1: three records, `n` = 1 to 3, each within 1 us of 15.9 us past a whole
    // millisecond, and `count` 3 in the last row of the table.
    let out = run_listing("piperine run cycles.msl");
    let records: Vec<&str> = out.lines().filter(|l| l.starts_with('@')).collect();
    assert_eq!(records.len(), 3, "{out}");
    for (k, line) in records.iter().enumerate() {
        let field = |name: &str| line.split_whitespace().find_map(|w| w.strip_prefix(name)).unwrap_or_else(|| panic!("`{name}` in `{line}`"));
        let t: f64 = field("t=").parse().expect("a time");
        assert_eq!(field("n="), (k + 1).to_string(), "{line}");
        let wanted = 15.9e-6 + k as f64 * 1e-3;
        assert!((t - wanted).abs() < 1e-6, "pulse {}: t = {t:e}, wanted {wanted:e} within 1 us", k + 1);
    }
    let last = out.lines().rev().find(|l| !l.trim().is_empty()).expect("a table row");
    assert_eq!(last.split_whitespace().last(), Some("3"), "the last row shows count 3: {last}");
}

#[test]
fn the_clipper_refusal_no_longer_names_sin() {
    // CAT-05 AC 3: the clipper's refusal names `settle` and `limexp`, not `sin` or `bound_step`,
    // and the page's prose around it does not wait for them either.
    let out = run_listing("piperine run clipper.msl");
    assert!(out.contains("`settle`") && out.contains("limexp"), "{out}");
    for gone in ["`sin`", "method sin", "bound_step"] {
        assert!(!out.contains(gone), "the refusal still names {gone}: {out}");
    }
    let html = content();
    let at = html.find("piperine run clipper.msl").expect("the clipper's command");
    let callout = &html[at..at + html[at..].find("</div>").expect("the callout closes")];
    let prose = plain(callout);
    for gone in ["once `sin`", "`bound_step` is compiled", "sin is compiled", "bound_step is compiled"] {
        assert!(!prose.contains(gone), "the clipper's prose still says {gone:?}");
    }
}

#[test]
fn the_chapter_shows_the_session_options() {
    // HII-22 AC 2: every option key of a run with its flag in a code span, and a listing that sets
    // some of them whose output is the binary's (an operating point of the divider: 7.5 V)
    let html = chapter(&content(), "cli");
    let spans = code_spans(&html);
    let has = |s: &str| spans.iter().any(|span| span == s || span.starts_with(&format!("{s} ")));
    for (key, flag) in [
        ("reltol", "--reltol"),
        ("abstol", "--abstol"),
        ("vntol", "--vntol"),
        ("trtol", "--trtol"),
        ("temp", "--temp"),
        ("resolution", "--resolution"),
        ("maxrounds", "--maxrounds"),
        ("maxmicro", "--maxmicro"),
        ("max_steps", "--max-steps"),
        ("nodeset", "--nodeset"),
    ] {
        assert!(has(key), "the chapter names the option `{key}`");
        assert!(has(flag), "the chapter names the flag `{flag}`");
    }
    let out = run_listing("piperine run lowpass.msl --analysis op --reltol");
    let row = out.lines().find(|l| l.starts_with("Lowpass.vout")).unwrap_or_else(|| panic!("the vout row in\n{out}"));
    assert_eq!(row.split_whitespace().last(), Some("7.500000e0"), "{out}");
}

#[test]
fn the_chapter_runs_each_new_analysis() {
    // HII-22 AC 2: one listing per new analysis, and `piperine info`, each the binary's output; the
    // values are the divider's: gain 3/4, input resistance 4 kOhm, one pole at -1/(R1||R2)/C, the
    // two sensitivities, a periodic orbit whose last row is its first, and the three tables of `info`
    let html = chapter(&content(), "cli");
    let spans = code_spans(&html);
    for key in ["--input", "--output", "--wrt", "--period", "--tstab"] {
        assert!(spans.iter().any(|span| span == key || span.starts_with(&format!("{key} "))), "the chapter names `{key}`");
    }
    let number = |line: &str, at: usize| -> f64 { line.split_whitespace().nth(at).and_then(|w| w.parse().ok()).unwrap_or_else(|| panic!("a number at {at} of `{line}`")) };
    let row = |out: &str, start: &str| out.lines().find(|l| l.starts_with(start)).map(str::to_string).unwrap_or_else(|| panic!("no `{start}` row in\n{out}"));

    let tf = run_listing("piperine run lowpass.msl --analysis tf");
    assert!((number(&row(&tf, "gain"), 1) - 0.75).abs() < 1e-6, "{tf}");
    assert!((number(&row(&tf, "input_impedance"), 1) - 4000.0).abs() < 1e-3, "{tf}");

    let pz = run_listing("piperine run lowpass.msl --analysis pz");
    let pole = row(&pz, "pole");
    assert!((number(&pole, 1) + 4000.0 / 3.0).abs() < 1e-2, "{pz}");

    let sens = run_listing("piperine run lowpass.msl --analysis sens");
    assert!((number(&row(&sens, "Lowpass.r1.r"), 1) + 1.875e-3).abs() < 1e-9, "{sens}");
    assert!((number(&row(&sens, "Lowpass.r2.r"), 1) - 6.25e-4).abs() < 1e-9, "{sens}");

    let pss = run_listing("piperine run pulse.msl --analysis pss");
    let orbit: Vec<&str> = pss.lines().skip_while(|l| !l.starts_with("time")).skip(1).collect();
    let (first, last) = (number(orbit[0], 1), number(orbit[orbit.len() - 1], 1));
    assert!((first - last).abs() < 1e-3, "the orbit closes: {first} against {last}");

    let info = run_listing("piperine info lowpass.msl");
    let titles: Vec<&str> = info.lines().filter(|l| ["params", "ports", "streams"].contains(l)).collect();
    assert_eq!(titles, ["params", "ports", "streams"], "{info}");
    assert!(info.contains("Lowpass.r1.r") && info.contains("1.000000e3"), "{info}");
}

#[test]
fn a_plot_draws_a_curve_and_a_legend_per_real_signal_and_a_lane_per_digital_one() {
    // Book chapter 16 (root-e2e, cli~1369f2e0): "Every printed real signal is a curve on one shared axis, with a
    // legend; every digital signal is a step trace in a lane of its own below": a node that follows a bit
    // gives one curve, one lane and a legend naming both
    let dir = tempfile::tempdir().expect("tmp");
    let design = "module Top() { signal n: Electrical; signal d: Bit = false; }\nanalog for Top { v(n) +> transition(if d.high() { 1.0 } else { 0.0 }); }\ndigital for Top { on at(1u) { d <- true; } }\n";
    write(dir.path(), "plot.msl", design);
    let out = Command::new(binary())
        .args(["run", "plot.msl", "--analysis", "transient", "--stop", "3e-6", "--print-step", "1e-6", "--print", "n", "--print", "d", "--plot", "out.svg"])
        .current_dir(dir.path())
        .env("PIPERINE_CACHE_DIR", shared_cache())
        .env_remove("PIPERINE_BACKEND")
        .output()
        .expect("run piperine");
    assert_eq!(out.status.code(), Some(0), "stderr:\n{}", String::from_utf8_lossy(&out.stderr));
    let svg = std::fs::read_to_string(dir.path().join("out.svg")).expect("the plot was written");
    assert_eq!(svg.matches("<path class=\"series\" data-signal=\"Top.n\"").count(), 1, "one curve for the real signal: {svg}");
    assert_eq!(svg.matches("<path class=\"lane\" data-signal=\"Top.d\"").count(), 1, "one step trace in a lane for the digital signal: {svg}");
    assert!(svg.contains("class=\"legend-label\" ") && svg.contains(">Top.n</text>") && svg.contains(">Top.d</text>"), "a legend names both: {svg}");
}

#[test]
fn blinky_runs_to_its_stop_in_the_book() {
    // HII-17 AC 4 and HII-22 AC 3: the Book's `Blinky`, a design with no analog node, prints its
    // three records and a table that ends at the stop, 3 us, with the counter at 3, exit 0 (a
    // listing is only held when the command exits as the listing claims)
    let out = run_listing("piperine run blinky.msl --analysis transient --stop 3e-6");
    assert_eq!(out.lines().filter(|l| l.starts_with('@')).count(), 3, "{out}");
    let last = out.lines().rev().find(|l| !l.trim().is_empty()).expect("a table row");
    let cells: Vec<&str> = last.split_whitespace().collect();
    assert_eq!(cells.first(), Some(&"3.000000e-6"), "the table ends at the stop: {last}");
    assert_eq!(cells.last(), Some(&"3"), "the counter has counted three edges: {last}");
}
