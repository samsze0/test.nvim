build:
	cargo build
.PHONY: build

build-release:
	cargo build --release
.PHONY: build-release

test: build
	cd test && ../target/debug/nvim-test-runner
.PHONY: test

test-release:
	@cd test && ../target/release/nvim-test-runner

log:
	tail -f /tmp/nvim-test-runner.log
.PHONY: log