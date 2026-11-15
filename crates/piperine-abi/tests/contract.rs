//! The C contract of abi.md §3 as `piperine-abi` compiles it (feature
//! `solver-abi-core`, SAC-01, SAC-02): every struct's size, alignment and
//! field offsets on a 64-bit target against a hand-written C-layout table,
//! every constant's pinned value, stride-aware list reading, and the crate's
//! empty dependency list.

use core::mem::{align_of, offset_of, size_of};
use piperine_abi::raw::*;

/// One struct of the table: its measured size and alignment against the C
/// layout, and each field's measured offset against the expected one.
fn check(name: &str, size: (usize, usize), align: (usize, usize), fields: &[(&str, usize, usize)]) {
    assert_eq!(size.0, size.1, "size_of::<{name}>()");
    assert_eq!(align.0, align.1, "align_of::<{name}>()");
    for (field, got, want) in fields {
        assert_eq!(got, want, "offset of {name}.{field}");
    }
}

#[test]
fn struct_layouts_match_the_abi() {
    assert_eq!(size_of::<usize>(), 8, "the table is for a 64-bit target");

    check("Str", (size_of::<Str>(), 16), (align_of::<Str>(), 8), &[
        ("ptr", offset_of!(Str, ptr), 0),
        ("len", offset_of!(Str, len), 8),
    ]);
    check("Bytes", (size_of::<Bytes>(), 16), (align_of::<Bytes>(), 8), &[
        ("ptr", offset_of!(Bytes, ptr), 0),
        ("len", offset_of!(Bytes, len), 8),
    ]);
    check("List", (size_of::<List<Pin>>(), 24), (align_of::<List<Pin>>(), 8), &[
        ("ptr", offset_of!(List<Pin>, ptr), 0),
        ("len", offset_of!(List<Pin>, len), 8),
        ("stride", offset_of!(List<Pin>, stride), 16),
    ]);
    check("Library", (size_of::<Library>(), 64), (align_of::<Library>(), 8), &[
        ("size", offset_of!(Library, size), 0),
        ("minor", offset_of!(Library, minor), 4),
        ("name", offset_of!(Library, name), 8),
        ("build", offset_of!(Library, build), 24),
        ("modules", offset_of!(Library, modules), 40),
    ]);
    check("Module", (size_of::<Module>(), 64), (align_of::<Module>(), 8), &[
        ("name", offset_of!(Module, name), 0),
        ("prototype", offset_of!(Module, prototype), 16),
        ("data", offset_of!(Module, data), 24),
        ("create", offset_of!(Module, create), 32),
        ("destroy", offset_of!(Module, destroy), 40),
        ("eval", offset_of!(Module, eval), 48),
        ("react", offset_of!(Module, react), 56),
    ]);
    check("Value", (size_of::<Value>(), 24), (align_of::<Value>(), 8), &[
        ("pin", offset_of!(Value, pin), 0),
        ("bytes", offset_of!(Value, bytes), 8),
    ]);
    check("Created", (size_of::<Created>(), 24), (align_of::<Created>(), 8), &[
        ("status", offset_of!(Created, status), 0),
        ("instance", offset_of!(Created, instance), 8),
        ("shape", offset_of!(Created, shape), 16),
    ]);
    check("Instance", (size_of::<Instance>(), 0), (align_of::<Instance>(), 1), &[]);
    check("Note", (size_of::<Note>(), 256), (align_of::<Note>(), 4), &[
        ("code", offset_of!(Note, code), 0),
        ("len", offset_of!(Note, len), 4),
        ("text", offset_of!(Note, text), 8),
    ]);
    check("Shape", (size_of::<Shape>(), 152), (align_of::<Shape>(), 8), &[
        ("size", offset_of!(Shape, size), 0),
        ("pins", offset_of!(Shape, pins), 8),
        ("terms", offset_of!(Shape, terms), 32),
        ("inputs", offset_of!(Shape, inputs), 56),
        ("watches", offset_of!(Shape, watches), 80),
        ("functions", offset_of!(Shape, functions), 104),
        ("reactions", offset_of!(Shape, reactions), 128),
    ]);
    check("Pin", (size_of::<Pin>(), 136), (align_of::<Pin>(), 8), &[
        ("name", offset_of!(Pin, name), 0),
        ("origin", offset_of!(Pin, origin), 16),
        ("kind", offset_of!(Pin, kind), 32),
        ("role", offset_of!(Pin, role), 36),
        ("global", offset_of!(Pin, global), 40),
        ("flags", offset_of!(Pin, flags), 44),
        ("layout", offset_of!(Pin, layout), 48),
        ("unit", offset_of!(Pin, unit), 80),
        ("init", offset_of!(Pin, init), 96),
        ("check", offset_of!(Pin, check), 112),
        ("abstol", offset_of!(Pin, abstol), 120),
    ]);
    check("Layout", (size_of::<Layout>(), 32), (align_of::<Layout>(), 8), &[
        ("size", offset_of!(Layout, size), 0),
        ("align", offset_of!(Layout, align), 4),
        ("fields", offset_of!(Layout, fields), 8),
    ]);
    check("Field", (size_of::<Field>(), 40), (align_of::<Field>(), 8), &[
        ("name", offset_of!(Field, name), 0),
        ("leaf", offset_of!(Field, leaf), 16),
        ("offset", offset_of!(Field, offset), 20),
        ("count", offset_of!(Field, count), 24),
        ("flags", offset_of!(Field, flags), 28),
        ("known", offset_of!(Field, known), 32),
    ]);
    check("Term", (size_of::<Term>(), 56), (align_of::<Term>(), 8), &[
        ("name", offset_of!(Term, name), 0),
        ("origin", offset_of!(Term, origin), 16),
        ("op", offset_of!(Term, op), 32),
        ("plus", offset_of!(Term, plus), 36),
        ("minus", offset_of!(Term, minus), 40),
        ("abstol", offset_of!(Term, abstol), 44),
        ("max", offset_of!(Term, max), 48),
    ]);
    check("Input", (size_of::<Input>(), 40), (align_of::<Input>(), 8), &[
        ("name", offset_of!(Input, name), 0),
        ("origin", offset_of!(Input, origin), 16),
        ("kind", offset_of!(Input, kind), 32),
    ]);
    check("Watch", (size_of::<Watch>(), 88), (align_of::<Watch>(), 8), &[
        ("name", offset_of!(Watch, name), 0),
        ("origin", offset_of!(Watch, origin), 16),
        ("kind", offset_of!(Watch, kind), 32),
        ("layout", offset_of!(Watch, layout), 40),
        ("event", offset_of!(Watch, event), 72),
        ("ttol", offset_of!(Watch, ttol), 76),
        ("etol", offset_of!(Watch, etol), 80),
    ]);
    check("Cell", (size_of::<Cell>(), 12), (align_of::<Cell>(), 4), &[
        ("kind", offset_of!(Cell, kind), 0),
        ("index", offset_of!(Cell, index), 4),
        ("part", offset_of!(Cell, part), 8),
    ]);
    check("Partial", (size_of::<Partial>(), 16), (align_of::<Partial>(), 4), &[
        ("write", offset_of!(Partial, write), 0),
        ("to", offset_of!(Partial, to), 4),
        ("read", offset_of!(Partial, read), 8),
        ("at", offset_of!(Partial, at), 12),
    ]);
    check("Function", (size_of::<Function>(), 112), (align_of::<Function>(), 8), &[
        ("name", offset_of!(Function, name), 0),
        ("origin", offset_of!(Function, origin), 16),
        ("flags", offset_of!(Function, flags), 32),
        ("reads", offset_of!(Function, reads), 40),
        ("writes", offset_of!(Function, writes), 64),
        ("partials", offset_of!(Function, partials), 88),
    ]);
    check("Reaction", (size_of::<Reaction>(), 112), (align_of::<Reaction>(), 8), &[
        ("name", offset_of!(Reaction, name), 0),
        ("origin", offset_of!(Reaction, origin), 16),
        ("flags", offset_of!(Reaction, flags), 32),
        ("on", offset_of!(Reaction, on), 40),
        ("reads", offset_of!(Reaction, reads), 64),
        ("writes", offset_of!(Reaction, writes), 88),
    ]);
    check("Statement", (size_of::<Statement>(), 24), (align_of::<Statement>(), 8), &[
        ("pin", offset_of!(Statement, pin), 0),
        ("offset", offset_of!(Statement, offset), 4),
        ("len", offset_of!(Statement, len), 8),
        ("flags", offset_of!(Statement, flags), 12),
        ("min_after", offset_of!(Statement, min_after), 16),
    ]);
    check("Eval", (size_of::<Eval>(), 48), (align_of::<Eval>(), 8), &[
        ("size", offset_of!(Eval, size), 0),
        ("input", offset_of!(Eval, input), 8),
        ("output", offset_of!(Eval, output), 16),
        ("seed", offset_of!(Eval, seed), 24),
        ("jac", offset_of!(Eval, jac), 32),
        ("note", offset_of!(Eval, note), 40),
    ]);
    check("React", (size_of::<React>(), 40), (align_of::<React>(), 8), &[
        ("size", offset_of!(React, size), 0),
        ("input", offset_of!(React, input), 8),
        ("host", offset_of!(React, host), 16),
        ("host_data", offset_of!(React, host_data), 24),
        ("note", offset_of!(React, note), 32),
    ]);
    check("Host", (size_of::<Host>(), 16), (align_of::<Host>(), 8), &[
        ("size", offset_of!(Host, size), 0),
        ("commit", offset_of!(Host, commit), 8),
    ]);
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

#[test]
fn every_constant_has_its_pinned_value() {
    let groups: &[(&str, &[(u32, u32)])] = &[
        ("kind", &[(kind::CONTINUOUS, 1), (kind::DISCRETE, 2), (kind::EVENT, 3)]),
        ("role", &[(role::PORT, 1), (role::PARAM, 2), (role::GLOBAL, 3), (role::PRIVATE, 4)]),
        ("global", &[
            (global::TEMPERATURE, 1), (global::RESOLUTION, 2), (global::DC, 3), (global::TRAN, 4),
            (global::AC, 5), (global::NOISE, 6), (global::START, 7), (global::SETTLE, 8),
            (global::NAMED, 9),
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
        ("field_flag", &[(field_flag::HAS_KNOWN, 1)]),
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
        assert_group(name, &values);
    }
    assert_eq!(GROUND, u32::MAX);
    assert_eq!(DEFAULT, u32::MAX - 1);
    assert_ne!(GROUND, DEFAULT);
    assert_eq!(MAJOR, 1);
    assert_eq!(MINOR, 0);
    assert_eq!(SYMBOL, "piperine_abi_1");
    assert_eq!(NOTE_TEXT, 248);
}

/// A `Pin` as a newer producer compiled it: 16 more bytes after the fields
/// this version knows.
#[repr(C)]
struct WiderPin {
    pin: Pin,
    appended: [u8; 16],
}

fn pin_named(name: &'static str) -> Pin {
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
    }
}

#[test]
fn list_reads_by_the_producers_stride() {
    let wide = [
        WiderPin { pin: pin_named("p"), appended: [0xAA; 16] },
        WiderPin { pin: pin_named("n"), appended: [0xBB; 16] },
        WiderPin { pin: pin_named("f"), appended: [0xCC; 16] },
    ];
    assert_eq!(size_of::<WiderPin>(), size_of::<Pin>() + 16);
    let list: List<Pin> = List {
        ptr: wide.as_ptr().cast::<Pin>(),
        len: wide.len(),
        stride: size_of::<Pin>() + 16,
    };
    let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["p", "n", "f"]);
    assert_eq!(list.get(1).map(|p| p.name.as_str()), Some("n"));
    assert_eq!(list.get(2).map(|p| p.kind), Some(kind::CONTINUOUS));
    assert!(list.get(3).is_none(), "an index past the end is None");
}

#[test]
fn empty_list_yields_nothing() {
    let list: List<Pin> = List::empty();
    assert!(!list.ptr.is_null(), "an empty list's pointer is non-null");
    assert_eq!(list.ptr as usize % align_of::<Pin>(), 0, "and aligned");
    assert_eq!(list.len, 0);
    assert!(list.get(0).is_none());
    assert_eq!(list.iter().count(), 0);

    let from_empty: List<Cell> = List::from_slice(&[]);
    assert!(!from_empty.ptr.is_null());
    assert_eq!(from_empty.iter().count(), 0);
    assert_eq!(Str::EMPTY.as_str(), "");
    assert!(Bytes::EMPTY.as_slice().is_empty());
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

#[test]
fn crate_declares_no_dependency() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let manifest = std::fs::read_to_string(path).expect("read the crate manifest");
    let deps = section(&manifest, "dependencies").expect("the manifest has a [dependencies] section");
    let entries: Vec<&str> = deps
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert!(entries.is_empty(), "piperine-abi declares dependencies: {entries:?}");
    for other in ["dev-dependencies", "build-dependencies"] {
        assert!(section(&manifest, other).is_none(), "piperine-abi has a [{other}] section");
    }
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
    let short: List<[u8; 8]> = List { ptr: buf.as_ptr(), len: 4, stride: 4 };
    assert!(!short.is_readable());
    assert!(short.get(0).is_none() && short.get(3).is_none(), "no element is read");
    assert_eq!(short.iter().count(), 0);
    // A stride that breaks the element's alignment is refused too.
    let pins = [pin_named("p"), pin_named("n")];
    let odd: List<Pin> = List { ptr: pins.as_ptr(), len: 1, stride: size_of::<Pin>() + 1 };
    assert!(!odd.is_readable() && odd.get(0).is_none());
    // A misaligned pointer is refused.
    let skewed: List<Pin> = List { ptr: pins.as_ptr().cast::<u8>().wrapping_add(4).cast::<Pin>(), len: 1, stride: size_of::<Pin>() };
    assert!(!skewed.is_readable() && skewed.get(0).is_none());
    // The producer's own stride, and a larger one, are read.
    assert!(List::from_slice(&pins).is_readable());
    assert_eq!(List::from_slice(&pins).get(1).map(|p| p.name.as_str()), Some("n"));
    assert!(List::<Pin>::empty().is_readable());
}
