//! The repository's own records after the command line (feature `msl-cli`, phase 7; CLI-20): the
//! buck's README says how to run it, the instruction files describe the host crates as built,
//! `GAPS.md` drops what closed and lists what is left, `ROADMAP.md` marks the CLI item done and
//! `.specs/STATE.md` holds the decision. Each test reads one file as text.

use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn the_buck_readme_says_how_to_run_it() {
    // CLI-20 AC 5: `examples/buck/README.md` names `piperine run startup` and no longer says that
    // nothing runs the converter but a Rust test
    let readme = read("examples/buck/README.md");
    assert!(readme.contains("piperine run startup"), "the README names `piperine run startup`");
    assert!(readme.contains("piperine test"), "the README names `piperine test`");
    // long-run LR-18: `startup` prints the whole start, no longer the last microsecond only
    assert!(readme.contains("prints the whole start"), "the README says `startup` prints the whole start");
    assert!(readme.contains("--print-step 1e-8 --csv") && readme.contains("--plot"), "the README shows the fine-grid waveform from flags");
    for stale in ["from = 599e-6", "last microsecond", "two-row table"] {
        assert!(!readme.contains(stale), "the README still says `{stale}`");
    }
    for stale in ["no host API or\nCLI runs MSL yet", "no host API or CLI runs MSL yet", "There is no testbench in MSL"] {
        assert!(!readme.contains(stale), "the README still says `{stale}`");
    }
    assert!(readme.contains("No such host exists yet"), "the Python sketch says the host it sketches does not exist");
}

#[test]
fn the_instruction_files_say_the_host_crates_build() {
    // CLI-20 AC 1: `CLAUDE.md` and `AGENTS.md` no longer say the host crates do not build; their
    // rows say what `msl-cli` left (`Project`/`Session` for `piperine-api`, the seven commands for
    // `piperine-cli`, `msl::format` for `piperine-lang`); `piperine-lang-server` is outside the
    // default build and deferred
    for path in ["CLAUDE.md", "AGENTS.md"] {
        let text = read(path);
        for stale in ["(does not build yet)", "do not build yet", "five do not build", "four crates do not\nbuild", "piperine-cli`, `piperine-project` and the language server do not"] {
            assert!(!text.contains(stale), "{path} still says `{stale}`");
        }
        assert!(text.contains("piperine-lang-server") && text.contains("outside the default build") && text.contains("deferred"), "{path} says the language server is outside the default build and deferred");
        assert!(text.contains("msl::format"), "{path} names `msl::format`");
    }
    let claude = read("CLAUDE.md");
    let row = |name: &str| claude.lines().find(|l| l.starts_with(&format!("| `{name}` |"))).unwrap_or_else(|| panic!("CLAUDE.md has a row for {name}")).to_string();
    let api = row("piperine-api");
    assert!(api.contains("`Project`") && api.contains("`Session`"), "the piperine-api row names Project and Session: {api}");
    let cli = row("piperine-cli");
    for command in ["check", "build", "run", "test", "new", "clean", "fmt"] {
        assert!(cli.contains(&format!("`{command}`")), "the piperine-cli row names `{command}`: {cli}");
    }
    assert!(row("piperine-lang").contains("msl::format"), "the piperine-lang row names `msl::format`");
    assert!(claude.contains("The root package `piperine`") && claude.contains("it builds"), "CLAUDE.md says the root package builds");
    let agents = read("AGENTS.md");
    for name in ["piperine-project", "piperine-api", "piperine-cli", "root `piperine` shell package"] {
        assert!(agents.contains(name), "AGENTS.md names {name}");
    }
}

