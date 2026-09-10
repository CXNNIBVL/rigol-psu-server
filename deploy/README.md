# Deploying to the lab PC

Four things, built and installed **on the lab PC itself**:

| Artefact | Goes to |
|---|---|
| the binary | `/usr/local/bin/rigol-psu-server` |
| `config.toml` | `/etc/rigol-psu/` — the safety boundary; not reproducible from this repo |
| `deploy/99-rigol.rules` | `/etc/udev/rules.d/` — pins each supply to a stable name |
| `deploy/rigol-psu.service` | `/etc/systemd/system/` |

The database directory (`/var/lib/rigol-psu`) is created by the unit.

Build on the target, not on a dev machine: a binary built on a newer
distribution fails to start on the lab PC with a `GLIBC_2.xx not found` error
that names nothing in this project, and the `hw_*` tests can only run where the
supplies are. The lab PC therefore needs Rust, Node and a C compiler, plus
one-time access to crates.io and the npm registry — §1 has the offline path.

Two ordering rules: install the udev rule **before** starting the service, or it
comes up with every supply offline; and run every command from §2 onwards **from
the root of the clone**.

## 1. What the lab PC needs

Install as yourself — the account that will own the bench. No dedicated service
account is created anywhere in this procedure; the same login runs the build,
the tests and the service.

```sh
# Fedora / RHEL
sudo dnf install -y gcc make nodejs npm git

# Debian / Ubuntu
sudo apt install -y build-essential nodejs npm git

# Rust, for your own user — https://rustup.rs
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Task runner, and the test runner the hardware profile needs
cargo install just --locked
cargo install cargo-nextest --locked
```

| Needs | Why |
|---|---|
| Rust 1.85+ | The workspace `rust-version`. |
| `gcc`, `make` | `rusqlite` is built `bundled`, so SQLite compiles from source; without a C compiler the build dies inside `libsqlite3-sys`. |
| Node 22 + npm | The Svelte bundle, embedded into the binary. |
| `cargo-nextest` | `just test` and `just test-hardware` are nextest profiles; plain `cargo test` ignores `.config/nextest.toml`, including the `hw_*` exclusion. |

Budget a few gigabytes: `target/` and `ui/node_modules` run to around 7 GB after
a full build with tests.

**If the lab PC has no route to crates.io or npm**, prepare the tree on a
machine that does and copy it across:

```sh
# on a networked machine, in a clone of this repo
mkdir -p .cargo && cargo vendor vendor/ >> .cargo/config.toml
(cd ui && npm ci)
rsync -a --exclude target/ ./ lab:~/rigol-psu-server/
```

Then build with `just build-ui-quick && cargo build --release` rather than
`just build`, which reinstalls `node_modules`. `node_modules` holds
platform-specific binaries, so both machines must have the same OS and
architecture.

## 2. The source

```sh
git clone ssh://git@git.ksat-stuttgart.de:443/source/src_collab-projects/rigol-psu-server.git
cd rigol-psu-server
git log -1 --format='%H %s'        # record what you are about to deploy
```

`~/rigol-psu-server` is fine; the build never needs a privileged location. If
the lab PC cannot reach the Git server, `rsync` as in §1 — but prefer the clone,
which records exactly which commit is running.

## 3. Device names

The kernel assigns `/dev/usbtmcN` in enumeration order, which changes across
reboots and replugs. With three identical DP832s the numbering can rotate
silently and send a profile to the wrong supply. The udev rule pins each unit to
a fixed name by serial number.

First confirm the `usbtmc` driver is bound — `lsusb` listing the supplies does
not tell you this:

```sh
ls -l /dev/usbtmc*        # one node per supply
lsmod | grep usbtmc
```

No nodes: `sudo modprobe usbtmc` and replug. Then find the serials with the
supplies connected:

```sh
for dev in /dev/usbtmc*; do
  printf '%s  ' "$dev"
  udevadm info -a -n "$dev" | grep -m1 'ATTRS{serial}'
done
```

Edit `deploy/99-rigol.rules`, replace the placeholder serials, install it:

```sh
sudo cp deploy/99-rigol.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
ls -l /dev/rigol-psu*        # one symlink per supply should now exist
```

