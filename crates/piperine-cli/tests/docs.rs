//! The repository's own records after the command line (feature `msl-cli`, phase 7; CLI-20): the
//! buck's README says how to run it, the instruction files describe the host crates as built,
//! `GAPS.md` drops what closed and lists what is left, `ROADMAP.md` is forward-looking and
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
fn the_buck_readme_says_when_to_run_the_buck_on_demand() {
    // integrated-tests IT-13 AC 1: the "On demand" block holds the four `cargo test` lines of every
    // registered target, says that `cargo test` skips them, when they run, and how to only compile them
    let readme = read("examples/buck/README.md");
    let block = &readme[readme.find("## On demand").expect("the README has an \"On demand\" block")..];
    for line in [
        "cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_boundary --test buck_power --test buck_supervisor --test buck_closed_loop",
        "cargo test -p piperine-api --test buck_long",
        "cargo test -p piperine-cli --test buck --test book_buck",
        "cargo test -p piperine --test buck_scenarios --test book_buck",
    ] {
        assert!(block.contains(line), "the block holds `{line}`");
    }
    for held in ["`cargo test` skips", "once at a feature's close", "examples/buck/**", "--no-run", "cargo test -p piperine --test integrated"] {
        assert!(block.contains(held), "the block says `{held}`");
    }
}

#[test]
fn the_instructions_name_the_integrated_gate_and_the_buck_on_demand() {
    // integrated-tests IT-13 AC 2: `CLAUDE.md` names the integrated target as the per-task gate, says that
    // `cargo test` skips the buck and points at the README's "On demand" block; "Where the tests are"
    // names `tests/integrated/` and `tests/e2e/integrated/`; `AGENTS.md` names the gate too
    let claude = read("CLAUDE.md");
    for held in ["cargo test -p piperine --test integrated", "cargo test -p piperine-cli --test integrated", "`cargo test` skips every target that simulates the buck", "examples/buck/README.md", "\"On demand\"", "tests/integrated/", "tests/e2e/integrated/"] {
        assert!(claude.contains(held), "CLAUDE.md names `{held}`");
    }
    let agents = read("AGENTS.md");
    for held in ["cargo test -p piperine --test integrated", "examples/buck/README.md", "tests/integrated/"] {
        assert!(agents.contains(held), "AGENTS.md names `{held}`");
    }
}

#[test]
fn the_buck_readme_points_at_the_scenarios() {
    // buck-scenarios BS-10 AC 1: S2 to S10 run from `tests/buck_scenarios.rs` over `tests/e2e/buck/`
    // through `Session::drive` and `Session::set_at`; the README no longer says `piperine-api` has no
    // drives or mid-run `set`, and only the Python sketch keeps "No such host exists yet"
    let readme = read("examples/buck/README.md");
    for held in ["tests/buck_scenarios.rs", "tests/e2e/buck/", "Session::drive", "Session::set_at", "S2 to S10"] {
        assert!(readme.contains(held), "the README names `{held}`");
    }
    for stale in ["has no drives, waits or mid-run `set`", "which no host offers yet", "no host offers"] {
        assert!(!readme.contains(stale), "the README still says `{stale}`");
    }
    assert!(readme.contains("No such host exists yet for Python"), "the Python sketch keeps saying that no Python host exists");
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
    // (L39, which stayed, is closed by `params-and-io`: `gaps_no_longer_lists_x5_l20_l39_h12`)
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
    let unraised = gaps.split("**Error codes of the spec that no source file mentions").nth(1).and_then(|s| s.split("`E0110").nth(1)).and_then(|s| s.split('`').next()).expect("section 10 lists the unraised codes");
    let listed: Vec<&str> = unraised.split_whitespace().collect();
    for raised in ["E0503", "E0504", "E0505", "E0507", "E0508", "E0803", "E0804", "E0805", "E0806"] {
        assert!(!listed.contains(&raised), "{raised} is raised and is listed as unraised");
    }
    assert!(listed.contains(&"E0111") && listed.contains(&"E0614"), "the list keeps the codes nothing raises");
}

