//! Persistence for presets and profiles.
//!
//! SQLite because this is shared, concurrently-edited state: several people can
//! have the UI open, and a TOML file would need hand-rolled locking to avoid
//! losing an edit. Migrations exist from the first commit because presets are
//! the only irreplaceable data in the system — everything else can be rebuilt
//! from the config file and the instruments themselves.
//!
//! What is deliberately *not* here: the per-channel clamps. Those live in a
//! root-owned TOML file so that nothing reachable from the API can weaken them,
//! and so that a missing file fails the server closed rather than open.

use std::path::Path;
use std::sync::Mutex;

use dp832::{ChannelId, Protection, Setpoint};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../../migrations/001_initial.sql")),
    (
        2,
        include_str!("../../../migrations/002_output_on_per_binding.sql"),
    ),
    (
        3,
        include_str!("../../../migrations/003_channel_claims.sql"),
    ),
];

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),

    #[error("no {kind} with id {id}")]
    NotFound { kind: &'static str, id: i64 },

    #[error("a {kind} named `{name}` already exists")]
    DuplicateName { kind: &'static str, name: String },

    #[error("preset `{name}` is used by {profiles} profile(s) and cannot be deleted")]
    PresetInUse { name: String, profiles: usize },

    #[error("`{0}` is not a channel")]
    BadChannel(String),

    #[error("{0}")]
    Invalid(String),
}

impl StoreError {
    pub fn code(&self) -> &'static str {
        match self {
            StoreError::Sql(_) => "STORAGE_ERROR",
            StoreError::NotFound { .. } => "NOT_FOUND",
            StoreError::DuplicateName { .. } => "DUPLICATE_NAME",
            StoreError::PresetInUse { .. } => "PRESET_IN_USE",
            StoreError::BadChannel(_) | StoreError::Invalid(_) => "VALIDATION_FAILED",
        }
    }
}

/// A named bundle of settings for one channel.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ChannelPreset {
    pub id: i64,
    pub name: String,
    pub volt: f64,
    pub curr_limit: f64,
    pub ovp_level: f64,
    pub ovp_enabled: bool,
    pub ocp_level: f64,
    pub ocp_enabled: bool,
}

impl ChannelPreset {
    pub fn setpoint(&self) -> Setpoint {
        Setpoint {
            volt: self.volt,
            curr_limit: self.curr_limit,
        }
    }

    pub fn protection(&self) -> Protection {
        Protection {
            ovp_level: self.ovp_level,
            ovp_enabled: self.ovp_enabled,
            ocp_level: self.ocp_level,
            ocp_enabled: self.ocp_enabled,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct NewChannelPreset {
    pub name: String,
    pub volt: f64,
    pub curr_limit: f64,
    pub ovp_level: f64,
    pub ovp_enabled: bool,
    pub ocp_level: f64,
    pub ocp_enabled: bool,
}

/// One rail of a profile: which preset goes to which physical channel, and when.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ProfileBinding {
    pub id: i64,
    pub slot: String,
    pub channel: ChannelId,
    pub preset: ChannelPreset,
    pub sequence_order: i64,
    pub delay_ms: i64,
    /// Whether power-on energises this rail. A profile may legitimately carry a
    /// rail it wants configured but brought up by hand — a programming voltage,
    /// say — so this is a statement about how *this* profile uses the preset,
    /// not about the preset itself.
    pub output_on: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct NewProfileBinding {
    pub slot: String,
    pub channel: String,
    pub preset_id: i64,
    #[serde(default)]
    pub sequence_order: i64,
    #[serde(default)]
    pub delay_ms: i64,
    /// Defaults to true: a rail named in a profile is normally one the profile
    /// powers.
    #[serde(default = "default_true")]
    pub output_on: bool,
}

fn default_true() -> bool {
    true
}

/// A named device, described as the set of rails it needs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DeviceProfile {
    pub id: i64,
    pub name: String,
    pub description: String,
    /// In power-up order.
    pub bindings: Vec<ProfileBinding>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct NewDeviceProfile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub bindings: Vec<NewProfileBinding>,
}

/// A profile's ownership of one channel.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Claim {
    pub slot: String,
    pub channel: ChannelId,
    pub profile_id: i64,
    /// Carried so the bench can name the owner without fetching every profile.
    pub profile_name: String,
}

pub struct Store {
    // A std mutex around a single connection. The queries here are a handful of
    // rows on a local file with WAL enabled, so they complete in microseconds
    // and holding an async worker for that is cheaper than the machinery to
    // avoid it. If this ever becomes contended, the change is a connection pool
    // behind the same interface.
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        Self::prepare(conn)
    }

