# Task runner for the Rigol PSU server.
#
# The UI build is an explicit step rather than something a build.rs shells out
# to: making `cargo check` depend on npm would be both slow and surprising.

default: 
  {{just_executable()}} --list

# Run the test suite (nextest). Excludes `hw_*` tests, which need real supplies.
test:
    cargo nextest run --workspace --all-features

# Doctests. Nextest does not run these, so they need their own invocation.
test-doc:
    cargo test --doc --all-features

# Everything, the way CI runs it.
test-all: test test-doc test-ui test-e2e

# Tests that require DP832s attached. Run this on the lab PC.
test-hardware:
    cargo nextest run --profile hardware --all-features

# Both feature configurations: a compile error must not be able to hide in the
# non-default one.
lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy -p server -p psu-core --all-targets --features simulation -- -D warnings
    # The domain crates on their own, without the features the server turns on.
    # Workspace builds unify features, so this is the only place their default
    # configuration is actually checked.
    cargo clippy -p psu-core -p dp832 --all-targets -- -D warnings

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
    cargo run -p server --features simulation -- --simulate 3 --bind 127.0.0.1:8080

# UI type check and component tests.
test-ui:
    cd ui && npm run check && npm run test

# Browser end-to-end tests (Playwright) against a simulated server.
# Builds the bundle first, so the tests never run against a stale UI.
test-e2e: build-ui
    cd ui && npx playwright test

# One-time, and on any machine that runs the e2e suite.
install-browsers:
    cd ui && npx playwright install chromium
