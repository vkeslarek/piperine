#!/usr/bin/env python3
"""Test-allocation audit for the P6 cleanup feature (CLN-01).

Emits mechanical facts about every `#[test]` function in the workspace so the
unit-vs-integration verdict can be made from evidence instead of memory. The
`kind_hint` column is **advisory** — a regex-level tool must never move a
test; `audit.md` records the human/agent verdict per row (design §Components).

Usage:
    audit_tests.py --root .                  # TSV report on stdout
    audit_tests.py --root . --summary        # per-crate counts
    audit_tests.py --root . --check <crate>  # regression check, exit 1 on a
                                             # placement violation
    audit_tests.py --root . --check-all      # every crate, exit 1 on any

Placement violations are `unit`-hinted tests living in a `tests/` target, or
`integration`-hinted tests living inline in `src/`. A verdict that knowingly
contradicts the hint is recorded in `tools/audit_allow.tsv`
(`<file>::<test>\treason`) and skipped by `--check`.
"""

import argparse
import os
import re
import sys

# ── What makes a test "integration": it drives a pipeline boundary ───────────
# Each entry is a needle searched in the test's own body and its file preamble.
PIPELINE_MARKERS = (
    "parse_and_elaborate",
    "parse_and_elaborate_seeded",
    "lower_bodies",
    "CircuitCompiler",
    "CompiledModule",
    "AnalogKernel::compile",
    "DigitalKernel::compile",
    "SimSession",
    "LiveSession",
    "PluginHost",
    "Command::new",
    "Connection::memory",
    "run_script",
    "CircuitBuilder",
    "DcSolver",
    "TransientSolver",
    "trybuild",
)

# Process-global state: a test touching these must keep its serialization
# guard when relocated (CLN-10).
GLOBAL_STATE_MARKERS = (
    "env::set_var",
    "env::remove_var",
    "set_current_dir",
    "facade_lock",
    "Python::with_gil",
    "std::env::var",
)

TEST_ATTR = re.compile(r"^\s*#\[(?:\w+::)?test\b")
IGNORE_ATTR = re.compile(r"^\s*#\[ignore")
FN_NAME = re.compile(r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)")
DISABLED_FILE = re.compile(r"^\s*#!\[cfg\(any\(\)\)\]")


class TestCase:
    """One `#[test]` function and the facts that classify it."""

    def __init__(self, crate, path, name, placement, body, preamble, disabled, ignored):
        self.crate = crate
        self.path = path
        self.name = name
        self.placement = placement  # "inline" | "tests"
        self.body = body
        self.preamble = preamble
        self.disabled = disabled
        self.ignored = ignored

    @property
    def markers(self):
        haystack = self.body + "\n" + self.preamble
        return [m for m in PIPELINE_MARKERS if m in haystack]

    @property
    def global_state(self):
        return [m for m in GLOBAL_STATE_MARKERS if m in self.body]

    @property
    def kind_hint(self):
        if self.markers:
            return "integration"
        if self.placement == "tests" and len(self.body.splitlines()) > 60:
            # A long test in an integration target with no recognised entry
            # point: too big to classify mechanically.
            return "unclear"
        return "unit"

    @property
    def evidence(self):
        parts = []
        if self.markers:
            parts.append("entry:" + ",".join(self.markers))
        if self.global_state:
            parts.append("global:" + ",".join(self.global_state))
        if self.disabled:
            parts.append("DISABLED_FILE")
        if self.ignored:
            parts.append("IGNORED")
        parts.append(f"lines:{len(self.body.splitlines())}")
        return " ".join(parts)

    @property
    def violation(self):
        """Placement that contradicts the hint (what `--check` reports)."""
        if self.kind_hint == "unit" and self.placement == "tests":
            return "unit test in a tests/ target"
        if self.kind_hint == "integration" and self.placement == "inline":
            return "integration test inline in src/"
        return None

    def key(self):
        return f"{self.path}::{self.name}"

    def row(self):
        return "\t".join(
            [self.crate, self.path, self.name, self.placement, self.kind_hint, self.evidence]
        )


