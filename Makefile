.PHONY: build release serve format format-check

build:
	python3 build.py

release:
	python3 build.py --release

serve:
	python3 -m http.server 8843 --bind 127.0.0.1 --directory dist

format:
	cargo fmt --all
	cargo fmt --manifest-path tad/Cargo.toml --all
	npm --prefix web run format

format-check:
	cargo fmt --all -- --check
	cargo fmt --manifest-path tad/Cargo.toml --all -- --check
	npm --prefix web run format:check
