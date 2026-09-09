# Rigol DP832 Bench Control Server — Plan

**Status:** M1–M5 built and green against the simulator; M6 (deployment on the
lab PC) outstanding, and nothing here has yet talked to a real DP832
**Started:** 2026-09-08
**Target:** Linux x86_64 lab PC, reachable over VPN only

This is the design record, kept current as the thing was built. Where a decision
turned out to be wrong, the section says so and says what replaced it, rather
than being quietly rewritten — the rejected alternatives are the useful part.

---

## 1. Summary

A single Rust binary running on the lab PC that owns three USB-attached Rigol DP832
power supplies (9 channels total), exposes a REST + WebSocket API for control and
telemetry, and serves an embedded Svelte SPA on the same port. Users open a browser
over the VPN and get a live bench dashboard; named **device profiles** stage the
setpoints for a given DUT across multiple rails, and the user explicitly commits
power-on.

Safety is the organising constraint: the server can be restarted, crashed, or
misconfigured without de-energising running hardware, and it cannot be talked into
exceeding operator-set per-channel limits.

---

## 2. Hardware and transport

- **3 × Rigol DP832**, identical models, 3 channels each (CH1 30V/3A, CH2 30V/3A,
  CH3 5V/3A). 9 addressable channels.
- **Transport: Linux kernel `usbtmc` driver.** The server opens a character device
  and reads/writes SCPI strings. No USBTMC framing implemented by us.
- **Stable device naming via udev.** Three identical units means kernel enumeration
  order (`/dev/usbtmc0..2`) shuffles across reboots and replugs. A udev rule matches
  on VID + serial number and mints a stable symlink per unit:

  ```
  /dev/rigol-psu1
  /dev/rigol-psu2
  /dev/rigol-psu3
  ```

  This is the single most important piece of the deployment. Without it, a preset
  can land on the wrong physical supply after a reboot.

### 2.1 Transport trait

```rust
trait Transport: Send {
    fn write(&mut self, cmd: &str) -> Result<(), TransportError>;
    fn query(&mut self, cmd: &str) -> Result<String, TransportError>;
}
```

Two implementations, both first-class in v1:

- `UsbTmcTransport` — the real device node.
- `SimulatedDp832` — an in-process fake speaking the same SCPI subset, with
  injectable behaviours (slow responses, disconnect, protection trip).

The simulator is **not optional**. Development happens on a machine with no Rigols
attached, and the degraded/offline code paths (§7.3) cannot otherwise be tested —
you cannot unplug a USB cable in CI.

### 2.2 SCPI surface used

| Purpose | Command |
|---|---|
| Identity | `*IDN?` |
| Measure V/I/P for a channel | `:MEASure:ALL? CH<n>` |
| Read setpoints | `:APPLy? CH<n>` |
| Set V and I limit | `:APPLy CH<n>,<volt>,<curr>` |
| Output on/off | `:OUTPut:STATe CH<n>,ON\|OFF` |
| Output state query | `:OUTPut:STATe? CH<n>` |
| OVP / OCP level + enable | `:OUTPut:OVP` / `:OUTPut:OCP` subtree |
| Front panel lock / unlock | `:SYSTem:LOCK ON\|OFF` |

`:MEASure:ALL?` returns voltage, current and power in one round-trip, so a full poll
of one PSU is 3 queries, not 9.

---

## 3. Technology decisions

| Area | Decision |
|---|---|
| Server language | Rust |
| HTTP framework | `axum` + `tokio` |
| Command API | REST + JSON; OpenAPI 3.1 generated via `utoipa` from the domain types |
| Live telemetry | WebSocket, full state snapshots at 2 Hz |
| Frontend | Svelte SPA (Vite), static build |
| Component library | Skeleton UI v5 (`@skeletonlabs/skeleton` + `skeleton-svelte`) on Tailwind 4 |
| Theme | Skeleton's built-in **Cerberus**, applied via `data-theme="cerberus"` |
| Frontend delivery | Embedded in the binary via `rust-embed`, served on the same port |
| User data store | SQLite via `rusqlite`, migrations from commit one |
| Operator config | Root-owned `config.toml` |
| Logging | `tracing` → journald |
| Build orchestration | `justfile` |
| Test runner | `cargo-nextest` (process-per-test); `cargo test --doc` for doctests |