class Scanner:
    """Walks the workspace and extracts every test case."""

    def __init__(self, root):
        self.root = os.path.abspath(root)

    def rust_files(self):
        """Every `.rs` under `crates/` and the root `tests/`, skipping build output."""
        for base in ("crates", "tests", "src"):
            top = os.path.join(self.root, base)
            for dirpath, dirnames, filenames in os.walk(top):
                dirnames[:] = [d for d in dirnames if d not in ("target", ".git")]
                for name in sorted(filenames):
                    if name.endswith(".rs"):
                        yield os.path.join(dirpath, name)

    def crate_of(self, path):
        rel = os.path.relpath(path, self.root)
        parts = rel.split(os.sep)
        if parts[0] == "crates":
            return parts[1]
        return "piperine"

    @staticmethod
    def placement_of(path):
        rel = path.replace(os.sep, "/")
        return "tests" if "/tests/" in rel or rel.endswith("/tests.rs") else "inline"

    @staticmethod
    def body_after(lines, start):
        """The braced body of the fn whose signature starts at `lines[start]`."""
        depth = 0
        collected = []
        for line in lines[start:]:
            collected.append(line)
            depth += line.count("{") - line.count("}")
            if depth <= 0 and "{" in "".join(collected):
                break
        return "\n".join(collected)

    def scan_file(self, path):
        with open(path, encoding="utf-8", errors="replace") as handle:
            text = handle.read()
        lines = text.splitlines()
        disabled = bool(lines and DISABLED_FILE.match(lines[0]))
        preamble = "\n".join(lines[:40])
        crate = self.crate_of(path)
        rel = os.path.relpath(path, self.root)
        placement = self.placement_of(path)
        cases = []
        index = 0
        while index < len(lines):
            if TEST_ATTR.match(lines[index]):
                ignored = False
                cursor = index + 1
                while cursor < len(lines) and not FN_NAME.match(lines[cursor]):
                    if IGNORE_ATTR.match(lines[cursor]):
                        ignored = True
                    cursor += 1
                if cursor < len(lines):
                    name = FN_NAME.match(lines[cursor]).group(1)
                    cases.append(
                        TestCase(
                            crate,
                            rel,
                            name,
                            placement,
                            self.body_after(lines, cursor),
                            preamble,
                            disabled,
                            ignored,
                        )
                    )
                index = cursor + 1
                continue
            index += 1
        return cases

    def scan(self):
        cases = []
        for path in self.rust_files():
            cases.extend(self.scan_file(path))
        return cases


def load_allowlist(root):
    """`<file>::<test>` entries whose verdict knowingly contradicts the hint."""
    path = os.path.join(
        root, ".specs", "features", "p6-cleanup-completeness", "tools", "audit_allow.tsv"
    )
    allowed = {}
    if not os.path.isfile(path):
        return allowed
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            key, _, reason = line.partition("\t")
            allowed[key.strip()] = reason.strip()
    return allowed


def report(cases):
    print("crate\tfile\ttest\tplacement\tkind_hint\tevidence")
    for case in cases:
        print(case.row())


def summary(cases):
    crates = {}
    for case in cases:
        bucket = crates.setdefault(
            case.crate,
            {"total": 0, "inline": 0, "tests": 0, "unit": 0, "integration": 0, "unclear": 0,
             "disabled": 0, "ignored": 0, "violations": 0},
        )
        bucket["total"] += 1
        bucket[case.placement] += 1
        bucket[case.kind_hint] += 1
        if case.disabled:
            bucket["disabled"] += 1
        if case.ignored:
            bucket["ignored"] += 1
        if case.violation:
            bucket["violations"] += 1
    header = ("crate", "total", "inline", "tests", "unit", "integration", "unclear",
              "disabled", "ignored", "violations")
    print("\t".join(header))
    for crate in sorted(crates):
        b = crates[crate]
        print("\t".join([crate] + [str(b[k]) for k in header[1:]]))
    print("\t".join(["TOTAL", str(sum(b["total"] for b in crates.values()))]
                    + [str(sum(b[k] for b in crates.values())) for k in header[2:]]))


def check(cases, allowed, crate=None):
    violations = [
        c for c in cases
        if c.violation and (crate is None or c.crate == crate) and c.key() not in allowed
    ]
    for case in violations:
        print(f"{case.path}::{case.name}: {case.violation} ({case.evidence})", file=sys.stderr)
    if violations:
        scope = crate or "workspace"
        print(f"{len(violations)} placement violation(s) in {scope}", file=sys.stderr)
        return 1
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="repository root")
    parser.add_argument("--summary", action="store_true", help="per-crate counts")
    parser.add_argument("--check", metavar="CRATE", help="fail on violations in CRATE")
    parser.add_argument("--check-all", action="store_true", help="fail on any violation")
    args = parser.parse_args()

    cases = Scanner(args.root).scan()
    allowed = load_allowlist(args.root)

    if args.check or args.check_all:
        return check(cases, allowed, args.check)
    if args.summary:
        summary(cases)
        return 0
    report(cases)
    return 0


if __name__ == "__main__":
    sys.exit(main())
