#![allow(dead_code)]
#![deny(unused_must_use)]

use core::str::FromStr;
use std::collections::BTreeMap;
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
        ["fix"] => fix(),
        ["update"] => update(),
        ["docs"] => docs(),
        ["clone", example, target] => clone(example, target),
        ["matrix"] => matrix(),
        ["clean"] => clean(root_dir()),
        _ => {
            println!("USAGE:");
            println!("\tcargo xtask ci");
            println!("\tcargo xtask check examples/nrf52/microbit/jukebox");
            println!("\tcargo xtask build examples/nrf52/microbit/jukebox");
            println!("\tcargo xtask clone examples/nrf52/microbit/jukebox target-folder");
            println!("\tcargo xtask update");
            println!("\tcargo xtask docs");
            println!("\tcargo xtask clean");
            Ok(())
        }
    }
}

static WORKSPACES: &[&str] = &[
    "examples/nrf52/microbit/ble",
    "examples/nrf52/microbit/bootloader",
    "examples/nrf52/microbit/compass",
    "examples/nrf52/microbit/esp8266",
    "examples/nrf52/microbit/jukebox",
    "examples/nrf52/adafruit-feather-nrf52840/neopixel",
    "examples/nrf52/adafruit-feather-nrf52840/bootloader",
    "examples/nrf52/adafruit-feather-nrf52840/bt-mesh",
    "examples/nrf52/nrf52840-dk/ble-mesh",
    "examples/stm32l0/lora-discovery",
    "examples/stm32l1/rak811",
    "examples/stm32l4/iot01a/wifi",
    "examples/stm32l4/iot01a/bootloader",
    "examples/rp/pico",
    "examples/stm32wl/nucleo-wl55/bootloader",
    "examples/stm32wl/nucleo-wl55/lorawan",
    "examples/stm32wl/nucleo-wl55/lorawan-dfu",
    "examples/stm32h7/nucleo-h743zi",
    "examples/stm32u5/iot02a",
    "examples/wasm/browser",
    "examples/std",
    "docs/modules/ROOT/examples/basic",
];

fn ci(batch: bool) -> Result<(), anyhow::Error> {
    let _e = xshell::pushenv("CI", "true");

    test_device()?;
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

    let mut docs_dir = root_dir();
    docs_dir.push("docs");
    docs_dir.push("modules");
    docs_dir.push("ROOT");
    docs_dir.push("examples");
    do_examples(docs_dir, &mut update_crate)?;
    Ok(())
}

fn fmt() -> Result<(), anyhow::Error> {
    let _p = xshell::pushd(root_dir())?;
    cmd!("cargo fmt").run()?;
    do_crates(WORKSPACES, &mut fmt_crate)?;
    Ok(())
}

fn fix() -> Result<(), anyhow::Error> {
    let _p = xshell::pushd(root_dir())?;
    cmd!("cargo fix").run()?;
    do_crates(WORKSPACES, &mut fix_crate)?;
    Ok(())
}

