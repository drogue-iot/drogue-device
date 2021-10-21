//! This build script copies the `memory.x` file from the crate root into
//! a directory where the linker can always find it at build time.
//! For many projects this is optional, as the linker always searches the
//! project root directory -- wherever `Cargo.toml` is. However, if you
//! are using a workspace or have a more complicated build setup, this
//! build script becomes required. Additionally, by requesting that
//! Cargo re-run the build script whenever `memory.x` is changed,
//! updating `memory.x` ensures a rebuild of the application with the
//! new memory settings.

use std::env;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

fn copy_config(out: &PathBuf, manifest_dir: &PathBuf, file: &str) {
    let file_path = manifest_dir.join(file);
    if Path::new(&file_path).exists() {
        fs::copy(&file, out.join(file)).expect("error copying file");
        println!("cargo:rerun-if-changed={}", file_path.display());
    } else {
        if env::var_os("CI").is_none() {
            panic!("Unable to locate config file {}.", file_path.display());
        } else {
            println!("Skipping missing configuration file when running in CI");
            let _ = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(out.join(file));
        }
    }
}

fn main() {
    let manifest_dir = &PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());

    // Copy credentials
    fs::create_dir_all(out.join("config")).expect("error creating output directory for config");
    copy_config(&out, &manifest_dir, "memory.x");
    println!("cargo:rustc-link-search={}", out.display());
}
