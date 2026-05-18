.PHONY: test update-expects fmt fmt-check check bench bench-export bench-diff

test:
	cargo test --release $(if $(F),-- $(F))

update-expects:
	cargo run --bin update-expects

fmt:
	cargo +nightly fmt

fmt-check:
	cargo +nightly fmt --check

check:
	RUSTFLAGS="-D warnings" cargo check
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

bench:
	cargo run --release -p zhc_bench

bench-export:
	cargo run --release -p zhc_bench -- export

bench-diff:
	cargo run --release -p zhc_bench -- diff
