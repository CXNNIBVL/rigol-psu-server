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

Install as yourself — the account that will own the bench. Nothing here needs
root beyond the package manager, and no dedicated service account is created at
any point: the same login runs the build, the tests and the service.

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

First confirm the kernel has bound the `usbtmc` driver — without a character
device there is nothing for a rule to link to, and `lsusb` showing the supplies
does not tell you this:

```sh
ls -l /dev/usbtmc*        # one node per supply
lsmod | grep usbtmc
```

If there are no nodes, `sudo modprobe usbtmc` and replug. Then find the serials
with the supplies connected:

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
ls -l /dev/rigol-psu*        # one symlink per supply should now exist
```

Each symlink should point at the `usbtmc` node whose serial you matched on —
`rigol-psu1 -> usbtmc0` and so on. The numbering behind the arrow is the part
that is allowed to move between reboots; the name in front of it is not.

The rule sets `MODE="0666"`, so the nodes are open to every local account. There
is **no group to create and no service account to add** — device access here is
a matter of the mode on the node, not of membership. That is also what lets the
hardware tests run as whoever is logged in.

The service therefore runs as an ordinary existing account, named by `User=` in
the unit. Nothing in this procedure creates a user.

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

  **A network address** — `bind = "192.168.x.y:8080"`, whatever the machine's
  address is on the network the bench should be usable from. Anyone who can
  reach it opens it in a browser, with nothing to set up per person, and anyone
  who can reach it can also drive it.

  **Loopback plus an SSH tunnel** — `bind = "127.0.0.1:8080"`. Nothing is
  exposed to the network at all, and access control becomes "who has an account
  on this machine", at the cost of a tunnel per person.

  Either way this is placement, not authentication. It narrows who can get to
  the bench; it does not identify them, and there is no audit trail in v1.

  An IPv6 address goes in brackets in both places — `bind =
  "[2001:db8::1]:8080"`, browsed as `http://[2001:db8::1]:8080`. Without them
  the config does not parse. Be deliberate about which v6 address: a global one
  is routable as it stands, with no NAT in front of it the way a private v4
  range has, so "it is only an internal address" does not follow the way it
  might for `192.168.x.y`. Whether anything outside can actually reach it is a
  question about the network, not the machine — §7 has a two-command way to
  find out.

  Whichever you pick, the address must actually exist on an interface of this
  machine — `ip -br addr` lists them. Binding to an address that is not up fails with
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
sudo sed -i "s/^User=.*/User=$USER/" /etc/systemd/system/rigol-psu.service
sudo systemctl daemon-reload
sudo systemctl enable --now rigol-psu
systemctl status rigol-psu
```

The unit creates `/var/lib/rigol-psu` for the presets and profiles database via
`StateDirectory`, so nothing else needs setting up.

That `sed` is the whole account story: it points the unit at the login you are
already using, and no user or group is created anywhere in this procedure. The
unit ships `User=root` so that a copy without the substitution still runs; the
substitution is what drops it to ordinary privileges.

Redo the `sed` after any later `cp` of the unit, or use a drop-in
(`sudo systemctl edit rigol-psu`) so the choice survives on its own.

If you change the account after the first start, `chown` the state directory to
match.
`StateDirectory=rigol-psu` creates `/var/lib/rigol-psu` owned by whoever `User=`
names, and a mismatch surfaces as a permission error on `data.db` that reads
nothing like an ownership problem.

The service is confined by the hardening block in the unit:
`ProtectSystem=strict`, a `StateDirectory`, and `DeviceAllow=char-usb`. It
has its device nodes, its config and its database, and nothing else. If a change
ever needs the server to write somewhere new, it needs a `ReadWritePaths` line
too — otherwise the write fails with an error that looks nothing like a
permissions problem.

## 7. Checking it works

On the lab PC first, against the address you configured:

```sh
journalctl -u rigol-psu -f
curl -s http://192.168.x.y:8080/api/state | head
```

Each supply should report `"state": "online"` with a `reported_serial` matching
its configured one. A supply that is switched off appears as `offline` and the
service still starts — refusing to boot over one unplugged unit would take the
whole bench down.

Then open it in a browser and confirm the UI loads and the readings move. A
stale or missing bundle is invisible from `curl`.

### Reaching it from another machine

Open `http://192.168.x.y:8080` from a machine that can reach that address. Plain
HTTP, no proxy, no CORS to configure: the SPA and the API are served from the
same origin and port, and the live readings arrive over a WebSocket whose URL
the page derives from whatever is in the address bar.

If the browser hangs while `curl` on the bench PC is perfectly happy, it is one
of two things, and one probe tells you which. Aim a connection at a port on the
same host where **nothing** is listening:

```sh
nc -z -v <address> <an-unused-port>
```

An instant `connection refused` means your packets arrive and the host is fine;
the problem is local — a host firewall rejecting the service's port, or a bind
to the wrong address. A silent timeout means they never arrive, and nothing you
configure on the bench PC will change that.

**If packets arrive: open the port.** Restrict it to the network the bench
should be usable from rather than opening it globally.

```sh
# firewalld — `firewall-cmd --get-active-zones` names the zone for that interface
sudo firewall-cmd --permanent --zone=<zone> --add-port=8080/tcp
sudo firewall-cmd --reload

# ufw
sudo ufw allow in on <iface> to any port 8080 proto tcp
```

**If they do not arrive: something upstream is dropping them.** Before asking
anyone to change a network rule, find out what already passes — probe a spread
of ports the same way, where `refused` means permitted through and `timeout`
means dropped. Networks commonly allow some ranges and not others, and moving
`bind` to a port that already works is free. Stay above 1024, or the unit needs
`AmbientCapabilities=CAP_NET_BIND_SERVICE` to bind it.

### When the address is not up yet at boot

Binding to a specific address requires that address to exist. If it is brought
up by something that runs after the service — a VPN client, a slow DHCP lease,
IPv6 duplicate-address detection — the server loses the race and exits:

```
Error: Os { code: 99, kind: AddrNotAvailable, message: "Cannot assign requested address" }
```

`Restart=on-failure` with `RestartSec=5` retries until the address appears, so
this is self-healing rather than fatal — it just leaves a confusing minute of
failures in the journal after every boot. To silence it, order the unit after
whatever brings the address up, as a drop-in rather than an edit to the shipped
unit:

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

Then open <http://localhost:8080>. The SPA derives its WebSocket URL from
whatever is in the browser's address bar, so the live readings work through the
tunnel with nothing to configure and no CORS surface — the UI and the API are
served from the same origin and port by design.

Forward to the same local port number where you can. Nothing breaks if you do
not (`-L 9000:127.0.0.1:8080` is fine), but matching ports makes the address in
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
