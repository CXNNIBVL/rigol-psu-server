# Task runner for the Rigol PSU server.
#
# The UI build is an explicit step rather than something a build.rs shells out
# to: making `cargo check` depend on npm would be both slow and surprising.

default: fmt lint test

# Run the test suite (nextest). Excludes `hw_*` tests, which need real supplies.
test:
    cargo nextest run --all-features

# Doctests. Nextest does not run these, so they need their own invocation.
test-doc:
    cargo test --doc --all-features

# Everything, the way CI runs it.
test-all: test test-doc

# Tests that require DP832s attached. Run this on the lab PC.
test-hardware:
    cargo nextest run --profile hardware --all-features

lint:
    cargo clippy --all-targets --all-features -- -D warnings

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

# Build the Svelte SPA into ui/dist, ready to be embedded.
build-ui:
    cd ui && npm ci && npm run build

# Release binary with the UI embedded.
build: build-ui
    cargo build --release

# Run against simulated supplies — no hardware, no udev rules, no VPN.
dev:
    cargo run -p server -- --simulate 3 --bind 127.0.0.1:8080
