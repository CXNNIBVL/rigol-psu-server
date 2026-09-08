-- Presets and profiles: the application's own data, the only irreplaceable
-- state in the system.
--
-- Operator configuration deliberately lives elsewhere. The per-channel clamps
-- are in a root-owned TOML file, not here, so that nothing reachable through
-- the API — or through a stray UPDATE, a migration bug, or a restore from an
-- older backup — can weaken them.

CREATE TABLE channel_presets (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE,
    volt        REAL    NOT NULL,
    curr_limit  REAL    NOT NULL,
    ovp_level   REAL    NOT NULL,
    ovp_enabled INTEGER NOT NULL,
    ocp_level   REAL    NOT NULL,
    ocp_enabled INTEGER NOT NULL,
    -- The intended output state. Recorded, but never acted on by an apply:
    -- energising is always a separate, deliberate step.
    output_on   INTEGER NOT NULL
);

CREATE TABLE device_profiles (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE,
    description TEXT    NOT NULL DEFAULT ''
);

CREATE TABLE profile_bindings (
    id             INTEGER PRIMARY KEY,
    profile_id     INTEGER NOT NULL REFERENCES device_profiles(id) ON DELETE CASCADE,
    -- Slot, not serial. Replacing a dead supply is then a udev-rule edit rather
    -- than a data migration.
    slot           TEXT    NOT NULL,
    channel        TEXT    NOT NULL,
    -- RESTRICT, not CASCADE: deleting a preset that a profile depends on must
    -- fail loudly rather than silently gut the profile.
    preset_id      INTEGER NOT NULL REFERENCES channel_presets(id) ON DELETE RESTRICT,
    -- Power-up order and the dwell after this rail comes up. Stored from the
    -- first migration even when unused, because devices whose rails must come
    -- up in sequence exist, and adding the column later is a migration.
    sequence_order INTEGER NOT NULL DEFAULT 0,
    delay_ms       INTEGER NOT NULL DEFAULT 0,
    -- One binding per physical channel per profile.
    UNIQUE (profile_id, slot, channel)
);

CREATE INDEX idx_bindings_profile ON profile_bindings (profile_id, sequence_order);
