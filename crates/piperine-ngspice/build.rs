use std::env;
use std::path::PathBuf;

fn main() {
    // Link against libngspice shared library
    println!("cargo:rustc-link-lib=ngspice");

    // Tell cargo to rerun if the header changes
    println!("cargo:rerun-if-changed=../../header/wrapper.h");

    let bindings = bindgen::Builder::default()
        .header("../../header/wrapper.h")
        .clang_arg("-I../../header")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_function("ngSpice_Init")
        .allowlist_function("ngSpice_Init_Sync")
        .allowlist_function("ngSpice_Command")
        .allowlist_function("ngGet_Vec_Info")
        .allowlist_function("ngSpice_Circ")
        .allowlist_function("ngSpice_CurPlot")
        .allowlist_function("ngSpice_AllPlots")
        .allowlist_function("ngSpice_AllVecs")
        .allowlist_function("ngSpice_running")
        .allowlist_function("ngSpice_SetBkpt")
        .allowlist_type("vector_info")
        .allowlist_type("vecvalues")
        .allowlist_type("vecvaluesall")
        .allowlist_type("vecinfo")
        .allowlist_type("vecinfoall")
        .allowlist_type("ngcomplex")
        .generate()
        .expect("Unable to generate bindings for ngspice");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("ngspice_bindings.rs"))
        .expect("Couldn't write bindings!");
}
