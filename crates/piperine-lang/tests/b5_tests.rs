use piperine_lang::parse_and_elaborate;
use piperine_lang::pom::ElabError;

#[test]
fn test_b5_implicit_widening_allowed() {
    let src = "
    mod B5_Allowed() {}
    digital B5_Allowed {
        var a: Boolean = false;
        var b: Quad = 0q0;
        b = a;
    }
    ";
    let prog = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).unwrap();
    // This should pass without error
}

#[test]
fn test_b5_implicit_cast_rejected() {
    let src = "
    mod B5_Rejected() {}
    digital B5_Rejected {
        var a: Integer = 1;
        var b: Real = 1.0;
        b = a;
    }
    ";
    let res = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy());
    assert!(res.is_err(), "Expected error for implicit cast from Integer to Real");
    if let Err(msg) = res {
        assert!(msg.to_string().contains("implicit cast from Integer to Real not allowed"));
    } else {
        panic!("Expected error string");
    }
}

#[test]
fn test_b5_explicit_cast_allowed() {
    let src = "
    mod B5_Explicit() {}
    digital B5_Explicit {
        var a: Integer = 1;
        var b: Real = 1.0;
        b = real(a);
    }
    ";
    let prog = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).unwrap();
}
