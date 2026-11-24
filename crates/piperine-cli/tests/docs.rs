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
    // (deferred by the user); "No trace records" (L49) left with `analog-operators` T37 and T38
    let gaps = read("GAPS.md");
    for stale in ["**`piperine-api`** (165 errors)", "CLI / host API / LSP | **do not build**", "an MSL project\n  can only be run from a Rust test"] {
        assert!(!gaps.contains(stale), "GAPS.md still says `{stale}`");
    }
    for left in ["No host sequences from the command line", "No Python host", "`piperine-lang-server`", "vai ficar para bem depois"] {
        assert!(gaps.contains(left), "GAPS.md lists `{left}`");
    }
    assert!(!gaps.contains("No trace records"), "No trace records (L49) is closed: emit! in analog is a trace");
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
    assert!(row("H3").contains("`ttol`"), "H3 lists what is left of the options");
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
    let unraised = gaps.split("**Error codes of the spec that no source file mentions").nth(1).and_then(|s| s.split("by the elaborator.\n`").nth(1)).and_then(|s| s.split('`').next()).expect("section 10 lists the unraised codes");
    let listed: Vec<&str> = unraised.split_whitespace().collect();
    for raised in ["E0503", "E0504", "E0505", "E0507", "E0508", "E0803", "E0804", "E0805", "E0806"] {
        assert!(!listed.contains(&raised), "{raised} is raised and is listed as unraised");
    }
    assert!(listed.contains(&"E0123") && listed.contains(&"E0807") && !listed.contains(&"E0702"), "the list keeps the codes nothing raises, and E0702 is raised since T45");
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
    for name in ["`piperine_derive_3(", "MD-18", "`Session::drive`", "`Session::set_at`", "S2"] {
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
    // L47 (`simparam`) closed with `analog-operators` T35 (`gaps_and_roadmap_follow_analog_operators`); L16 keeps H.5 to H.7; L58 drops `db10`/`db20` and `absdelta`
    let row = |id: &str| gaps.lines().find(|l| l.starts_with(&format!("| {id} |"))).unwrap_or_else(|| panic!("GAPS.md has the row {id}")).to_string();
    assert!(!gaps.contains("| L47 |"), "L47 (simparam) is closed: it ran from source with analog-operators");
    // L16 left with composite-values phase B: H.5 to H.7 run (`gaps_drops_what_composite_values_phase_b_closed`)
    assert!(!row("L58").contains("db10") && !row("L58").contains("absdelta") && !row("L58").contains("`limexp`"), "L58 (limexp left with unknown-semantics-events T17): {}", row("L58"));
    // (`### S2.` was kept here until `host-ii` closed it: `gaps_drops_what_host_ii_closed`)
    // (`### S4.` was kept here until `analog-structure` closed it, T35: `gaps_drops_what_analog_structure_closed`)
    // (`### S6.` was kept here until `explicit-unknowns` closed it by the redesign and `### S8.` until its phase A closed that:
    // `gaps_closes_s6_and_tracks_the_flag`)
    // (`### S3.` was kept here until `generics` phase C closed it: `gaps_drops_s3`)
    for kept in ["### B3.", "### B4."] {
        assert!(gaps.contains(kept), "GAPS.md keeps {kept}");
    }
    // the counts follow, as `host-ii` moved them, `unknown-semantics` (L18, L19) after it and
    // `analog-structure` (nine loud rows and S4) after that, `unknown-semantics-events` (L52) after
    // that: 2 silent, 2 broken, 40 loud; `params-and-io` (L20, L39) took the loud rows to 38, `composite-values`
    // phase A (L5, L6, L10) and L63 to 35, `Pfd3` (L51: its event reads ran, its assignment now does) to 34, `resolved-nets` (L22, L23) to 32, and `value-semantics` closed S7 (the spec amendment had added it)
    // and `composite-values` phase B (L1, L2, L3, L16) to 28, L64 (an unknown composite carrier read) to 29, L65 (a flag read on an `Option`) to 30; L66 (a `match` whose value is an `Option`) was found and closed by `composite-values` itself;
    // `explicit-unknowns` replaced S6 by S8 (still 2 silent), and its phase A closed S8 and L64 and L65: 1 silent, 28 loud; the split of L68 from L58 (explicit-unknowns phase B owns it) makes 29; `params-and-io` B5 closed L28 (integer, enum and `Option` params) and B6 L57 (files and tables): 27
    assert!(gaps.contains("0 silent, 2 broken, 17 loud"), "the counts of §9 follow (17 loud: L13 closed by `composite-values` phase D; L76 and L77 found by the `generics` phase C round 4 fix; 16 before, L30 and L31 closed by `generics` phase C; L72 closed by `analog-operators`, L73 and L74 found by its closing round, since L71, the host write of a `Logic`, was recorded by `explicit-unknowns`)");
    let rows = |section: &str, next: &str| gaps.split(section).nth(1).and_then(|s| s.split(next).next()).expect("a section").lines().filter(|l| l.starts_with("### ")).count();
    assert_eq!(rows("## 3. Silent", "## 4."), 0, "no silent entry (S3 closed by `generics` phase C)");
    assert_eq!(rows("## 2. Broken", "## 3."), 2, "two broken entries");
    let loud = gaps.split("## 4. Loud").nth(1).and_then(|s| s.split("## 5.").next()).expect("section 4");
    assert_eq!(loud.lines().filter(|l| l.starts_with("| L")).count(), 17, "seventeen loud rows (`composite-values` phase D closed L13; L76, an item only some iterations make, and L77, generation in a hoisted module, found by the `generics` phase C round 4 fix; `generics` phase C closed L30 and L31; `composite-values` phase C closed L24; `analog-operators` closed L43, L44, L45, L47, L49, L60 and L72, the last of which it found, and found L73, the flow of a flow branch read as a value, and L74, the forms of a `fn` inside a `ddx` that have no finite symbolic reading; L70 closed by T54 and L69 by T58; L71, the host write of a `Logic`, was found by `explicit-unknowns`; L68, `Logic`, `logic` and `matches_bits`, was split from L58 and closed by `explicit-unknowns` phase B; L28 and L57 closed by `params-and-io`; `generics` closed L4, L7, L8 and L9 and found L69 and L70)");
}

