use std::env;
use std::collections::HashMap;

/// Meteo-France API configuration
pub const METEO_API_BASE: &str = "https://public-api.meteofrance.fr/public/DPRadar/v1";
pub const ZONE: &str = "METROPOLE";
pub const OBSERVATION: &str = "LAME_D_EAU";
pub const MAILLE: u32 = 500;

/// Rain intensity thresholds (ACRR in 0.01 mm units)
pub fn thresholds() -> Vec<(&'static str, f64)> {
    vec![
        ("light", 10.0),      // 0.1 mm - light rain
        ("moderate", 50.0),   // 0.5 mm - moderate rain
        ("heavy", 100.0),     // 1.0 mm - heavy rain
    ]
}

/// Colors for each rain level (ARGB format for Karoo)
pub fn colors() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("light", "#00FF00");      // Green
    m.insert("moderate", "#FFFF00");   // Yellow
    m.insert("heavy", "#FF0000");      // Red
    m
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