#[test]
fn gaps_drops_what_closed_and_lists_what_is_left() {
    // CLI-20 AC 2: `GAPS.md` no longer says the CLI and the API do not build, and lists what is
    // left: no host sequences from the command line, no Python host, the language server
    // (deferred by the user), no trace records
    let gaps = read("GAPS.md");
    for stale in ["**`piperine-api`** (165 errors)", "CLI / host API / LSP | **do not build**", "an MSL project\n  can only be run from a Rust test"] {
        assert!(!gaps.contains(stale), "GAPS.md still says `{stale}`");
    }
    for left in ["No host sequences from the command line", "No Python host", "`piperine-lang-server`", "vai ficar para bem depois", "No trace records"] {
        assert!(gaps.contains(left), "GAPS.md lists `{left}`");
    }
    // and what this feature's own reading found
    // (the digital-only transient was listed here too; `host-ii` closed it, L56, and
    // `gaps_drops_what_host_ii_closed` holds that it is gone)
    for found in ["A DC sweep's points deliver no records"] {
        assert!(gaps.contains(found), "GAPS.md lists `{found}`");
    }
    // a gap the command line refuses is loud, not silent (verification F1)
    let silent = gaps.split("## 3. Silent").nth(1).and_then(|s| s.split("## 4.").next()).expect("GAPS.md has a section 3");
    let loud = gaps.split("## 4. Loud").nth(1).and_then(|s| s.split("## 5.").next()).expect("GAPS.md has a section 4");
    assert!(!silent.contains("A DC sweep's points deliver no records") && loud.contains("A DC sweep's points deliver no records"), "the dc gap is listed as loud");
}

#[test]
fn gaps_drops_what_host_ii_closed() {
    // HII-23 AC 1: S2, L56, X3, X4, X7, X8 and X9 are gone from `GAPS.md` as entries (their ids may
    // still be named in prose that says they closed), L39 stays, and what H1 to H4, H6 and H9 say
    // is what is left, not what the feature delivered
    let gaps = read("GAPS.md");
    assert!(!gaps.contains("### S2."), "S2 is still a section");
    assert!(!gaps.contains("| L56 |"), "L56 is still a row");
    for closed in ["X3", "X4", "X7", "X8", "X9"] {
        assert!(!gaps.contains(&format!("- **{closed} \u{2014}")), "{closed} is still an entry");
    }
    assert!(!gaps.contains("A digital-only transient is refused"), "the refusal is still listed");
    assert!(gaps.contains("| L39 | a top module with a continuous port"), "L39 stays");
    let row = |id: &str| gaps.lines().find(|l| l.starts_with(&format!("| {id} |"))).unwrap_or_else(|| panic!("a row {id}")).to_string();
    assert!(row("H1").contains("`Session::drive`") && row("H1").contains("`force` and `release`"), "H1 says what is delivered and what is left");
    assert!(row("H3").contains("`ttol`") && row("H3").contains("`tnom`"), "H3 lists what is left of the options");
    assert!(row("H4").contains("`piperine info`"), "H4 names the delivered metadata");
}

#[test]
fn gaps_drops_what_root_e2e_closed() {
    // root-e2e RE-04 AC 1 (T42): H11, U5 and U10 are gone (the Book is held block by block and
    // sentence by sentence, the parity tests live under `tests/e2e/`), U1 names the twins' new home,
    // section 11 stays, the three observations are host entries with an owner, and the run codes are
    // not listed as unraised
    let gaps = read("GAPS.md");
    assert!(!gaps.contains("| H11 |"), "H11 is still a row");
    for closed in ["U5", "U10"] {
        assert!(!gaps.contains(&format!("- **{closed}.")), "{closed} is still an entry");
    }
    assert!(gaps.contains("## 11. The Book against the binary"), "section 11 stays");
    let unproven = gaps.split("## 8. Unproven").nth(1).and_then(|s| s.split("## 9.").next()).expect("GAPS.md has a section 8");
    let u1 = unproven.split("- **U2.").next().expect("U1 is the first entry");
    assert!(u1.contains("tests/e2e/twins") && !u1.contains("tests/ngspice"), "U1 names tests/e2e/twins: {u1}");
    let row = |id: &str| gaps.lines().find(|l| l.starts_with(&format!("| {id} |"))).unwrap_or_else(|| panic!("a row {id}")).to_string();
    assert!(row("H16").contains("`dt_min`") && row("H16").contains("Owner: `host-analyses`"), "H16: {}", row("H16"));
    assert!(row("H17").contains("`--stream <name>`") && row("H17").contains("Owner: `host-analyses`"), "H17: {}", row("H17"));
    assert!(row("H18").contains("`E0171`") && row("H18").contains("Owner: `cli-packages`"), "H18: {}", row("H18"));
    let unraised = gaps.split("**Error codes of the spec that no source file mentions").nth(1).and_then(|s| s.split("`E0102").nth(1)).and_then(|s| s.split('`').next()).expect("section 10 lists the unraised codes");
    let listed: Vec<&str> = unraised.split_whitespace().collect();
    for raised in ["E0503", "E0504", "E0505", "E0507", "E0508", "E0803", "E0804", "E0805", "E0806"] {
        assert!(!listed.contains(&raised), "{raised} is raised and is listed as unraised");
    }
    assert!(listed.contains(&"E0105") && listed.contains(&"E0906"), "the list keeps the codes nothing raises");
}