    pub fn in_memory() -> Result<Self, StoreError> {
        Self::prepare(Connection::open_in_memory()?)
    }

    fn prepare(conn: Connection) -> Result<Self, StoreError> {
        // WAL lets readers proceed while a writer commits, which is what makes
        // several people with the UI open a non-event.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // Without this, the RESTRICT on profile_bindings.preset_id is inert and
        // deleting an in-use preset would silently break profiles.
        conn.pragma_update(None, "foreign_keys", "ON")?;

        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    /// Apply any migrations the database has not seen, tracked in
    /// `PRAGMA user_version`.
    fn migrate(&self) -> Result<(), StoreError> {
        let mut conn = self.conn.lock().unwrap();
        let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

        let pending: Vec<&(i64, &str)> = MIGRATIONS.iter().filter(|(v, _)| *v > current).collect();
        if pending.is_empty() {
            return Ok(());
        }

        // Suspended for the duration: a migration that rebuilds a table has to
        // drop and recreate a parent, which enforcement would reject, and the
        // pragma cannot be changed inside a transaction. The result is checked
        // explicitly below rather than assumed.
        conn.pragma_update(None, "foreign_keys", "OFF")?;
        let result = (|| -> Result<(), StoreError> {
            for (version, sql) in pending {
                let tx = conn.transaction()?;
                tx.execute_batch(sql)?;
                tx.pragma_update(None, "user_version", version)?;
                tx.commit()?;
                tracing::info!(version, "applied migration");
            }
            Ok(())
        })();
        conn.pragma_update(None, "foreign_keys", "ON")?;
        result?;

        let broken: i64 =
            conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })?;
        if broken > 0 {
            return Err(StoreError::Invalid(format!(
                "migration left {broken} dangling reference(s); refusing to continue"
            )));
        }
        Ok(())
    }

    // -- presets ------------------------------------------------------------

    pub fn presets(&self) -> Result<Vec<ChannelPreset>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, volt, curr_limit, ovp_level, ovp_enabled, \
             ocp_level, ocp_enabled FROM channel_presets ORDER BY name",
        )?;
        let rows = stmt
            .query_map([], preset_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn preset(&self, id: i64) -> Result<ChannelPreset, StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, name, volt, curr_limit, ovp_level, ovp_enabled, \
             ocp_level, ocp_enabled FROM channel_presets WHERE id = ?1",
            params![id],
            preset_from_row,
        )
        .optional()?
        .ok_or(StoreError::NotFound { kind: "preset", id })
    }

    pub fn create_preset(&self, new: &NewChannelPreset) -> Result<ChannelPreset, StoreError> {
        validate_preset(new)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO channel_presets \
             (name, volt, curr_limit, ovp_level, ovp_enabled, ocp_level, ocp_enabled) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                new.name,
                new.volt,
                new.curr_limit,
                new.ovp_level,
                new.ovp_enabled,
                new.ocp_level,
                new.ocp_enabled
            ],
        )
        .map_err(|e| duplicate_or(e, "preset", &new.name))?;
        let id = conn.last_insert_rowid();
        drop(conn);
        self.preset(id)
    }

    pub fn update_preset(
        &self,
        id: i64,
        new: &NewChannelPreset,
    ) -> Result<ChannelPreset, StoreError> {
        validate_preset(new)?;
        let conn = self.conn.lock().unwrap();
        let changed = conn
            .execute(
                "UPDATE channel_presets SET name = ?2, volt = ?3, curr_limit = ?4, \
                 ovp_level = ?5, ovp_enabled = ?6, ocp_level = ?7, ocp_enabled = ?8 \
                 WHERE id = ?1",
                params![
                    id,
                    new.name,
                    new.volt,
                    new.curr_limit,
                    new.ovp_level,
                    new.ovp_enabled,
                    new.ocp_level,
                    new.ocp_enabled
                ],
            )
            .map_err(|e| duplicate_or(e, "preset", &new.name))?;
        drop(conn);
        if changed == 0 {
            return Err(StoreError::NotFound { kind: "preset", id });
        }
        self.preset(id)
    }

    /// Deleting a preset a profile depends on is refused rather than allowed to
    /// silently gut the profile.
    pub fn delete_preset(&self, id: i64) -> Result<(), StoreError> {
        let preset = self.preset(id)?;
        let conn = self.conn.lock().unwrap();
        let uses: i64 = conn.query_row(
            "SELECT COUNT(*) FROM profile_bindings WHERE preset_id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        if uses > 0 {
            return Err(StoreError::PresetInUse {
                name: preset.name,
                profiles: uses as usize,
            });
        }
        conn.execute("DELETE FROM channel_presets WHERE id = ?1", params![id])?;
        Ok(())
    }

    // -- profiles -----------------------------------------------------------

    pub fn profiles(&self) -> Result<Vec<DeviceProfile>, StoreError> {
        // Collected in its own scope so the lock is released before `profile()`
        // reacquires it for each row.
        let ids: Vec<i64> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT id FROM device_profiles ORDER BY name")?;
            let ids = stmt
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<i64>, _>>()?;
            ids
        };
        ids.into_iter().map(|id| self.profile(id)).collect()
    }

    pub fn profile(&self, id: i64) -> Result<DeviceProfile, StoreError> {
        let conn = self.conn.lock().unwrap();
        let (name, description): (String, String) = conn
            .query_row(
                "SELECT name, description FROM device_profiles WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(StoreError::NotFound {
                kind: "profile",
                id,
            })?;

        let mut stmt = conn.prepare(
            "SELECT b.id, b.slot, b.channel, b.sequence_order, b.delay_ms, b.output_on, \
                    p.id, p.name, p.volt, p.curr_limit, p.ovp_level, p.ovp_enabled, \
                    p.ocp_level, p.ocp_enabled \
             FROM profile_bindings b JOIN channel_presets p ON p.id = b.preset_id \
             WHERE b.profile_id = ?1 ORDER BY b.sequence_order, b.slot, b.channel",
        )?;
        let bindings = stmt
            .query_map(params![id], |row| {
                let channel_text: String = row.get(2)?;
                Ok((
                    channel_text,
                    ProfileBinding {
                        id: row.get(0)?,
                        slot: row.get(1)?,
                        channel: ChannelId::Ch1, // replaced below
                        sequence_order: row.get(3)?,
                        delay_ms: row.get(4)?,
                        output_on: row.get(5)?,
                        preset: ChannelPreset {
                            id: row.get(6)?,
                            name: row.get(7)?,
                            volt: row.get(8)?,
                            curr_limit: row.get(9)?,
                            ovp_level: row.get(10)?,
                            ovp_enabled: row.get(11)?,
                            ocp_level: row.get(12)?,
                            ocp_enabled: row.get(13)?,
                        },
                    },
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|(text, mut binding)| {
                binding.channel = text
                    .parse()
                    .map_err(|_| StoreError::BadChannel(text.clone()))?;
                Ok(binding)
            })
            .collect::<Result<Vec<_>, StoreError>>()?;

        Ok(DeviceProfile {
            id,
            name,
            description,
            bindings,
        })
    }

    pub fn create_profile(&self, new: &NewDeviceProfile) -> Result<DeviceProfile, StoreError> {
        let id = {
            let mut conn = self.conn.lock().unwrap();
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO device_profiles (name, description) VALUES (?1, ?2)",
                params![new.name, new.description],
            )
            .map_err(|e| duplicate_or(e, "profile", &new.name))?;
            let id = tx.last_insert_rowid();
            write_bindings(&tx, id, &new.bindings)?;
            tx.commit()?;
            id
        };
        self.profile(id)
    }

    /// Replaces the profile wholesale, bindings included. Editing a profile is
    /// always "here is what it should now be", which avoids a half-applied edit
    /// leaving a device description that powers the wrong rails.
    pub fn update_profile(
        &self,
        id: i64,
        new: &NewDeviceProfile,
    ) -> Result<DeviceProfile, StoreError> {
        {
            let mut conn = self.conn.lock().unwrap();
            let tx = conn.transaction()?;
            let changed = tx
                .execute(
                    "UPDATE device_profiles SET name = ?2, description = ?3 WHERE id = ?1",
                    params![id, new.name, new.description],
                )
                .map_err(|e| duplicate_or(e, "profile", &new.name))?;
            if changed == 0 {
                return Err(StoreError::NotFound {
                    kind: "profile",
                    id,
                });
            }
            tx.execute(
                "DELETE FROM profile_bindings WHERE profile_id = ?1",
                params![id],
            )?;
            write_bindings(&tx, id, &new.bindings)?;
            tx.commit()?;
        }
        self.profile(id)
    }

    pub fn delete_profile(&self, id: i64) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        let changed = conn.execute("DELETE FROM device_profiles WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(StoreError::NotFound {
                kind: "profile",
                id,
            });
        }
        Ok(())
    }
}

