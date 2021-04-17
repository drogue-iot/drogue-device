
# Contributing

Thank you for your interest in the project and for considering contributing.

This guide should help you get started: creating a build and test environment, as well as contributing your work.

All contributions are welcome! While this guide will focus on contributing code, we would also encourage you to
contribute by reporting issues, providing feedback, suggesting new ideas. Or just by saying "hi" in the chat.

If you just want to run Drogue IoT device, take a look at one of the [examples](examples/). If you do not have any
board that is supported, fear not! Have a look at the [std examples](examples/std), which should run on any platform where
ther rust standard library compiles.

## Before you start

Before you start working on a fix or new feature, we would recommend to reach out to us and tell us about it. Maybe
we already have this in our heads (and forgot to create an issue for it), or maybe we have an alternative already.

In any case, it is always good to create an issue, or join the chat and tell us about your issues or plans. We will
definitely try to help you.

If you want to get started making changes to this project, you will need a few things. The following sub-sections
should help you get ready.

### Pre-requisites

In any case, you will need:

* An environment capable of building rust (Linux, Mac OS X or Windows are all supported).
* Some tools
  * The [rust tooclhain](https://rustup.rs).
  * git

### Optional requirements

* **A supported development kit** - Drogue device is about ... devices, so having a kit that you can run will help you test and validate code for peripherals (see [examples](examples/) for what boards we have working examples for).

* **Rust Nightly** – Drogue-device relies on features only available in Rust nightly. You can use nightly either by running all
  rust commands with `+nightly`, or just change default to nightly by running `rustup default nightly`.

* **An IDE** – Whatever works best for you. Eclipse, Emacs, IntelliJ, Vim, … [^1] should all be usable with this
  project. We do not require any specific IDE. We also do not commit any IDE specific files either.

[^1]: This list is sorted in alphabetical order, not in the order of any preference.

## Building

While the build is based on `cargo`, the CI (Continuous Integration) build uses `cargo xtask`, and installs the toolchains listed in [rust-toolchains](rust-toolchains).

## Testing

To run all tests:

    cargo test

### IDE based testing

You can also run cargo tests directly from your IDE. How this works, depends on your IDE.

However, as tests are compiled and executed on the host machine, the same requirements, as when running
tests on the host machine, apply (see above).

## Flashing examples

All examples use either `cargo embed` or `cargo run`, and this may require installing these utilities:

* `cargo install cargo-embed`
* `cargo install probe-run`

## Contributing your work

Thank you for reading the document up to this point and for taking the next step.

### Pre-flight check

Before creating a pull-request (PR), you should do some pre-flight checks, which the CI will run later on anyway.
Running locally will give you quicker results, and safe us a bit of time and CI resources.

It is as easy as running:

    cargo xtask ci

This will:

* Check source code formatting
* Run `cargo check`
* Run the build for all examples
* Run `cargo clippy`

The `clippy` checks should be seen as *suggestions*. Take a look at them, in some cases you will learn something new. If
it sounds reasonable, it might be wise to fix it. Maybe it flags files you didn't even touch. In this case just ignore
them, was we might not have fixed all the clippy suggestions ourselves.

### Creating a PR

Nothing fancy, just a normal PR. The CI will be triggered and come back with results. People tend to pay more attention
to PRs that show up "green". So maybe check back and ensure that the CI comes up "green" for your PR as well. If it
doesn't, and you don't understand why, please reach out to us.

There are bonus points for adding your own tests ;-)
