.PHONY: build test fmt clippy check run clean

build:
	cargo build --workspace

test:
	cargo test --workspace

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

check: fmt clippy test

run:
	cargo run -p simplineage-cli -- hello

clean:
	cargo clean
