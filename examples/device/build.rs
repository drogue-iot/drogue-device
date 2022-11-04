use std::{env, fs::File, io::Write, path::PathBuf};

fn main() {
    select_board();

    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());

    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}

fn select_board() {
    let mut found = None;
    let mut total = 0;
    for env in env::vars() {
        if env.0.starts_with("CARGO_FEATURE_BOARD") {
            total += 1;
            found = env
                .0
                .strip_prefix("CARGO_FEATURE_BOARD+")
                .map(|s| s.to_string());
        }
    }
    if total > 1 {
        panic!("More than one board feature configured, only 1 board can be configured at a time");
    }

    if found.is_none() {
        panic!("No board configured, must configure a board")
    }

    // Board
    let board_name = found.unwrap().to_lowercase();
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());

    // Linker script
    let memory_x = &PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join(format!("{}/memory.x", board_name));
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(&std::fs::read(memory_x).unwrap())
        .unwrap();
}
