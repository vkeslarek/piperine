//! The structural guards over the tracking files the repository keeps next to its sources (CC-12 AC 2):
//! the buck's README names the commands that run it, `CLAUDE.md` names the integrated gate and the host
//! crates, `ROADMAP.md` is forward-looking, `GAPS.md` lists exactly the error codes no source raises, and
//! the `generics` feature's `checks.md` gathers every check of its task files once. The tests that
//! pinned a sentence a closed feature left in a repository record left with code-clarity T10.

use piperine_repository_guards::SourceTree;

/// Reads the files of the repository the guards below judge.
struct RepositoryRecords;

impl RepositoryRecords {
    /// The text of a file of the repository, by path from its root.
    fn read(path: &str) -> String {
        std::fs::read_to_string(SourceTree::workspace_root().join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
    }
}

#[test]
fn the_buck_readme_says_how_to_run_it() {
    // CLI-20 AC 5: `examples/buck/README.md` names `piperine run startup` and no longer says that
    // nothing runs the converter but a Rust test
    let readme = RepositoryRecords::read("examples/buck/README.md");
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
    let readme = RepositoryRecords::read("examples/buck/README.md");
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
    // integrated-tests IT-13 AC 2, narrowed by the CLAUDE.md rewrite of 2026-11-24 (AGENTS.md now
    // points at CLAUDE.md): `CLAUDE.md` names the integrated gate, the buck's README and the root's
    // integrated tests
    let claude = RepositoryRecords::read("CLAUDE.md");
    for held in ["cargo test -p piperine --test integrated", "examples/buck/README.md", "tests/integrated/"] {
        assert!(claude.contains(held), "CLAUDE.md names `{held}`");
    }
}

#[test]
fn the_buck_readme_points_at_the_scenarios() {
    // buck-scenarios BS-10 AC 1: S2 to S10 run from `tests/buck_scenarios.rs` over `tests/e2e/buck/`
    // through `Session::drive` and `Session::set_at`; the README no longer says `piperine-api` has no
    // drives or mid-run `set`, and only the Python sketch keeps "No such host exists yet"
    let readme = RepositoryRecords::read("examples/buck/README.md");
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
    // CLI-20 AC 1, narrowed by the CLAUDE.md rewrite of 2026-11-24: the crate table's rows say the
    // host library holds `Project` and `Session`
    let claude = RepositoryRecords::read("CLAUDE.md");
    let row = |name: &str| claude.lines().find(|l| l.starts_with(&format!("| `{name}` |"))).unwrap_or_else(|| panic!("CLAUDE.md has a row for {name}")).to_string();
    let api = row("piperine-api");
    assert!(api.contains("`Project`") && api.contains("`Session`"), "the piperine-api row names Project and Session: {api}");
    assert!(RepositoryRecords::read("AGENTS.md").contains("CLAUDE.md"), "AGENTS.md points at CLAUDE.md");
}

#[test]
fn the_roadmap_is_forward_looking() {
    // ROADMAP.md rewrite (user, 2026-11-21: "limpar ... coloca certinho, tabelado, o que tem no pool de
    // desenvolvimento"): the roadmap lists only what is in flight or waits, in tables, and no longer
    // carries delivery history (that lives in STATE.md, held by the `state_records_*` tests above);
    // what long-run, host-ii and msl-fixes left is still named as a future row
    let roadmap = RepositoryRecords::read("ROADMAP.md");
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
fn gaps_section_10_lists_exactly_the_owned_codes() {
    // params-and-io PI-14 AC 1 (T44): section 10's list of the codes no source mentions is exactly the rows of `code_owners.tsv`, and its
    // heading counts them.
    let gaps = RepositoryRecords::read("GAPS.md");
    let owners = RepositoryRecords::read("crates/piperine-lang/tests/support/code_owners.tsv");
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
fn the_generics_checks_file_holds_every_phase() {
    // generics GN-31 AC 1 (T115): `checks.md` gathers the build line, the union of the `Run` lines and every check of the four
    // task files (the GN-30 checks once), so one `check_proofs.py generics` runs the whole feature
    let checks = RepositoryRecords::read(".specs/features/generics/checks.md");
    assert!(checks.contains("**Build**: `cargo build"), "the build line");
    for file in ["tasks.md", "tasks-c.md", "tasks-d.md", "tasks-e.md"] {
        let tasks = RepositoryRecords::read(&format!(".specs/features/generics/{file}"));
        let section = tasks.split("\n## Checks\n").nth(1).and_then(|s| s.split("\n## Handoff").next()).expect("a Checks section");
        for line in section.lines().filter(|l| l.starts_with("**C")) {
            let (id, rest) = line.trim_start_matches("**").split_once("** - ").expect("a check line");
            if rest.contains("(GN-30,") {
                continue;
            }
            assert!(checks.contains(&format!("**{id}** - ")), "{file}: the check {id} is in checks.md");
        }
        for run in tasks.lines().filter(|l| l.starts_with("**Run ")) {
            assert!(checks.contains(run), "{file}: `{run}` is in checks.md");
        }
    }
    assert_eq!(checks.matches("(GN-30, AC 1").count(), 1, "the twins check stands once");
    assert_eq!(checks.matches("(GN-30, AC 2").count(), 1, "the warning-free build check stands once");
}

#[test]
fn the_generics_check_ids_are_unique_across_its_phases() {
    // generics GN-31 AC 1: a check id names one claim in the feature's `checks.md` and in the four task files (the GN-30 checks of
    // phases C to E stand once, as the twins and warning-free-build checks of `tasks.md`), so a proof is reported under its own claim
    let mut seen: Vec<(String, String)> = Vec::new();
    for file in ["tasks.md", "tasks-c.md", "tasks-d.md", "tasks-e.md", "checks.md"] {
        let text = RepositoryRecords::read(&format!(".specs/features/generics/{file}"));
        let section = if file == "checks.md" { text.split("\n## Checks\n").nth(1) } else { text.split("\n## Checks\n").nth(1).and_then(|s| s.split("\n## Handoff").next()) };
        let mut ids: Vec<&str> = Vec::new();
        for line in section.expect("a Checks section").lines().filter(|l| l.starts_with("**C")) {
            let id = line.trim_start_matches("**").split("** - ").next().expect("a check id");
            if file != "tasks.md" && file != "checks.md" && line.contains("(GN-30,") {
                continue;
            }
            assert!(!ids.contains(&id), "{file}: the check id {id} is used twice");
            ids.push(id);
            if file != "checks.md" {
                if let Some((other, _)) = seen.iter().find(|(known, _)| known == id) {
                    panic!("{file}: the check id {id} is also in {other}");
                }
                seen.push((id.to_string(), file.to_string()));
            }
        }
    }
}