#[test]
fn gaps_drops_the_value_semantics_gap() {
    // value-semantics VS-10 AC 2: the front end meets Appendix G.71, so GAPS.md has no S7, names no feature
    // `value-semantics` as a plan, and no longer says `E0331` is raised
    let gaps = read("GAPS.md");
    assert!(!gaps.contains("### S7."), "S7 is closed");
    assert!(!gaps.contains("| S3, S6, S7 |") && !gaps.contains("| S3, S7, S8 ") && !gaps.contains("| S3, S8 ") && gaps.contains("| none (S3 closed by `generics` phase C; S6 and S8 closed by `explicit-unknowns`) |"), "the section table lists no silent entry (S3 closed by `generics` phase C; S6 and S8 closed by `explicit-unknowns`)");
    assert!(!gaps.contains("E0331") && !gaps.contains("(`value-semantics`)"), "no entry names the retired code or the plan");
    assert!(gaps.contains("0 silent, 2 broken, 17 loud"), "the counts follow (17 loud with L76 and L77 and L13 closed by `composite-values` phase D; 27 loud after `params-and-io` closed L28 and L57; 28 since composite-values phase B closed L1 to L3 and L16; L64 and L65 came and were closed by `explicit-unknowns`, L66 by `composite-values` itself; S8 closed by `explicit-unknowns`)");
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
    // deletion point of the contract lowering (Q1 (b)); since 2026-11-21 MD-78 supersedes it whole
    let state = read(".specs/STATE.md");
    let at = state.find("### MD-70:").expect("STATE.md holds MD-70");
    let decision = state[at..].split("\n### MD-71").next().expect("a decision");
    for name in ["runs in the generated Rust", "`Host.commit_with(", "`maybe`", "`generics`", "`std/values.msl`", "64", "extends MD-62", "MD-64"] {
        assert!(decision.contains(name), "MD-70 names {name}");
    }
    assert!(decision.contains("Superseded whole by MD-78"), "MD-70 is marked superseded");
}

#[test]
fn state_holds_the_explicit_unknowns_decision() {
    // EU-19 AC 1: MD-78 holds the six points of "MD entry", supersedes MD-70 whole and MD-64 in
    // part, and moves MD-76's D7 into the trait.
    let state = read(".specs/STATE.md");
    let at = state.find("### MD-78:").expect("STATE.md holds MD-78");
    let decision = state[at..].split("\n---").next().expect("a decision").split_whitespace().collect::<Vec<_>>().join(" ");
    for fact in [
        "1. **A value is what its type says.**",
        "2. **Uncertainty and absence are types.**",
        "3. **A carrier starts with a value or with its type's `Default`.**",
        "4. **The device ABI stays major 1 and is edited in place.**",
        "5. **Traits are capabilities.**",
        "6. **`unknown-semantics-per-level` is dropped**",
        "It supersedes MD-70 whole and MD-64 in its analysis of unknown-ness; it moves MD-76's D7 into the trait. MD-74 stands.",
    ] {
        assert!(decision.contains(fact), "MD-78 lacks {fact}");
    }
    let md70_at = state.find("### MD-70:").expect("STATE.md holds MD-70");
    let md70 = state[md70_at..].split("\n### ").next().expect("MD-70 body");
    assert!(md70.contains("Superseded whole by MD-78"), "MD-70 is marked superseded");
    let md64 = state[state.find("### MD-64:").expect("MD-64")..].split("\n---").next().expect("MD-64 body").to_string();
    assert!(md64.contains("Superseded in part by MD-78"), "MD-64 is marked");
}

