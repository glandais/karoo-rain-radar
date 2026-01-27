use std::env;

/// Meteo-France API configuration
pub const METEO_API_BASE: &str = "https://public-api.meteofrance.fr/public/DPRadar/v1";
pub const ZONE: &str = "METROPOLE";
pub const OBSERVATION: &str = "LAME_D_EAU";
pub const MAILLE: u32 = 500;

const MIN: f64 = 0.1;
const MAX: f64 = 2.0;
const STEPS: u32 = 5;
const MUL: f64 = (MAX - MIN) / ((STEPS - 1) as f64);

/// Rain intensity thresholds (ACRR in 0.01 mm units)
/// Returns (level_name, threshold_value) pairs for r1 through r50
pub fn thresholds() -> Vec<f64> {
    (0..=STEPS).map(|i| 100.0 * (MIN + i as f64 * MUL)).collect()
}

/// Download polling interval in seconds
pub const POLLING_INTERVAL_SECONDS: u64 = 10;

/// Get API key from environment
pub fn get_api_key() -> anyhow::Result<String> {
    // First try environment variable
    if let Ok(key) = env::var("METEO_FRANCE_API_KEY") {
        if !key.is_empty() {
            return Ok(key);
        }
    }

    // Try loading from .env file (dotenvy should have loaded it already)
    Err(anyhow::anyhow!("METEO_FRANCE_API_KEY not set"))
}