Each symlink should point at the `usbtmc` node whose serial you matched —
`rigol-psu1 -> usbtmc0` and so on. The number behind the arrow may move between
reboots; the name in front of it may not.

The rule sets `MODE="0666"`, so the nodes are open to every local account: there
is no group to create and no service account to add, and that is also what lets
the hardware tests run as whoever is logged in. The service runs as an ordinary
existing account, named by `User=` in the unit.

## 4. Configuration

```sh
sudo install -d -o rigol -g rigol /etc/rigol-psu
sudo cp config.example.toml /etc/rigol-psu/config.toml
sudo chown root:root /etc/rigol-psu/config.toml
sudo chmod 644 /etc/rigol-psu/config.toml
```

Three things must be right before the service will start:

- **`bind`** has no default: v1 has no authentication, so exposing the service
  must be a deliberate act. `0.0.0.0` is not one of the sensible choices.

  **A network address** — `bind = "192.168.x.y:8080"`, the machine's address on
  the network the bench should be usable from. Anyone who can reach it opens it
  in a browser with no per-person setup — and can also drive it.

  **Loopback plus an SSH tunnel** — `bind = "127.0.0.1:8080"`. Nothing is
  exposed to the network; access control becomes "who has an account on this
  machine", at the cost of a tunnel per person.

  Either way this is placement, not authentication: it narrows who can reach the
  bench, does not identify them, and there is no audit trail in v1.

  An IPv6 address goes in brackets in both places — `bind = "[2001:db8::1]:8080"`,
  browsed as `http://[2001:db8::1]:8080`; without brackets the config does not
  parse. A global v6 address is routable as it stands, with no NAT in front of
  it, so "it is only an internal address" does not follow the way it might for
  `192.168.x.y`. §7 has a two-command check for what can actually reach it.

  The address must exist on an interface of this machine (`ip -br addr` lists
  them). Binding to an address that is not up fails with `Cannot assign
  requested address`; see §7.
- **`serial`** for each supply must match what the instrument reports. A
  mismatch quarantines that supply — readable and configurable, but outputs
  cannot be switched on until the file is corrected and the service restarted.
  There is no way to clear that from the UI.
- **Per-channel limits** cannot be raised through the API and are not editable
  from the UI — the whole reason they live here. The server refuses to start if
  they are missing.

Keep the finished file in version control. It is the safety boundary, and the
one piece of the deployment that is not reproducible from this repository.

## 5. Build, and check before installing

```sh
just build          # npm ci, vite build, then cargo build --release
```

The first build is slow — SQLite compiles from source and the dependency graph
is cold. Later builds are quick.

Then check what you built, **before** it replaces the running binary:

```sh
just test           # the simulator suite; touches no hardware
./target/release/rigol-psu-server --help
```

`just test` is safe to run at any time, service up or down.

### Looking at the UI without touching the supplies

The release binary does **not** contain `--simulate` or the fault-injection
routes: the `simulation` feature is off by default so the shipped binary cannot
fabricate instrument state. A build that does have it must go elsewhere:

```sh
cargo run --release --target-dir target/sim \
  -p server --features simulation -- --simulate 3 --bind 127.0.0.1:18080
```

**`--target-dir` is not optional here.** Both builds write
`target/release/rigol-psu-server`, so building the simulation one in the default
directory silently overwrites the binary you are about to install — with fault
injection compiled in and no sign of it. `target/sim` makes that impossible.

This is a different feature set from what you are installing. It confirms the UI
bundle and the stack; the real check is §7.

### Hardware tests

```sh
sudo systemctl stop rigol-psu     # the service holds the device handles
just test-hardware
```

Stop the service first: two processes with the supplies open at once fails in
ways that look like a flaky instrument.

There are no `hw_*` tests in the tree yet. The profile is prepared for the open
hardware risks — lock release including the `SIGKILL` case, sustained poll rate,
`usbtmc` timeout and disconnect semantics on this kernel — and reports zero
tests until they are written. That is expected.

## 6. Install the binary and the service

Keep the outgoing binary — it is the whole rollback plan and costs one `cp`:

```sh
sudo cp -f /usr/local/bin/rigol-psu-server /usr/local/bin/rigol-psu-server.prev 2>/dev/null || true
sudo install -o root -g root -m 755 target/release/rigol-psu-server /usr/local/bin/

sudo cp deploy/rigol-psu.service /etc/systemd/system/
sudo sed -i "s/^User=.*/User=$USER/" /etc/systemd/system/rigol-psu.service
sudo systemctl daemon-reload
sudo systemctl enable --now rigol-psu
systemctl status rigol-psu
```

The unit creates `/var/lib/rigol-psu` for the database via `StateDirectory`, so
nothing else needs setting up.

That `sed` is the whole account story: it points the unit at the login you
already use. The unit ships `User=root` so a copy without the substitution still
runs; the substitution drops it to ordinary privileges. Redo it after any later
`cp` of the unit, or use a drop-in (`sudo systemctl edit rigol-psu`) so the
choice survives on its own.

If you change the account after the first start, `chown` `/var/lib/rigol-psu` to
match — `StateDirectory` creates it owned by whoever `User=` names, and a
mismatch surfaces as a permission error on `data.db`.

The unit is confined: `ProtectSystem=strict`, a `StateDirectory`,
`DeviceAllow=char-usb`. If a change ever needs the server to write somewhere
new, it needs a `ReadWritePaths` line too, or the write fails with an error that
looks nothing like a permissions problem.

## 7. Checking it works

On the lab PC, against the address you configured:

```sh
journalctl -u rigol-psu -f
curl -s http://192.168.x.y:8080/api/state | head
```

Each supply should report `"state": "online"` with a `reported_serial` matching
its configured one. A supply switched off appears as `offline` and the service
still starts — refusing to boot over one unplugged unit would take the whole
bench down.

Then open it in a browser: confirm the UI loads and the readings move. A stale
or missing bundle is invisible from `curl`.

### Reaching it from another machine

Open `http://192.168.x.y:8080` from a machine that can reach that address. Plain
HTTP, no proxy, no CORS: the SPA and the API share an origin and port, and the
live readings arrive over a WebSocket whose URL the page derives from the
address bar.

If the browser hangs while `curl` on the bench PC is happy, one probe tells you
which of two things it is. Aim a connection at a port where **nothing** is
listening:

```sh
nc -z -v <address> <an-unused-port>
```

Instant `connection refused`: your packets arrive, the host is fine, the problem
is local — a host firewall on the service's port, or a bind to the wrong
address. Silent timeout: they never arrive, and nothing on the bench PC will
change that.

**If packets arrive: open the port**, restricted to the network the bench should
be usable from:

```sh
# firewalld — `firewall-cmd --get-active-zones` names the zone for that interface
sudo firewall-cmd --permanent --zone=<zone> --add-port=8080/tcp
sudo firewall-cmd --reload

# ufw
sudo ufw allow in on <iface> to any port 8080 proto tcp
```

**If they do not arrive: something upstream is dropping them.** Before asking
for a network-rule change, find what already passes — probe a spread of ports
the same way, `refused` = permitted through, `timeout` = dropped. Moving `bind`
to a port that already works is free. Stay above 1024, or the unit needs
`AmbientCapabilities=CAP_NET_BIND_SERVICE`.

### When the address is not up yet at boot

Binding needs the address to exist. If something brings it up after the service
— a VPN client, a slow DHCP lease, IPv6 duplicate-address detection — the server
loses the race and exits:

```
Error: Os { code: 99, kind: AddrNotAvailable, message: "Cannot assign requested address" }
```

`Restart=on-failure` with `RestartSec=5` retries until the address appears, so
this is self-healing — it just leaves a confusing minute of failures in the
journal after every boot. To silence it, order the unit after whatever brings
the address up, as a drop-in:

```sh
sudo systemctl edit rigol-psu
```

```ini
[Unit]
After=wg-quick@wg0.service
Wants=wg-quick@wg0.service
```

Name your own unit there — `systemctl list-units --type=service` will find it.

### Reaching a loopback bind from your own machine

```sh
ssh -N -L 8080:127.0.0.1:8080 lab      # leave running
```

Then open <http://localhost:8080>. The SPA derives its WebSocket URL from the
address bar, so the live readings work through the tunnel with nothing to
configure. Forward to the same local port where you can: nothing breaks
otherwise (`-L 9000:127.0.0.1:8080` is fine), but matching ports keeps the
address in your browser meaning the same as the one in `config.toml`.

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
and nothing else.

