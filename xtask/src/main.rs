#![allow(dead_code)]
#![deny(unused_must_use)]

use std::{env, fs, path::PathBuf};

use xshell::cmd;

fn main() -> Result<(), anyhow::Error> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let args = args.iter().map(|s| &**s).collect::<Vec<_>>();

    match &args[..] {
        ["ci"] => test_ci(),
        ["update"] => update(),
        _ => {
            println!("USAGE cargo xtask [ci]");
            Ok(())
        }
    }
}

fn update() -> Result<(), anyhow::Error> {
    let _p = xshell::pushd(root_dir())?;
    cmd!("cargo update").run()?;
    let mut examples_dir = root_dir();
    examples_dir.push("examples");
    do_examples(examples_dir, &update_example)?;
    Ok(())
}

fn test_ci() -> Result<(), anyhow::Error> {
    let _e = xshell::pushenv("CI", "true");
    test_device()?;
    let mut examples_dir = root_dir();
    examples_dir.push("examples");
    do_examples(examples_dir, &test_example)?;
    Ok(())
}

fn test_device() -> Result<(), anyhow::Error> {
    let mut device = root_dir();
    device.push("device");

    let _p = xshell::pushd(&device)?;

    cmd!("cargo test --all --features 'std wifi+esp8266'").run()?;
    Ok(())
}

fn do_examples<F: Fn(PathBuf) -> Result<(), anyhow::Error>>(
    current_dir: PathBuf,
    f: &F,
) -> Result<(), anyhow::Error> {
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.ends_with("Cargo.toml") {
            f(path.clone())?;
        }

        let file_type = entry.file_type()?;
        if file_type.is_dir() && !path.ends_with("target") {
            do_examples(path, f)?;
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

fn update_example(project_file: PathBuf) -> Result<(), anyhow::Error> {
    println!("Updating example {}", project_file.to_str().unwrap_or(""));
    let _p = xshell::pushd(project_file.parent().unwrap())?;
    cmd!("cargo update").run()?;
    Ok(())
}

fn root_dir() -> PathBuf {
    let mut xtask_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    xtask_dir.pop();
    xtask_dir
}
