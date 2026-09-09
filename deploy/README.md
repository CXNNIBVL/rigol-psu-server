# Deploying to the lab PC

Three files, a database directory, and a binary built **on the lab PC itself**.
Everything the server needs at runtime is either in this directory or created by
it.

Building on the target rather than shipping a binary from a development machine
buys two things. There is no glibc skew: a binary built on a newer distribution
than the target does not start at all, and it fails before any of this project's
code runs, with an error about `GLIBC_2.xx not found` that says nothing about
the server. And the `hw_*` tests can only run where the supplies are — the lab
PC is the only place the open risks in §11 of the plan can be settled, and that
needs a toolchain there anyway.

The cost is that the lab PC needs Rust, Node and a C compiler, and needs to
reach crates.io and the npm registry at least once. See §1 if it cannot.

Two things about the order below. The udev rule has to exist before the service
starts, or the server will find no devices and come up with every supply
offline. And every command from §2 onwards is run **from the root of the clone**,
not from this directory.

## 1. What the lab PC needs

Install as yourself — not as root beyond the package manager, and **not** as the
`rigol` service user, which is `nologin` and owns nothing but the runtime.

```sh
# Fedora / RHEL
sudo dnf install -y gcc make nodejs npm git

# Debian / Ubuntu
sudo apt install -y build-essential nodejs npm git

# Rust, for your own user — https://rustup.rs
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# The task runner, and the test runner the hardware profile needs
cargo install just --locked
cargo install cargo-nextest --locked
```

| | Why |
|---|---|
| Rust 1.85+ | The workspace `rust-version`. |
| `gcc`, `make` | `rusqlite` is built `bundled`, so SQLite is compiled from source. Without a C compiler the build dies inside `libsqlite3-sys` with an error that never says "install gcc". |
| Node 22 + npm | The Svelte bundle, which is embedded into the binary. |
| `cargo-nextest` | `just test` and `just test-hardware` are nextest profiles; plain `cargo test` ignores the configuration in `.config/nextest.toml`, including the `hw_*` exclusion. |

Budget a few gigabytes of disk: `target/` and `ui/node_modules` together run to
around 7 GB after a full build with tests.

**If the lab PC has no route to crates.io or npm**, prepare the tree on a
machine that does and copy it across:

```sh
# on a networked machine, in a clone of this repo
mkdir -p .cargo && cargo vendor vendor/ >> .cargo/config.toml
(cd ui && npm ci)
rsync -a --exclude target/ ./ lab:~/rigol-psu-server/
```

Then build on the target with `just build-ui-quick && cargo build --release`
rather than `just build`, which would try to reinstall `node_modules`. Note that
`node_modules` contains platform-specific binaries (esbuild, and Rollup's native
module), so the two machines must have the same OS and architecture for this to
work at all.

## 2. The source

Clone it as your own user. `~/rigol-psu-server` is as good a place as any; the
build never needs to live anywhere privileged.

```sh
git clone ssh://git@git.ksat-stuttgart.de:443/source/src_collab-projects/rigol-psu-server.git
cd rigol-psu-server
git log -1 --format='%H %s'        # record what you are about to deploy
```

If the lab PC cannot reach the Git server, `rsync` as in §1. Prefer the clone
where you can: it records exactly which commit is running, which is the first
question anyone asks when the bench misbehaves.

## 3. Device names

The kernel assigns `/dev/usbtmcN` in enumeration order, which changes across
reboots and replugs. With three identical DP832s that means the numbering can
rotate silently, and a profile intended for one supply would be applied to
another. The rule pins each unit to a fixed name by its serial number.

Find the serials with the supplies connected:

```sh
for dev in /dev/usbtmc*; do
  printf '%s  ' "$dev"
  udevadm info -a -n "$dev" | grep -m1 'ATTRS{serial}'
done
```

Edit `deploy/99-rigol.rules`, replacing the placeholder serials, then install it:

```sh
sudo cp deploy/99-rigol.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
ls -l /dev/rigol-psu*        # three symlinks should now exist
```

The rule grants the `rigol` group access, so create it and add the service user:

```sh
sudo groupadd -f rigol
sudo useradd -r -g rigol -s /usr/sbin/nologin rigol || true
```

Add **yourself** to that group as well if you intend to run `just test-hardware`
— those tests open the device nodes directly, as you, and the nodes are
`0660 root:rigol`:

```sh
sudo usermod -aG rigol "$USER"     # log out and back in for it to take effect
```

## 4. Configuration

```sh
sudo install -d -o rigol -g rigol /etc/rigol-psu
sudo cp config.example.toml /etc/rigol-psu/config.toml
sudo chown root:root /etc/rigol-psu/config.toml
sudo chmod 644 /etc/rigol-psu/config.toml
```

Then edit it. Three things must be right before the service will start:

- **`bind`** has no default, because there is no authentication in v1 (§6 of
  the plan) and exposing the service must therefore be a deliberate act. Two
  postures make sense; `0.0.0.0` is neither of them.

  **A VPN address** — `bind = "192.168.x.y:2410"`. Anyone on the VPN opens that
  address in a browser and gets the bench, with nothing to set up per person.
  Everyone who can reach it can also drive it.

  **Loopback plus an SSH tunnel** — `bind = "127.0.0.1:2410"`. Nothing is
  exposed to the network at all, and access control becomes "who has an account
  on the lab PC", at the cost of a tunnel per person.

  Either way this is placement, not authentication. It narrows who can get to
  the bench; it does not identify them, and there is no audit trail in v1.

  Whichever you pick, the address must actually exist on an interface of the lab
  PC — `ip -br addr` lists them. Binding to an address that is not up fails with
  `Cannot assign requested address` and the process exits; see §7.
- **`serial`** for each supply must match what the instrument reports. A
  mismatch quarantines that supply: it can be read and configured, but its
  outputs cannot be switched on until this file is corrected and the service
  restarted. There is no way to clear that from the UI.
- **Per-channel limits** cannot be raised through the API and are not editable
  from the UI. That is the whole reason they live here. The server refuses to
  start if the file is missing.

Keep the finished file in version control. It is the safety boundary, and it is
the one piece of the deployment that is not reproducible from this repository.

## 5. Build, and check before installing

```sh
just build          # npm ci, vite build, then cargo build --release
```

The first build takes a while — SQLite is compiled from source and the whole
dependency graph is cold. Later builds are quick.

Then check what you just built, **before** it replaces the running binary:

```sh
just test           # the simulator suite; touches no hardware
./target/release/rigol-psu-server --help
```

`just test` is safe to run at any time, service up or down, and is the cheapest
signal that the build is sound.

### Looking at the UI without touching the supplies

The release binary does **not** contain `--simulate` or the fault-injection
routes: the `simulation` feature is off by default precisely so the shipped
binary cannot fabricate instrument state. Building one that does is fine, but it
must go somewhere else:

```sh
cargo run --release --target-dir target/sim \
  -p server --features simulation -- --simulate 3 --bind 127.0.0.1:18080
```

**`--target-dir` is not optional here.** Both builds write to
`target/release/rigol-psu-server`, so building the simulation one in the default
directory silently overwrites the binary you were about to install — and you
would install a server with fault injection compiled in and never see a sign of
it. Keeping it in `target/sim` makes that impossible.

Even so, this is a different feature set from what you are installing. It
confirms the UI bundle and the stack; it is not a test of the artifact. The real
check is §7.

### Hardware tests

Now that there is a toolchain on the bench, the `hw_*` profile can run:

```sh
sudo systemctl stop rigol-psu     # the service holds the device handles
just test-hardware
```

Stop the service first. Two processes with the supplies open at once is not a
configuration this system is designed for, and the failure looks like a flaky
instrument rather than a mistake.

