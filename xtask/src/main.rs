#![allow(dead_code)]
#![deny(unused_must_use)]

use std::io::Write;
use std::{env, fs, path::PathBuf};

use xshell::cmd;

fn main() -> Result<(), anyhow::Error> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let args = args.iter().map(|s| &**s).collect::<Vec<_>>();

    match &args[..] {
        ["ci"] => ci(),
        ["check_device"] => check_device(),
        ["test_device"] => test_device(),
        ["check", example] => check(&[example]),
        ["build", example] => build(&[example]),
        ["fmt"] => fmt(),
        ["update"] => update(),
        ["docs"] => docs(),
        ["matrix"] => matrix(),
        _ => {
            println!("USAGE:");
            println!("\tcargo xtask ci");
            println!("\tcargo xtask check examples/nrf52/microbit");
            println!("\tcargo xtask build examples/nrf52/microbit");
            println!("\tcargo xtask update");
            println!("\tcargo xtask docs");
            Ok(())
        }
    }
}

static WORKSPACES: &[&str] = &[
    "examples/nrf52/microbit",
    "examples/stm32l0/lora-discovery",
    "examples/stm32l1/rak811",
    "examples/stm32l4/iot01a-wifi",
    "examples/rp/pico",
    "examples/stm32wl/nucleo-wl55",
    "examples/stm32h7/nucleo-h743zi",
    "examples/stm32u5/iot02a",
    "examples/bsp/iot02a",
    "examples/bsp/nucleo-h743zi",
    //"examples/wasm/browser",
    "examples/std",
    //"apps/ble",
];

fn ci() -> Result<(), anyhow::Error> {
    let _e = xshell::pushenv("CI", "true");

    check_device()?;
    check(WORKSPACES)?;
    build(WORKSPACES)?;
    docs()?;
    Ok(())
}

fn matrix() -> Result<(), anyhow::Error> {
    let mut items = Vec::new();
    for workspace in WORKSPACES {
        items.push(format!("\"{}\"", workspace));
    }
    println!("{}", items.join(","));
    Ok(())
}

fn update() -> Result<(), anyhow::Error> {
    let _p = xshell::pushd(root_dir())?;
    cmd!("cargo update").run()?;
    let mut examples_dir = root_dir();
    examples_dir.push("examples");
    do_examples(examples_dir, &mut update_crate)?;
    Ok(())
}

fn fmt() -> Result<(), anyhow::Error> {
    let _p = xshell::pushd(root_dir())?;
    cmd!("cargo fmt").run()?;
    do_crates(WORKSPACES, &mut fmt_crate)?;
    Ok(())
}

fn check(workspaces: &[&str]) -> Result<(), anyhow::Error> {
    let _e = xshell::pushenv("RUSTFLAGS", "-Dwarnings");
    do_crates(workspaces, &mut check_crate)?;
    Ok(())
}

fn check_device() -> Result<(), anyhow::Error> {
    let mut device = root_dir();
    device.push("device");
    let _p = xshell::pushd(&device)?;
    cmd!("cargo fmt --check").run()?;
    cmd!("cargo check --all --features 'std wifi+esp8266 wifi+eswifi lora lora+rak811 tcp+smoltcp tls'").run()?;
    Ok(())
}

fn build(workspaces: &[&str]) -> Result<(), anyhow::Error> {
    let _e = xshell::pushenv("RUSTFLAGS", "-Dwarnings");
    do_crates(workspaces, &mut build_crate)?;
    Ok(())
}

fn test_device() -> Result<(), anyhow::Error> {
    let mut device = root_dir();
    device.push("device");
    let _p = xshell::pushd(&device)?;
    cmd!("cargo test --all --features 'std wifi+esp8266 wifi+eswifi lora lora+rak811 tcp+smoltcp tls'").run()?;
    Ok(())
}

fn do_crates<F: FnMut(PathBuf) -> Result<(), anyhow::Error>>(
    workspaces: &[&str],
    f: &mut F,
) -> Result<(), anyhow::Error> {
    for workspace in workspaces {
        let mut crate_dir = root_dir();
        crate_dir.push(workspace);
        f(crate_dir)?;
    }

    Ok(())
}

fn check_crate(project_file: PathBuf) -> Result<(), anyhow::Error> {
    println!("Checking {}", project_file.to_str().unwrap_or(""));
    let _p = xshell::pushd(project_file)?;
    cmd!("cargo fmt --check").run()?;
    cmd!("cargo check").run()?;
    Ok(())
}

fn build_crate(project_file: PathBuf) -> Result<(), anyhow::Error> {
    println!("Building {}", project_file.to_str().unwrap_or(""));
    let _p = xshell::pushd(project_file)?;
    cmd!("cargo build --release").run()?;
    Ok(())
}

fn update_crate(project_file: PathBuf) -> Result<(), anyhow::Error> {
    println!("Updating {}", project_file.to_str().unwrap_or(""));
    let _p = xshell::pushd(project_file.parent().unwrap())?;
    cmd!("cargo update").run()?;
    Ok(())
}

fn fmt_crate(project_file: PathBuf) -> Result<(), anyhow::Error> {
    println!("Formatting {}", project_file.to_str().unwrap_or(""));
    let _p = xshell::pushd(project_file.parent().unwrap())?;
    cmd!("cargo fmt").run()?;
    Ok(())
}

fn docs() -> Result<(), anyhow::Error> {
    generate_examples_page()
}

const MAIN_CATEGORIES: [&str; 8] = [
    "basic", "ble", "wifi", "lorawan", "uart", "display", "other", "cloud",
];
fn generate_examples_page() -> Result<(), anyhow::Error> {
    for kw in MAIN_CATEGORIES {
        let output = root_dir()
            .join("docs")
            .join("modules")
            .join("ROOT")
            .join("pages")
            .join(format!("examples_{}.adoc", kw));
        //println!("Output file: {:?}", output);

        let mut fh = std::fs::File::create(output).expect("unable to open file");
        let mut examples_dir = root_dir();
        examples_dir.push("examples");
        let mut entries = Vec::new();
        do_examples(examples_dir, &mut |project_file| {
            let contents = fs::read_to_string(&project_file).expect("error reading file");
            let t = contents.parse::<toml::Value>().unwrap();
            let relative = project_file.strip_prefix(root_dir())?.parent();
            let other = vec!["other".into()];
            if t.get("package").is_some() {
                let keywords = t["package"]
                    .get("keywords")
                    .map(|k| k.as_array().unwrap())
                    .unwrap_or(&other);
                let description = t["package"]
                    .get("description")
                    .map(|s| s.as_str().unwrap())
                    .unwrap_or("Awesome example");
                for package_kw in keywords {
                    if let toml::Value::String(s) = package_kw {
                        if s == kw {
                            entries.push(format!(
                            "* link:https://github.com/drogue-iot/drogue-device/tree/main/{}[{}]",
                            relative.unwrap().display(),
                            description
                        ));
                        }
                    }
                }
            }
            //println!("Value: {:?}", t["package"]);
            // println!("Keywords for {:?}: {:?}", relative, keywords,);
            Ok(())
        })?;
        entries.sort();
        for entry in entries.iter() {
            writeln!(fh, "{}", entry).unwrap();
        }
    }
    Ok(())
}

fn do_examples<F: FnMut(PathBuf) -> Result<(), anyhow::Error>>(
    current_dir: PathBuf,
    f: &mut F,
) -> Result<(), anyhow::Error> {
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.ends_with("_build") {
            continue;
        }

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

fn root_dir() -> PathBuf {
    let mut xtask_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    xtask_dir.pop();
    xtask_dir
}
