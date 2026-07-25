//! Compile-fail gates (PLG-05/24): a malformed `#[pip::device]` use is a
//! compile error, never a silent mis-registration.

#[test]
fn malformed_device_usage_fails_to_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/device_missing_type.rs");
    t.compile_fail("tests/ui/device_type_not_a_string.rs");
    t.compile_fail("tests/ui/device_on_a_function.rs");
}