### 3.1 Rejected alternatives

**gRPC.** Browsers cannot speak gRPC natively; it requires `tonic-web` on the server
plus a grpc-web JS client and a `.proto` build step, and browser grpc-web still
cannot do bidirectional streaming. The API surface here is roughly a dozen endpoints,
and the streaming requirement is satisfied by a WebSocket in a few lines. The
toolchain cost buys nothing. Typed clients are recovered via the OpenAPI spec.

**Tauri.** Tauri builds a native desktop binary wrapping a webview, installed on a
client machine. It does not serve a UI over a port to remote browsers, which is the
stated requirement. Rejected as a category error — but note that choosing Svelte
keeps Tauri available later: the same frontend can be wrapped as a native desktop
client against the same HTTP API with no rewrite.

**Flutter Web.** Viable, and the user is comfortable in Flutter. Rejected because it
ships a multi-megabyte CanvasKit/skwasm payload and paints to a canvas, making text
selection, zoom and accessibility second-class, and it adds the Dart/Flutter
toolchain to the build. The deciding factor was that Svelte preserves the Tauri
path. *Revisit if a native mobile app becomes a requirement — Flutter Web → Flutter
mobile is a much shorter hop than Svelte → native.*

**Server-Sent Events** instead of WebSocket. Close call. WebSocket chosen for
symmetry (commands and state on one connection) and future-proofing.

**Delta updates** over the WebSocket. Rejected: full snapshots of 9 channels are
~2 KB of JSON at 2 Hz, which is free, and they make reconnection stateless and
eliminate an entire class of client-desync bug.

---

## 4. Storage split

The dividing line: **edited through the app → SQLite. Edited with root on the box →
TOML.**

### 4.1 SQLite (`/var/lib/rigol-psu/data.db`)

Everything the application owns at runtime: channel presets, device profiles,
profile→channel bindings, UI preferences. Transactional, correct under the concurrent
multi-user access from §6, one file to back up. Schema migrations from the first
commit — presets are the only irreplaceable data in the system.

### 4.2 `config.toml` (root-owned, e.g. `/etc/rigol-psu/config.toml`)

Two things deliberately kept **out** of the database:

1. **Hard per-channel safety clamps.** In the DB, a stray `UPDATE`, a migration bug,
   or a restore from an older backup silently removes protection — the system fails
   *open*. In a root-owned TOML file the limits are greppable, git-diffable, and the
   server **refuses to start if the file is missing** — failing closed.
2. **Deployment topology.** Slot → label mapping and expected serials, bind address,
   poll rates. This changes when the *hardware* changes, so it belongs next to the
   udev rule it mirrors.

```toml
bind = "10.8.0.4:8080"          # no default; must be explicit

[poll]
measurement_hz = 2.0
setpoint_hz = 0.5

[[psu]]
slot   = "psu1"
device = "/dev/rigol-psu1"
label  = "Bench Left"
serial = "DP8A1234567"          # asserted at startup, see §5

  [[psu.channel]]
  id       = "CH1"
  max_volt = 5.5
  max_curr = 2.0

  [[psu.channel]]
  id       = "CH2"
  max_volt = 15.0
  max_curr = 3.0

  [[psu.channel]]
  id       = "CH3"
  max_volt = 5.5
  max_curr = 3.0
```

Telemetry is neither: it is transient. The server holds only the latest sample per
channel in memory and broadcasts it; nothing is retained (§7.2).

---

## 5. Device identity and quarantine

Because udev pins a serial to a **slot** name (`rigol-psu1`), the slot is the stable
logical identity:

- **The database stores slot keys** (`psu1/CH1`). When a supply dies and is replaced,
  you edit one udev rule and every stored profile keeps working. Serial-keyed profiles
  would all point at a dead unit.
- **`config.toml` records the expected serial as an assertion.** At startup the server
  issues `*IDN?` on each device and compares.

### 5.1 Serial mismatch → quarantine