#[test]
fn the_per_level_feature_is_recorded_as_dropped() {
    // EU-19 AC 3: the spec of `unknown-semantics-per-level` opens with a line that says it is
    // dropped, by whom and when; ROADMAP.md no longer has the `KNOWN_LEVELS` line.
    let spec = read(".specs/features/unknown-semantics-per-level/spec.md");
    let first = spec.lines().next().expect("a first line");
    assert!(first.contains("DROPPED") && first.contains("the user on 2026-11-21") && first.contains("MD-78"), "{first}");
    assert!(!read("ROADMAP.md").contains("KNOWN_LEVELS"), "ROADMAP.md still lists KNOWN_LEVELS");
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
fn gaps_closes_s6_and_tracks_the_flag() {
    // EU-19 AC 2 (T40; replaces `gaps_closes_s6_and_g6_by_the_redesign`): L18 and L19 stay closed;
    // S6 and G6 closed with the redesign of `explicit-unknowns`; S8, which said the implementation
    // still carries the known flag, closed when phase A landed, and so did L64 and L65. Phase B closed L68
    // (`Logic`, `logic` and `matches_bits`, split from L58).
    let gaps = read("GAPS.md");
    for closed in ["L18", "L19"] {
        assert!(!gaps.lines().any(|l| l.starts_with(&format!("| {closed} |"))), "{closed} is still a row");
    }
    let l59 = gaps.lines().find(|l| l.starts_with("| L59 |")).expect("L59 keeps its non-literal closure and `expect` clauses");
    assert!(l59.contains("when it is not a literal") && l59.contains("message that is not a string literal") && !l59.contains("unknown"), "{l59}");
    assert!(!gaps.contains("Unknown control is a fault, not a merge"), "the fault of an unknown `if` is still listed");
    assert!(!gaps.contains("### S6."), "S6 is closed by the redesign");
    assert!(!gaps.contains("- **G6 \u{2014} unknown integers are per value** (S6)."), "G6 is closed by the redesign");
    assert!(gaps.contains("G6 closed by `explicit-unknowns`"), "the closure of G6 is recorded");
    assert!(!gaps.contains("### S8."), "S8 is closed by phase A of `explicit-unknowns`");
    for closed in ["L64", "L65"] {
        assert!(!gaps.lines().any(|l| l.starts_with(&format!("| {closed} |"))), "{closed} is still a row");
    }
    assert!(gaps.contains("S6, S8 and G6 closed by `explicit-unknowns`") || gaps.contains("(S6 and S8 closed by `explicit-unknowns`)"), "the closures are recorded");
    // phase B (T45, T47): L68 is closed and no row of section 11 names it, since every listing it held runs
    assert!(!gaps.lines().any(|l| l.starts_with("| L68 |")), "L68 is closed by phase B");
    let logic = gaps.lines().filter(|l| l.starts_with("| values#18 |") || l.starts_with("| values~c30bad4d |") || l.starts_with("| core-std~6e04c51d |")).collect::<Vec<_>>();
    assert!(logic.is_empty(), "the three Book rows of phase B run and are no row of section 11: {logic:?}");
    let coverage = gaps.split("## 9. Coverage by spec part").nth(1).and_then(|s| s.split("## 10.").next()).expect("GAPS.md has a section 9");
    let row = coverage.lines().find(|l| l.starts_with("| I §3–§3.3 ")).expect("the coverage row of §3.3");
    assert!(!row.contains("`level`"), "the coverage row still lists `level` as a gap: {row}");
}

#[test]
fn gaps_rows_carry_no_unknown_control_clause() {
    // EU-19 (replaces `gaps_carries_the_unknown_index_merge_to_composite_values`): with no merge,
    // L3, L5 and L24 do not carry the loop bound or the index merge any more (L3 and L5 left with composite-values).
    let gaps = read("GAPS.md");
    // L24 left with composite-values phase C: it is no row, and no row carries the merge clause
    for id in ["L3", "L5", "L24"] {
        assert!(!gaps.lines().any(|l| l.starts_with(&format!("| {id} |"))), "{id} is still a row");
    }
    assert!(!gaps.contains("merges every element it may name") && !gaps.contains("US-04"), "a GAPS row carries the index merge");
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
    // L60 (the ideal step of `slew`) closed with `analog-operators` T8 and T9 (`gaps_and_roadmap_follow_analog_operators`)
    assert!(!gaps.contains("| L60 |"), "L60 is closed: the step that read a jump is retaken");
    // the cluster is delivered and the counts follow: 1 silent, 2 broken (the loud count is held by `gaps_drops_what_msl_fixes_closed`)
    assert!(gaps.contains("| Analog structure (delivered, MD-71) |"), "the cluster is marked delivered");
}

#[test]
fn gaps_keeps_l40_for_the_spec_models_that_wait() {
    // analog-structure AS-18 AC 4, Q2 (T36): one L40 line is left, for the three spec models that wait on
    // another feature's dependency: the stdlib `idt` and `Water` (`generics`); the §18 `Diode` runs since
    // `params-and-io` (`elab_structure::the_section_18_diode_type_checks_with_its_option_param`)
    let gaps = read("GAPS.md");
    let rows: Vec<&str> = gaps.lines().filter(|l| l.starts_with("| L40 |")).collect();
    assert_eq!(rows.len(), 1, "one L40 row: {rows:?}");
    for name in ["`idt`", "`Water`", "`generics`"] {
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
fn gaps_drops_what_generics_phase_a_closed() {
    // generics GN-31 (T43): phase A closed L4 (generic enums), L7 (generic functions, `self`, `impl`, traits), L9 (a
    // capturing closure for an `Fn` bound) and X6 (a generic body is checked once, Q1 (c)); S3 keeps only the generic
    // module, which phase C checks (GN-13); §9 Coverage names the generic module as the hole of I §3.4 to §3.6
    let gaps = read("GAPS.md");
    for gone in ["| L4 |", "| L7 |", "| L9 |", "- **X6"] {
        assert!(!gaps.contains(gone), "GAPS.md still holds `{gone}`");
    }
    // S3 was narrowed to the generic module here and closed by phase C (`gaps_drops_s3`)
    let row = gaps.lines().find(|l| l.starts_with("| I §3.4–§3.6 files, crates, packages |")).expect("the Coverage row");
    assert!(row.contains("S3") && row.contains("closed"), "{row}");
    let roadmap = read("ROADMAP.md");
    assert!(roadmap.contains("phases A to C delivered"), "the ROADMAP lane row says phases A to C are delivered (phases A and B until T82)");
}

#[test]
fn gaps_drops_what_generics_phase_b_closed() {
    // generics GN-31 (T52): phase B closed L8 (`impl` on a nature, a function that probes a handle); L53 waited only on
    // `unknown-semantics-events`, which closed it (T31: `gaps_no_longer_lists_l51_or_l53`); the ROADMAP lane row says
    // phases A and B are delivered
    let gaps = read("GAPS.md");
    assert!(!gaps.contains("| L8 |"), "L8 closed with generics phase B");
    assert!(!gaps.contains("| L53 |"), "L53 closed with unknown-semantics-events");
    let roadmap = read("ROADMAP.md");
    assert!(roadmap.contains("phases A to C delivered"), "the ROADMAP lane row says phases A to C are delivered (phases A and B until T82)");
}

#[test]
fn state_records_the_generics_decision() {
    // generics design "Tech Decisions", T52: `.specs/STATE.md` holds the next free `MD-` entry, which says generics are
    // monomorphized at elaboration, dispatch is static, a handle function is inlined and `check` types every item
    let state = read(".specs/STATE.md");
    let at = state.find("### MD-81:").expect("STATE.md holds MD-81");
    let decision = state[at..].split("\n### MD-82").next().expect("a decision");
    for name in ["monomorphized at elaboration", "turbofish", "Static dispatch", "inlined at each call", "`E0133`", "`E0126`", "extends MD-64", "supersedes nothing", "`piperine check` types every item"] {
        assert!(decision.contains(name), "MD-81 names {name}");
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
fn gaps_drops_what_composite_values_phase_a_closed() {
    // composite-values CV-37 AC 1 (T29): L5, L6 and L10 leave `GAPS.md`; L9 is left only for the part that
    // waits on `generics` (a capturing closure passed to a user `fn`); section 9 and section 11 follow
    let gaps = read("GAPS.md");
    for id in ["L5", "L6", "L10"] {
        assert!(!gaps.contains(&format!("| {id} |")), "GAPS.md still has the row {id}");
    }
    // L9, left for the part that waited on `generics`, closed with generics phase A (T37)
    assert!(!gaps.contains("| L9 |"), "L9 closed with generics phase A");
    // the rows that stay point at gaps that stay: the Book's types#17 refusal moved off L6 (analog#2 runs since `unknown-semantics-events`)
    let book = gaps.split("## 11. The Book against the binary").nth(1).expect("section 11");
    for stale in ["| L5 |", "| L6 |", "| L10 |"] {
        assert!(!book.contains(stale), "section 11 still names {stale}");
    }
    assert!(!book.contains("| types#17 |") && !book.contains("| types#11 |") && !book.contains("| analog#2 |"), "types#17 runs since generics T36 and T37, types#11 runs and analog#2 runs");
    // section 9: the rows of I §1-§2 and I §3-§3.3 no longer list the loops as a hole
    let coverage = gaps.split("## 9. Coverage by spec part").nth(1).and_then(|s| s.split("## 10.").next()).expect("section 9");
    let functions = coverage.lines().find(|l| l.starts_with("| I §3–§3.3 ")).expect("the coverage row of §3.3");
    assert!(functions.contains("loops, recursion, closures") && functions.contains("run") && !functions.contains("handles, loops"), "{functions}");
    let values = coverage.lines().find(|l| l.starts_with("| I §1–§2 ")).expect("the coverage row of §1 and §2");
    assert!(values.contains("`let mut`, assignment, patterns"), "{values}");
    // the counts follow: 39 loud, then 38 when `composite-values` closed L63 (§1.4.4 literal typing), 35 with `params-and-io`
    assert!(gaps.contains("17 loud"), "the loud count is 17 with L76 and L77, L13 closed by `composite-values` phase D (28 once `params-and-io`, `composite-values`, `resolved-nets` and the last of `unknown-semantics-events` closed rows; L64 and L65 came and were closed by `explicit-unknowns` phase A, L66 by `composite-values` itself)");
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
    // list `settle`; L53 closed with the feature's last batch
    let gaps = read("GAPS.md");
    assert!(!gaps.contains("| L52 |"), "GAPS.md still has the row L52");
    let row = |id: &str| gaps.lines().find(|l| l.starts_with(&format!("| {id} |"))).unwrap_or_else(|| panic!("GAPS.md has the row {id}")).to_string();
    assert!(!row("L58").contains("`limexp`"), "L58 still lists limexp: {}", row("L58"));
    assert!(!gaps.contains("| L51 |"), "L51 (`Pfd3`) is closed: its event reads ran and `composite-values` lifted the assignment");
    assert!(!gaps.contains("| L53 |"), "L53 is closed by the feature's last batch");
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
    assert!(gaps.contains("| L21 |") && gaps.contains("| H13 |"), "the neighbours keep their numbers");
    let owners = read("crates/piperine-lang/tests/support/code_owners.tsv");
    let mut allowed: Vec<&str> = owners.lines().filter(|line| !line.starts_with('#') && !line.is_empty()).filter_map(|line| line.split('\t').next()).collect();
    allowed.sort_unstable();
    let listed = gaps.split("**Error codes of the spec that no source file mentions").nth(1).and_then(|s| s.split("by the elaborator.\n`").nth(1)).and_then(|s| s.split('`').next()).expect("section 10 lists the unraised codes");
    let mut listed: Vec<&str> = listed.split_whitespace().collect();
    listed.sort_unstable();
    assert_eq!(listed, allowed, "section 10 lists exactly the rows of code_owners.tsv");
    assert!(gaps.contains(&format!("({} of ", allowed.len())), "the heading counts the listed codes");
    let roadmap = read("ROADMAP.md");
    let row = roadmap.lines().find(|line| line.contains("| `params-and-io` |")).expect("the roadmap has a params-and-io row");
    assert!(row.contains("B5–B6") && row.contains("composite-values") && row.contains("Delivered"), "the row says B1 to B6 are delivered, B5 and B6 on composite-values: {row}");
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
    // `modules#8` ran once `generics` T71 flipped the generic `idt`: it has no row left
    assert!(!gaps.lines().any(|l| l.starts_with("| modules#8 |")), "modules#8 runs: it has no Book row");
    assert!(gaps.lines().any(|l| l.starts_with("| L21 |")), "L21 stays: resolved natures are not compiled yet");
}

#[test]
fn gaps_drops_what_composite_values_phase_b_closed() {
    // composite-values CV-37 AC 1 (T63): L1, L2, L3 and L16 leave `GAPS.md`; L4 stays for generic enums only, which wait on `generics` GN-06 (an enum
    // without `#[derive(Repr)]` runs as a value); `normal` leaves L58; the note
    // that enum variant names are not carried leaves section 4.7; section 9 follows
    let gaps = read("GAPS.md");
    for id in ["L1", "L2", "L3", "L16"] {
        assert!(!gaps.contains(&format!("| {id} |")), "GAPS.md still has the row {id}");
    }
    // L4, kept for generic enums, closed with generics phase A (T38)
    assert!(!gaps.contains("| L4 |"), "L4 closed with generics phase A");
    let l58 = gaps.lines().find(|l| l.starts_with("| L58 |")).expect("the row L58").to_string();
    assert!(!l58.contains("`normal`"), "L58 still lists `normal`: {l58}");
    assert!(!gaps.contains("Enum variant names are not carried"), "section 4.7 still says enum variant names are not carried");
    // section 11: the Book rows that named L1 and L3 follow their remaining refusal, a generic function or struct (L7)
    let book = gaps.split("## 11. The Book against the binary").nth(1).expect("section 11");
    for stale in ["| types#7 | L3 |", "| generics#4 | L1 |"] {
        assert!(!book.contains(stale), "section 11 still names {stale}");
    }
    assert!(!book.contains("| types#7 |") && !book.contains("| generics#4 |"), "types#7 runs since generics T37 and generics#4 since T38");
    // section 9: the coverage of values, functions, the stdlib and the inherited library
    let coverage = gaps.split("## 9. Coverage by spec part").nth(1).and_then(|s| s.split("## 10.").next()).expect("section 9");
    let values = coverage.lines().find(|l| l.starts_with("| I §1–§2 ")).expect("the coverage row of §1 and §2");
    assert!(values.contains("structs, tuples, arrays and payload enums run"), "{values}");
    let functions = coverage.lines().find(|l| l.starts_with("| I §3–§3.3 ")).expect("the coverage row of §3.3");
    assert!(functions.contains("arrays, composites") && !functions.contains("handles, arrays and composites"), "{functions}");
    let stdlib = coverage.lines().find(|l| l.starts_with("| VII §22 ")).expect("the coverage row of §22");
    assert!(stdlib.contains("`normal` runs"), "{stdlib}");
    let inherited = coverage.lines().find(|l| l.starts_with("| H inherited library")).expect("the coverage row of H");
    assert!(inherited.contains("100 %") && inherited.contains("H.5 to H.7 run"), "{inherited}");
    assert!(gaps.contains("17 loud"), "the loud count follows (L76 and L77 found by the `generics` phase C round 4 fix)");
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

#[test]
fn the_scenario_package_states_its_measured_step_count() {
    // transient-performance TP-10 AC 3 (T23): the comment of `tests/e2e/buck/Piperine.toml` states the
    // accepted steps of its 5.2 ms of converter time, as measured, and `moved.md` keeps its columns
    let manifest = read("tests/e2e/buck/Piperine.toml");
    let comment = manifest.lines().filter(|line| line.starts_with('#')).collect::<Vec<_>>().join(" ");
    assert!(comment.contains("accepted steps") && comment.contains("5.2 ms"), "the comment states a step count for 5.2 ms: {comment}");
    assert!(!comment.contains("about 6 M steps"), "the count of before the feature is gone");
    let moved = read(".specs/features/transient-performance/moved.md");
    let header = moved.lines().find(|line| line.starts_with("| Task |")).expect("moved.md has its table header");
    for column in ["Old", "New", "Requirement"] {
        assert!(header.contains(column), "moved.md lacks the column `{column}`: {header}");
    }
    // every moved golden is a row of cells, none of them empty, and none of them a record or a digital column
    let rows: Vec<Vec<&str>> = moved
        .lines()
        .filter(|line| line.starts_with("| T") && !line.starts_with("| Task |"))
        .map(|line| line.trim_matches('|').split(" | ").map(str::trim).collect())
        .collect();
    assert!(!rows.is_empty(), "moved.md lists its moved goldens");
    for row in &rows {
        assert_eq!(row.len(), 5, "a moved golden has task, golden, old, new and requirement: {row:?}");
        assert!(row.iter().all(|cell| !cell.is_empty()), "a cell of a moved golden is empty: {row:?}");
        assert!(!row[1].contains("record") && !row[1].contains("column"), "a record or a digital column moved: {row:?}");
    }
    let monitor = rows.iter().find(|row| row[0] == "T16").expect("the T16 row of the monitor listing");
    assert!(monitor[1].contains("monitor.msl"), "the T16 row names the monitor listing: {monitor:?}");
    assert!(monitor[3].contains("`zone` records are equal"), "the T16 row says its records did not move: {monitor:?}");
    assert!(monitor[4].contains("TP-03"), "the T16 row names its requirement: {monitor:?}");
}

#[test]
fn transient_performance_closes_v12() {
    // transient-performance TP-10 AC 1 and AC 3 (T24): GAPS.md has no V12 row, ROADMAP.md marks `multi-rate`
    // delivered by `transient-performance`, and `diagnosis.md` holds the "After" table of its section 2
    let gaps = read("GAPS.md");
    assert!(!gaps.lines().any(|line| line.starts_with("| V12 |")), "GAPS.md still has a V12 row");
    let roadmap = read("ROADMAP.md");
    let row = roadmap.lines().find(|line| line.contains("| `multi-rate`")).expect("the roadmap has a multi-rate row");
    assert!(row.contains("delivered by `transient-performance`"), "{row}");
    let diagnosis = read(".specs/features/transient-performance/diagnosis.md");
    let after = diagnosis.split("## 11. After").nth(1).expect("diagnosis.md has an \"After\" section");
    for count in ["Accepted steps", "Rejected attempts", "Steps ending on a landing", "Steps ending on the controller's proposal", "Instants run"] {
        assert!(after.contains(count), "the After table lacks `{count}`");
    }
}

#[test]
fn state_records_the_observable_landings_and_the_tick_epoch() {
    // transient-performance T24: the decision entry
    let state = read(".specs/STATE.md");
    assert!(state.contains("### MD-80:") && state.contains("`transient-performance`") && state.contains("observable rule"), "STATE.md holds the decision entry");
    assert!(state.contains("`HOST_RESTART_FRACTION`") && state.contains("growth clamp stays 1.5") && state.contains("(T26, TP-13"), "MD-80 records T26");
}

#[test]
fn gaps_no_longer_lists_l28_or_its_book_rows() {
    // params-and-io PI-01 to PI-03 (T35): the params of an integer, enum, `Option` or module type run, so GAPS.md has no L28
    // row and its section 11 holds no row of the debouncer, the PWM, its check or the watchdog
    let gaps = read("GAPS.md");
    assert!(!gaps.lines().any(|line| line.starts_with("| L28 |")), "GAPS.md still has an L28 row");
    for closed in ["| digital#7 |", "| digital#11 |", "| digital#14 |", "| core-std#10 |", "| packages#12 |", "| cheatsheet#10 |", "| L57 |"] {
        assert!(!gaps.lines().any(|line| line.starts_with(closed)), "section 11 still lists {closed}");
    }
}

#[test]
fn gaps_no_longer_lists_this_features_ids() {
    // params-and-io PI-14 AC 1 (T44): L20, L28, L39, L57, X5 and H12 are gone from `GAPS.md` as entries and rows, with the section 11
    // rows of the Book listings they held.
    let gaps = read("GAPS.md");
    for closed in ["| L20 |", "| L28 |", "| L39 |", "| L57 |", "| H12 |", "- **X5 \u{2014}"] {
        assert!(!gaps.contains(closed), "{closed} is still in GAPS.md");
    }
}

#[test]
fn gaps_section_10_lists_exactly_the_owned_codes() {
    // params-and-io PI-14 AC 1 (T44): section 10's list of the codes no source mentions is exactly the rows of `code_owners.tsv`, and its
    // heading counts them.
    let gaps = read("GAPS.md");
    let owners = read("crates/piperine-lang/tests/support/code_owners.tsv");
    let mut allowed: Vec<&str> = owners.lines().filter(|line| !line.starts_with('#') && !line.is_empty()).filter_map(|line| line.split('\t').next()).collect();
    allowed.sort_unstable();
    let listed = gaps.split("**Error codes of the spec that no source file mentions").nth(1).and_then(|s| s.split("by the elaborator.\n`").nth(1)).and_then(|s| s.split('`').next()).expect("section 10 lists the unraised codes");
    let mut listed: Vec<&str> = listed.split_whitespace().collect();
    listed.sort_unstable();
    assert_eq!(listed, allowed, "section 10 lists exactly the rows of code_owners.tsv");
    assert!(gaps.contains(&format!("({} of ", allowed.len())), "the heading counts the listed codes");
    for delivered in ["E0614", "E0703", "E0704", "E0705", "E0706"] {
        assert!(!allowed.contains(&delivered), "{delivered} is raised and is no entry");
    }
}

#[test]
fn gaps_drops_s3() {
    // generics GN-07 AC 2, GN-13 (T71): generic modules are typed once at their definition, so an unreached one with an error
    // fails the check; GAPS.md has no entry S3, no silent section entry, and the section table lists none
    let gaps = read("GAPS.md");
    assert!(!gaps.contains("### S3."), "GAPS.md still has the entry S3");
    assert!(gaps.contains("| §3 Silent (read this first) | none (S3 closed by `generics` phase C"), "the section table lists no silent entry");
    assert!(gaps.contains("0 silent"), "the counts hold no silent entry");
    assert!(!gaps.contains("the check of an unreached generic module (S3) |"), "the Coverage row no longer names S3 as a hole");
}

#[test]
fn gaps_drops_what_generics_phase_c_closed() {
    // generics GN-31 AC 1 (T82): generic modules (L30) and generation in a body and a `digital` block (L31) run, so neither row is
    // in `GAPS.md`; the codes `E0322`, `E0340` and `E0342` are raised and leave the list of codes raised nowhere; the lane row
    // says phase C is delivered
    let gaps = read("GAPS.md");
    for gone in ["| L30 |", "| L31 |", "E0225 E0322", "E0342\n"] {
        assert!(!gaps.contains(gone), "GAPS.md still holds `{gone}`");
    }
    // the stdlib items that check and run left L58 (`since`, `Stable`, `Setup`, `Hold`, `MinPulse`); only the refused remain
    let stdlib = gaps.lines().find(|line| line.starts_with("| L58 |")).expect("GAPS.md holds L58");
    for running in ["`since`", "`Stable`", "`Setup`", "`Hold`", "`MinPulse`"] {
        assert!(!stdlib.contains(running), "L58 still lists {running}, which runs");
    }
    for refused in ["`idtmod`", "`noise_table`", "`inertial`", "`AckWithin`"] {
        assert!(stdlib.contains(refused), "L58 keeps {refused}, still refused");
    }
    let owners = read("crates/piperine-lang/tests/support/code_owners.tsv");
    for code in ["E0322", "E0340", "E0342"] {
        assert!(!owners.contains(code), "code_owners.tsv still lists {code}");
    }
    let roadmap = read("ROADMAP.md");
    assert!(roadmap.contains("phases A to C delivered"), "the ROADMAP lane row says phase C is delivered");
}

#[test]
fn state_records_generic_modules_and_generation() {
    // generics design-c.md "Tech Decisions" (T82): MD-81 gains item 6, the monomorphization of a module per instance and the
    // unrolling of generation at elaboration
    let state = read(".specs/STATE.md");
    let at = state.find("### MD-81:").expect("STATE.md holds MD-81");
    let decision = state[at..].split("\n### MD-82").next().expect("MD-81 ends");
    for needed in ["6. **Generic modules and generation", "BodyKey", "GeneratedItems", "`E0341`"] {
        assert!(decision.contains(needed), "MD-81 names {needed}");
    }
}

#[test]
fn gaps_drops_l70() {
    // generics GN-31 (T54): `encode` and `decode` of every layout run, so the row L70 leaves `GAPS.md` and no section of the
    // gaps record names the two `P0001` refusals it listed
    let gaps = read("GAPS.md");
    assert!(!gaps.contains("| L70 |"), "GAPS.md still has the row L70");
    assert!(!gaps.contains("`encode` on a value of a type whose layout is derived"), "the L70 refusal text is gone");
}

#[test]
fn gaps_drops_the_cast_and_same_intrinsics() {
    // generics GN-27 AC 1 (T55): `cast` and `same` compile from `std/values.msl`, so the row L58 no longer says they run as the
    // checker's own intrinsics, and no source of the language crate names the two variants
    let gaps = read("GAPS.md");
    let l58 = gaps.lines().find(|l| l.starts_with("| L58 |")).expect("the row L58").to_string();
    assert!(!l58.contains("`cast` and `same`") && !l58.contains("Intrinsic::{Cast, Same}"), "L58 still names the intrinsics: {l58}");
    let intrinsic = read("crates/piperine-lang/src/types/intrinsic.rs");
    assert!(!intrinsic.contains("Same,") && !intrinsic.contains("Cast,"), "types::intrinsic still has a variant for `cast` or `same`");
}

#[test]
fn gaps_drops_l69() {
    // generics GN-32, GN-31 (T58): a hand-written `Repr` on a generic type runs and a carrier of a hand-written type starts at the
    // levels of its `Default`, so the row L69 leaves `GAPS.md` with no trace of its refusals
    let gaps = read("GAPS.md");
    assert!(!gaps.contains("| L69 |"), "GAPS.md still has the row L69");
    assert!(!gaps.contains("a carrier of a type with a hand-written `Repr` and no initial value"), "the L69 refusal text is gone");
    assert!(!gaps.contains("a hand-written `Repr` on a generic type"), "the L69 refusal text is gone");
}

#[test]
fn gaps_and_roadmap_follow_analog_operators() {
    // analog-operators AO-12 (T44): `GAPS.md` drops L43, L44, L45, L47, L49 and L60, the noise row of H2 and U7; it found L72, the
    // `emit!` reached from `analog` through a `fn`, and closed it in the closing round, and its counts follow; `ROADMAP.md` marks the feature delivered and
    // `.specs/STATE.md` holds its decision
    let gaps = read("GAPS.md");
    for id in ["L43", "L44", "L45", "L47", "L49", "L60", "L72"] {
        assert!(!gaps.contains(&format!("| {id} |")), "GAPS.md still has the row {id}");
    }
    assert!(!gaps.contains("- **U7."), "GAPS.md still has the entry U7");
    let h2 = gaps.lines().find(|l| l.starts_with("| H2 |")).expect("GAPS.md has the row H2");
    assert!(h2.contains("`noise` (delivered by `analog-operators`"), "H2 records the noise analysis as delivered: {h2}");
    assert!(!gaps.contains("| L72 |"), "L72, the `emit!` reached from `analog` through a `fn`, runs as a trace of its call and is closed");
    assert!(gaps.contains("| Analog operators (delivered, MD-82) |"), "the cluster is marked delivered");
    assert!(gaps.contains("0 silent, 2 broken, 17 loud") && gaps.contains("8 unproven"), "the counts follow");
    let roadmap = read("ROADMAP.md");
    assert!(roadmap.contains("analog-operators") && roadmap.contains("delivered"), "ROADMAP.md marks analog-operators delivered");
    let state = read(".specs/STATE.md");
    let at = state.find("### MD-82:").expect("STATE.md holds MD-82");
    let decision = state[at..].split("\n## ").next().expect("a decision");
    assert!(decision.contains("`simparam(name)` is an `Option<f64>`") && decision.contains("`tnom` is an ordinary"), "MD-82 records the session decisions");
}

#[test]
fn gaps_drops_what_composite_values_phase_c_closed() {
    // composite-values CV-37 (T90): L24 (arrays of carriers, element and field commits, EU-04 AC 3) leaves GAPS.md; section 5 gains the
    // QC3 spec hole; the Book rows that named the array of carriers no longer do
    let gaps = read("GAPS.md");
    assert!(!gaps.contains("| L24 |"), "GAPS.md still has the row L24");
    assert!(gaps.contains("QC3 — an array of carriers as a function parameter"), "section 5 has the QC3 spec hole");
    let book = gaps.split("## 11. The Book against the binary").nth(1).expect("section 11");
    assert!(!book.contains("the arrays of carriers of `MemoryBank`") && !book.contains("(an array of carriers, L24)"), "section 11 still names the array of carriers");
    // the counts follow: the loud count in the header is the number of `L` rows the tables hold
    let rows = gaps.lines().filter(|line| line.strip_prefix("| L").is_some_and(|rest| rest.split_once(" |").is_some_and(|(id, _)| id.chars().all(|c| c.is_ascii_digit())))).count();
    assert!(gaps.contains(&format!("2 broken, {rows} loud (L1 to L77")), "the header counts the {rows} loud rows");
    // section 9: the digital row says the arrays of carriers run; the cluster table says phase C closed L24
    assert!(gaps.contains("arrays of carriers, element and field commits run (composite-values phase C)"), "section 9 says the arrays of carriers run");
    assert!(gaps.contains("phase C closed L24 (arrays of carriers, MD-79); phase D (number widths) follows"), "the cluster row says phase C closed L24");
    // H9: E0803 is reachable by a param an array length reads, the thing QC2 delivered
    let h9 = gaps.lines().find(|line| line.starts_with("| H9 |")).expect("the row H9");
    assert!(h9.contains("reachable by a param an array length reads") && h9.contains("QC2"), "H9 states what QC2 delivered: {h9}");
}

#[test]
fn gaps_drops_what_composite_values_phase_d_closed() {
    // composite-values CV-37 (T126): L13 leaves `GAPS.md` with no residue (user Q4 (a): `f32`, `f16` and `f128` complete, integers of
    // any width), the coverage row of I §1-§2 no longer lists floats other than `f64` and widths over 64 as a hole, a `Logic<N>` of
    // more than 64 levels is no refusal, and the loud count is 19 (17 once merged with `generics` phase C: S3, L30, L31, L69, L70 closed, L76, L77 found)
    let gaps = read("GAPS.md");
    assert!(!gaps.contains("| L13 |"), "L13 is still a row");
    assert!(!gaps.contains("`Logic<100>`: its field `binary`"), "the Logic<100> refusal is still listed");
    let coverage = gaps.split("## 9. Coverage by spec part").nth(1).and_then(|s| s.split("## 10.").next()).expect("section 9");
    let values = coverage.lines().find(|l| l.starts_with("| I §1–§2 ")).expect("the coverage row of §1 and §2");
    assert!(!values.contains("floats other than `f64`") && !values.contains("widths over 64"), "{values}");
    assert!(gaps.contains("0 silent, 2 broken, 17 loud"), "the counts follow");
    // the feature's own records say it: the traceability marks CV-29 to CV-33 done, MD-79 gains the item, the roadmap row says delivered
    let spec = read(".specs/features/composite-values/spec.md");
    for id in ["CV-29", "CV-30", "CV-31", "CV-32", "CV-33"] {
        let row = spec.lines().find(|l| l.starts_with(&format!("| {id} |"))).unwrap_or_else(|| panic!("a traceability row {id}"));
        assert!(row.contains("Done"), "{row}");
    }
    let state = read(".specs/STATE.md");
    assert!(state.contains("Number widths: `f32` is Rust's `f32`; `f16`, `f128` and integers wider than 64"), "MD-79 names the number widths");
    let roadmap = read("ROADMAP.md");
    let row = roadmap.lines().find(|l| l.contains("`composite-values` phase D")).expect("the roadmap row of phase D");
    assert!(row.to_lowercase().contains("delivered"), "{row}");
    let rs = read(".specs/features/ir-rust/spec.md");
    assert!(rs.contains("superseded by composite-values Q4 (2026-11-23)"), "RS-Q6 is superseded");
    for table in ["CLAUDE.md", "AGENTS.md"] {
        assert!(read(table).contains("piperine-numbers"), "{table} lists the crate");
    }
}

#[test]
fn gaps_no_longer_lists_l51_or_l53() {
    // unknown-semantics-events UE-07 AC 1 (T31): `GAPS.md` has neither the row L51 nor the row L53, the Book rows of
    // `handshake` (values#8, values~c389077a) are no longer listed as refused in section 11, and section 9 no longer
    // names functions that return `Event` among what does not run
    let gaps = read("GAPS.md");
    assert!(!gaps.contains("| L51 |") && !gaps.contains("| L53 |"), "GAPS.md still has the row L51 or L53");
    assert!(!gaps.contains("| values#8 |") && !gaps.contains("| values~c389077a |"), "section 11 still lists the `handshake` rows");
    let coverage = gaps.split("## 9. Coverage by spec part").nth(1).and_then(|s| s.split("## 10.").next()).expect("GAPS.md has a section 9");
    assert!(!coverage.contains("functions that return `Event`") && !coverage.contains("`Event` functions"), "section 9 still lists functions that return `Event`, in either spelling");
    assert!(gaps.contains("L51, L53"), "the closed list of section 4 names L53");
}

#[test]
fn gaps_l77_is_owned_by_generics_phase_e() {
    // generics phase C round 5 finding 18: L77, generation in a hoisted module's body, is the feature's own leftover (generation
    // and hoisting are both its forms), owned by its phase E task T118, not by a spec lane
    let gaps = read("GAPS.md");
    let l77 = gaps.lines().find(|l| l.starts_with("| L77 |")).expect("the row L77");
    assert!(l77.contains("Owner `generics` phase E, T118") && !l77.contains("Owner `spec-closure`"), "L77 is owned by generics phase E: {l77}");
    let tasks = read(".specs/features/generics/tasks-e.md");
    assert!(tasks.contains("### T118: Generation in the body of a hoisted module"), "the task T118 is planned");
}