#[test]
fn the_roadmap_and_state_record_root_e2e() {
    // root-e2e RE-04 AC 2 and 3 (T43): the roadmap marks queue items 4 and 4b delivered and the lane
    // `root-e2e` delivered; STATE.md holds MD-72 naming the book targets, the inventory and `tests/e2e/`
    // and amending MD-28; the instruction files say the root holds the integrated tests
    let roadmap = read("ROADMAP.md");
    let queue = roadmap.split("## Queue after ir-rust").nth(1).expect("ROADMAP.md has the queue");
    for item in ["4. ", "4b. "] {
        let line = queue.lines().find(|l| l.starts_with(item)).unwrap_or_else(|| panic!("the queue has item {item}"));
        assert!(line.starts_with(&format!("{item}\u{2705} **Delivered")) && line.contains("root-e2e"), "item {item} is delivered: {line}");
    }
    assert!(roadmap.lines().any(|l| l.starts_with("| B next | `root-e2e`") && l.ends_with("| delivered |")), "the lane table says delivered");
    let state = read(".specs/STATE.md");
    let at = state.find("### MD-72:").expect("STATE.md holds MD-72");
    let decision = state[at..].split("\n---").next().expect("a decision");
    for name in ["`tests/book/`", "`tests/book/inventory.tsv`", "`tests/e2e/`", "`tests/scenarios.rs`", "MD-28"] {
        assert!(decision.contains(name), "MD-72 names {name}");
    }
    for file in ["AGENTS.md", "CLAUDE.md"] {
        let text = read(file);
        assert!(text.contains("tests/book/") && text.contains("tests/e2e/") && text.contains("tests/scenarios.rs"), "{file} says the root holds the integrated tests");
        assert!(!text.contains("holds no Rust target"), "{file} still says the root holds no Rust target");
    }
    assert!(read("CLAUDE.md").contains("`tests/e2e/twins/*.cir`"), "CLAUDE.md's parity row names tests/e2e/");
}

#[test]
fn the_roadmap_lists_the_host_follow_ups() {
    // HII-23 AC 2: the roadmap records `host-ii` as delivered and lists the follow-ups: the host
    // follow-up feature (what needs new solver mechanisms) and `root-e2e`
    let roadmap = read("ROADMAP.md");
    assert!(roadmap.lines().any(|l| l.starts_with("3e. ✅ **Delivered") && l.contains("host-ii") && l.contains("MD-69")), "item 3e records host-ii");
    assert!(roadmap.lines().any(|l| l.starts_with("| B | `host-ii`") && l.ends_with("| delivered |")), "the lane table says delivered");
    assert!(roadmap.lines().any(|l| l.starts_with("| B next | host follow-up") && l.contains("`force`") && l.contains("continued transients")), "the host follow-up row");
    assert!(roadmap.lines().any(|l| l.starts_with("| B next | `root-e2e`")), "the root-e2e row");
}

#[test]
fn state_records_the_derived_param_decision() {
    // HII-23 AC 3: `.specs/STATE.md` holds MD-69 naming the generated derive function, the unchanged
    // MD-18 and the library surface of the drive and the timed `set`; the instruction files name the
    // new api modules and the new command
    let state = read(".specs/STATE.md");
    let at = state.find("### MD-69:").expect("STATE.md holds MD-69");
    let decision = state[at..].split("\n---").next().expect("a decision");
    for name in ["`piperine_derive_1(", "MD-18", "`Session::drive`", "`Session::set_at`", "S2"] {
        assert!(decision.contains(name), "MD-69 names {name}");
    }
    let claude = read("CLAUDE.md");
    let api = claude.lines().find(|l| l.starts_with("| `piperine-api` |")).expect("a piperine-api row");
    for module in ["`metadata.rs`", "`params.rs`", "`drive`", "`set_at`"] {
        assert!(api.contains(module), "the piperine-api row names {module}");
    }
    assert!(claude.lines().any(|l| l.starts_with("| `piperine-cli` |") && l.contains("`info`")), "the piperine-cli row names `info`");
    assert!(read("AGENTS.md").contains("check build run test info"), "AGENTS.md lists `info`");
}

