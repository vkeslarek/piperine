//! Stamp the host target triple for the release resolver
//! (`release::ReleaseRef::host_triple`) — the exact `TARGET` cargo is
//! building for, matched against release asset names at runtime.

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    println!("cargo:rustc-env=PIPERINE_HOST_TRIPLE={target}");
    println!("cargo:rerun-if-changed=build.rs");
}
