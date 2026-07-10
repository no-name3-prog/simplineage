# Local helpers use stock cargo only.
# Full suite (nextest, deny, audit, llvm-cov) runs in GitHub Actions or Docker/devcontainer.
.PHONY: build test fmt clippy doc check bench docker docker-dev clean help

help:
	@echo "SimpLineage — local targets (stock Cargo; no extra installs)"
	@echo "  make build      cargo build --workspace"
	@echo "  make test       cargo test --workspace"
	@echo "  make fmt        cargo fmt --all"
	@echo "  make clippy     cargo clippy -D warnings"
	@echo "  make doc        cargo doc"
	@echo "  make check      fmt + clippy + test + doc (light gate)"
	@echo "  make bench      cargo bench -p simplineage-core"
	@echo "  make docker     build runtime image (needs Docker only)"
	@echo "  make docker-dev interactive shell with CI tools (needs Docker only)"
	@echo ""
	@echo "Authoritative checks: open a PR and wait for CI Success."

build:
	cargo build --workspace

test:
	cargo test --workspace --all-features
	cargo test --workspace --doc --all-features

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

doc:
	cargo doc --workspace --no-deps --all-features --document-private-items

check: fmt clippy test doc
	@echo "Light local gate OK. Rely on GitHub Actions CI Success for merge."

bench:
	cargo bench -p simplineage-core

docker:
	docker build -t simplineage:local .

docker-dev:
	docker compose --profile dev run --rm dev

clean:
	cargo clean
