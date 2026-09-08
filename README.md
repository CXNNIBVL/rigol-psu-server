# Rigol PSU bench server

Control and monitor three USB-attached Rigol DP832 power supplies over HTTP,
with a browser UI served from the same port.

The full design, including the reasoning behind each decision and the
alternatives that were rejected, is in **[docs/PLAN.md](docs/PLAN.md)**.

## Status

| Milestone | State |
|---|---|
| M1 — `dp832` driver crate (SCPI, USBTMC, simulator) | done |
| M2 — server skeleton (config, actors, polling, API, WebSocket) | done |
| M3 — Svelte dashboard | done |
| M4 — safety hardening | largely landed with M2 |
| M5 — presets and profiles (SQLite) | not started |
| M6 — deployment on the lab PC | files in `deploy/`, not yet installed |

## Running without hardware

```sh
just dev          # three simulated supplies on 127.0.0.1:8080
```

`--simulate` and the fault-injection routes exist only in builds with the
`simulation` feature; the release binary does not contain them.

## Running against real supplies

1. Install the udev rule and edit in your serial numbers:
   `deploy/99-rigol.rules` — see the comments in the file for how to find them.
2. Copy `config.example.toml` to `/etc/rigol-psu/config.toml` and set the bind
   address, serials and per-channel limits.
3. Install `deploy/rigol-psu.service`.

The per-channel limits in `config.toml` cannot be raised through the API and are
not editable from the UI. The server refuses to start without them.

## Development

```sh
just               # fmt, lint, test
just test          # Rust, nextest, simulator only
just test-ui       # svelte-check + Vitest component tests
just test-e2e      # Playwright against the real binary
just test-all      # all of the above
just lint          # clippy over both feature configurations
```

The e2e suite needs browser binaries once per machine:
`just install-browsers`.

`hw_*` tests need real supplies and are excluded from ordinary runs; on the lab
PC use `just test-hardware`.
