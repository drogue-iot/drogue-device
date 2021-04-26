#![allow(dead_code)]
#![deny(unused_must_use)]

use std::{env, fs, path::PathBuf};

use xshell::cmd;

fn main() -> Result<(), anyhow::Error> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let args = args.iter().map(|s| &**s).collect::<Vec<_>>();

    match &args[..] {
        ["ci"] => test_ci(),
        _ => {
            println!("USAGE cargo xtask [ci]");
            Ok(())
        }
    }
}

fn test_ci() -> Result<(), anyhow::Error> {
    let _e = xshell::pushenv("CI", "true");
    test_workspace()?;
    let mut examples_dir = root_dir();
    examples_dir.push("examples");
    test_examples(examples_dir)?;
    Ok(())
}

fn test_workspace() -> Result<(), anyhow::Error> {
    let _p = xshell::pushd(root_dir())?;
    cmd!("cargo test --all --features std").run()?;
    Ok(())
}

fn test_examples(current_dir: PathBuf) -> Result<(), anyhow::Error> {
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.ends_with("Cargo.toml") {
            test_example(path.clone())?;
        }

        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            test_examples(path)?;
        }
    }

    Ok(())
}

fn test_example(project_file: PathBuf) -> Result<(), anyhow::Error> {
    println!("Building example {}", project_file.to_str().unwrap_or(""));
    let _p = xshell::pushd(project_file.parent().unwrap())?;
    cmd!("cargo build --release").run()?;
    Ok(())
}

fn root_dir() -> PathBuf {
    let mut xtask_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    xtask_dir.pop();
    xtask_dir
}