A mismatch means either a deliberate hardware swap or something miswired. The server
**starts anyway** — refusing to boot takes the whole bench down — but marks the unit
**quarantined** and surfaces a persistent banner in the UI. Quarantine is a third
device state alongside online and offline: *online but distrusted*.

| Operation on a quarantined PSU | Allowed? |
|---|---|
| Read measurements | yes |
| Read setpoints | yes |
| Write setpoints / OVP / OCP | yes |
| **Turn an output on** | **refused** |
| Turn an output off | yes |

Channels on a quarantined unit remain OFF until the mismatch is resolved.

**Clearing quarantine is operator-only**: correct `config.toml`, restart the service.
There is deliberately **no "trust this unit" button in the UI** — the person who would
click it is the same person who should be fixing the config, so a dismiss button
reduces the check to decoration.

**Interaction with §8:** if a channel on a quarantined unit is *already live* when the
server starts, it is **not** turned off. Adoption (§8) takes precedence; quarantine
prevents new energisation, it does not cut existing power.

---

## 6. Access and concurrency

- **No authentication in v1.** Access control is placement: either the VPN, or a
  loopback bind reached over an SSH tunnel. Both are supported and the choice is one
  line of `config.toml`; the tunnel exposes nothing to the network at all.
- **`--bind` is mandatory with no default.** Exposing the service beyond localhost is
  a deliberate act, not an accident of a default value.
- **Multiple simultaneous users are expected.** The server is authoritative: all state
  changes broadcast to every connected client at 2 Hz, so two open browser tabs
  converge on truth rather than fighting over stale local state.
- **No audit trail** in v1.

---

## 7. Runtime model

### 7.1 One actor per PSU

A USBTMC handle is a single serialised resource — one command at a time, no
interleaving. Each PSU gets a `tokio` task owning its handle exclusively, fed by an
mpsc command queue. Polling and user commands both go through it; **user commands are
prioritised over poll traffic.**

The three PSUs poll concurrently, so a 2 Hz cycle is ~3 sequential round-trips per
device with substantial headroom.

### 7.2 Polling

- **Measurements at 2 Hz** (`:MEASure:ALL? CH<n>`).
- **Setpoints at 0.5 Hz** (`:APPLy?`, `:OUTPut:STATe?`, OVP/OCP state).

The setpoint poll exists because **the server is not the only writer.** Someone can
walk up and turn the physical knob or press Output On. Polling only measurements
would leave the UI showing correct readings alongside a stale setpoint, and any
preset-diff logic would be working from fiction. 0.5 Hz detects out-of-band changes
within ~2 s.

**No history.** The server keeps only the most recent sample per channel — enough to
answer "what is this rail doing right now" — and discards it on the next poll. There
is no ring buffer, no charting, and no persistence of telemetry.

This is a deliberate cut. Trend data was judged unlikely to be used often enough to
justify its cost, and *any* history feature drags in retention policy, a query API,
memory or storage growth, and a charting dependency in the UI. Should it be wanted
later, it re-enters as a scoped feature, not an increment: the natural shape is an
in-memory ring buffer feeding a `GET /api/history` endpoint, with durable logging a
further step beyond that. Nothing in this design forecloses it — the actor already
produces the sample stream any buffer would consume.

### 7.3 Failure handling

When a PSU stops answering (timeout, unplugged mid-session):

- Mark the device **offline, with a machine-readable error reason** attached
  (`TIMEOUT`, `DEVICE_GONE`, `IO_ERROR`) and surface it in the UI.
- Set affected channel state to **`Unknown`** — explicitly, as its own variant.
  **Never render a fake `0.0 V` for a rail whose real state is unknown.** It may
  still be live.
- Retry with exponential backoff; reconnect and re-adopt automatically when the
  device reappears.
- If only 2 of 3 PSUs are present at startup, **start degraded** with the third shown
  offline. Refusing to boot because one supply is switched off is worse than showing
  it greyed out.

### 7.4 Front panel lock

The front panel **is** locked while the server is actively driving a supply, accepted
on the basis that the mains switch remains as a physical kill path.

Acquired and released **lazily** to bound the blast radius:

- Acquire on the first command issued to that PSU.
- Release on graceful shutdown (SIGTERM handler).
- Release after an idle timeout with no commands to that PSU.

