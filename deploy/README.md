# Deploying to the lab PC

Three files and a binary. Everything the server needs at runtime is either in
this directory or created by it.

The order matters: the udev rule has to exist before the service starts, or the
server will find no devices and come up with every supply offline.

## 1. Device names

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

Edit `99-rigol.rules`, replacing the placeholder serials, then install it:

```sh
sudo cp 99-rigol.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
ls -l /dev/rigol-psu*        # three symlinks should now exist
```

The rule grants the `rigol` group access, so create it and add the service user:

```sh
sudo groupadd -f rigol
sudo useradd -r -g rigol -s /usr/sbin/nologin rigol || true
```

## 2. Configuration

```sh
sudo install -d -o rigol -g rigol /etc/rigol-psu
sudo cp ../config.example.toml /etc/rigol-psu/config.toml
sudo chown root:root /etc/rigol-psu/config.toml
sudo chmod 644 /etc/rigol-psu/config.toml
```

Then edit it. Three things must be right before the service will start:

- **`bind`** has no default. The service is expected to be reachable only over
  the VPN, so set it to the VPN address rather than `0.0.0.0`.
- **`serial`** for each supply must match what the instrument reports. A
  mismatch quarantines that supply: it can be read and configured, but its
  outputs cannot be switched on until this file is corrected and the service
  restarted. There is no way to clear that from the UI.
- **Per-channel limits** cannot be raised through the API and are not editable
  from the UI. That is the whole reason they live here. The server refuses to
  start if the file is missing.

## 3. The binary and the service

Built on a development machine and copied over — the lab PC needs no toolchain:

```sh
just build                                  # on the dev machine
scp target/release/rigol-psu-server lab:/tmp/
```

Then on the lab PC:

```sh
sudo install -o root -g root -m 755 /tmp/rigol-psu-server /usr/local/bin/
sudo cp rigol-psu.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now rigol-psu
systemctl status rigol-psu
```

The unit creates `/var/lib/rigol-psu` for the presets and profiles database via
`StateDirectory`, so nothing else needs setting up.

## Checking it works

```sh
journalctl -u rigol-psu -f
curl -s http://<bind-address>/api/state | head
```

Each supply should report `"state": "online"` with a `reported_serial` matching
its configured one. A supply that is switched off appears as `offline` and the
service still starts — refusing to boot over one unplugged unit would take the
whole bench down.

## Upgrading

```sh
sudo systemctl stop rigol-psu
sudo install -o root -g root -m 755 /tmp/rigol-psu-server /usr/local/bin/
sudo systemctl start rigol-psu
```

Stopping the service **does not de-energise anything**. Outputs stay exactly as
they are and are adopted again on the way back up, so an upgrade does not end a
running experiment. The front panels are released on `SIGTERM`; the unit allows
15 seconds for that, which is why `TimeoutStopSec` should not be shortened.

Database migrations run at startup. They are applied in a transaction and the
result is checked for dangling references, so a failed migration refuses to open
the database rather than running against a half-changed schema. Back up
`/var/lib/rigol-psu/data.db` before a version jump — it holds the presets and
profiles, which are the only irreplaceable state in the system.

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
