//! Fault injection for simulated supplies. **Compiled only with the
//! `simulation` feature.**
//!
//! This exists so browser tests can reach the states that matter most and are
//! otherwise unreachable: a supply going offline mid-session, a channel reading
//! `Unknown`, a quarantine banner. Those are precisely the states that rot
//! silently, because nobody sees them in normal operation.
//!
//! It is gated twice. The feature keeps it out of the shipped binary entirely;
//! the runtime check below keeps it unmounted unless the server was actually
//! started against simulated supplies. An endpoint that fabricates instrument
//! state deserves more than one lock.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::post;
use axum::{Json, Router};
use dp832::{Fault, SimulatedDp832};
use serde::Deserialize;

use crate::error::ApiError;
use crate::routes::{Accepted, AppState};

/// The simulated supplies this process owns, by slot.
pub struct SimRegistry {
    supplies: BTreeMap<String, SimulatedDp832>,
}

impl SimRegistry {
    pub fn new(supplies: BTreeMap<String, SimulatedDp832>) -> Self {
        Self { supplies }
    }

    fn get(&self, slot: &str) -> Option<&SimulatedDp832> {
        self.supplies.get(slot)
    }
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/sim/{slot}/fault", post(post_fault))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultKind {
    None,
    Timeout,
    Disconnected,
    Garbage,
}

impl From<FaultKind> for Fault {
    fn from(k: FaultKind) -> Self {
        match k {
            FaultKind::None => Fault::None,
            FaultKind::Timeout => Fault::Timeout,
            FaultKind::Disconnected => Fault::Disconnected,
            FaultKind::Garbage => Fault::Garbage,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct FaultBody {
    pub kind: FaultKind,
}

async fn post_fault(
    State(state): State<AppState>,
    Path(slot): Path<String>,
    Json(body): Json<FaultBody>,
) -> Result<Json<Accepted>, ApiError> {
    // The second lock: even in a simulation build, this refuses to do anything
    // unless the process is actually driving simulated supplies.
    let registry: &Arc<SimRegistry> = state.simulation.as_ref().ok_or_else(|| {
        ApiError::not_found("this server is not running against simulated supplies")
    })?;

    let sim = registry
        .get(&slot)
        .ok_or_else(|| ApiError::not_found(format!("no simulated supply in slot `{slot}`")))?;

    sim.set_fault(body.kind.into());
    tracing::warn!(slot = %slot, "fault injected via the simulation API");
    Ok(Json(Accepted { ok: true }))
}