fn generate_batch_command(
    workspaces: &[&str],
    cmds: Vec<&str>,
) -> Result<xshell::Cmd, anyhow::Error> {
    let mut crate_target: Vec<(String, Option<String>)> = Vec::new();
    const MAX_LEVEL: usize = 3;
    do_crates(workspaces, &mut |project_dir| {
        let mut level = 0;
        let mut folder = project_dir.clone();
        loop {
            let config_file = folder.join(".cargo").join("config.toml");
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
                break;
            } else {
                // Use no target if reached max backtracking
                if level > MAX_LEVEL {
                    crate_target.push((
                        project_dir.join("Cargo.toml").to_str().unwrap().to_string(),
                        None,
                    ));
                    break;
                }
                folder = folder.join("..");
                level += 1;
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
    cmd!("cargo check --all --features 'std wifi+esp8266 wifi+eswifi tcp+smoltcp tls'").run()?;
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
    cmd!("cargo test -- --nocapture").run()?;
    Ok(())
}

fn test_device() -> Result<(), anyhow::Error> {
    let mut device = root_dir();
    device.push("device");
    let _p = xshell::pushd(&device)?;
    cmd!("cargo fmt --check").run()?;
    cmd!("cargo test --all --features 'std wifi+esp8266 wifi+eswifi tcp+smoltcp tls'").run()?;
    // Sanity check that we can build on cortex-m
    cmd!("cargo build --no-default-features --features 'wifi+esp8266 wifi+eswifi tcp+smoltcp tls ble+nrf52840 embassy-nrf/nrf52840 embassy-nrf/time-driver-rtc1 embassy-executor/time' --target thumbv7em-none-eabihf").run()?;
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
    let _p = xshell::pushd(project_file)?;
    cmd!("cargo fmt").run()?;
    Ok(())
}

fn fix_crate(project_file: PathBuf) -> Result<(), anyhow::Error> {
    println!("Fixing {}", project_file.to_str().unwrap_or(""));
    let _p = xshell::pushd(project_file.parent().unwrap())?;
    cmd!("cargo fix").run()?;
    Ok(())
}

fn docs() -> Result<(), anyhow::Error> {
    generate_examples_page()
}

fn clone(example: &str, target_dir: &str) -> Result<(), anyhow::Error> {
    let source_dir = root_dir().join(example);
    let project_file = source_dir.join("Cargo.toml");

    let target_dir = PathBuf::from_str(target_dir)?;

    fs::create_dir_all(&target_dir)?;
    let current_rev = cmd!("git rev-parse HEAD").output()?.stdout;
    let mut current_rev = String::from_utf8(current_rev)?;
    current_rev.pop();

    let contents = fs::read_to_string(&project_file).expect("error reading file");
    let mut t = contents.parse::<toml::Value>().unwrap();

    if let Some(deps) = t.get_mut("dependencies") {
        for dep in [
            "drogue-device",
            "drogue-lorawan-app",
            "drogue-blinky-app",
            "ble",
        ]
        .iter()
        {
            if let Some(toml::Value::Table(table)) = deps.get_mut(dep) {
                table.remove("path");
                table.insert(
                    "git".to_string(),
                    toml::Value::String(
                        "https://github.com/drogue-iot/drogue-device.git".to_string(),
                    ),
                );
                table.insert("rev".to_string(), toml::Value::String(current_rev.clone()));
            }
        }
    }

    fs::copy(
        root_dir().join("rust-toolchain.toml"),
        target_dir.join("rust-toolchain.toml"),
    )?;
    fs::write(target_dir.join("Cargo.toml"), toml::to_string_pretty(&t)?)?;

    // Locate closes .cargo dir
    let mut cargo_dir: PathBuf = ".cargo".into();
    loop {
        let d = source_dir.join(&cargo_dir);
        if d.exists() {
            cargo_dir = d;
            break;
        }
        let p: PathBuf = "..".into();
        cargo_dir = p.join(cargo_dir);
    }

    // Copy files
    fs::create_dir_all(&target_dir.join(".cargo"))?;
    copy_files(
        &cargo_dir,
        vec!["config.toml".into()],
        &target_dir.join(".cargo"),
    )?;

    // Copy files
    copy_files(
        &source_dir,
        vec!["src".into(), "memory.x".into(), "build.rs".into()],
        &target_dir,
    )?;
    Ok(())
}

fn copy_files(
    source_dir: &PathBuf,
    mut files: Vec<PathBuf>,
    target_dir: &PathBuf,
) -> Result<(), anyhow::Error> {
    while !files.is_empty() {
        let mut next_files = Vec::new();
        for file in files.drain(..) {
            let source_file = source_dir.join(&file);
            if let Ok(metadata) = source_file.metadata() {
                let file_type = metadata.file_type();
                if file_type.is_dir() {
                    fs::create_dir_all(&target_dir.join(&file))?;
                    for entry in fs::read_dir(&source_file)? {
                        let entry = entry?;
                        let name = entry.file_name().into_string().unwrap();
                        next_files.push(file.join(name));
                    }
                } else {
                    let src = source_dir.join(&file);
                    let dst = target_dir.join(&file);
                    fs::copy(src, dst)?;
                }
            }
        }
        files = next_files;
    }
    Ok(())
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
    let nav = root_dir()
        .join("docs")
        .join("modules")
        .join("ROOT")
        .join("examples_nav.adoc");
    let mut nav = std::fs::File::create(nav).expect("unable to open file");
    let mut nav_entries: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
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
                            if let Some(e) = nav_entries.get_mut(kw) {
                                e.push((
                                    format!("{}/README.adoc", relative.unwrap().display(),),
                                    description.to_string(),
                                ));
                            } else {
                                nav_entries.insert(
                                    kw.to_string(),
                                    vec![(
                                        format!("{}/README.adoc", relative.unwrap().display(),),
                                        description.to_string(),
                                    )],
                                );
                            }
                            entries.push(format!(
                                "* xref:{}/README.adoc[{}] (link:https://github.com/drogue-iot/drogue-device/tree/main/{}[github])",
                                relative.unwrap().display(),
                                description,
                                relative.unwrap().display(),
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
    writeln!(nav, "* xref:examples.adoc[Examples]").unwrap();
    for (kw, entries) in nav_entries.iter_mut() {
        entries.sort();
        let mut v: Vec<char> = kw.chars().collect();
        v[0] = v[0].to_uppercase().nth(0).unwrap();
        let s: String = v.into_iter().collect();
        writeln!(nav, "** {}", s).unwrap();
        for entry in entries.iter() {
            writeln!(nav, "*** xref:{}[{}]", entry.0, entry.1).unwrap();
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