impl Store {
    // -- claims -------------------------------------------------------------

    /// Record that `profile_id` owns these channels, replacing any previous
    /// owner. A channel has one owner: two profiles cannot both have written
    /// the values it currently holds.
    pub fn claim(
        &self,
        profile_id: i64,
        channels: &[(String, ChannelId)],
    ) -> Result<(), StoreError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for (slot, channel) in channels {
            tx.execute(
                "INSERT INTO channel_claims (slot, channel, profile_id) VALUES (?1, ?2, ?3) \
                 ON CONFLICT (slot, channel) DO UPDATE SET profile_id = excluded.profile_id",
                params![slot, channel.as_scpi(), profile_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn release(&self, slot: &str, channel: ChannelId) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM channel_claims WHERE slot = ?1 AND channel = ?2",
            params![slot, channel.as_scpi()],
        )?;
        Ok(())
    }

    pub fn claims(&self) -> Result<Vec<Claim>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT c.slot, c.channel, c.profile_id, p.name \
             FROM channel_claims c JOIN device_profiles p ON p.id = c.profile_id",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let channel: String = row.get(1)?;
                Ok((
                    channel,
                    Claim {
                        slot: row.get(0)?,
                        channel: ChannelId::Ch1, // replaced below
                        profile_id: row.get(2)?,
                        profile_name: row.get(3)?,
                    },
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        rows.into_iter()
            .map(|(text, mut claim)| {
                claim.channel = text
                    .parse()
                    .map_err(|_| StoreError::BadChannel(text.clone()))?;
                Ok(claim)
            })
            .collect()
    }
}

fn write_bindings(
    tx: &rusqlite::Transaction<'_>,
    profile_id: i64,
    bindings: &[NewProfileBinding],
) -> Result<(), StoreError> {
    for binding in bindings {
        let channel: ChannelId = binding
            .channel
            .parse()
            .map_err(|_| StoreError::BadChannel(binding.channel.clone()))?;
        if binding.delay_ms < 0 {
            return Err(StoreError::Invalid("delay_ms cannot be negative".into()));
        }
        tx.execute(
            "INSERT INTO profile_bindings \
             (profile_id, slot, channel, preset_id, sequence_order, delay_ms, output_on) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                profile_id,
                binding.slot,
                channel.as_scpi(),
                binding.preset_id,
                binding.sequence_order,
                binding.delay_ms,
                binding.output_on
            ],
        )
        .map_err(|e| match &e {
            rusqlite::Error::SqliteFailure(err, _)
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                StoreError::Invalid(format!(
                    "binding {}/{} is invalid: either the preset does not exist, \
                     or the channel is bound twice in this profile",
                    binding.slot, binding.channel
                ))
            }
            _ => StoreError::Sql(e),
        })?;
    }
    Ok(())
}

