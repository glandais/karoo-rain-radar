use std::env;

/// Meteo-France API configuration
pub const METEO_API_BASE: &str = "https://public-api.meteofrance.fr/public/DPRadar/v1";
pub const ZONE: &str = "METROPOLE";
pub const OBSERVATION: &str = "LAME_D_EAU";
pub const MAILLE: u32 = 500;

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