#[test]
fn gaps_narrows_what_buck_scenarios_ran() {
    // buck-scenarios BS-10 AC 2: U3 keeps only S1 at 3.3 V and S1b and names `buck-scenarios` for S2 to
    // S10, and H1 no longer says the buck's scenarios have no host (it keeps what is delivered and what
    // is left)
    let gaps = read("GAPS.md");
    let u3 = gaps.split("- **U3.").nth(1).and_then(|s| s.split("- **U4.").next()).expect("GAPS.md has an entry U3");
    assert!(u3.contains("S2 to S10") && u3.contains("`buck-scenarios`"), "U3 says S2 to S10 run: {u3}");
    assert!(u3.contains("S1 at 3.3 V") && u3.contains("S1b"), "U3 keeps S1 at 3.3 V and S1b: {u3}");
    assert!(!u3.contains("S1 to S9") && !u3.contains("need H1"), "U3 no longer needs H1: {u3}");
    let row = |id: &str| gaps.lines().find(|l| l.starts_with(&format!("| {id} |"))).unwrap_or_else(|| panic!("a row {id}")).to_string();
    let h1 = row("H1");
    assert!(!h1.contains("S1 to S9") && !h1.contains("have no host yet"), "H1 still says the scenarios have no host: {h1}");
    assert!(h1.contains("S2 to S10") && h1.contains("`buck-scenarios`"), "H1 says S2 to S10 run on the Rust host: {h1}");
    assert!(h1.contains("`Session::drive`") && h1.contains("`force` and `release`"), "H1 keeps what is delivered and what is left: {h1}");
}

#[test]
fn state_records_root_e2e() {
    // root-e2e RE-04 AC 2 and 3 (T43), the delivery record moved out of ROADMAP.md on its 2026-11-21
    // rewrite (see `the_roadmap_is_forward_looking`): STATE.md holds MD-72 naming the book targets, the inventory and `tests/e2e/`
    // and amending MD-28; the instruction files say the root holds the integrated tests
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
    assert!(!row("L58").contains("db10") && !row("L58").contains("absdelta") && !row("L58").contains("`limexp`") && row("L58").contains("`normal`"), "L58 (limexp left with unknown-semantics-events T17): {}", row("L58"));
    // (`### S2.` was kept here until `host-ii` closed it: `gaps_drops_what_host_ii_closed`)
    // (`### S4.` was kept here until `analog-structure` closed it, T35: `gaps_drops_what_analog_structure_closed`)
    for kept in ["### S3.", "### S6.", "### S7.", "### B3.", "### B4."] {
        assert!(gaps.contains(kept), "GAPS.md keeps {kept}");
    }
    // the counts follow, as `host-ii` moved them, `unknown-semantics` (L18, L19) after it and
    // `analog-structure` (nine loud rows and S4) after that, `unknown-semantics-events` (L52) after
    // that: 2 silent, 2 broken, 40 loud; the value-semantics amendment of the spec then added S7
    // (the front end is the earlier language): 3 silent; `params-and-io` closed L20 and L39 (38 loud);
    // `resolved-nets` T10 closed L22 and L23 (36 loud)
    assert!(gaps.contains("3 silent, 2 broken, 36 loud"), "the counts of §9 follow");
    let rows = |section: &str, next: &str| gaps.split(section).nth(1).and_then(|s| s.split(next).next()).expect("a section").lines().filter(|l| l.starts_with("### ")).count();
    assert_eq!(rows("## 3. Silent", "## 4."), 3, "three silent entries");
    assert_eq!(rows("## 2. Broken", "## 3."), 2, "two broken entries");
    let loud = gaps.split("## 4. Loud").nth(1).and_then(|s| s.split("## 5.").next()).expect("section 4");
    assert_eq!(loud.lines().filter(|l| l.starts_with("| L")).count(), 36, "thirty-six loud rows");
}

#[test]
fn state_records_msl_fixes() {
    // msl-fixes DOC-01 AC 2: the queue marks `msl-fixes` delivered and says what it leaves;
    // `.specs/STATE.md` holds MD-67 naming the catalogue, `E0161`, the `(is, val)` pair and the
    // amendment of abi.md A4
    let state = read(".specs/STATE.md");
    let at = state.find("### MD-67:").expect("STATE.md holds MD-67");
    let decision = state[at..].split("\n---").next().expect("a decision");
    for name in ["Appendix H", "`E0161`", "`(is, val)`", "`abi.md` A4", "`fatal`"] {
        assert!(decision.contains(name), "MD-67 names {name}");
    }
}

