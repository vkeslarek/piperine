//! T17 — policy gates (LOV-01, LOV-05, MD-31): registry-driven guards,
//! never a hardcoded test-name list. Pattern: `tests/delegation_guard.rs`
//! (source-walking, a `const` registry so a new entity is exhaustively
//! accounted for rather than silently unguarded).
//!
//! Two invariants:
//!
//! 1. **Concept budget** (`ideal.md` §2) — `core/` names exactly five
//!    concepts (value class, net, value graph, cell, block). A sixth
//!    appearing during migration is a signal to stop, not to add.
//! 2. **No catch-all** (the A0c defence) — a dispatch over `Op` or a
//!    `NetKind` never has a `_ =>` arm: an unhandled construct must fail
//!    loud by name, never fall through silently.
//!
//! MD-31 obligation: both guards below were proven able to fail during
//! this task — a dummy `core/` file and an injected `_ =>` arm were each
//! added locally, the corresponding test was observed to go red naming
//! the violation, then the injection was reverted. Recorded in this
//! commit's message (not left as dead code here — MD-35 forbids that).

use std::fs;
use std::path::PathBuf;

/// The repository root — this target lives in `piperine-ir`'s own
/// `tests/`, two levels below the workspace root.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

// ─── Guard 1: the concept budget ────────────────────────────────────────