Reinstall the unit and `daemon-reload` only if it changed —
`git log --oneline <old>..<new> -- deploy/` after a pull will say. Same for
`99-rigol.rules`, which also needs `udevadm control --reload-rules`.

Stopping the service **does not de-energise anything**. Outputs stay as they are
and are adopted again on the way back up, so an upgrade does not end a running
experiment. Front panels are released on `SIGTERM`; the unit allows 15 seconds,
which is why `TimeoutStopSec` should not be shortened.

Database migrations run at startup, in a transaction, with the result checked
for dangling references — a failed migration refuses to open the database rather
than running against a half-changed schema. The `data.db` copy above is still
worth taking: it holds the presets and profiles, the only irreplaceable state.

## 9. Rolling back

```sh
sudo systemctl stop rigol-psu
sudo install -o root -g root -m 755 /usr/local/bin/rigol-psu-server.prev /usr/local/bin/rigol-psu-server
sudo systemctl start rigol-psu
```

Seconds, and it needs no toolchain, network or working build. Rebuilding an old
commit (`git checkout <sha> && just build`) is the fallback when the previous
binary is gone.

A rollback across a migration is the one case this does not cover: an older
binary may not understand a schema a newer one has migrated. Restore the
pre-upgrade `data.db` alongside the old binary, with the service stopped.

## When something is wrong

**A supply is quarantined.** Its serial does not match `config.toml`. Either the
units were swapped and the file needs updating, or the udev rule is pointing at
the wrong device. Check the Metadata panel on that supply in the UI: it shows
the configured and reported serials side by side.

**Every supply is offline.** Usually the udev rule has not been applied, so the
symlinks the config points at do not exist. `ls -l /dev/rigol-psu*` says
whether they are there, and `journalctl -u rigol-psu` names the path it could
not open. If the symlinks exist but the service still cannot open them, check
their mode — the rule sets `0666`, and a stricter one somewhere else in
`/etc/udev/rules.d/` may be winning.

**No `/dev/rigol-psu*` symlinks, but `/dev/usbtmc*` exists.** The rule is not
matching. The usual cause is `ATTRS{iSerial}` instead of `ATTRS{serial}`:
`iSerial` is what `lsusb -v` calls the field, it does not exist in sysfs, and
udev fires no warning for a match on an attribute that is not there. Compare
your rule against what udev actually reports:

```sh
udevadm info -a -n /dev/usbtmc0 | grep -m1 'ATTRS{serial}'
sudo udevadm test /sys/class/usbmisc/usbtmc0 2>&1 | grep -iE 'rigol|symlink'
```

**The service starts degraded: `could not open supply ... Operation not
permitted`.** Note which error this is. `Operation not permitted` (EPERM) is the
device cgroup refusing the open, and the mode on the node has nothing to do with
it — `0666` will not help. It means `DeviceAllow=` in the unit does not cover
these devices:

```sh
systemctl show rigol-psu -p DeviceAllow      # expect: char-usb rw
grep -E '^ *180 ' /proc/devices              # the group name for the usbtmc nodes
```

`DeviceAllow=` takes a device group name from `/proc/devices` (`usb`, for major
180), not the udev subsystem name (`usbmisc`). They are different strings for
the same hardware, and the wrong one fails silently at unit-load and loudly at
every open.

`Permission denied` (EACCES) is the other error and a different problem: that one
really is the mode or ownership on `/dev/rigol-psu*`.

**No `/dev/usbtmc*` at all, though `lsusb` lists the supplies.** The `usbtmc`
driver has not bound. `sudo modprobe usbtmc`, then replug or
`sudo udevadm trigger --subsystem-match=usbmisc`.

**The UI answers on the bench PC but a remote browser times out.** A host
firewall or something upstream is dropping the port. §7 has the one-command
probe that tells you which, and what to do in either case.

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
in `bind` does not exist on any interface, or did not exist yet when the service
started. See §7.

**The service fails to start with a config error.** It is fail-closed on
configuration by design: without a config there are no clamps, and without
clamps nothing should be controllable. `journalctl -u rigol-psu -n 20` names the
field.

**`just test-hardware` reports flaky instruments.** The service is still running
and holding the device handles. Stop it first; see §5.
