//! Entry point.
//!
//! Startup is deliberately fail-closed on configuration and fail-soft on
//! hardware: a missing or invalid config file stops the server, because running
//! with limits weaker than the operator intended is worse than not running at
//! all; but a supply that cannot be opened only marks that supply offline,
//! because refusing to boot over one switched-off unit takes the whole bench
//! down.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use dp832::{Transport, UsbTmcTransport};
use psu_core::{stale_claims, Config, Store, Supervisor};
use tracing_subscriber::EnvFilter;

use server::routes::{self, AppState};

#[cfg(feature = "simulation")]
use server::simulation;

/// How long to wait on a supply before declaring it unresponsive.
const INSTRUMENT_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Parser, Debug)]
#[command(
    name = "rigol-psu-server",
    about = "Bench control for Rigol DP832 supplies"
)]
struct Args {
    /// Path to the operator configuration file.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Override the configured listen address.
    #[arg(long, value_name = "ADDR")]
    bind: Option<SocketAddr>,

    /// Where presets and profiles are stored.
    #[arg(
        long,
        value_name = "PATH",
        default_value = "/var/lib/rigol-psu/data.db"
    )]
    database: PathBuf,

    /// Run against N simulated supplies instead of hardware. Development only.
    #[cfg(feature = "simulation")]
    #[arg(long, value_name = "N")]
    simulate: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    let simulated = simulate_count(&args);

    let mut config = match (&args.config, simulated) {
        (Some(path), _) => Config::load(path)?,
        (None, Some(n)) => {
            tracing::warn!("running against {n} simulated supplies; no hardware will be touched");
            Config::parse(&synthetic_config(n, args.bind), "<simulated>")?
        }
        (None, None) => {
            // Fail closed: without a config there are no clamps, and without
            // clamps nothing should be controllable.
            return Err("--config is required".into());
        }
    };

    if let Some(bind) = args.bind {
        config.bind = bind;
    }

    // Simulated runs keep their data in memory: a throwaway bench should not
    // leave presets behind, or pick up someone else's.
    let store = Arc::new(match simulated {
        Some(_) => {
            tracing::warn!("presets and profiles are in memory and will not be kept");
            Store::in_memory()?
        }
        None => {
            if let Some(parent) = args.database.parent() {
                std::fs::create_dir_all(parent)?;
            }
            Store::open(&args.database)?
        }
    });

    let (supervisor, sim_registry) = start_supervisor(&config, simulated);
    let supervisor = Arc::new(supervisor);

    let state = AppState {
        supervisor: Arc::clone(&supervisor),
        store: Arc::clone(&store),
        #[cfg(feature = "simulation")]
        simulation: sim_registry,
    };
    #[cfg(not(feature = "simulation"))]
    let _ = sim_registry;

    // Ownership is verified against the instruments rather than remembered. A
    // claim that no longer matches what its channel holds is dropped here, which
    // is what makes "powered by this profile" a statement about the bench rather
    // than about something the server once did.
    let verifier = tokio::spawn({
        let supervisor = Arc::clone(&supervisor);
        let store = Arc::clone(&store);
        async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(1));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                let snapshot = supervisor.snapshot();
                let (claims, profiles) = match (store.claims(), store.profiles()) {
                    (Ok(c), Ok(p)) => (c, p),
                    (c, p) => {
                        if let Err(e) = c {
                            tracing::warn!(error = %e, "could not read channel claims");
                        }
                        if let Err(e) = p {
                            tracing::warn!(error = %e, "could not read profiles");
                        }
                        continue;
                    }
                };

                let stale = stale_claims(&claims, &profiles, &snapshot);
                for claim in &stale {
                    tracing::info!(
                        slot = %claim.slot,
                        channel = %claim.channel,
                        profile = %claim.profile_name,
                        "releasing a claim: the channel no longer holds what the profile specifies"
                    );
                    if let Err(e) = store.release(&claim.slot, claim.channel) {
                        tracing::warn!(error = %e, "could not release a claim");
                    }
                }

                let live: Vec<_> = claims
                    .into_iter()
                    .filter(|c| {
                        !stale
                            .iter()
                            .any(|s| s.slot == c.slot && s.channel == c.channel)
                    })
                    .collect();
                supervisor.set_claims(&live);
            }
        }
    });

    // Publish on a timer rather than on every change, so a burst of commands
    // cannot flood clients and everyone sees the same coherent picture.
    let publisher = tokio::spawn({
        let supervisor = Arc::clone(&supervisor);
        let interval = config.poll.measurement_interval();
        async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                supervisor.publish();
            }
        }
    });

    let app = routes::router(state)
        .fallback(routes::fallback)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    tracing::info!(address = %config.bind, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    publisher.abort();
    verifier.abort();

    // Hands every front panel back. Outputs are left exactly as they are:
    // stopping the service must not de-energise somebody's experiment.
    tracing::info!("releasing front panels");
    supervisor.shutdown().await;

    Ok(())
}

