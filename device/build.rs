use std::{env, fs::File, io::Write, path::PathBuf};

fn main() {
    let board_name = env::vars_os()
        .map(|(a, _)| a.to_string_lossy().to_string())
        .find(|x| x.starts_with("CARGO_FEATURE_BOARD+"))
        .map(|s| {
            s.strip_prefix("CARGO_FEATURE_BOARD+")
                .unwrap()
                .to_ascii_lowercase()
                .replace('_', "-")
        });

    if let Some(board_name) = board_name {
        let data_dir = PathBuf::from(format!("../boards/{}", board_name));
        let in_memory_x = std::fs::read(data_dir.join("memory.x")).unwrap();

        gen_memory(&board_name, &in_memory_x[..]);
    }

    pub fn gen_memory(board_name: &str, in_memory_x: &[u8]) {
        let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

        std::fs::create_dir_all(format!("{}/boards/{}", out_dir.display(), board_name)).unwrap();
        let out_memory_x = format!("{}/boards/{}/memory.x", out_dir.display(), board_name);
        File::create(&out_memory_x)
            .unwrap()
            .write_all(in_memory_x)
            .unwrap();

        println!("cargo:rerun-if-changed=build.rs");
        println!(
            "cargo:rustc-link-search={}/boards/{}",
            out_dir.display(),
            board_name
        );
    }
}
