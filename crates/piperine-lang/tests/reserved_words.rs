//! DVC-06 — the six verification keywords (`constraint`, `require`, `target`,
//! `tol`, `cover`, `global`) are reserved at the parser level: each one used
//! as an identifier — in a declaration name or in an expression — is a parse
//! error naming the word.

use piperine_lang::parse_str;

const RESERVED: [&str; 6] = ["constraint", "require", "target", "tol", "cover", "global"];

fn assert_reserved(src: &str, word: &str) {
    let err = match parse_str(src) {
        Ok(_) => panic!("`{word}` as an identifier must be a parse error: {src}"),
        Err(err) => err,
    };
    let msg = err.to_string();
    assert!(
        msg.contains(word) && msg.contains("reserved"),
        "error must name `{word}` as reserved, got: {msg}"
    );
}

#[test]
fn reserved_word_rejected_as_declaration_name() {
    for word in RESERVED {
        let src = format!(
            "mod M ( inout p : Electrical, inout n : Electrical ) {{ param {word} : Real = 1.0; }}"
        );
        assert_reserved(&src, word);
    }
}

#[test]
fn reserved_word_rejected_in_expression() {
    for word in RESERVED {
        let src = format!(
            "mod M ( inout p : Electrical, inout n : Electrical ) {{ param x : Real = 1.0; }}
             analog M {{ I(p, n) <+ {word} * V(p, n); }}"
        );
        assert_reserved(&src, word);
    }
}
