#![allow(dead_code)]
#![deny(unused_must_use)]

use std::io::Write;
use std::{env, fs, path::PathBuf};

use xshell::cmd;

fn main() -> Result<(), anyhow::Error> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let args = args.iter().map(|s| &**s).collect::<Vec<_>>();

    match &args[..] {
        ["ci"] => ci(false),
        ["ci_batch"] => ci(true),
        ["check_device"] => check_device(),
        ["test_device"] => test_device(),
        ["test_examples"] => test_examples(),
        ["check", example] => check(&[example]),
        ["build", example] => build(&[example], false),
        ["fmt"] => fmt(),
        ["update"] => update(),
        ["docs"] => docs(),
        ["matrix"] => matrix(),
        ["clean"] => clean(root_dir()),
        _ => {
            println!("USAGE:");
            println!("\tcargo xtask ci");
            println!("\tcargo xtask check examples/nrf52/microbit");
            println!("\tcargo xtask build examples/nrf52/microbit");
            println!("\tcargo xtask update");
            println!("\tcargo xtask docs");
            println!("\tcargo xtask clean");
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
    "examples/wasm/browser",
    "examples/std",
    "docs/modules/ROOT/examples/basic",
    //"apps/ble",
];

fn ci(batch: bool) -> Result<(), anyhow::Error> {
    let _e = xshell::pushenv("CI", "true");

    check_device()?;
    check(WORKSPACES)?;
    build(WORKSPACES, batch)?;
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

fn generate_batch_command(
    workspaces: &[&str],
    cmds: Vec<&str>,
) -> Result<xshell::Cmd, anyhow::Error> {
    let mut crate_target: Vec<(String, Option<String>)> = Vec::new();
    do_crates(workspaces, &mut |project_dir| {
        let config_file = project_dir.join(".cargo").join("config.toml");
        if config_file.exists() {
            let contents = fs::read_to_string(&config_file).expect("error reading file");
            let t = contents.parse::<toml::Value>().unwrap();
            if let Some(build) = t.get("build") {
                let target = if let Some(toml::Value::String(target)) = build.get("target") {
                    Some(target.clone())
                } else {
                    None
                };
                crate_target.push((
                    project_dir.join("Cargo.toml").to_str().unwrap().to_string(),
                    target,
                ));
            }
        }
        Ok(())
    })?;
    let mut c = xshell::Cmd::new("cargo");
    c = c.arg("batch");
    for (ws, target) in crate_target.iter() {
        c = c
            .arg("---")
            .args(cmds.clone())
            .arg("--manifest-path")
            .arg(ws);
        if let Some(target) = target {
            c = c.arg("--target").arg(target);
        }
    }
    Ok(c)
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

fn build(workspaces: &[&str], batch: bool) -> Result<(), anyhow::Error> {
    let _e = xshell::pushenv("RUSTFLAGS", "-Dwarnings");
    if batch {
        generate_batch_command(workspaces, vec!["build", "--release"])?.run()?;
    } else {
        do_crates(workspaces, &mut build_crate)?;
    }
    Ok(())
}

fn test_examples() -> Result<(), anyhow::Error> {
    let api = env::var_os("DROGUE_CLOUD_API").unwrap();
    let token = env::var_os("DROGUE_CLOUD_ACCESS_TOKEN").unwrap();
    let mut tests = root_dir();
    tests.push("examples");
    tests.push("tests");
    let _p = xshell::pushd(&tests)?;
    let _e = xshell::pushenv("DROGUE_CLOUD_API", api);
    let _e = xshell::pushenv("DROGUE_CLOUD_ACCESS_TOKEN", token);
    cmd!("cargo test").run()?;
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

fn clean(path: PathBuf) -> Result<(), anyhow::Error> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if path.ends_with("target") {
                println!("Removing {}", path.display());
                fs::remove_dir_all(path)?;
            } else {
                clean(path)?;
            }
        }
    }
    Ok(())
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
                        if s == "ignore" {
                            break;
                        }
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

        if path.ends_with("tests") {
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
