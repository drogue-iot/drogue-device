use config::{Config, File};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = ".drogue/config.toml";
const CI_ENV_VAR: &str = "CI";

lazy_static! {
    // Configuration entries are pulled from the CONFIG_FILE found
    // beneath $HOME and the project manifest directory. Additionally,
    // all the latter's parents will be searched. Precedence is
    // determined by the order of calls to merge(), i.e. the last
    // merge "wins". The CONFIG_FILE beneath $HOME has the lowest
    // precedence.
    static ref CONFIG: HashMap<String, String> = {
        let mut config = Config::default();
        let global = PathBuf::from(env::var_os("HOME").unwrap()).join(CONFIG_FILE);
        if global.is_file() {
            println!("cargo:rerun-if-changed={}", global.display());
            config.merge(File::from(global.as_path())).unwrap();
        }
        let mut path = PathBuf::new();
        for c in PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).components() {
            path.push(c);
            let local = path.join(CONFIG_FILE);
            if local.is_file() && local != global {
                println!("cargo:rerun-if-changed={}", local.display());
                config.merge(File::from(local)).unwrap();
            }
        }
        config.try_into().unwrap_or(HashMap::default())
    };
}

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

pub fn write_config(key: &str, value: &str) {
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out.join(key), value).expect("Unable to write config file");
}

pub fn configure(key: &str) {
    match CONFIG.get(key) {
        Some(v) => write_config(key, v),
        None => match env::var_os(CI_ENV_VAR) {
            Some(_) => write_config(key, CI_ENV_VAR),
            None => panic!("`{}` missing from ~/{}", key, CONFIG_FILE),
        },
    }
}