This means a crashed server leaves an *idle* bench unlocked, rather than leaving all
three supplies locked with a live rail and only the mains switch for recovery.

---

## 8. Restart posture

The service will restart — deploys, crashes, `systemctl restart`. The DP832s keep
their outputs on regardless.

**On startup the server adopts live channels in place and changes nothing.** A
four-hour soak test survives a service restart. A software restart must never decide
to de-energise hardware someone else is depending on; a running experiment is not the
server's to end.

**But** if an adopted live channel's setpoints exceed the current clamps in
`config.toml`, the server:

- surfaces it loudly as a **clamp violation** banner in the UI, and
- **refuses further writes to that channel** until a human resolves it,
- while **leaving the output energised**.

Turning everything off on startup was considered and rejected: deterministic, but it
kills a running test on every deploy.

---

## 9. Presets and profiles

### 9.1 Model

```
ChannelPreset            — the primitive
    id, name
    volt, curr_limit
    ovp_level, ovp_enabled
    ocp_level, ocp_enabled

DeviceProfile            — a named DUT, composed of presets
    id, name, description

ProfileBinding           — one rail of a profile
    profile_id
    slot, channel         — e.g. "psu1", "CH2"
    preset_id
    order                 — power-up sequence position
    delay_ms              — dwell after this rail comes up
    output_on             — does power-on energise this rail? intent only, §9.2
```

`output_on` sits on the **binding**, not the preset. It first went on the preset,
which made "does this rail come up on power-on" a property of the settings
rather than of how a profile uses them — so a preset shared by two profiles had
to auto-start in both. Wanting 3v3 to come up automatically on a mainboard but
stay staged on a sensor board meant two near-identical presets, which is exactly
the duplication presets exist to prevent. It belongs beside `order` and
`delay_ms`, which are already statements about how *this profile* powers *this
rail*. Migration `002` moves it and backfills from each binding's preset, so
profiles written before the change power up identically after it.

`order` and `delay_ms` are stored from day one even if always left at defaults.
Rail sequencing matters for devices whose supplies must come up in a defined order,
and adding an ordering column later is a migration; adding it now is free.

### 9.2 Apply semantics — staged, then committed

Applying a profile **never energises anything.**

1. **Validate.** Every bound channel is checked against its clamps. If any target
   channel is currently ON, the apply is refused unless `force` is set. If any bound
   PSU is offline or quarantined, the apply is **refused outright** — see §10.
2. **Stage.** Setpoints, current limits and OVP/OCP are written to all bound channels.
   The stored `output_on` intent is recorded but **not acted on**.
3. **Arm.** The UI presents an explicit **Power On** control reflecting the profile's
   intent and its sequencing order.
4. **Commit.** The user presses it. Rails energise in `order`, honouring `delay_ms`.

This keeps the single destructive action behind a deliberate human gesture, while
still letting the profile express what "on" should mean.

---

## 10. Safety rules

These are the invariants. Everything above serves them.

1. **Hard clamps live in root-owned TOML, never in the database, and are never
   overridable by the API.**
2. **`force` overrides exactly one rule: "the channel must be OFF before applying a
   preset."** It does **not** override clamps, quarantine, or offline-device refusal.
   A `force` that bypasses clamps would make the entire config-file carve-out theatre.
3. **A value exceeding a clamp is rejected with a clear error — never silently clamped
   down.** Silently receiving different settings than you asked for is worse than a
   refusal.

   **The clamp governs output, not trip points.** This first also clamped OVP and
   OCP, which was wrong: a trip point sits *above* the value it protects by
   definition, so a channel limited to 5 V could not carry an OVP at 5.5 V and
   the only way to configure it was to switch protection off — strictly less safe
   than allowing a backstop that cannot itself produce a volt. A trip point
   *below* its own setpoint is the real mistake, and it is rejected when a preset
   is saved, where it can be judged against the numbers it sits beside.
4. **Partial application of a multi-rail profile is a hard refusal, never forceable.**
   Half-powering a board can push current through an unpowered rail's protection
   diodes.
