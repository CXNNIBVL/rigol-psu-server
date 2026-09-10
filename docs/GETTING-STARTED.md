# Getting started

Everything here runs against **simulated supplies** — no DP832, no udev rules,
no VPN, no lab PC. The simulator reaches every state the real system can,
failure states included, and it is what the test suite runs against. Real
hardware lives in [deploy/README.md](../deploy/README.md).

Target: from a clean machine to a bench in your browser in about ten minutes,
most of it the first `cargo build`.

---

## 1. What you need

Five things. Only the first two are unusual enough to be worth explaining.

| | Why |
|---|---|
| **Rust 1.85+** | The workspace `rust-version`. Newer is fine. |
| **A C toolchain** (`cc`, plus `make`) | `rusqlite` is built with the `bundled` feature, so SQLite itself is compiled from source. Without a C compiler the build fails deep inside `libsqlite3-sys` and the error does not say "install gcc". |
| **Node 22 LTS + npm** | The Svelte 5 / Vite 8 toolchain. |
| **`just`** | Every workflow in this repo is a `just` recipe. Nothing is documented as a raw `cargo`/`npm` incantation, because the recipes carry ordering constraints that are easy to get wrong (see §4). |
| **`cargo-nextest`** | The Rust tests are configured through `.config/nextest.toml` — process isolation, timeouts, and the `hw_*` exclusion. Plain `cargo test` ignores all of it. |

### Installing them

```sh
# Rust, via rustup — https://rustup.rs
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Fedora / RHEL
sudo dnf install -y gcc make nodejs npm

# Debian / Ubuntu
sudo apt install -y build-essential nodejs npm

# The two cargo tools
cargo install just --locked
cargo install cargo-nextest --locked
```

`just` is also packaged for most distributions (`dnf install just`,
`apt install just`, `brew install just`) if you would rather not build it.

Check what you have:

```sh
rustc --version          # >= 1.85
cc --version
node --version           # >= 22
just --version
cargo nextest --version
```

Nothing else needs installing globally. The Node dependencies are local to
`ui/`, and SQLite, as noted, is compiled into the binary rather than linked
against a system library — one fewer thing to have the wrong version of on the
lab PC.

---

## 2. First run

```sh
git clone <this repo> && cd rigol-psu-server
just build-ui        # once, on a fresh clone: npm ci, then vite build
just dev
```

`just build-ui` is the only step that installs the Node dependencies. `just dev`
uses `build-ui-quick`, which rebuilds the bundle *without* reinstalling
`node_modules` — right for the dev loop, but it has nothing to work with on a
clone that has never run `npm ci`. Do it once and you can forget about it.

`just dev` then compiles the workspace, SQLite included — a few minutes the
first time, seconds after that — and starts the server on
<http://127.0.0.1:8080> with three simulated supplies.

You should see a warning in the log saying no hardware will be touched, and a
second one saying presets and profiles are in memory and will not be kept. Both
are expected under `--simulate`; see §5.

Open the address and you have the bench: nine channels across three supplies,
live readouts updating twice a second, and everything on the **Bench**,
**Profiles** and **Settings** tabs working end to end.

If instead you get a page saying *"The UI has not been built"*, you started the
server some way other than `just dev` — see §4.

---

## 3. Where things are

```
crates/dp832/     transport trait, SCPI codec, the usbtmc device, the simulator
crates/core/      domain model, clamps, profile apply logic, the per-supply actors
crates/server/    axum routes, WebSocket, OpenAPI, asset embedding, main
migrations/       versioned SQL, applied at startup
ui/               the Svelte SPA (src/lib/components is where the screens are)
deploy/           udev rule, systemd unit — first-class files, not README snippets
```

Structural changes land in `crates/core` (domain model, clamps, apply logic)
and `crates/server` (routes, WebSocket); read those crates' module docs before
reworking either.

---

## 4. The development loop

**`just dev` rebuilds the UI bundle before starting the server, and it has to.**
`rust-embed` bakes `ui/dist` into the binary at compile time, so a server
started without rebuilding the bundle serves whatever was there last — which on
screen looks exactly like a UI fix that did not work.

So:

| You changed | Do this |
|---|---|
| Rust | `just dev` — recompiles and restarts. |
| Svelte, and you want a fast loop | `just dev` in one terminal, `cd ui && npm run dev` in another. Vite serves on its own port with hot reload and proxies `/api` (and the WebSocket) to 127.0.0.1:8080. |
| Svelte, and you want to see it as shipped | `just dev` again. |

The Vite dev server is the right tool while you are iterating on a component;
the embedded bundle is what the e2e tests and the release binary use, so confirm
anything non-trivial there before calling it done.

