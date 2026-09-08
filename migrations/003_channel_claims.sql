-- Which profile owns which channel.
--
-- A claim records that a profile wrote a channel's setpoints. It is not a
-- memory of an action: it is checked against the instrument on every setpoint
-- poll and dropped the moment the channel stops holding what the claiming
-- profile specifies. That is what makes it safe to say "powered by this
-- profile" — the statement is corroborated rather than remembered.
--
-- It still says nothing about which physical board is connected. A claim means
-- "this profile owns these rails", never "this device is on the bench".

CREATE TABLE channel_claims (
    -- Slot, not serial, matching how bindings are keyed.
    slot       TEXT    NOT NULL,
    channel    TEXT    NOT NULL,
    -- Deleting a profile releases its claims: they describe that profile's
    -- ownership and mean nothing without it.
    profile_id INTEGER NOT NULL REFERENCES device_profiles(id) ON DELETE CASCADE,
    PRIMARY KEY (slot, channel)
);

CREATE INDEX idx_claims_profile ON channel_claims (profile_id);