5. **Unknown state is never rendered as zero.** (§7.3)
6. **The server never de-energises a channel it did not energise**, except on explicit
   user command. (§8)
7. **Missing `config.toml` → refuse to start.** Fail closed.

---

## 11. Open risks

Flagged as risks rather than decisions; each needs real hardware to settle.

1. **Lock release behaviour.** The command itself is settled — `:SYSTem:LOCK`. What
   remains unverified is that the lock **reliably releases** on the shutdown path
   (§7.4), and what state the panel is left in if the process dies without running its
   SIGTERM handler. An unreleasable lock changes the safety calculus, since the
   fallback becomes the mains switch on a possibly-live rail. Verify both the graceful
   path and a `SIGKILL` on real hardware.
2. **Sustained poll rate.** 2 Hz across 3 devices should have ample headroom, but the
   interaction of poll traffic with prioritised user commands on a shared handle is
   unverified. Measure before committing to the rate; the config makes it tunable.
3. **Kernel `usbtmc` behaviour with three simultaneous handles** — timeout and `ioctl`
   semantics vary by kernel version, particularly around device removal mid-transfer.
   Validate the disconnect path on the actual target kernel.

---

## 12. Repository layout

```
rigol-psu-server/
├── README.md
├── justfile
├── Cargo.toml
├── config.example.toml
├── crates/
│   ├── dp832/              # transport trait, SCPI codec, usbtmc + simulator
│   ├── core/               # domain model, clamps, profile apply logic, actors
│   └── server/             # axum routes, websocket, rust-embed, main
├── migrations/             # versioned SQL
├── docs/
│   └── PLAN.md             # this document
├── ui/                     # Svelte SPA (Vite)
└── deploy/
    ├── 99-rigol.rules      # udev, VID + serial → /dev/rigol-psuN
    ├── rigol-psu.service   # systemd unit
    └── README.md
```

The udev rule, systemd unit and example config are **first-class files in the repo**,
not README snippets — they are part of the deliverable.

---

## 13. API surface (sketch)

REST, JSON, typed error codes. OpenAPI generated from the Rust types.

```
GET    /api/state                      full snapshot: devices, channels, health
GET    /api/devices                    slot, label, serial, online|offline|quarantined
GET    /api/devices/{slot}/channels
PUT    /api/devices/{slot}/channels/{ch}/setpoint    { volt, curr_limit }
PUT    /api/devices/{slot}/channels/{ch}/protection  { ovp, ocp }
POST   /api/devices/{slot}/channels/{ch}/output      { on: bool }

GET    /api/presets
POST   /api/presets
PUT    /api/presets/{id}
DELETE /api/presets/{id}

GET    /api/profiles
POST   /api/profiles
PUT    /api/profiles/{id}
DELETE /api/profiles/{id}
POST   /api/profiles/{id}/apply        { force?: bool }   -> stages setpoints only
POST   /api/profiles/{id}/power-on                        -> sequenced energisation
POST   /api/profiles/{id}/power-off

WS     /api/stream                     full state snapshot @ 2 Hz
```

### 13.1 Error model

Machine-readable codes in the body so the UI can render something specific rather
than dumping a string:

```json
{
  "code": "CLAMP_EXCEEDED",
  "message": "Requested 12.0 V exceeds configured limit of 5.5 V",
  "channel": "psu1/CH1",
  "limit": 5.5,
  "requested": 12.0
}
```

Codes: `CLAMP_EXCEEDED`, `CHANNEL_LIVE` (retry with `force`), `DEVICE_QUARANTINED`,
`DEVICE_OFFLINE`, `SCPI_TIMEOUT`, `VALIDATION_FAILED`, `PARTIAL_APPLY_REFUSED`.

---

## 14. UI scope

Build order chosen so a half-finished v1 is still useful.

1. **Bench** — all 9 channels live: V/I/P readouts, per-channel on/off, inline
   setpoint edit. Instantaneous values only, no trend display. Offline and
   unknown channels visually distinct from zero.
2. **Profiles** — browse, create, edit device profiles; apply one; power a
   device up in sequence, and power-cycle it.
3. **Rail settings** — CRUD on the channel-preset primitives.

