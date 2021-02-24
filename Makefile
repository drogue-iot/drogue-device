all: test examples

test:
	cargo test --all --manifest-path=./rt/Cargo.toml

examples:
	for i in $$(find examples/ -name "Cargo.toml"); do d=$$(dirname $$i); pushd $$d; cargo build --release; popd; done

.PHONY: all test examples
