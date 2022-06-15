use config::{Config, File};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

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
        let mut config = Config::builder();
        let global = PathBuf::from(env::var_os("HOME").unwrap()).join(CONFIG_FILE);
        if global.is_file() {
            config = config.add_source(File::from(global.as_path()));
        }
        let mut path = PathBuf::new();
        for c in PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).components() {
            path.push(c);
            let local = path.join(CONFIG_FILE);
            if local.is_file() && local != global {
                config = config.add_source(File::from(local));
            }
        }

        // Override using environment variable config
        if let Some(cfg) = env::var_os("DROGUE_CONFIG") {
            config = config.add_source(File::from(PathBuf::from(cfg)));
        }
        let config = config.build().unwrap_or(Config::default());
        config.try_deserialize().unwrap_or(HashMap::default())
    };
}

pub fn configure(key: &str) -> &str {
    match CONFIG.get(key) {
        Some(v) => v,
        None => match env::var_os(CI_ENV_VAR) {
            Some(_) => CI_ENV_VAR,
            None => panic!("`{}` missing from ~/{}", key, CONFIG_FILE),
        },
    }
}
