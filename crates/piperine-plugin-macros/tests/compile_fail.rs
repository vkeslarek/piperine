//! Compile-fail gates (PLG-05/24): a malformed `#[pip::device]` use is a
//! compile error, never a silent mis-registration.

#[test]
fn malformed_device_usage_fails_to_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/device_missing_type.rs");
    t.compile_fail("tests/ui/device_type_not_a_string.rs");
    t.compile_fail("tests/ui/device_on_a_function.rs");
}

/// PLG-06/11: an unknown hook phase name is a compile error (the catalog is
/// frozen at five), as is a malformed `#[pip::script]`/`#[pip::hook]` use.
#[test]
fn malformed_script_and_hook_usage_fails_to_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/hook_unknown_phase.rs");
    t.compile_fail("tests/ui/hook_phase_as_string.rs");
    t.compile_fail("tests/ui/hook_missing_phase.rs");
    t.compile_fail("tests/ui/script_missing_name.rs");
    t.compile_fail("tests/ui/script_on_a_struct.rs");
}
