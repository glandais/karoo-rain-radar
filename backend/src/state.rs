use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::RadarCache;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    /// Cached radar data (None if not yet fetched)
    pub radar_cache: Arc<RwLock<Option<RadarCache>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            radar_cache: Arc::new(RwLock::new(None)),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
