.PHONY: check
check:
	cargo fmt -- --check
	cargo clippy
# Minimum required Rust is 1.46 due to #[track_caller]
	cargo +1.46 check --tests
	cargo audit

.PHONY: release
release: target/release/bard

target/release/bard:
	cargo build --release

.PHONY: examples
examples:
	cargo test
	cd default && cargo run -- make
	cd example && cargo run -- make