#[test]
fn the_roadmap_marks_the_cli_item_done() {
    // CLI-20 AC 3: item 3 of "Queue after ir-rust" is marked done; item 3b is left as it is
    let roadmap = read("ROADMAP.md");
    let queue = roadmap.split("## Queue after ir-rust").nth(1).expect("ROADMAP.md has the queue");
    let item = queue.lines().find(|l| l.starts_with("3. ")).expect("the queue has an item 3");
    assert!(item.starts_with("3. ✅ **Delivered"), "item 3 is marked done: {item}");
    assert!(item.contains("msl-cli"), "item 3 names the feature: {item}");
    assert!(queue.lines().any(|l| l.starts_with("3b. **Spec-completeness planning pass")), "item 3b stays as it is");
}

#[test]
fn state_records_the_cli_decision() {
    // CLI-20 AC 4: `.specs/STATE.md` holds a decision naming `piperine-api`, the `[run.<name>]`
    // table, the bench crate and `msl::format`
    let state = read(".specs/STATE.md");
    let at = state.find("### MD-65:").expect("STATE.md holds MD-65");
    let decision = state[at..].split("\n---").next().expect("a decision");
    for name in ["`piperine-api`", "`[run.<name>]`", "`benches/`", "`msl::format`"] {
        assert!(decision.contains(name), "MD-65 names {name}");
    }
}

#[test]
fn gaps_says_a_host_run_is_bounded() {
    // long-run LR-18 AC 2: a host run records what it prints and keeps no step, so `GAPS.md` no
    // longer lists the unbounded recording of a host run as open; the solver's own default keeps
    // every step (N2) and `GAPS.md` still says so; the loud gap of `on above` closed with msl-fixes
    let gaps = read("GAPS.md");
    for stale in ["a host\n  needs a bounded recording", "a 600 µs run held 25 GB"] {
        assert!(!gaps.contains(stale), "GAPS.md still says `{stale}`");
    }
    assert!(gaps.contains("it records only the signals it prints"), "GAPS.md says a host run records only what it prints");
    assert!(gaps.contains("(N2)") && gaps.contains("keeps every accepted step"), "GAPS.md still says the solver's default keeps every step");
    // (the loud gap of `on above(..) { emit!(..) }` closed with msl-fixes FIX-13, L55)
    assert!(!gaps.contains("`on above(..) { emit!(..) }` is `P0001`"), "GAPS.md no longer lists the closed `on above` gap");
    assert!(gaps.contains("prints the whole 600 µs start"), "the command-line row names the whole-start run");
}

#[test]
fn the_roadmap_lists_what_the_long_run_leaves() {
    // long-run LR-18 AC 2: `ROADMAP.md` marks `long-run` delivered and names the three futures
    // of decision 6
    let roadmap = read("ROADMAP.md");
    let queue = roadmap.split("## Queue after ir-rust").nth(1).expect("ROADMAP.md has the queue");
    let item = queue.lines().find(|l| l.starts_with("3c. ")).expect("the queue has an entry for long-run");
    assert!(item.starts_with("3c. ✅ **Delivered") && item.contains("long-run"), "the entry is delivered and names the feature: {item}");
    for future in ["Multi-rate stepping", "Surrogate or averaged models", "Periodic steady state"] {
        assert!(queue.contains(future), "ROADMAP.md names `{future}`");
    }
}

#[test]
fn state_records_the_observation_decision() {
    // long-run LR-07 AC 3, LR-18: `.specs/STATE.md` holds MD-66, which says observation is the
    // run's, and extends MD-65 without superseding it
    let state = read(".specs/STATE.md");
    let at = state.find("### MD-66:").expect("STATE.md holds MD-66");
    let decision = state[at..].split("\n---").next().expect("a decision");
    assert!(decision.contains("observation is the run's"), "MD-66 says observation is the run's");
    assert!(decision.contains("`[run.<name>]`") && decision.contains("extends MD-65"), "MD-66 names the run table and MD-65");
}