This was originally four sections: **device health** was a fourth, carrying
online/offline/quarantined state, serial-mismatch and clamp-violation banners
and connection error reasons. It was folded into the bench, because a page
listing three supplies and their condition, separate from the page showing what
those supplies are doing, made you hold two views in your head to answer one
question. The health information did not shrink — it moved next to the readings
it explains.

The preset library is labelled **Rail settings** in the product. "Preset" is the
schema's word; what an operator saves is a rail's voltage and current limit
under a name.

Three decisions worth recording:

The profile editor derives its channel list from **live state**, not from a
hard-coded table, so a rail cannot be bound to a slot that is not configured and
each channel's operator limits are visible at the moment a preset is chosen. It
also warns *before* submission when a preset exceeds the chosen channel's limit
or a channel is bound twice — the server still refuses both, but there is no
value in letting someone assemble a profile that is certain to be rejected.

Editing a profile seeds the form once and then leaves it alone. A form that
re-synced from server state mid-edit would discard whatever the user was typing
every time a snapshot arrived — which, at 2 Hz, is constantly.

The editor is mounted only while its dialog is open, rather than keyed on the
profile being edited. Keying looked equivalent and was not: the key was
`editing?.id ?? 'new'`, which is the same string when the dialog is closed and
when it is opening for a *new* profile — so creating one and then pressing New
reused the component and everything in it, and the second form arrived
pre-filled with the profile just saved.

### 14.1 Component library

**Skeleton UI v5** with the built-in **Cerberus** theme. (Recorded as v3 when
this section was first written, from documentation that was a major version
behind; v5.0.1 is what is installed.) Skeleton is imported CSS-first alongside
Tailwind 4 and the theme is selected with a `data-theme` attribute on `<html>`,
so there is no theme build step and no runtime theming machinery to maintain.

Dark mode needs a second thing on the same element and it is easy to miss:
Tailwind 4's `dark` variant follows `prefers-color-scheme` unless told
otherwise, so `class="dark"` on `<html>` did nothing until
`@custom-variant dark (&:where(.dark, .dark *))` was declared. Until then the
app rendered its light palette on every machine whose OS was set to light,
whatever the theme attribute said.
Skeleton publishes a Vite + Svelte installation path, so adopting it does not
drag in SvelteKit — the SPA stays a plain Vite build whose output is embedded in
the binary.

The resulting bundle is roughly 135 KB of JavaScript and 155 KB of CSS, about
60 KB gzipped in total. (Recorded as 24 KB gzipped when this section was first
written, which was measured before Skeleton's own stylesheet and the profile and
settings views existed.) That is the concrete form of the trade made in §3.1
against Flutter Web, whose CanvasKit payload is measured in megabytes — still
two orders of magnitude apart.

Navigation is Skeleton's **Navigation in `rail` layout** — a vertical strip of
icon-plus-label triggers down the left edge, carrying a count of supplies needing
attention so a unit dropping out is visible while you are editing a profile, not
only while you happen to be looking at the bench. Channel output is Skeleton's
**SegmentedControl**, which shows both states at once with the current one
indicated, rather than two buttons that leave the state to be inferred. When a
supply is not answering the control has **no segment selected**: defaulting to
"Off" would assert something about a rail nobody can see.

Note that the anatomy components are styled by `@skeletonlabs/skeleton-common`,
which reaches the app through `@skeletonlabs/skeleton-svelte`'s stylesheet. The
base `@skeletonlabs/skeleton` import only carries utilities like `btn` and
`card`, so without that second import the components render as unstyled markup
with nothing to say why.

Two conventions worth fixing now, because a component library makes it easy to
drift:

- **Dark theme is the default.** This is a bench instrument read across a room.
- **Status colour is never the only signal.** Offline, `Unknown`, quarantined and
  clamp-violating channels each carry a text label as well as a colour. A rail
  whose state is unknown must not be distinguishable from a healthy one by hue
  alone — that is precisely the case where a misread is expensive.
- **One accent, meaning "active".** The green that marks a live rail is the same
  green that marks the selected section in the rail and the `On` segment of a
  channel's control. A second highlight colour meaning almost-but-not-quite the
  same thing is how a palette stops meaning anything. Everything else stays
  neutral, which is what lets the accent read as information rather than
  decoration.