#[test]
fn state_records_the_unknown_semantics_decision() {
    // US-07 (design "New project-level decision"): `.specs/STATE.md` holds MD-70, which says the
    // merge runs in generated code, the ABI's additive entry, the record's `maybe` and the
    // deletion point of the contract lowering (Q1 (b)); it extends MD-62 and MD-64 and
    // supersedes nothing
    let state = read(".specs/STATE.md");
    let at = state.find("### MD-70:").expect("STATE.md holds MD-70");
    let decision = state[at..].split("\n---").next().expect("a decision");
    for name in ["runs in the generated Rust", "`Host.commit_with(", "`maybe`", "`generics`", "`std/values.msl`", "64", "extends MD-62", "MD-64"] {
        assert!(decision.contains(name), "MD-70 names {name}");
    }
    assert!(decision.contains("It supersedes nothing"), "MD-70 supersedes nothing");
}

#[test]
fn abi_md_describes_the_unreleased_contract() {
    // EU-16, EU-17 AC 5 (replaces `abi_states_commits_under_unknown_control`, which pinned
    // `commit_with`, `commit_flag` and rule C7): the contract is still major 1 and says it is
    // unreleased; `MINOR` is 0; `Field` has four fields; the shipped layouts hold no flag byte;
    // a drive pin carries the driven value and a plain `driving` byte; one `commit`.
    let abi = read(".specs/features/device-abi/abi.md");
    let flat = abi.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(flat.contains("| version | `MAJOR` 1, `MINOR` 0, symbol `piperine_abi_1`;"), "major 1, minor 0, symbol piperine_abi_1");
    assert!(flat.contains("pub struct Field { name: Str, leaf: u32, offset: u32, count: u32 }"), "Field has four fields");
    assert!(flat.contains("`BIT` is `{ v: BOOL @0 }` (size 1, align 1)") && flat.contains("`BYTE` is `{ v: U8 @0 }` (size 1, align 1)") && flat.contains("`F64` is `{ v: F64 @0 }` (size 8, align 8)"), "the three shipped layouts");
    assert!(flat.contains("`driving: BOOL`") && flat.contains("0 is a drive *absent*"), "the drive pin carries a plain `driving` byte");
    assert!(flat.contains("**Unreleased.** This ABI has had no release"), "abi.md says it is unreleased");
    assert!(flat.contains("Until the first release it changes in place"), "abi.md says it changes in place until the first release");
    assert!(flat.contains("## 2. Entry points (6)"), "six entry points");
    for gone in ["HAS_KNOWN", "commit_with", "commit_flag", "known byte", "- **C7."] {
        assert!(!abi.contains(gone), "abi.md still says {gone}");
    }
}

#[test]
fn gaps_drops_l18_and_l19() {
    // US-07 AC 1 (C42): `unknown-semantics` closed L18 (`Unknown` as an initial value, `level`,
    // `merge`, `same`, `cast`, `logic`, `matches_bits`) and L19 (unknown control is a merge, not a
    // fault): neither is a row, and no instruction file calls the merge a gap; S6 and G6 (one flag
    // per value, an ABI change) stay
    let gaps = read("GAPS.md");
    for closed in ["L18", "L19"] {
        assert!(!gaps.lines().any(|l| l.starts_with(&format!("| {closed} |"))), "{closed} is still a row");
    }
    assert!(!gaps.contains("Unknown control is a fault, not a merge"), "the fault of an unknown `if` is still listed");
    assert!(gaps.contains("### S6. Unknown integers are one flag per value"), "S6 stays");
    assert!(gaps.contains("- **G6 \u{2014} unknown integers are per value** (S6)."), "G6 stays");
    let coverage = gaps.split("## 9. Coverage by spec part").nth(1).and_then(|s| s.split("## 10.").next()).expect("GAPS.md has a section 9");
    let row = coverage.lines().find(|l| l.starts_with("| I §3–§3.3 ")).expect("the coverage row of §3.3");
    assert!(!row.contains("L19") && !row.contains("`level`"), "the coverage row still lists the merge and `level` as gaps: {row}");
}

#[test]
fn gaps_carries_the_unknown_index_merge_to_composite_values() {
    // US-04 AC 11, 12 (C31): the loops and the arrays do not exist yet, so their merge criteria
    // travel with the rows that will add them: L5 (loops, bound 64) and L3 and L24 (an index that
    // may be unknown)
    let gaps = read("GAPS.md");
    let row = |id: &str| gaps.lines().find(|l| l.starts_with(&format!("| {id} |"))).unwrap_or_else(|| panic!("a row {id}")).to_string();
    assert!(row("L5").contains("64 iterations") && row("L5").contains("US-04 AC 11"), "L5 carries the loop bound");
    for id in ["L3", "L24"] {
        assert!(row(id).contains("merges every element it may name") && row(id).contains("US-04 AC 12"), "{id} carries the index merge");
    }
}