#[test]
fn gaps_drops_what_msl_fixes_closed() {
    // msl-fixes DOC-01 AC 1: `GAPS.md` drops the entries the feature closed, keeps what is left
    // (`simparam`, S3, H.5 to H.7, `normal`, `limexp`), and the counts follow
    let gaps = read("GAPS.md");
    for id in ["B1", "B2", "S1", "S5"] {
        assert!(!gaps.contains(&format!("### {id}.")), "GAPS.md still has the entry {id}");
    }
    for id in ["L11", "L14", "L15", "L17", "L41", "L46", "L48", "L54", "L55"] {
        assert!(!gaps.contains(&format!("| {id} |")), "GAPS.md still has the row {id}");
    }
    // L47 keeps `simparam` only; L16 keeps H.5 to H.7; L58 drops `db10`/`db20` and `absdelta`
    let row = |id: &str| gaps.lines().find(|l| l.starts_with(&format!("| {id} |"))).unwrap_or_else(|| panic!("GAPS.md has the row {id}")).to_string();
    assert!(row("L47").contains("`simparam`") && !row("L47").contains("`freq()`, "), "L47 keeps simparam: {}", row("L47"));
    assert!(row("L16").contains("H.5, H.6, H.7") && !row("L16").contains("unwrap expect"), "L16 keeps H.5 to H.7: {}", row("L16"));
    assert!(!row("L58").contains("db10") && !row("L58").contains("absdelta") && row("L58").contains("`limexp`") && row("L58").contains("`normal`"), "L58: {}", row("L58"));
    // (`### S2.` was kept here until `host-ii` closed it: `gaps_drops_what_host_ii_closed`)
    for kept in ["### S3.", "### S4.", "### S6.", "### B3.", "### B4."] {
        assert!(gaps.contains(kept), "GAPS.md keeps {kept}");
    }
    // the counts follow, as `host-ii` moved them: 3 silent, 2 broken
    assert!(gaps.contains("3 silent, 2 broken, 49 loud"), "the counts of §9 follow");
    let rows = |section: &str, next: &str| gaps.split(section).nth(1).and_then(|s| s.split(next).next()).expect("a section").lines().filter(|l| l.starts_with("### ")).count();
    assert_eq!(rows("## 3. Silent", "## 4."), 3, "three silent entries");
    assert_eq!(rows("## 2. Broken", "## 3."), 2, "two broken entries");
    let loud = gaps.split("## 4. Loud").nth(1).and_then(|s| s.split("## 5.").next()).expect("section 4");
    assert_eq!(loud.lines().filter(|l| l.starts_with("| L")).count(), 49, "forty-nine loud rows");
}

#[test]
fn the_roadmap_and_state_record_msl_fixes() {
    // msl-fixes DOC-01 AC 2: the queue marks `msl-fixes` delivered and says what it leaves;
    // `.specs/STATE.md` holds MD-67 naming the catalogue, `E0161`, the `(is, val)` pair and the
    // amendment of abi.md A4
    let roadmap = read("ROADMAP.md");
    let queue = roadmap.split("## Queue after ir-rust").nth(1).expect("ROADMAP.md has the queue");
    let item = queue.lines().find(|l| l.starts_with("3d. ")).expect("the queue has an entry for msl-fixes");
    assert!(item.starts_with("3d. ✅ **Delivered") && item.contains("msl-fixes"), "the entry is delivered and names the feature: {item}");
    for left in ["S3", "H.5 to H.7", "`simparam`", "`limexp`"] {
        assert!(item.contains(left), "the entry names what is left: `{left}`");
    }
    let state = read(".specs/STATE.md");
    let at = state.find("### MD-67:").expect("STATE.md holds MD-67");
    let decision = state[at..].split("\n---").next().expect("a decision");
    for name in ["Appendix H", "`E0161`", "`(is, val)`", "`abi.md` A4", "`fatal`"] {
        assert!(decision.contains(name), "MD-67 names {name}");
    }
}
