//! Tests for the HTTP surface: status codes, error bodies, and the gating of
//! the simulation routes.
//!
//! The error shape is load-bearing. The UI has to tell "you asked for more volts
//! than the operator permits" apart from "that supply is not answering" and say
//! something specific about each, so these assert on codes and fields rather
//! than on prose.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use dp832::{SimulatedDp832, Transport};
use http_body_util::BodyExt;
use psu_core::{Config, Supervisor};
use serde_json::Value;
use server::AppState;
use tower::ServiceExt;

const CONFIG: &str = r#"
bind = "127.0.0.1:0"

[poll]
measurement_hz = 50.0
setpoint_hz = 25.0

[[psu]]
slot = "psu1"
device = "/dev/rigol-psu1"
label = "Bench Left"
serial = "SIM-0001"

  [[psu.channel]]
  id = "CH1"
  max_volt = 5.5
  max_curr = 2.0

  [[psu.channel]]
  id = "CH2"
  max_volt = 15.0
  max_curr = 3.0

  [[psu.channel]]
  id = "CH3"
  max_volt = 5.0
  max_curr = 3.0
"#;

struct Harness {
    router: axum::Router,
    supervisor: Arc<Supervisor>,
}

fn harness() -> Harness {
    let config = Config::parse(CONFIG, "test.toml").unwrap();
    let mut sims = BTreeMap::new();
    sims.insert("psu1".to_string(), SimulatedDp832::new("SIM-0001"));

    let supervisor = Arc::new(Supervisor::start(&config, |supply| {
        sims.get(&supply.slot)
            .cloned()
            .map(|s| Box::new(s) as Box<dyn Transport>)
            .ok_or_else(|| format!("no simulator for {}", supply.slot))
    }));

    // Deliberately no simulation registry: this is what a production build
    // looks like at runtime, and the fault routes must refuse to work.
    let state = AppState {
        supervisor: Arc::clone(&supervisor),
        store: Arc::new(psu_core::Store::in_memory().unwrap()),
        #[cfg(feature = "simulation")]
        simulation: None,
    };
    Harness {
        router: server::router(state),
        supervisor,
    }
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn put(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn post(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn state_lists_every_configured_channel() {
    let h = harness();
    let response = h
        .router
        .clone()
        .oneshot(Request::get("/api/state").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let json = body_json(response).await;
    assert_eq!(json["devices"].as_array().unwrap().len(), 1);
    assert_eq!(json["devices"][0]["channels"].as_array().unwrap().len(), 3);

    h.supervisor.shutdown().await;
}

#[tokio::test]
async fn a_clamp_breach_returns_422_with_both_numbers() {
    let h = harness();
    let response = h
        .router
        .clone()
        .oneshot(put(
            "/api/devices/psu1/channels/CH1/setpoint",
            serde_json::json!({ "volt": 12.0, "curr_limit": 1.0 }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_json(response).await;
    assert_eq!(json["code"], "CLAMP_EXCEEDED");
    assert_eq!(json["slot"], "psu1");
    assert_eq!(json["channel"], "CH1");
    // The UI renders both without parsing the message.
    assert_eq!(json["violation"]["requested"], 12.0);
    assert_eq!(json["violation"]["limit"], 5.5);

    h.supervisor.shutdown().await;
}

#[tokio::test]
async fn an_unknown_channel_is_a_404_not_a_500() {
    let h = harness();
    let response = h
        .router
        .clone()
        .oneshot(post(
            "/api/devices/psu1/channels/CH9/output",
            serde_json::json!({ "on": true }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    h.supervisor.shutdown().await;
}

#[tokio::test]
async fn an_unknown_slot_is_a_404() {
    let h = harness();
    let response = h
        .router
        .clone()
        .oneshot(post(
            "/api/devices/nope/channels/CH1/output",
            serde_json::json!({ "on": true }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = body_json(response).await;
    assert_eq!(json["code"], "UNKNOWN_SLOT");

    h.supervisor.shutdown().await;
}

#[tokio::test]
async fn rejects_a_non_finite_setpoint() {
    // NaN compares false against every limit, so it would sail past the clamp.
    let h = harness();
    let request = Request::builder()
        .method("PUT")
        .uri("/api/devices/psu1/channels/CH1/setpoint")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"volt": 1e999, "curr_limit": 1.0}"#))
        .unwrap();

    let response = h.router.clone().oneshot(request).await.unwrap();
    assert!(
        response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "got {}",
        response.status()
    );

    h.supervisor.shutdown().await;
}

#[tokio::test]
async fn a_valid_setpoint_is_accepted() {
    let h = harness();
    let response = h
        .router
        .clone()
        .oneshot(put(
            "/api/devices/psu1/channels/CH1/setpoint",
            serde_json::json!({ "volt": 3.3, "curr_limit": 1.0 }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    h.supervisor.shutdown().await;
}

#[cfg(feature = "simulation")]
#[tokio::test]
async fn fault_injection_is_refused_without_a_simulation_registry() {
    // The second of the two locks: even in a simulation build, the route does
    // nothing unless this process is actually driving simulated supplies.
    let h = harness();
    let response = h
        .router
        .clone()
        .oneshot(post(
            "/api/sim/psu1/fault",
            serde_json::json!({ "kind": "disconnected" }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    h.supervisor.shutdown().await;
}
