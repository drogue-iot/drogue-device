use std::{env, fs::File, io::Write, path::PathBuf};
extern crate drogue_metaboard_gen;
use drogue_metaboard_gen::*;

fn main() {
    let crate_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let board_name = env::vars_os()
        .map(|(a, _)| a.to_string_lossy().to_string())
        .find(|x| x.starts_with("CARGO_FEATURE_BOARD+"))
        .expect("No board Cargo feature enabled")
        .strip_prefix("CARGO_FEATURE_BOARD+")
        .unwrap()
        .to_ascii_lowercase()
        .replace('_', "-");

    let data_dir = PathBuf::from(format!("src/boards/{}", board_name));
    let in_memory_x = std::fs::read(data_dir.join("memory.x")).unwrap();

    gen_memory(&board_name, &in_memory_x[..]);
}