/// What one `core/` source file IS — one of the five concepts, or
/// infrastructure the concepts are built on top of (never a sixth
/// concept in disguise).
enum Role {
    Concept(&'static str),
    Infrastructure(&'static str),
}

struct CoreFile {
    file: &'static str,
    role: Role,
}

impl CoreFile {
    fn describe(&self) -> String {
        match &self.role {
            Role::Concept(d) => format!("concept: {d}"),
            Role::Infrastructure(d) => format!("infrastructure: {d}"),
        }
    }
}

/// The full, exhaustive account of `crates/piperine-ir/src/core/`.
/// EXACTLY five `Role::Concept` entries — that count is itself asserted
/// below, so the registry cannot silently grow a sixth concept either.
const CORE_FILES: &[CoreFile] = &[
    CoreFile { file: "value.rs", role: Role::Concept("value class — ideal.md §1.1") },
    CoreFile { file: "net.rs", role: Role::Concept("net — ideal.md §1.2") },
    CoreFile { file: "op.rs", role: Role::Concept("value graph — ideal.md §1.3") },
    CoreFile { file: "cell.rs", role: Role::Concept("cell — ideal.md §1.4") },
    CoreFile { file: "block.rs", role: Role::Concept("block — ideal.md §1.5") },
    CoreFile { file: "module.rs", role: Role::Infrastructure("arena storage (Vecs + newtype ids), not a concept") },
    CoreFile { file: "sigref.rs", role: Role::Infrastructure("chunk-list representation (ideal.md §3), not an op") },
    CoreFile { file: "trigger.rs", role: Role::Infrastructure("trigger vocabulary — part of the block concept, §1.5") },
    CoreFile { file: "mod.rs", role: Role::Infrastructure("module wiring: id newtypes + re-exports (<=60 lines, MD-34)") },
];

#[test]
fn the_registry_itself_declares_exactly_five_concepts() {
    let concepts = CORE_FILES.iter().filter(|f| matches!(f.role, Role::Concept(_))).count();
    assert_eq!(
        concepts, 5,
        "ideal.md §2: five concepts, no sixth — CORE_FILES itself must name exactly five \
         Role::Concept entries (found {concepts})"
    );
}

/// The concept-budget guard: every `.rs` file actually present in
/// `core/` must be accounted for in [`CORE_FILES`]. A NEW file — a sixth
/// concept, or undeclared infrastructure — fails this test by name.
#[test]
fn core_contains_no_file_unaccounted_for_in_the_concept_registry() {
    let core_dir = repo_root().join("crates/piperine-ir/src/core");
    let known: Vec<&str> = CORE_FILES.iter().map(|f| f.file).collect();
    let mut unknown = Vec::new();
    for entry in fs::read_dir(&core_dir).expect("read core/").flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !known.contains(&name.as_str()) {
            unknown.push(name);
        }
    }
    let registry_dump: Vec<String> = CORE_FILES.iter().map(|f| format!("  {} — {}", f.file, f.describe())).collect();
    assert!(
        unknown.is_empty(),
        "core/ contains file(s) not accounted for in CORE_FILES (a sixth concept, or \
         undeclared infrastructure) — account for them there or delete them: {unknown:?}\n\
         Known files:\n{}",
        registry_dump.join("\n")
    );
}

/// Every file [`CORE_FILES`] names must actually exist — the registry
/// cannot silently drift stale after a file is renamed or removed.
#[test]
fn every_registered_core_file_actually_exists() {
    let core_dir = repo_root().join("crates/piperine-ir/src/core");
    let mut missing = Vec::new();
    for f in CORE_FILES {
        if !core_dir.join(f.file).is_file() {
            missing.push(f.file);
        }
    }
    assert!(missing.is_empty(), "CORE_FILES names file(s) that no longer exist: {missing:?}");
}

// ─── Guard 2: no catch-all in a dispatch over `Op` or a net kind ────────

/// One dispatch site: a file, the function it lives in (`fn_marker` — a
/// unique `"fn name("` substring that isolates just this function's
/// body, so an unrelated function with a same-named `op`/`net` parameter
/// — e.g. `interp.rs::eval_pure(op: PureOp, ...)` — is never confused
/// with the `Op`/`NetKind` dispatch this guard actually means), and the
/// `match_marker` that opens the specific `match` to check WITHIN that
/// function body (may occur more than once inside one function — the
/// verifier's `check_op` performs the same net-kind match twice, once
/// for `ReadSignal` and once for `Drive`; both are checked).
struct DispatchSite {
    file: &'static str,
    fn_marker: &'static str,
    match_marker: &'static str,
    description: &'static str,
}

/// Every known dispatch over `Op` or `NetKind` in `piperine-ir`, plus
/// (T18) `piperine-codegen`'s `backend/` — the first Phase-4 dispatch over
/// `Op` outside `piperine-ir` (design.md §6's stated eventual target for
/// this guard). Four per-op dispatch functions in `piperine-ir` (verifier,
/// interpreter, printer, differentiator) plus the two `NetKind` dispatches
/// the verifier and printer each perform, plus three in `piperine-codegen`
/// (`backend::OpCtx::compile_op`, `backend::PortMap::walk`,
/// `backend::OpCtx::collect_reads_writes`). `lower/` (Phase 6, T29) has no
/// dispatch yet — nothing to register there today.
const DISPATCH_SITES: &[DispatchSite] = &[
    DispatchSite {
        file: "crates/piperine-ir/src/verify.rs",
        fn_marker: "fn check_op(",
        match_marker: "match op {",
        description: "verifier's per-Op dispatch (the A0c defence itself)",
    },
    DispatchSite {
        file: "crates/piperine-ir/src/interp.rs",
        fn_marker: "fn eval_op(",
        match_marker: "match op {",
        description: "reference interpreter's per-Op dispatch",
    },
    DispatchSite {
        file: "crates/piperine-ir/src/print.rs",
        fn_marker: "fn write_op(",
        match_marker: "match op {",
        description: "printer's per-Op dispatch",
    },
    DispatchSite {
        file: "crates/piperine-ir/src/diff.rs",
        fn_marker: "fn d_op(",
        match_marker: "match op {",
        description: "differentiator's per-Op dispatch",
    },
    DispatchSite {
        file: "crates/piperine-ir/src/verify.rs",
        fn_marker: "fn check_op(",
        match_marker: "net_at(*net).kind() {",
        description: "verifier's net-kind dispatch (ReadSignal/Drive output typing, both occurrences)",
    },
    DispatchSite {
        file: "crates/piperine-ir/src/print.rs",
        fn_marker: "fn write_net(",
        match_marker: "match &net.kind {",
        description: "printer's net-kind dispatch",
    },
    DispatchSite {
        file: "crates/piperine-codegen/src/backend/engine.rs",
        fn_marker: "fn compile_op(",
        match_marker: "match op {",
        description: "T18 analog/digital backend's per-Op emission dispatch (Cranelift codegen)",
    },
    DispatchSite {
        file: "crates/piperine-codegen/src/backend/engine.rs",
        fn_marker: "fn walk(",
        match_marker: "match op {",
        description: "T18 backend's PortMap scan — per-Op dispatch over contribute/force/drive sites",
    },
    DispatchSite {
        file: "crates/piperine-codegen/src/backend/engine.rs",
        fn_marker: "fn collect_reads_writes(",
        match_marker: "match op {",
        description: "T18 backend's seed-value scan — per-Op dispatch collecting reads/writes",
    },
];

/// Extract the balanced-brace body of the function starting at
/// `fn_marker`'s first occurrence in `text` — from `fn_marker`'s own
/// opening `{` through its matching closing `}`. Isolates one function
/// so a marker cannot bleed into a later, unrelated one.
fn function_body<'t>(text: &'t str, fn_marker: &str) -> &'t str {
    let start = text.find(fn_marker).unwrap_or_else(|| panic!("fn marker `{fn_marker}` not found"));
    let open = start + text[start..].find('{').expect("fn signature must be followed by `{`");
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut i = open;
    loop {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &text[open..=i];
                }
            }
            _ => {}
        }
        i += 1;
        assert!(i < bytes.len(), "unbalanced braces scanning function `{fn_marker}`");
    }
}

