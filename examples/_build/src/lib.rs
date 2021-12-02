use std::env;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

pub fn copy_file(filename: &str) {
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let manifest_dir = &PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let file_path = manifest_dir.join(filename);
    if Path::new(&file_path).exists() {
        fs::copy(&file_path, out.join(filename)).expect("error copying file");
        println!("cargo:rerun-if-changed={}", file_path.display());
    } else {
        let _ = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out.join(filename));
    }
}
