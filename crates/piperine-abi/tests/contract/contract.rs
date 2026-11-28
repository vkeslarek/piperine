//! The C contract of abi.md §3 as `piperine-abi` compiles it (feature
//! `solver-abi-core`, SAC-01, SAC-02, SAC-20, SAC-21, SAC-22): every constant's
//! pinned value, stride-aware list reading and its refusal of a stride below
//! the element, the crate's empty dependency list, the raw views' private
//! pointer fields, and `unsafe` confined to `raw.rs`, `thunk.rs` and
//! `caller.rs`. The struct layout table (size, alignment, field offsets) is a
//! unit test of `raw.rs`, where the private pointer fields can be named.

use core::mem::{align_of, size_of};
use piperine_abi::raw::*;

#[test]
fn every_constant_has_its_pinned_value() {
    let groups: &[(&str, &[(u32, u32)])] = &[
        ("kind", &[(kind::CONTINUOUS, 1), (kind::DISCRETE, 2), (kind::EVENT, 3)]),
        ("role", &[(role::PORT, 1), (role::PARAM, 2), (role::GLOBAL, 3), (role::PRIVATE, 4)]),
        ("global", &[
            (global::TEMPERATURE, 1), (global::RESOLUTION, 2), (global::DC, 3), (global::TRAN, 4),
            (global::AC, 5), (global::NOISE, 6), (global::START, 7), (global::SETTLE, 8),
            (global::NAMED, 9), (global::TEMPERATURE_GIVEN, 10),
        ]),
        ("pin_flag", &[
            (pin_flag::STRUCTURAL, 1), (pin_flag::OPTIONAL, 2), (pin_flag::HAS_INIT, 4),
            (pin_flag::HAS_CHECK, 8), (pin_flag::QUIET, 16), (pin_flag::RECORD, 32),
            (pin_flag::RELAX, 64),
        ]),
        ("leaf", &[
            (leaf::BOOL, 1), (leaf::U8, 2), (leaf::U16, 3), (leaf::U32, 4), (leaf::U64, 5),
            (leaf::I8, 6), (leaf::I16, 7), (leaf::I32, 8), (leaf::I64, 9), (leaf::F32, 10),
            (leaf::F64, 11),
        ]),
        ("op", &[(op::FLOW, 1), (op::DDT, 2), (op::DELAY, 3)]),
        ("input", &[(input::LIMIT, 1), (input::NOISE, 2), (input::STIM, 3)]),
        ("watch", &[(watch::VAR, 1), (watch::CROSS, 2), (watch::CAP, 3), (watch::TRACE, 4)]),
        ("cell", &[
            (cell::PIN, 1), (cell::TERM, 2), (cell::INPUT, 3), (cell::WATCH, 4), (cell::AMBIENT, 5),
        ]),
        ("part", &[
            (part::VALUE, 1), (part::PREVIOUS, 2), (part::CHANGED, 3), (part::TIME, 4),
            (part::LAST, 5), (part::AGE, 6), (part::PROPOSED, 7), (part::LIMITED, 8),
            (part::PSD, 9), (part::RE, 10), (part::IM, 11),
        ]),
        ("ambient", &[
            (ambient::TIME, 1), (ambient::FREQ, 2), (ambient::STATIC, 3), (ambient::GMIN, 4),
            (ambient::SOURCE_SCALE, 5), (ambient::TICK, 6), (ambient::HORIZON, 7),
        ]),
        ("fn_flag", &[(fn_flag::OWNS_CORRECTION, 1)]),
        ("reaction_flag", &[(reaction_flag::IRREVERSIBLE, 1)]),
        ("statement_flag", &[(statement_flag::LATER, 1), (statement_flag::REPLACE, 2)]),
        ("status", &[(status::OK, 1), (status::FAULT, 2), (status::RESHAPE, 3), (status::FINISH, 4)]),
    ];
    for (name, pairs) in groups {
        for (got, want) in pairs.iter() {
            assert_eq!(got, want, "{name}: pinned value");
        }
        let values: Vec<u32> = pairs.iter().map(|(v, _)| *v).collect();
        ContractReading::assert_group(name, &values);
    }
    assert_eq!(GROUND, u32::MAX);
    assert_eq!(DEFAULT, u32::MAX - 1);
    assert_ne!(GROUND, DEFAULT);
    assert_eq!(MAJOR, 1);
    // the contract is unreleased: major 1, edited in place, minor 0 (abi.md §3)
    assert_eq!(MINOR, 0);
    assert_eq!(SYMBOL, "piperine_abi_1");
    assert_eq!(NOTE_TEXT, 248);
}

