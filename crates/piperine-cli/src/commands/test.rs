pub fn execute(dir: String) {
    crate::commands::build::execute(None);
    println!("Running tests in: {}", dir);
    // TODO: test runner
}