---

## 5. What a simulated run does and does not keep

`--simulate` keeps presets and profiles **in memory**. Every restart starts from
an empty store, which is what the tests want and usually not what a dev bench
wants. To keep them:

```sh
cargo run -p server --features simulation -- \
  --simulate 3 --bind 127.0.0.1:8080 --database ./dev.db
```

The server says at startup which of the two it is doing. `*.db` files are
gitignored. Migrations run at startup, inside a transaction, and a failed one
refuses to open the database rather than leaving a half-changed schema behind.

### Reaching the failure states

The states most worth looking at are the ones you cannot produce on demand: a
supply going offline mid-session, a channel reading `Unknown`, a quarantine
banner. In a simulation build there is a route for that:

```sh
curl -X POST localhost:8080/api/sim/psu2/fault \
  -H 'content-type: application/json' -d '{"kind":"disconnected"}'
```

`kind` is one of `none`, `timeout`, `disconnected`, `garbage`; `none` clears it.
Watch the UI react, then clear it and watch it recover.

This endpoint is gated twice — the `simulation` feature keeps it out of the
shipped binary entirely, and a runtime check keeps it unmounted unless the
process is actually driving simulated supplies. An endpoint that can fabricate
instrument state deserves more than one lock.

---

## 6. Running the tests

```sh
just test          # Rust, nextest, simulator only
just test-doc      # doctests — nextest does not run these
just test-ui       # svelte-check + Vitest component tests
just test-e2e      # Playwright against the real binary
just test-all      # all four, in that order
just lint          # clippy over both feature configurations
just fmt           # rustfmt
```

`just` on its own lists the recipes.

Before the first `just test-e2e`, once per machine:

```sh
just install-browsers      # playwright install chromium
```

The e2e suite starts the real binary itself (`--simulate 3` on port 18099) and
drives it through a browser: real server, real WebSocket, real SPA, nothing
mocked. Outside CI it reuses an already-running server on that port, so a wedged
one from a previous run is worth checking for when results look impossible.

`just lint` runs clippy three times over different feature configurations on
purpose — a compile error must not be able to hide in the one you do not build
locally. If CI fails on lint and your machine is green, that is usually why.

Tests named `hw_*` need real DP832s and are excluded from every ordinary run by
`default-filter` in `.config/nextest.toml`. There are none yet; the profile
exists for the open hardware risks — lock release including the `SIGKILL` case,
sustained poll rate, `usbtmc` timeout and disconnect semantics on this kernel —
which can only be settled on the bench. Note that this exclusion is a *nextest*
mechanism: `cargo test` would happily try to run them.

---

## 7. When you do have hardware

Read [deploy/README.md](../deploy/README.md) in full before plugging anything
in. The short version of why it is not optional:

- Supplies are addressed through `/dev/rigol-psuN` symlinks pinned by serial
  number in a udev rule. `/dev/usbtmcN` numbering rotates across reboots and
  replugs, and with three identical DP832s that silently points a profile at the
  wrong unit.
- `config.toml` is the safety boundary. Per-channel limits live there, cannot be
  raised through the API, and are not editable from the UI. The server refuses
  to start without it.
- A serial mismatch quarantines that supply: readable and configurable, but its
  outputs cannot be energised until the file is corrected and the service
  restarted. There is no way to clear that from the UI.

The build happens **on the lab PC**, not here: a binary built on one
distribution does not necessarily start on another, and the `hw_*` tests can
only run where the supplies are. So the lab PC gets the same toolchain as §1,
and `just build` produces a single binary with the UI embedded — that plus the
config, the udev rule and the systemd unit is the entire deployment.

---

## 8. Things that go wrong first

**"The UI has not been built".** `ui/dist` was missing at compile time, so
`build.rs` wrote a placeholder page. Run `just build-ui`, then rebuild.

**`--config is required`.** You started the binary without `--config` and
without `--simulate`. Fail-closed is intentional: without a config there are no
clamps, and without clamps nothing should be controllable.

**The build dies in `libsqlite3-sys`.** No C compiler. See §1.

**`vite: not found`, or `just dev` fails before the server starts.** The Node
dependencies were never installed. `just build-ui` once; see §2.

**`just dev` serves a stale UI.** You are running the server directly rather
than through the recipe, or `npm run build` failed and you missed it in the
scroll.

**Address already in use.** A previous `just dev` (port 8080) or a Playwright
`webServer` (port 18099) is still alive.

**A nextest test is killed after 5 seconds.** That timeout is the point:
everything in the default set runs against the simulator, so a test measured in
seconds is stuck, not slow.