#[test]
fn gaps_drops_what_analog_structure_closed() {
    // analog-structure AS-18 AC 4 (T36): `GAPS.md` drops L26, L27, L34 to L38, L42, L50 and S4, and the
    // stdlib `slew` leaves the list of L58 (it runs; an ideal step is L60, owned by `analog-operators`)
    let gaps = read("GAPS.md");
    for id in ["L26", "L27", "L34", "L35", "L36", "L37", "L38", "L42", "L50"] {
        assert!(!gaps.contains(&format!("| {id} |")), "GAPS.md still has the row {id}");
    }
    assert!(!gaps.contains("### S4."), "GAPS.md still has the entry S4");
    let row = |id: &str| gaps.lines().find(|l| l.starts_with(&format!("| {id} |"))).unwrap_or_else(|| panic!("GAPS.md has the row {id}")).to_string();
    assert!(!row("L58").contains("`slew`"), "L58 no longer lists slew: {}", row("L58"));
    assert!(row("L60").contains("slew") && row("L60").contains("analog-operators"), "L60 records the ideal step and its owner: {}", row("L60"));
    // the cluster is delivered and the counts follow: 2 silent, 2 broken, 40 loud (checked above)
    assert!(gaps.contains("| Analog structure (delivered, MD-71) |"), "the cluster is marked delivered");
}

#[test]
fn gaps_keeps_l40_for_the_spec_models_that_wait() {
    // analog-structure AS-18 AC 4, Q2 (T36): one L40 line is left, for the three spec models that wait on
    // another feature's dependency: the stdlib `idt` and `Water` (`generics`) and the §18 `Diode`
    // (`params-and-io`)
    let gaps = read("GAPS.md");
    let rows: Vec<&str> = gaps.lines().filter(|l| l.starts_with("| L40 |")).collect();
    assert_eq!(rows.len(), 1, "one L40 row: {rows:?}");
    for name in ["`idt`", "`Water`", "`Diode`", "`generics`", "`params-and-io`"] {
        assert!(rows[0].contains(name), "L40 names {name}: {}", rows[0]);
    }
}

#[test]
fn state_records_analog_structure() {
    // analog-structure AS-18 AC 4 (T36): the queue marks item 7 delivered and names what is left; STATE.md
    // holds MD-71 with the decisions of the design: a contribution stays in place, the derivative site, the
    // key params, hoisting and the reach
    let state = read(".specs/STATE.md");
    let at = state.find("### MD-71:").expect("STATE.md holds MD-71");
    let decision = state[at..].split("\n---").next().expect("a decision");
    for name in ["switch branch", "derivative site", "key params", "`Name~p=v`", "hoisted", "`Terminal::Own`", "`p.inflow()`"] {
        assert!(decision.contains(name), "MD-71 names {name}");
    }
}

