build:
	cargo build
.PHONY: build

test: build
	cd test && ../target/debug/nvim-test-runner
.PHONY: test

log:
	tail -f /tmp/nvim-test-runner.log
.PHONY: log