#[cfg(feature = "simulation")]
fn simulate_count(args: &Args) -> Option<usize> {
    args.simulate
}

#[cfg(not(feature = "simulation"))]
fn simulate_count(_args: &Args) -> Option<usize> {
    None
}

type SimRegistryHandle = Option<Arc<SimRegistryAlias>>;

#[cfg(feature = "simulation")]
type SimRegistryAlias = simulation::SimRegistry;
#[cfg(not(feature = "simulation"))]
type SimRegistryAlias = ();

fn start_supervisor(config: &Config, simulated: Option<usize>) -> (Supervisor, SimRegistryHandle) {
    match simulated {
        #[cfg(feature = "simulation")]
        Some(_) => {
            use std::collections::BTreeMap;

            let mut sims = BTreeMap::new();
            for supply in &config.supplies {
                sims.insert(
                    supply.slot.clone(),
                    dp832::SimulatedDp832::new(supply.serial.clone()),
                );
            }
            let registry = Arc::new(simulation::SimRegistry::new(sims.clone()));
            let supervisor = Supervisor::start(config, |supply| {
                sims.get(&supply.slot)
                    .cloned()
                    .map(|s| Box::new(s) as Box<dyn Transport>)
                    .ok_or_else(|| format!("no simulated supply for {}", supply.slot))
            });
            (supervisor, Some(registry))
        }
        #[cfg(not(feature = "simulation"))]
        Some(_) => unreachable!("--simulate is unavailable without the simulation feature"),
        None => {
            let supervisor = Supervisor::start(config, |supply| {
                UsbTmcTransport::open(&supply.device, INSTRUMENT_TIMEOUT)
                    .map(|t| Box::new(t) as Box<dyn Transport>)
            });
            (supervisor, None)
        }
    }
}

/// A config for simulated supplies, with the DP832's own ratings as limits.
///
/// Only reachable in a simulation build; real deployments always load a
/// root-owned file.
fn synthetic_config(count: usize, bind: Option<SocketAddr>) -> String {
    let bind = bind.unwrap_or_else(|| "127.0.0.1:8080".parse().unwrap());
    let mut text = format!("bind = \"{bind}\"\n");
    for n in 1..=count {
        text.push_str(&format!(
            r#"
[[psu]]
slot = "psu{n}"
device = "/dev/rigol-psu{n}"
label = "Simulated supply {n}"
serial = "SIM-{n:04}"

  [[psu.channel]]
  id = "CH1"
  max_volt = 30.0
  max_curr = 3.0

  [[psu.channel]]
  id = "CH2"
  max_volt = 30.0
  max_curr = 3.0

  [[psu.channel]]
  id = "CH3"
  max_volt = 5.0
  max_curr = 3.0
"#
        ));
    }
    text
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => tracing::error!(error = %e, "could not install SIGTERM handler"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("interrupt received"),
        _ = terminate => tracing::info!("SIGTERM received"),
    }
}
