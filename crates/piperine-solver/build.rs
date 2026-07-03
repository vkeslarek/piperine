use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    // Only download if we're on Linux x86_64 for now, as that's what we have a link for
    if target_os == "linux" && target_arch == "x86_64" {
        let openvaf_bin = out_dir.join("openvaf");
        
        if !openvaf_bin.exists() {
            let tarball = out_dir.join("openvaf.tar.gz");
            
            // Download the OpenVAF-Reloaded executable
            let url = "https://fides.fe.uni-lj.si/openvaf/download/openvaf-reloaded-20260616-linux_x64.tar.gz";
            let status = Command::new("curl")
                .arg("-sL")
                .arg(url)
                .arg("-o")
                .arg(&tarball)
                .status()
                .expect("Failed to download OpenVAF");
            
            if status.success() {
                // Extract
                Command::new("tar")
                    .arg("-xzf")
                    .arg(&tarball)
                    .arg("-C")
                    .arg(&out_dir)
                    .status()
                    .expect("Failed to extract OpenVAF");
                    
                // The extracted binary might be named `openvaf` or `openvaf-r` or be in a subfolder.
                // In OpenVAF-Reloaded tarballs it's usually just an `openvaf` or `openvaf-reloaded` binary.
                // Let's find it.
                let mut found_bin = None;
                for entry in fs::read_dir(&out_dir).unwrap() {
                    let entry = entry.unwrap();
                    let name = entry.file_name().into_string().unwrap();
                    if name.contains("openvaf") && !name.ends_with(".tar.gz") {
                        found_bin = Some(entry.path());
                        break;
                    }
                }
                
                if let Some(bin) = found_bin
                    && bin != openvaf_bin {
                        fs::rename(bin, &openvaf_bin).unwrap();
                    }
                
                let _ = fs::remove_file(tarball);
            }
        }
        
        println!("cargo:rustc-env=OPENVAF_BIN={}", openvaf_bin.display());
    } else {
        // Fallback to system openvaf
        println!("cargo:rustc-env=OPENVAF_BIN=openvaf");
    }
    
    // Ensure symbols like `pow` from libm are exported to loaded OSDI plugins
    println!("cargo:rustc-link-arg-tests=-rdynamic");
}