- **On and off are shown, not said.** A live channel gets a coloured border and
  full opacity; an off one is dimmed. Both signals are non-hue — a border is
  either present or not, and dimming is a lightness change — so the distinction
  survives colour blindness without a badge. The exceptional states are exempt
  from this and keep their labels: nobody should have to infer "we do not know
  what this rail is doing" from styling. A channel needing attention is also
  never dimmed, so it cannot recede into the background.
- **Dividers separate kinds of thing, not every thing.** A rule sits between a
  channel's readings and its controls, between one supply and the next, and
  between a profile's identity and its rails — boundaries where the content
  changes character. Vertical rules separate the three readings, which are three
  quantities rather than one sentence. They are not used to subdivide lists,
  where spacing already does the work.

---

## 15. Build and deployment

Two-stage build driven by a `justfile` — **not** a `build.rs` that shells out to npm,
which would make `cargo check` slow and surprising:

```
just build-ui     # vite build -> ui/dist
just build        # build-ui, then cargo build --release
```

**Built on the target**, from a clone of this repository, rather than shipped as a
binary from a development machine. Copying a binary between distributions makes the
build host's glibc a silent deployment dependency, and the failure — the process not
starting at all — says nothing about this project. Building on the lab PC also puts a
toolchain where the supplies are, which the `hw_*` profile (§15.1) requires regardless.

The Svelte bundle is embedded via `rust-embed`, so what gets installed is still a
**single binary** with no runtime asset paths to get wrong, plus `config.toml`, the
udev rule and the systemd unit. The procedure is `deploy/README.md`.

---

### 15.1 Testing

Tests run under **`cargo-nextest`**, configured in `.config/nextest.toml`.
Process-per-test isolation matters here: the simulator holds locks and the
per-supply actors spawn tasks, so one wedged test must not be able to hang or
corrupt the rest of the run.

Three profiles:

- **`default`** — everything that runs against the simulator. No retries, on the
  grounds that these tests are deterministic by construction and a flaky one is a
  real bug; retrying would hide exactly the races the actor model is most likely
  to grow. A 5 s slow-timeout catches deadlocks, since nothing in this set should
  take more than milliseconds.
- **`ci`** — same set, with JUnit output.
- **`hardware`** — tests named `hw_*`, excluded from every other profile by
  `default-filter`. These need real DP832s and run on the lab PC. This is where
  the §11 open risks get settled: lock release behaviour including the `SIGKILL`
  case, sustained poll rate under concurrent command traffic, and `usbtmc`
  timeout and disconnect semantics on the target kernel.

Note that **nextest does not run doctests** — `just test-doc` covers those
separately, and `just test-all` runs both.

### 15.2 UI and end-to-end testing

Three layers, at deliberately different weights.

**Component tests — Vitest + `@testing-library/svelte`.** No browser, milliseconds
per test. This is where display logic belongs: that a channel in `Unknown` renders
as "Unknown" and never as `0.00 V`, that a clamp violation is shown as one, that
the sequencing editor orders rails correctly.

**End-to-end — Playwright against the real binary.** Playwright's `webServer`
config launches `cargo run -p server -- --simulate 3` before the suite and tears
it down afterwards, so the tests drive the actual stack — real server, real
WebSocket, real SPA — with nothing mocked and no hardware attached. The simulator
is what makes this cheap; without it, E2E would mean either a mocked API (testing
a fiction) or a bench (untestable in CI).

A simulated run keeps its presets and profiles **in memory**, so each suite —
and each restart of a dev server — starts from an empty store and cannot pick up
what a previous run left behind. That is the default, not a rule: passing
`--database <path>` alongside `--simulate` keeps them on disk, which is what you
want for a dev bench you return to. The server says which it is doing at
startup. `--database` on its own defaults to `/var/lib/rigol-psu/data.db`, the
path the systemd unit owns.

**Visual regression: deliberately not adopted.** A dashboard whose readings change
every 500 ms is a snapshot-flake generator, and the effort would go into masking
regions rather than catching defects.

#### Fault injection, and how it is gated

