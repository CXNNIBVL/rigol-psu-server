# Rigol PSU bench server

Control and monitor USB-attached Rigol DP832 power supplies from your browser.
One process serves the HTTP API and the UI on the same port; one `[[psu]]`
block per supply in the config.

**Using a bench someone already runs?** Start at [Using the bench](#using-the-bench).

**Running or changing it yourself?** [docs/GETTING-STARTED.md](docs/GETTING-STARTED.md)
for simulated supplies on a laptop, or [deploy/README.md](deploy/README.md) for
the lab PC.

## Using the bench

Open the address you were given. Three tabs:

| Tab | For |
|---|---|
| **Bench** | Live readings, about twice a second, and direct control of each channel. A badge counts supplies needing attention — offline, quarantined, or holding a limit violation; open the supply to see which. |
| **Profiles** | Named setups spanning several channels. Create, edit, apply. |
| **Settings** | Per-channel building blocks: voltage, current limit, over-voltage (OVP) and over-current (OCP) protection. |

## Applying a profile

Staging a profile and switching it on are separate steps, deliberately:

- **Apply** checks the setpoints against the configured limits and stages them.
  It never energises a rail.
- **Power on** energises the rails the profile marks on, in sequence — a second,
  deliberate action.
- **Power off** switches every rail off, and is always allowed.

Apply is refused unless every target channel is already off. `force` overrides
that one rule and nothing else: not a limit clamp, not a quarantine, not a
profile that reaches a supply the server currently can't talk to.

## Limits and safety

- Per-channel limits live in `config.toml`. They cannot be raised through the
  API or the UI, and the server will not start without them.
- A supply whose serial does not match the config is **quarantined**: readable
  and configurable, but its outputs stay locked off until the config is
  corrected and the service restarted.
- Presets and profiles are stored in SQLite (`/var/lib/rigol-psu/data.db` by
  default). Simulated runs keep them in memory only.

## Running it yourself

Everything is a `just` recipe — run `just` for the list.

- **No hardware** (simulated supplies): [docs/GETTING-STARTED.md](docs/GETTING-STARTED.md)
  — `just build-ui` once, then `just dev`.
- **The lab PC**: [deploy/README.md](deploy/README.md).

`--simulate` and the fault-injection routes exist only in `simulation`-feature
builds; the release binary has neither.
