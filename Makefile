SHELL := /bin/bash

all: test examples

test:
	cargo test --all

examples:
	for i in $$(find examples/ -name "Cargo.toml"); do \
		d=$$(dirname $$i); \
		pushd $$d; \
		cargo build --release || exit 1; \
		popd; \
	done

.PHONY: all test examples