#[test]
fn the_host_has_one_commit() {
    // EU-16 AC 2 (abi.md §2 entry 6): the solver's `Host` is its `size`, `commit` and, since arrays of carriers
    // (CV-27, QC1: appended in place at minor 0), `commit_element`: no `commit_with`, no commit flags, so it is 24
    // bytes and the contract is minor 0. Superseded here: it read "16 bytes, the one commit".
    assert_eq!(size_of::<Host>(), 24);
    assert_eq!(core::mem::offset_of!(Host, commit), 8);
    assert_eq!(core::mem::offset_of!(Host, commit_element), 16);
    assert_eq!(MINOR, 0);
}

/// A `Pin` as a newer producer compiled it: 16 more bytes after the fields
/// this version knows.
#[repr(C)]
struct WiderPin {
    pin: Pin<'static>,
    appended: [u8; 16],
}

#[test]
fn list_reads_by_the_producers_stride() {
    let wide = [
        WiderPin { pin: ContractReading::pin_named("p"), appended: [0xAA; 16] },
        WiderPin { pin: ContractReading::pin_named("n"), appended: [0xBB; 16] },
        WiderPin { pin: ContractReading::pin_named("f"), appended: [0xCC; 16] },
    ];
    assert_eq!(size_of::<WiderPin>(), size_of::<Pin>() + 16);
    // SAFETY: `wide` holds three elements `size_of::<Pin>() + 16` bytes apart
    // and outlives `list`.
    let list: List<Pin> = unsafe { List::from_raw_parts(wide.as_ptr().cast::<Pin>(), wide.len(), size_of::<Pin>() + 16) };
    let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["p", "n", "f"]);
    assert_eq!(list.get(1).map(|p| p.name.as_str()), Some("n"));
    assert_eq!(list.get(2).map(|p| p.kind), Some(kind::CONTINUOUS));
    assert!(list.get(3).is_none(), "an index past the end is None");
}

#[test]
fn empty_list_yields_nothing() {
    let list: List<Pin> = List::empty();
    assert!(!list.as_ptr().is_null(), "an empty list's pointer is non-null");
    assert_eq!(list.as_ptr() as usize % align_of::<Pin>(), 0, "and aligned");
    assert_eq!(list.len(), 0);
    assert!(list.get(0).is_none());
    assert_eq!(list.iter().count(), 0);

    let from_empty: List<Cell> = List::from_slice(&[]);
    assert!(!from_empty.as_ptr().is_null());
    assert_eq!(from_empty.iter().count(), 0);
    assert_eq!(Str::EMPTY.as_str(), "");
    assert!(Bytes::EMPTY.as_slice().is_empty());
}

#[test]
fn crate_declares_no_dependency() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let manifest = std::fs::read_to_string(path).expect("read the crate manifest");
    let deps = ContractReading::section(&manifest, "dependencies").expect("the manifest has a [dependencies] section");
    let entries: Vec<&str> = deps
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert!(entries.is_empty(), "piperine-abi declares dependencies: {entries:?}");
    assert!(ContractReading::section(&manifest, "build-dependencies").is_none(), "piperine-abi has a [build-dependencies] section");
    let dev = ContractReading::section(&manifest, "dev-dependencies").unwrap_or_default();
    let dev_entries: Vec<&str> = dev.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    assert!(dev_entries.iter().all(|l| l.starts_with("piperine-device-fixtures")), "piperine-abi has a dev-dependency other than the fixtures crate: {dev_entries:?}");
    assert!(
        !manifest.contains("dependencies."),
        "piperine-abi declares a dependency through a dotted table"
    );
}

#[test]
fn a_list_whose_stride_is_below_its_element_is_refused() {
    // An older producer's element is 4 bytes; this consumer's `T` is 8: reading
    // element 3 would run past the producer's 16 bytes.
    let buf: [[u8; 8]; 2] = [[1; 8], [2; 8]];
    // SAFETY: 4 elements of 4 bytes lie in `buf`'s 16 bytes.
    let short: List<[u8; 8]> = unsafe { List::from_raw_parts(buf.as_ptr(), 4, 4) };
    assert!(!short.is_readable());
    assert!(short.get(0).is_none() && short.get(3).is_none(), "no element is read");
    assert_eq!(short.iter().count(), 0);
    // A stride that breaks the element's alignment is refused too.
    let pins = [ContractReading::pin_named("p"), ContractReading::pin_named("n")];
    // SAFETY: one element at `pins`, which outlives the list.
    let odd: List<Pin> = unsafe { List::from_raw_parts(pins.as_ptr(), 1, size_of::<Pin>() + 1) };
    assert!(!odd.is_readable() && odd.get(0).is_none());
    // A misaligned pointer is refused.
    // SAFETY: the pointer lies inside `pins`; the list is never read (it is refused).
    let skewed: List<Pin> =
        unsafe { List::from_raw_parts(pins.as_ptr().cast::<u8>().wrapping_add(4).cast::<Pin>(), 1, size_of::<Pin>()) };
    assert!(!skewed.is_readable() && skewed.get(0).is_none());
    // The producer's own stride, and a larger one, are read.
    assert!(List::from_slice(&pins).is_readable());
    assert_eq!(List::from_slice(&pins).get(1).map(|p| p.name.as_str()), Some("n"));
    assert!(List::<Pin>::empty().is_readable());
}

