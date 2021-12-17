mod tests {
    use duct::cmd;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    #[test]
    fn test_std_cloud() {
        let app = setup();
        let a = app.clone();
        let result = panic_catch_after(std::time::Duration::from_secs(300), move || {
            let e = std::thread::spawn(move || {
                run_example("std/cloud", std::time::Duration::from_secs(90));
            });

            let result = receive_message(&a);

            println!("Joining example thread");
            let e = e.join();
            println!("Example thread joined");
            let _ = e.unwrap();
            result
        });

        teardown(&app);

        if let Ok(Some(output)) = result {
            println!("V: {:?}", output);
            assert_eq!(output["application"].as_str().unwrap(), app);
            assert_eq!(output["device"].as_str().unwrap(), "device1");
            assert_eq!(
                output["datacontenttype"].as_str().unwrap(),
                "application/json"
            );
            assert_eq!(output["data"]["temp"].as_i64().unwrap(), 22);
        } else {
            assert!(false);
        }
    }

    fn setup() -> String {
        let api = env!("DROGUE_CLOUD_API");
        let access_token = env!("DROGUE_CLOUD_ACCESS_TOKEN");
        // Login
        cmd!("drg", "login", api, "--access-token", access_token)
            .run()
            .unwrap();

        let uuid = uuid::Uuid::new_v4().to_string();
        let app = format!("test-{}", uuid.to_string());
        let password = "hey-rodney";
        let device = "device1";

        configure(&app, device, password);

        let spec = format!(
            "{{\"credentials\":{{\"credentials\":[{{\"pass\":\"{}\"}}]}}}}",
            password
        );

        cmd!("drg", "create", "app", &app).run().unwrap();
        cmd!("drg", "create", "device", "--app", &app, device, "--spec", spec)
            .run()
            .unwrap();
        app
    }

    fn teardown(app: &str) {
        cmd!("drg", "delete", "app", app).run().unwrap();
    }

    fn receive_message(app: &str) -> Option<Value> {
        let mut result: Option<Value> = None;
        for _ in 0..10 {
            let output = cmd!("drg", "stream", &app, "-n", "1")
                .stdout_capture()
                .stderr_to_stdout()
                .read();

            println!("OUTPUT: {:?}", output);

            if let Ok(output) = output {
                match serde_json::from_str(&output) {
                    Ok(value) => {
                        result = Some(value);
                        return result;
                    }
                    Err(e) => {
                        println!("error parsing test output as JSON '{}': {:?}", &output, e);
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        println!("Receive message completed successfully");
        result
    }

    fn config_file() -> PathBuf {
        let mut config = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        config.push("config.toml");
        config
    }

    const HOSTNAME: &str = "http.sandbox.drogue.cloud";
    const PORT: u16 = 443;

    fn configure(app: &str, device: &str, password: &str) {
        let contents = format!(
            "http-username = \"{}@{}\"\nhttp-password = \"{}\"\nhostname = \"{}\"\nport = \"{}\"",
            device, app, password, HOSTNAME, PORT,
        );
        fs::write(config_file(), contents).expect("unable to write config file");
    }

    fn run_example(example: &'static str, d: Duration) {
        cmd!("cargo", "build")
            .dir(format!("../{}", example))
            .stdout_capture()
            .stderr_to_stdout()
            .env("DROGUE_CONFIG", config_file())
            .env("DEFMT_LOG", "trace")
            .env("RUST_LOG", "trace")
            .run()
            .unwrap();
        let c = cmd!("cargo", "run")
            .dir(format!("../{}", example))
            .stdout_capture()
            .stderr_to_stdout()
            .env("DROGUE_CONFIG", config_file())
            .env("DEFMT_LOG", "trace")
            .env("RUST_LOG", "trace")
            .start()
            .unwrap();
        let end = Instant::now() + d;
        while Instant::now() < end {
            match c.try_wait() {
                Ok(None) => {
                    println!("Wait a little longer");
                    std::thread::sleep(Duration::from_secs(1));
                }
                Ok(Some(o)) => {
                    println!("Example success: {:?}", o);
                    assert!(o.status.success());
                }
                Err(e) => {
                    println!("Error running command: {:?}", e);
                    assert!(false);
                }
            }
        }
        println!("Killing example");
        let _ = c.kill();
    }

    fn panic_catch_after<T, F>(d: std::time::Duration, f: F) -> std::thread::Result<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T,
        F: Send + 'static,
        F: std::panic::UnwindSafe,
    {
        std::panic::catch_unwind(|| {
            let (done_tx, done_rx) = std::sync::mpsc::channel();
            let handle = std::thread::spawn(move || {
                let val = f();
                done_tx.send(()).expect("Unable to send completion signal");
                val
            });

            match done_rx.recv_timeout(d) {
                Ok(_) => handle.join().expect("Thread panicked"),
                Err(_) => panic!("Thread took too long"),
            }
        })
    }
}