The UI states most worth testing are the ones that cannot be produced on demand: a
supply going offline mid-session, a channel reading `Unknown`, a quarantine
banner, a clamp violation on channels adopted after a restart. These are precisely
the states §7.3, §8 and §14.1 exist for, and precisely the ones that rot silently,
because in normal operation nobody ever sees them.

Reaching them from a browser test requires a control surface over
`SimulatedDp832::set_fault`, e.g. `POST /api/sim/{slot}/fault`. That is a useful
test hook and a dangerous endpoint, so it is gated **twice**:

1. **Compile-time**, behind a `simulation` Cargo feature. The release binary does
   not contain the routes, and `--simulate` is not available in it either.
2. **Runtime**, by refusing to mount the routes unless the server was actually
   started in simulation mode.

Belt-and-braces is warranted here. This server's entire safety posture rests on
the API being unable to be talked into something dangerous, and an endpoint that
fabricates instrument state deserves more than one lock — the feature flag keeps
it out of the shipped artifact, and the runtime check means that even a
simulation build cannot expose it while real supplies are attached.

**The cost, stated plainly:** CI builds the workspace twice, and a compile error
can hide in the non-default feature combination. `just lint` therefore checks both
the default and the `simulation` feature sets, so the second configuration cannot
quietly break.

---

## 16. Milestones

1. **M1 — Talk to a supply.** `dp832` crate: transport trait, SCPI codec, usbtmc
   implementation, simulator.

   *Done, against the simulator.* Reading `*IDN?` and `:MEASure:ALL?` from real
   hardware is still outstanding — see M6 — so nothing here has met an
   instrument yet.
2. **M2 — Server skeleton.** Config loading with fail-closed validation, per-PSU
   actors, polling, `GET /api/state`, WebSocket snapshots. Runs against
   `--simulate 3`, with the `simulation` feature and its fault-injection routes
   (§15.2) established here so M3's E2E harness has something to drive.

   *Done.*
3. **M3 — Dashboard.** Svelte SPA on Skeleton/Cerberus, live readouts, manual
   channel control, embedded and served. Vitest component tests and the Playwright
   E2E harness land with it. First genuinely useful build.

   *Done.*
4. **M4 — Safety.** Clamps, quarantine, adoption on restart, clamp-violation freeze,
   `force` semantics, error taxonomy. Tested end-to-end against the simulator.

   *Mostly landed with M2.* Clamp enforcement, quarantine, adoption and the
   error taxonomy were built into the actor rather than bolted on afterwards:
   they shape its control flow, and shipping write endpoints that skip them —
   even briefly — would have meant an interval where the API could be talked
   into something the config forbids. `force` and the OpenAPI spec were left to
   M5 — `force` because it only becomes meaningful once profiles exist (§9.2) —
   and both landed there.
5. **M5 — Presets and profiles.** SQLite, migrations, CRUD, staged apply, sequenced
   power-on, profiles UI.

   *Done.* Presets and profiles live in SQLite with migrations tracked in
   `PRAGMA user_version`. A preset that a profile depends on cannot be deleted
   (`ON DELETE RESTRICT`), because cascading would silently gut the profile.
   `force` is implemented and bounded to the single rule §10 gives it; the API
   reports `forceable` on a refusal so the UI offers the override only where it
   could actually apply.

   The **OpenAPI spec** outstanding from M4 is now done, served at
   `GET /api/openapi.json`. The schema derives live in `psu-core` and `dp832`
   behind an **optional `openapi` feature**: no types are mirrored in the server
   crate, so the document cannot drift from what the handlers return, and the
   domain crates still carry no web dependency for any consumer that does not
   ask for one. The alternative — restating every response type in the server —
   would have meant two definitions of each and a standing invitation for them
   to disagree.

   One property is worth guarding explicitly, and there is a test for it:
   `measurement`, `setpoint` and `output_on` must be documented as nullable. A
   schema claiming they are always present would lead a generated client to
   default them to zero, which is precisely the lie §7.3 exists to prevent.
6. **M6 — Deployment.** udev rule, systemd unit, front panel lock lifecycle,
   build and install on the target. Resolve the §11 risks on real hardware.