#[test]
fn the_buck_spec_holds_the_bench_bounds() {
    // BS-09 AC 2 to AC 5: what the scenarios cite is in the SPEC they are judged against
    let spec = read("examples/buck/SPEC.md");
    let budget = &spec[spec.find("### 4.3 Regulation budget").expect("SPEC has §4.3")..spec.find("## 5.").expect("§5 follows §4.3")];
    let cells = |label: &str| -> Vec<String> {
        let row = budget.lines().find(|l| l.starts_with(&format!("| {label}"))).unwrap_or_else(|| panic!("SPEC §4.3 has a row `{label}`"));
        row.trim_matches('|').split('|').map(|cell| cell.trim().to_string()).collect()
    };
    // AC 2: §4.3 has a 3.3 V column, the middle one, with each row and the total
    assert!(spec.contains("| Contribution | At 0.8 V | At 3.3 V | At 5.0 V |"), "§4.3 has a 3.3 V column");
    for (label, at_3v3) in [("ADC quantization", "±3.1 mV"), ("Ripple aliasing", "±1.7 mV"), ("Divider", "±6.6 mV"), ("ADC reference", "±3.3 mV")] {
        assert_eq!(cells(label)[2], at_3v3, "§4.3 row `{label}` at 3.3 V");
    }
    assert!(
        spec.contains("| **Total, worst case** | **±6.5 mV** (target ±10 mV) | **±14.7 mV = 0.45 %** (target ±1 %) | **±20 mV = 0.41 %** (target ±1 %) |"),
        "§4.3 total row"
    );
    // AC 3: S2b has its settling tolerance
    let s2b = spec.lines().find(|l| l.starts_with("| S2b |")).expect("the S2b row");
    assert!(s2b.contains("within ±1 % of 1.0 V at 1.38 ms ±0.05 ms"), "{s2b}");
    // AC 4: §10.1 defines the stimulus shapes, the measurements and the bounds with their derivations
    let section = &spec[spec.find("### 10.1 How the bench measures").expect("SPEC has §10.1")..spec.find("## 11.").expect("§11 follows §10.1")];
    for held in ["Setpoint write", "staircase", "Line step", "Brown-out", "Settling time", "Deviation", "10 ns", "2.02 µs", "4.52 µs", "0.1 A", "3.4 A/µs", "±2 %", "1.452 ms", "450 µs", "9.2 codes", "≈"] {
        assert!(section.contains(held), "§10.1 holds `{held}`");
    }
    // AC 5: the cleared fault code is `Clear`, never `None`
    assert!(spec.contains("(`Clear`, `OverCurrent`, `OverVoltage`)"), "§8.5 names `Clear`");
    assert!(!spec.contains("`None`"), "the SPEC no longer names the cleared fault code `None`");
}

#[test]
fn the_book_no_longer_says_settle_is_not_compiled() {
    // unknown-semantics-events UE-07 AC 2 (T22): `settle` runs, so the Book's status paragraph, the buck's
    // "Run it" box and the mixed chapter's introduction no longer list it among what the compiler refuses
    let book = read("docs/book/content.html");
    for stale in ["`settle` event.", "<code>settle</code> event.", "the check's <code>settle</code> is not compiled yet", "not compiled yet and is refused with P0001: the <code>settle</code> event"] {
        assert!(!book.contains(stale), "the Book still says {stale:?}");
    }
    assert!(!book.contains("both commands refuse the file and run nothing"), "the buck's Run it box still says the file is refused");
}

#[test]
fn gaps_no_longer_lists_settle_or_limexp() {
    // unknown-semantics-events UE-07 AC 1 (T23): `GAPS.md` drops L52 (`settle`) and `limexp` from L58, section 1's
    // "stdlib that runs" names `settle` and `limexp`, the coverage rows of IV section 13 and V section 15 no longer
    // list `settle`; L51 and L53 stay until the feature's last batch
    let gaps = read("GAPS.md");
    assert!(!gaps.contains("| L52 |"), "GAPS.md still has the row L52");
    let row = |id: &str| gaps.lines().find(|l| l.starts_with(&format!("| {id} |"))).unwrap_or_else(|| panic!("GAPS.md has the row {id}")).to_string();
    assert!(!row("L58").contains("`limexp`"), "L58 still lists limexp: {}", row("L58"));
    assert!(row("L51").contains("only `Pfd3` remains") && !row("L51").contains("refused"), "L51 names only Pfd3: {}", row("L51"));
    assert!(row("L53").contains("`Event`"), "L53 stays");
    let runs = gaps.lines().find(|l| l.starts_with("| stdlib that runs |")).expect("section 1 lists the stdlib that runs");
    assert!(runs.contains("`settle`") && runs.contains("`limexp`"), "section 1 names settle and limexp: {runs}");
    let coverage = gaps.split("## 9. Coverage by spec part").nth(1).and_then(|s| s.split("## 10.").next()).expect("GAPS.md has a section 9");
    for prefix in ["| IV §12–§13 ", "| V §15–§18 "] {
        let line = coverage.lines().find(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("the coverage row {prefix}"));
        assert!(!line.contains("`settle`"), "{line}");
        assert!(!line.contains("an event as a `bool`"), "{line}");
    }
}

