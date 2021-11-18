use config;
use std::collections::HashMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

pub fn copy_config(out: &PathBuf, manifest_dir: &PathBuf, file: &str) {
    let file_path = manifest_dir.join(file);
    if Path::new(&file_path).exists() {
        fs::copy(&file, out.join(file)).expect("error copying file");
        println!("cargo:rerun-if-changed={}", file_path.display());
    } else {
        let _ = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out.join(file));
    }
}

pub fn write_config(out: &PathBuf, key: &str, value: &str) {
    fs::write(out.join(key), value).expect("Unable to write config file");
}

pub fn configure(out: &PathBuf, keys: &[&str]) {
    let cfg = PathBuf::from(env::var_os("HOME").unwrap()).join(".drogue/config.toml");
    println!("cargo:rerun-if-changed={}", cfg.display());
    let mut settings = config::Config::default();
    let filename = cfg.to_str().unwrap();
    settings
        .merge(config::File::with_name(filename).required(false))
        .unwrap();
    let map = settings.try_into::<HashMap<String, String>>().unwrap();
    for key in keys {
        match map.get(key as &str) {
            Some(v) => write_config(out, key, v),
            None => write_config(out, key, ""),
        }
    }
}
