use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=../../header/wrapper.h");

    // Tell cargo to look for ngspice in the system
    println!("cargo:rustc-link-lib=ngspice");

    // Try common library paths
    println!("cargo:rustc-link-search=/usr/lib");
    println!("cargo:rustc-link-search=/usr/local/lib");
    println!("cargo:rustc-link-search=/usr/lib/x86_64-linux-gnu");

    let bindings = bindgen::Builder::default()
        .header("../../header/wrapper.h")
        // Allowlist only ngspice functions
        .allowlist_function("ngSpice_.*")
        .allowlist_function("ngGet_.*")
        .allowlist_function("ngCM_.*")
        // Allowlist types
        .allowlist_type("vecvalues.*")
        .allowlist_type("vector_info.*")
        .allowlist_type("vecinfo.*")
        .allowlist_type("vecinfoall.*")
        .allowlist_type("evt_node_info.*")
        // Generate function pointers for callbacks
        .allowlist_var(".*")
        // Derive Debug for types
        .derive_debug(true)
        .derive_default(true)
        // Use core instead of std for no_std compatibility
        .use_core()
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