There are no `hw_*` tests in the tree yet — the profile is prepared for the §11
open risks (lock release including the `SIGKILL` case, sustained poll rate,
`usbtmc` timeout and disconnect semantics on this kernel), and the run reports
zero tests until they are written. That is expected, not a broken setup.

## 6. Install the binary and the service

Keep the outgoing binary. It is the whole rollback plan, and it costs one `cp`:

```sh
sudo cp -f /usr/local/bin/rigol-psu-server /usr/local/bin/rigol-psu-server.prev 2>/dev/null || true
sudo install -o root -g root -m 755 target/release/rigol-psu-server /usr/local/bin/

sudo cp deploy/rigol-psu.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now rigol-psu
systemctl status rigol-psu
```

The unit creates `/var/lib/rigol-psu` for the presets and profiles database via
`StateDirectory`, so nothing else needs setting up.

The service runs as `rigol`, confined by the hardening block in the unit:
`ProtectSystem=strict`, a `StateDirectory`, and `DeviceAllow=char-usbmisc`. It
has its device nodes, its config and its database, and nothing else. If a change
ever needs the server to write somewhere new, it needs a `ReadWritePaths` line
too — otherwise the write fails with an error that looks nothing like a
permissions problem.

## 7. Checking it works

On the lab PC first, against the address you configured:

```sh
journalctl -u rigol-psu -f
curl -s http://192.168.x.y:2410/api/state | head
```

Each supply should report `"state": "online"` with a `reported_serial` matching
its configured one. A supply that is switched off appears as `offline` and the
service still starts — refusing to boot over one unplugged unit would take the
whole bench down.

Then open it in a browser and confirm the UI loads and the readings move. A
stale or missing bundle is invisible from `curl`.

### Reaching a VPN bind from a browser

Open `http://192.168.x.y:2410` from any machine on the VPN. Plain HTTP, no
proxy, no CORS to configure: the SPA and the API are served from the same origin
and port, and the live readings arrive over a WebSocket whose URL the page
derives from whatever is in the address bar.

Two things stop this working, and neither of them looks like the other:

**The port is not open.** The service is listening but the lab PC's firewall
drops the connection, so the browser hangs and then times out while `curl` on
the lab PC itself is perfectly happy. Open it on the interface the VPN uses,
not globally:

```sh
# firewalld (Fedora / RHEL) — check which zone the VPN interface is in first
firewall-cmd --get-active-zones
sudo firewall-cmd --permanent --zone=<zone> --add-port=2410/tcp
sudo firewall-cmd --reload

# ufw (Debian / Ubuntu)
sudo ufw allow in on <vpn-iface> to any port 2410 proto tcp
```

**The VPN interface is not up when the service starts.** Binding to a specific
address requires that address to exist, so on a reboot the server can lose the
race with the VPN client and exit immediately:

```
Error: Os { code: 99, kind: AddrNotAvailable, message: "Cannot assign requested address" }
```

`Restart=on-failure` with `RestartSec=5` means it keeps retrying and comes up on
its own once the interface appears, so this is self-healing rather than fatal —
but it leaves a confusing minute of failures in the journal after every boot.
The tidy fix is to order the unit after whatever brings the VPN up, with a drop-in
rather than an edit to the shipped unit file:

```sh
sudo systemctl edit rigol-psu
```

```ini
[Unit]
After=wg-quick@wg0.service
Wants=wg-quick@wg0.service
```

Substitute your own VPN unit — `systemctl list-units --type=service | grep -iE 'wg|vpn|openvpn'` will name it.

### Reaching a loopback bind from your own machine

```sh
ssh -N -L 2410:127.0.0.1:2410 lab      # leave running
```

Then open <http://localhost:2410>. The SPA derives its WebSocket URL from
whatever is in the browser's address bar, so the live readings work through the
tunnel with nothing to configure and no CORS surface — the UI and the API are
served from the same origin and port by design.