#[test]
fn raw_views_have_no_public_pointer_field() {
    let raw = ContractReading::raw_sources();
    for field in [
        "pub ptr:", "pub len: usize", "pub stride:", "pub prototype:", "pub data:", "pub create:", "pub destroy:",
        "pub eval:", "pub react:",
    ] {
        let found: Vec<&str> = raw.lines().filter(|l| l.trim_start().starts_with(field)).collect();
        assert!(found.is_empty(), "raw.rs exposes `{field}`: {found:?}");
    }
    let unsafe_constructors = raw.lines().filter(|l| l.contains("pub const unsafe fn from_raw_parts")).count();
    assert_eq!(unsafe_constructors, 4, "Str, Bytes, List and Module each have one unsafe constructor");
    // Safe code builds only valid views.
    let s = Str::from_static("abc");
    assert_eq!((s.as_str(), s.len()), ("abc", 3));
    let b = Bytes::from_static(&[1, 2]);
    assert_eq!(b.as_slice(), [1, 2]);
}

#[test]
fn no_unsafe_outside_raw_thunk_and_caller() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut seen = 0;
    for entry in std::fs::read_dir(&src).expect("read src") {
        let path = entry.expect("dir entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if ["raw.rs", "raw", "thunk.rs", "caller.rs"].contains(&name.as_str()) {
            continue;
        }
        seen += 1;
        let found = ContractReading::unsafe_lines(&std::fs::read_to_string(&path).expect("read a source"));
        assert!(found.is_empty(), "{name} uses unsafe: {found:?}");
    }
    assert!(seen >= 6, "the scan saw {seen} files");
    // The scan catches what it forbids and ignores comments.
    assert_eq!(ContractReading::unsafe_lines("    let x = unsafe { *p };").len(), 1);
    assert!(ContractReading::unsafe_lines("    // nothing here is `unsafe`").is_empty());
}

/// Reads the contract's sources and constants for the tests.
pub struct ContractReading;

impl ContractReading {
    /// The text of `raw.rs` and of every file of `raw/`, one after the other.
    fn raw_sources() -> String {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut text = std::fs::read_to_string(src.join("raw.rs")).expect("read raw.rs");
        let mut parts: Vec<_> = std::fs::read_dir(src.join("raw")).expect("read raw/").map(|e| e.expect("entry").path()).collect();
        parts.sort();
        for part in parts {
            text.push_str(&std::fs::read_to_string(part).expect("read a raw section"));
        }
        text
    }

    /// A group of constants: no member is 0 and no two are equal.
    fn assert_group(name: &str, group: &[u32]) {
        for (i, a) in group.iter().enumerate() {
            assert_ne!(*a, 0, "{name}: a constant is 0");
            for b in &group[i + 1..] {
                assert_ne!(a, b, "{name}: duplicate value {a}");
            }
        }
    }

    /// The pin named helper `pin_named`.
    fn pin_named(name: &'static str) -> Pin<'static> {
        Pin {
            name: Str::from_static(name),
            origin: Str::from_static("t.msl:1:1"),
            kind: kind::CONTINUOUS,
            role: role::PORT,
            global: 0,
            flags: 0,
            layout: Layout { size: 8, align: 8, fields: List::empty() },
            unit: Str::EMPTY,
            init: Bytes::EMPTY,
            check: 0,
            abstol: [0.0, 0.0],
            count: 0,
        }
    }

    /// The text of one `[section]` of a manifest, `None` when the header is absent.
    fn section(manifest: &str, name: &str) -> Option<String> {
        let header = format!("[{name}]");
        let mut out = None;
        for line in manifest.lines() {
            let t = line.trim();
            if t.starts_with('[') {
                if t == header {
                    out = Some(String::new());
                } else if out.is_some() {
                    break;
                }
                continue;
            }
            if let Some(text) = out.as_mut() {
                text.push_str(line);
                text.push('\n');
            }
        }
        out
    }

    /// Code lines (comments aside) of `text` that use `unsafe`.
    fn unsafe_lines(text: &str) -> Vec<String> {
        text.lines()
            .map(str::trim_start)
            .filter(|l| !l.starts_with("//") && l.contains("unsafe"))
            .map(str::to_string)
            .collect()
    }
}
