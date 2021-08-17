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

fn copy_config(out: &PathBuf, file: &str) {
    if Path::new(file).exists() {
        fs::copy(file, out.join(file)).expect("error copying file");
        println!("cargo:rerun-if-changed={}", file);
    } else {
        if env::var_os("CI").is_none() {
            panic!("Unable to locate config file {}.", file);
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
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());

    // Copy credentials
    fs::create_dir_all(out.join("config")).expect("error creating output directory for config");
    copy_config(&out, "config/dev_eui.txt");
    copy_config(&out, "config/app_eui.txt");
    copy_config(&out, "config/app_key.txt");

    // By default, Cargo will re-run a build script whenever
    // any file in the project changes. By specifying `memory.x`
    // here, we ensure the build script is only re-run when
    // `memory.x` is changed.
    println!("cargo:rerun-if-changed=memory.x");
}
