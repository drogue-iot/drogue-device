#![allow(dead_code)]
#![deny(unused_must_use)]

use std::io::Write;
use std::{env, fs, path::PathBuf};

use xshell::cmd;

fn main() -> Result<(), anyhow::Error> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let args = args.iter().map(|s| &**s).collect::<Vec<_>>();

    match &args[..] {
        ["ci"] => test_ci(),
        ["update"] => update(),
        ["docs"] => docs(),
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
    do_examples(examples_dir, 1, usize::MAX, &mut update_example)?;
    Ok(())
}

fn test_ci() -> Result<(), anyhow::Error> {
    let _e = xshell::pushenv("CI", "true");
    let _e = xshell::pushenv("RUSTFLAGS", "-Dwarnings");
    test_device()?;
    let mut examples_dir = root_dir();
    examples_dir.push("examples");

    //    do_examples(examples_dir, 1, 3, &mut test_example)?;
    for example in &[
        "nrf52/microbit",
        "stm32l0/lora-discovery",
        "stm32l1/rak811",
        "stm32l4/iot01a-wifi",
        "rp/pico",
        "stm32wl/nucleo-wl55",
        "stm32h7/nucleo-h743zi",
        "wasm/browser",
        "std",
    ] {
        let mut example_dir = examples_dir.clone();
        example_dir.push(example);
        check_example(example_dir)?;
    }
    Ok(())
}

fn test_device() -> Result<(), anyhow::Error> {
    let mut device = root_dir();
    device.push("device");

    let _p = xshell::pushd(&device)?;

    cmd!("cargo test --all --features 'std wifi+esp8266 wifi+eswifi tls lora tcp+smoltcp'").run()?;
    Ok(())
}

fn do_examples<F: FnMut(PathBuf) -> Result<(), anyhow::Error>>(
    current_dir: PathBuf,
    depth: usize,
    max_depth: usize,
    f: &mut F,
) -> Result<(), anyhow::Error> {
    if depth > max_depth {
        return Ok(());
    }
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.ends_with("Cargo.toml") {
            f(path.clone())?;
        }

        let file_type = entry.file_type()?;
        if file_type.is_dir() && !path.ends_with("target") {
            do_examples(path, depth + 1, max_depth, f)?;
        }
    }

    Ok(())
}

fn check_example(project_file: PathBuf) -> Result<(), anyhow::Error> {
    println!("Building example {}", project_file.to_str().unwrap_or(""));
    let _p = xshell::pushd(project_file)?;
    cmd!("cargo build --release").run()?;
    Ok(())
}

fn update_example(project_file: PathBuf) -> Result<(), anyhow::Error> {
    println!("Updating example {}", project_file.to_str().unwrap_or(""));
    let _p = xshell::pushd(project_file.parent().unwrap())?;
    cmd!("cargo update").run()?;
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
        do_examples(examples_dir, 1, usize::MAX, &mut |project_file| {
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

fn root_dir() -> PathBuf {
    let mut xtask_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    xtask_dir.pop();
    xtask_dir
}