Forward to the same local port number where you can. Nothing breaks if you do
not (`-L 9000:127.0.0.1:2410` is fine), but matching ports makes the address in
your browser mean the same thing as the address in `config.toml`, which is worth
something at 2 a.m.

## 8. Upgrading

```sh
cd ~/rigol-psu-server
git pull
just build
just test

sudo cp /var/lib/rigol-psu/data.db ~/data.db.$(date +%F)
sudo systemctl stop rigol-psu
sudo cp -f /usr/local/bin/rigol-psu-server /usr/local/bin/rigol-psu-server.prev
sudo install -o root -g root -m 755 target/release/rigol-psu-server /usr/local/bin/
sudo systemctl start rigol-psu
```

Build and test *before* stopping the service: a compile error should cost time
and nothing else, and the bench should not sit dead waiting for `cargo build`.

Reinstall `deploy/rigol-psu.service` and `daemon-reload` only if the unit
changed — `git log --oneline <old>..<new> -- deploy/` after a pull will say. The
same goes for `99-rigol.rules`, which additionally needs
`udevadm control --reload-rules`.

Stopping the service **does not de-energise anything**. Outputs stay exactly as
they are and are adopted again on the way back up, so an upgrade does not end a
running experiment. The front panels are released on `SIGTERM`; the unit allows
15 seconds for that, which is why `TimeoutStopSec` should not be shortened.

Database migrations run at startup. They are applied in a transaction and the
result is checked for dangling references, so a failed migration refuses to open
the database rather than running against a half-changed schema. The copy above
is worth taking anyway: `data.db` holds the presets and profiles, which are the
only irreplaceable state in the system.

## 9. Rolling back

```sh
sudo systemctl stop rigol-psu
sudo install -o root -g root -m 755 /usr/local/bin/rigol-psu-server.prev /usr/local/bin/rigol-psu-server
sudo systemctl start rigol-psu
```

Seconds, and it needs no toolchain, no network and no working build. Rebuilding
an old commit (`git checkout <sha> && just build`) is the fallback when the
previous binary is gone, but that is minutes rather than seconds and the bench
is down for all of them.

A rollback across a migration is the one case this does not cover: an older
binary may not understand a schema a newer one has already migrated. That is
what the pre-upgrade copy of `data.db` is for — restore it alongside the old
binary, with the service stopped.

## When something is wrong

**A supply is quarantined.** Its serial does not match `config.toml`. Either the
units were swapped and the file needs updating, or the udev rule is pointing at
the wrong device. Check the Metadata panel on that supply in the UI: it shows
the configured and reported serials side by side.

**Every supply is offline.** Usually the udev rule has not been applied, or the
service user is not in the `rigol` group. `ls -l /dev/rigol-psu*` and
`id rigol` will say which.

**A channel is frozen after a restart.** It was found live with setpoints
outside its configured limits. The server deliberately leaves it energised —
a restart must not end someone's experiment — and refuses writes to it until a
human resolves it, either by correcting the limits or by switching that channel
off.

**The build dies in `libsqlite3-sys`.** No C compiler. See §1.

**The build dies in `npm ci`, or `vite: not found`.** No route to the npm
registry, or `node_modules` was never installed. See §1 for the offline path.

**The UI says "The UI has not been built".** `ui/dist` was empty when the binary
was compiled, so `build.rs` embedded a placeholder page. You built with
`cargo build --release` directly instead of `just build`. Rebuild with
`just build` and reinstall.

**The service will not start: `Cannot assign requested address`.** The address
in `bind` does not exist on any interface — usually the VPN is not up yet. See
§7.

**The UI is reachable from the lab PC but not over the VPN.** Firewall, almost
always. See §7.

**The service fails to start with a config error.** It is fail-closed on
configuration by design: without a config there are no clamps, and without
clamps nothing should be controllable. `journalctl -u rigol-psu -n 20` names the
field.

**`just test-hardware` reports flaky instruments.** The service is still running
and holding the device handles. Stop it first; see §5.
