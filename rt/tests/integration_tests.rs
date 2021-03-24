/*
 * Copyright 2019-2020, Ulf Lilleengen
 * License: Apache License 2.0 (see the file LICENSE or http://apache.org/licenses/LICENSE-2.0.html).
 */

// use cortex_m_rt::exception;
use drogue_device::prelude::*;
use std::sync::Once;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        env_logger::init();
    });
}

fn panic_after<T, F>(d: Duration, f: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T,
    F: Send + 'static,
{
    let (done_tx, done_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let val = f();
        done_tx.send(()).expect("Unable to send completion signal");
        val
    });

    match done_rx.recv_timeout(d) {
        Ok(_) => handle.join().expect("Thread panicked"),
        Err(_) => panic!("Thread took too long"),
    }
}

static INITIALIZED: AtomicBool = AtomicBool::new(false);

#[test]
fn test_device_setup() {
    setup();

    thread::spawn(|| {
        device!(MyDevice = configure; 1024);
    });

    panic_after(Duration::from_secs(5), move || {
        while !INITIALIZED.load(Ordering::SeqCst) {}
    })
}

fn configure() -> MyDevice {
    MyDevice {
        initialized: &INITIALIZED,
    }
}

struct MyDevice {
    initialized: &'static AtomicBool,
}

impl Device for MyDevice {
    fn mount(&'static self, _: DeviceConfiguration<Self>, _: &mut Supervisor) {
        log::info!("MOUNTED!");
        self.initialized.store(true, Ordering::SeqCst);
    }
}
