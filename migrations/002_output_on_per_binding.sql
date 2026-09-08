-- Move `output_on` from the preset to the binding.
--
-- It was a column on channel_presets, which made "does this rail come up on
-- power-on" a property of the *settings* rather than of how a profile uses
-- them. A preset shared by two profiles therefore had to auto-start in both:
-- wanting 3v3 to come up automatically on one device and stay staged on
-- another meant two near-identical presets, which is the duplication presets
-- exist to prevent.
--
-- It belongs next to sequence_order and delay_ms, which are already statements
-- about how this profile powers this rail.

ALTER TABLE profile_bindings ADD COLUMN output_on INTEGER NOT NULL DEFAULT 1;

-- Backfill from the preset each binding points at, so existing profiles keep
-- powering up exactly as they did before.
UPDATE profile_bindings
SET output_on = (SELECT output_on FROM channel_presets WHERE id = preset_id);

-- Rebuild rather than ALTER TABLE ... DROP COLUMN. SQLite re-parses the stored
-- CREATE statement when dropping a column, and fails with "incomplete input"
-- when that statement contains comments — which ours does, deliberately.
CREATE TABLE channel_presets_rebuilt (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE,
    volt        REAL    NOT NULL,
    curr_limit  REAL    NOT NULL,
    ovp_level   REAL    NOT NULL,
    ovp_enabled INTEGER NOT NULL,
    ocp_level   REAL    NOT NULL,
    ocp_enabled INTEGER NOT NULL
);

INSERT INTO channel_presets_rebuilt
SELECT id, name, volt, curr_limit, ovp_level, ovp_enabled, ocp_level, ocp_enabled
FROM channel_presets;

DROP TABLE channel_presets;
ALTER TABLE channel_presets_rebuilt RENAME TO channel_presets;
