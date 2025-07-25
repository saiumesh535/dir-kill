.PHONY: install build test release clean

install:
	cargo install --path .

build:
	cargo build --release

test:
	cargo test

release:
	./scripts/build-release.sh

clean:
	cargo clean
	rm -rf release/