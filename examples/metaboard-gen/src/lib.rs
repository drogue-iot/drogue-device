use std::{env, fs::File, io::Write, path::PathBuf};

pub fn gen_memory(board_name: &str, in_memory_x: &[u8]) {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    std::fs::create_dir_all(format!("{}/src/boards/{}", out_dir.display(), board_name));
    let out_memory_x = format!("{}/src/boards/{}/memory.x", out_dir.display(), board_name);
    File::create(&out_memory_x)
        .unwrap()
        .write_all(in_memory_x)
        .unwrap();

    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rustc-link-search={}/src/boards/{}",
        out_dir.display(),
        board_name
    );
}
