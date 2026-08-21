//! LRM-COVERS: 9.5
//! §9.5 file I/O (LRMC-C7-09, T46): `FileTable` (piperine-solver) +
//! `parse_scanf`/`format_display` (piperine-lang) round-trip real files,
//! and every descriptor error is typed and loud (never a silent zero
//! descriptor or a swallowed I/O failure).
//!
//! Placement/dev-dependency note: mirrors T45's own
//! `system_task_migration_completeness.rs` — `piperine-lang`'s
//! dev-dependency on `piperine-solver` (added there) is reused here
//! rather than duplicated.

use piperine_lang::eval::tasks::{format_display, parse_scanf};
use piperine_lang::eval::Value;
use piperine_solver::prelude::{FileError, FileTable};

fn tmp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("piperine-t46-{}-{name}", std::process::id()))
}

#[test]
fn fdisplay_then_fscanf_round_trips_typed_values() {
    let path = tmp_path("roundtrip.txt");
    let _ = std::fs::remove_file(&path);

    let mut table = FileTable::new();
    let fd = table.open(path.to_str().unwrap()).expect("$fopen");

    // $fdisplay(fd, "%d %s %h", 42, "ok", 255);
    let line = format_display("%d %s %h\n", &[Value::Int(42), Value::Str("ok".into()), Value::Int(255)], "top")
        .expect("format_display");
    table.write(fd, &line).expect("$fwrite/$fdisplay");
    table.close(fd).expect("$fclose");

    // Reopen (LRM's own "$fopen re-reads what was written" idiom) and
    // $fscanf it back.
    let fd2 = table.open(path.to_str().unwrap()).expect("$fopen (read)");
    let contents = table.read_to_string(fd2).expect("$fscanf's data source");
    let values = parse_scanf("%d %s %h", contents.trim_end()).expect("$fscanf");
    assert_eq!(values, vec![Value::Int(42), Value::Str("ok".into()), Value::Int(255)]);
    table.close(fd2).expect("$fclose");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn fseek_ftell_rewind_and_feof_track_position() {
    let path = tmp_path("seek.txt");
    let _ = std::fs::remove_file(&path);
    let mut table = FileTable::new();
    let fd = table.open(path.to_str().unwrap()).expect("$fopen");
    table.write(fd, "abcdef").expect("$fwrite");
    table.flush(fd).expect("$fflush");

    table.rewind(fd).expect("$rewind");
    assert_eq!(table.tell(fd).unwrap(), 0);
    assert!(!table.eof(fd).unwrap(), "not at EOF right after $rewind");

    assert_eq!(table.getc(fd).unwrap(), Some(b'a'), "$fgetc reads and consumes");
    assert_eq!(table.tell(fd).unwrap(), 1);

    table.seek(fd, 5).expect("$fseek");
    assert_eq!(table.getc(fd).unwrap(), Some(b'f'));
    assert_eq!(table.getc(fd).unwrap(), None, "$fgetc at EOF returns None");
    assert!(table.eof(fd).unwrap(), "$feof true once every byte is consumed");

    table.close(fd).expect("$fclose");
    let _ = std::fs::remove_file(&path);
}

// ── typed, loud descriptor errors ────────────────────────────────────────

#[test]
fn a_closed_or_unknown_descriptor_fails_loud_on_every_operation() {
    let mut table = FileTable::new();
    let path = tmp_path("close.txt");
    let _ = std::fs::remove_file(&path);
    let fd = table.open(path.to_str().unwrap()).expect("$fopen");
    table.close(fd).expect("$fclose");

    for result in [
        table.close(fd).map(|_| ()),
        table.write(fd, "x").map(|_| ()),
        table.tell(fd).map(|_| ()),
        table.getc(fd).map(|_| ()),
        table.flush(fd).map(|_| ()),
    ] {
        assert!(matches!(result, Err(FileError::UnknownDescriptor(_))), "got: {result:?}");
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn opening_an_unwritable_path_fails_loud() {
    let mut table = FileTable::new();
    // A path inside a nonexistent directory always fails to open/create.
    let bogus = tmp_path("does-not-exist-dir").join("also-missing").join("f.txt");
    let err = table.open(bogus.to_str().unwrap()).expect_err("opening under a missing directory must fail loud");
    assert!(matches!(err, FileError::OpenFailed { .. }), "got: {err:?}");
}

// ── parse_scanf's own specifier coverage ─────────────────────────────────

#[test]
fn parse_scanf_handles_every_radix_specifier() {
    let values = parse_scanf("%d %h %o %b %f %s %c", "42 ff 17 101 3.5 word Q").unwrap();
    assert_eq!(
        values,
        vec![
            Value::Int(42),
            Value::Int(0xff),
            Value::Int(0o17),
            Value::Int(0b101),
            Value::Real(3.5),
            Value::Str("word".into()),
            Value::Int('Q' as i64),
        ]
    );
}

#[test]
fn parse_scanf_matches_literal_characters_and_skips_whitespace_runs() {
    let values = parse_scanf("x=%d, y=%d", "x=1,   y=2").unwrap();
    assert_eq!(values, vec![Value::Int(1), Value::Int(2)]);
}

#[test]
fn parse_scanf_fails_loud_on_a_literal_mismatch() {
    let err = parse_scanf("x=%d", "y=1").unwrap_err();
    assert!(format!("{err}").contains("literal"), "{err}");
}

#[test]
fn parse_scanf_fails_loud_on_an_invalid_token_for_its_radix() {
    // No leading digit at all: the tokenizer finds nothing to consume.
    let err = parse_scanf("%d", "not_a_number").unwrap_err();
    assert!(format!("{err}").contains("found no token"), "{err}");

    // A token that overflows i64 is a real "wrong for this radix" case.
    let err2 = parse_scanf("%d", "99999999999999999999").unwrap_err();
    assert!(format!("{err2}").contains("not valid"), "{err2}");
}