#[test]
fn gaps_no_longer_lists_x5_l20_l39_h12() {
    // params-and-io PI-14 AC 1 (T28): X5, L20, L39 and H12 are gone from `GAPS.md` as entries and rows
    // (the numbering of the other sections is kept), section 10 lists exactly the rows of the guard's
    // allow-list `code_owners.tsv`, and the roadmap and the state record what B1 to B4 delivered
    let gaps = read("GAPS.md");
    for closed in ["| L20 |", "| L39 |", "| H12 |", "- **X5 \u{2014}"] {
        assert!(!gaps.contains(closed), "{closed} is still in GAPS.md");
    }
    assert!(gaps.contains("| L21 |") && gaps.contains("| H13 |") && gaps.contains("- **X6 \u{2014}"), "the neighbours keep their numbers");
    let owners = read("crates/piperine-lang/tests/support/code_owners.tsv");
    let mut allowed: Vec<&str> = owners.lines().filter(|line| !line.starts_with('#') && !line.is_empty()).filter_map(|line| line.split('\t').next()).collect();
    allowed.sort_unstable();
    let listed = gaps.split("**Error codes of the spec that no source file mentions").nth(1).and_then(|s| s.split("`E0110").nth(1)).and_then(|s| s.split('`').next()).expect("section 10 lists the unraised codes");
    let mut listed: Vec<&str> = std::iter::once("E0110").chain(listed.split_whitespace()).collect();
    listed.sort_unstable();
    assert_eq!(listed, allowed, "section 10 lists exactly the rows of code_owners.tsv");
    assert!(gaps.contains(&format!("({} of ", allowed.len())), "the heading counts the listed codes");
    let roadmap = read("ROADMAP.md");
    let row = roadmap.lines().find(|line| line.contains("| `params-and-io` |")).expect("the roadmap has a params-and-io row");
    assert!(row.contains("B5–B6") && row.contains("composite-values"), "the row says B5 and B6 wait on composite-values: {row}");
    let state = read(".specs/STATE.md");
    assert!(state.contains("### MD-75:") && state.contains("`params-and-io`"), "STATE.md holds the decision entry");
}

#[test]
fn gaps_no_longer_lists_project_or_signal_flow_natures() {
    // resolved-nets T10 (RN-10 AC 1, RN-01, RN-08): a nature the project declares and a signal-flow
    // nature run, so L22 and L23 are no rows, and `modules#8` of the §11 table keeps only the
    // generic `idt` that `generics` owns
    let gaps = read("GAPS.md");
    for closed in ["L22", "L23"] {
        assert!(!gaps.lines().any(|l| l.starts_with(&format!("| {closed} |"))), "{closed} is still a row");
    }
    assert!(!gaps.contains("`discrete nature Byte`") && !gaps.contains("`signal-flow nature VSignal`"), "a probe of the closed rows is still listed");
    let row = gaps.lines().find(|l| l.starts_with("| modules#8 |")).expect("the Book row modules#8");
    assert!(row.contains("| L30 |") && row.contains("`generics`") && !row.contains("signal-flow nature"), "modules#8 keeps only the generic `idt`: {row}");
    assert!(gaps.lines().any(|l| l.starts_with("| L21 |")), "L21 stays: resolved natures are not compiled yet");
}

#[test]
fn the_roadmap_is_forward_looking() {
    // ROADMAP.md rewrite (user, 2026-11-21: "limpar ... coloca certinho, tabelado, o que tem no pool de
    // desenvolvimento"): the roadmap lists only what is in flight or waits, in tables, and no longer
    // carries delivery history (that lives in STATE.md, held by the `state_records_*` tests above);
    // what long-run, host-ii and msl-fixes left is still named as a future row
    let roadmap = read("ROADMAP.md");
    for section in ["## Development pool", "### In flight", "### Waiting, in order", "### After the review", "## Decisions waiting for the user"] {
        assert!(roadmap.contains(section), "ROADMAP.md has `{section}`");
    }
    for stale in ["## Queue after ir-rust", "✅ **Delivered", "PHDL", "Python host:", "## V1 status"] {
        assert!(!roadmap.contains(stale), "ROADMAP.md still carries `{stale}`");
    }
    let row = |name: &str| roadmap.lines().find(|l| l.contains(&format!("| `{name}`"))).unwrap_or_else(|| panic!("ROADMAP.md has a `{name}` row"));
    for future in ["multi-rate", "surrogate-models", "periodic-small-signal", "host-iii", "transient-performance", "ngspice-models", "spec-closure", "code-clarity"] {
        row(future);
    }
    assert!(row("host-iii").contains("`force`") && row("host-iii").contains("continued transients"), "the host follow-up row");
    assert!(row("spec-closure").contains("item 13"), "spec-closure is item 13");
    assert!(roadmap.contains("stops after item 13"), "the roadmap says where the automatic run stops");
}