/// Within `body`, find every occurrence of `match_marker` and check the
/// balanced-brace block that follows each for a TOP-LEVEL `_ =>` arm —
/// "top level" meaning depth 1 relative to THAT match's own opening
/// brace, so a legitimate nested match inside one arm's body (e.g.
/// `diff.rs::d_op`'s `match wrt { ... }` inside the `ReadAcross` arm) is
/// never mistaken for a catch-all on the outer dispatch. Returns the
/// number of catch-all arms found.
fn top_level_catch_all_count(body: &str, match_marker: &str) -> usize {
    let bytes = body.as_bytes();
    let mut found = 0usize;
    for (start, _) in body.match_indices(match_marker) {
        let open = start + body[start..].find('{').expect("match_marker must be followed by `{`");
        let mut depth = 0i32;
        let mut i = open;
        loop {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ if depth == 1 && (body[i..].starts_with("_ =>") || body[i..].starts_with("_=>")) => {
                    found += 1;
                }
                _ => {}
            }
            i += 1;
            assert!(i < bytes.len(), "unbalanced braces scanning match `{match_marker}` at byte {start}");
        }
    }
    found
}

#[test]
fn dispatch_site_registry_is_not_empty() {
    assert!(!DISPATCH_SITES.is_empty(), "DISPATCH_SITES must name at least the four per-Op dispatches");
}

/// The no-catch-all guard itself.
#[test]
fn no_catch_all_arm_in_any_op_or_net_kind_dispatch() {
    let root = repo_root();
    let mut offences = Vec::new();
    for site in DISPATCH_SITES {
        let text = fs::read_to_string(root.join(site.file)).unwrap_or_else(|e| panic!("read {}: {e}", site.file));
        let body = function_body(&text, site.fn_marker);
        let hits = top_level_catch_all_count(body, site.match_marker);
        if hits > 0 {
            offences.push(format!(
                "{} (`{}`, `{}`): {} — {} top-level `_ =>` arm(s)",
                site.file, site.fn_marker, site.match_marker, site.description, hits
            ));
        }
    }
    assert!(offences.is_empty(), "no_catch_all_arm_in_any_op_or_net_kind_dispatch found:\n{}", offences.join("\n"));
}

/// Every site names a function and a match marker that actually appear —
/// a registry entry that silently stopped matching anything would make
/// the guard above vacuously pass.
#[test]
fn every_dispatch_site_marker_actually_appears_in_its_function() {
    let root = repo_root();
    for site in DISPATCH_SITES {
        let text = fs::read_to_string(root.join(site.file)).unwrap_or_else(|e| panic!("read {}: {e}", site.file));
        let body = function_body(&text, site.fn_marker);
        assert!(
            body.contains(site.match_marker),
            "{}: fn `{}` no longer contains match marker `{}` ({}) — the registry entry is stale",
            site.file,
            site.fn_marker,
            site.match_marker,
            site.description
        );
    }
}
