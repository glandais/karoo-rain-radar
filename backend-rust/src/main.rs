mod config;
mod error;
mod models;
mod routes;
mod services;
mod state;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use axum::{
    routing::get,
    Router,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use reqwest::Client;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::{info, error, Level};
use tracing_subscriber::FmtSubscriber;

use crate::config::POLLING_INTERVAL_SECONDS;
use crate::models::RadarCache;
use crate::services::{fetch_latest_timestamp, fetch_radar_data, parse_hdf5};
use crate::state::AppState;

/// Parse radar timestamp from format 'YYYYMMDDTHHMMSSZ' to DateTime
fn parse_radar_timestamp(date_str: &str) -> Option<DateTime<Utc>> {
    // Format: "20260127T143500Z"
    NaiveDateTime::parse_from_str(date_str, "%Y%m%dT%H%M%SZ")
        .ok()
        .map(|dt| dt.and_utc())
}

/// Calculate timestamp when we should start polling for new data
fn calculate_next_poll_time(date_str: &str) -> f64 {
    if let Some(radar_time) = parse_radar_timestamp(date_str) {
        // New data expected at radar_time + 7 minutes
        radar_time.timestamp() as f64 + 7.0 * 60.0
    } else {
        0.0
    }
}

/// Check if new data is available and download it
async fn check_and_download(state: &AppState, client: &Client) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();

    let log_time = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");

    // Check if it's time to poll
    {
        let cache = state.radar_cache.read().await;
        if let Some(ref c) = *cache {
            let next_poll_time = calculate_next_poll_time(&c.date_str);
            if now < next_poll_time {
                return;
            }
        }
    }

    // Check latest available timestamp
    match fetch_latest_timestamp(client).await {
        Ok(latest_timestamp) => {
            let current_timestamp = {
                let cache = state.radar_cache.read().await;
                cache.as_ref().map(|c| c.date_str.clone())
            };

            if current_timestamp.as_ref() != Some(&latest_timestamp) {
                // New data available, download it
                info!("[{}] New data available: {}", log_time, latest_timestamp);

                match fetch_radar_data(client).await {
                    Ok(data) => {
                        match parse_hdf5(&data) {
                            Ok(parsed) => {
                                let date_str = parsed.date_str.clone();

                                // Store raw data in cache (no pre-computation)
                                let radar_cache = RadarCache {
                                    date_str: date_str.clone(),
                                    data: parsed.data,
                                    projdef: parsed.projdef,
                                    x_origin: parsed.x_origin,
                                    y_origin: parsed.y_origin,
                                    x_scale: parsed.x_scale,
                                    y_scale: parsed.y_scale,
                                };

                                {
                                    let mut cache = state.radar_cache.write().await;
                                    *cache = Some(radar_cache);
                                }
                                info!("[{}] Radar data downloaded and stored: {}", log_time, date_str);

                                let next_poll = calculate_next_poll_time(&date_str);
                                let wait_seconds = next_poll - now;
                                info!("[{}] Next poll in {:.0} seconds", log_time, wait_seconds);
                            }
                            Err(e) => {
                                error!("[{}] HDF5 parse error: {}", log_time, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("[{}] Download error: {}", log_time, e);
                    }
                }
            } else {
                info!(
                    "[{}] Waiting for new data (current: {:?})...",
                    log_time, current_timestamp
                );
            }
        }
        Err(e) => {
            error!("[{}] Timestamp fetch error: {}", log_time, e);
        }
    }
}

/// Background ticker loop that polls for new data
async fn ticker_loop(state: AppState) {
    let client = Client::new();

    loop {
        check_and_download(&state, &client).await;
        tokio::time::sleep(Duration::from_secs(POLLING_INTERVAL_SECONDS)).await;
    }
}

#[tokio::main]
async fn main() {
    // Load .env file if present
    dotenvy::dotenv().ok();

    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Create application state
    let state = AppState::new();

    // Start background ticker
    let ticker_state = state.clone();
    tokio::spawn(async move {
        ticker_loop(ticker_state).await;
    });

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Determine static files directory
    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static");

    // Build router
    let app = Router::new()
        .route("/api/radar", get(routes::get_radar))
        .route("/api/health", get(routes::health))
        .nest_service("/static", ServeDir::new(&static_dir))
        .fallback_service(ServeDir::new(&static_dir).append_index_html_on_directories(true))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
