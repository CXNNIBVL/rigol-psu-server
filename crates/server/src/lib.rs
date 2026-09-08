//! HTTP and WebSocket front end.
//!
//! Exposed as a library as well as a binary so that the routes can be exercised
//! directly by tests, without binding a socket or spawning a process.

pub mod assets;
pub mod error;
pub mod routes;

#[cfg(feature = "simulation")]
pub mod simulation;

pub use routes::{router, AppState};