fn preset_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChannelPreset> {
    Ok(ChannelPreset {
        id: row.get(0)?,
        name: row.get(1)?,
        volt: row.get(2)?,
        curr_limit: row.get(3)?,
        ovp_level: row.get(4)?,
        ovp_enabled: row.get(5)?,
        ocp_level: row.get(6)?,
        ocp_enabled: row.get(7)?,
    })
}

fn validate_preset(new: &NewChannelPreset) -> Result<(), StoreError> {
    if new.name.trim().is_empty() {
        return Err(StoreError::Invalid("a preset needs a name".into()));
    }
    // NaN compares false against every clamp, so it would sail straight through
    // the limit check later. Reject it at the door.
    for (label, value) in [
        ("volt", new.volt),
        ("curr_limit", new.curr_limit),
        ("ovp_level", new.ovp_level),
        ("ocp_level", new.ocp_level),
    ] {
        if !value.is_finite() {
            return Err(StoreError::Invalid(format!("{label} must be a number")));
        }
        if value < 0.0 {
            return Err(StoreError::Invalid(format!("{label} cannot be negative")));
        }
    }
    Ok(())
}

fn duplicate_or(e: rusqlite::Error, kind: &'static str, name: &str) -> StoreError {
    match &e {
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            StoreError::DuplicateName {
                kind,
                name: name.to_string(),
            }
        }
        _ => StoreError::Sql(e),
    }
}
