use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::services::calculate_forecast;
use crate::state::AppState;

/// Query parameters for forecast endpoint
#[derive(Debug, Deserialize)]
pub struct ForecastQuery {
    pub lat: f64,
    pub lng: f64,
    /// Sample radius in km (default: 30)
    #[serde(default = "default_radius")]
    pub radius_km: f64,
}

fn default_radius() -> f64 {
    30.0
}

/// Forecast response
#[derive(Debug, Serialize)]
pub struct ForecastResponse {
    /// Data timestamp
    pub timestamp: String,
    /// Rain rate at current location (mm/h)
    pub current_rain_rate: f64,
    /// Whether it's currently raining
    pub is_raining: bool,
    /// Rain intensities for 12 sectors around the user (mm/h)
    /// Index 0 = North, clockwise (30° each sector)
    pub sectors: [f64; 12],
    /// Minutes until rain change (positive = rain arriving, negative = rain ending)
    /// null if no change expected within analysis radius
    pub minutes_to_change: Option<i32>,
    /// Distance to nearest rain edge in km
    pub distance_to_edge_km: Option<f64>,
    /// Human-readable status
    pub status_text: String,
}

/// GET /api/rain/forecast - Get rain forecast data for a specific location
pub async fn get_forecast(
    State(state): State<AppState>,
    Query(query): Query<ForecastQuery>,
) -> Result<Json<ForecastResponse>, AppError> {
    // Get cached radar data
    let cache_guard = state.radar_cache.read().await;
    let cache = cache_guard.as_ref().ok_or(AppError::DataNotAvailable)?;

    let forecast = calculate_forecast(
        query.lat,
        query.lng,
        query.radius_km,
        &cache.data,
        &cache.projdef,
        cache.x_origin,
        cache.y_origin,
        cache.x_scale,
        cache.y_scale,
    )?;

    // Generate human-readable status
    let status_text = if forecast.is_raining {
        match forecast.minutes_to_change {
            Some(minutes) if minutes < 0 => {
                let abs_min = (-minutes).abs();
                if abs_min <= 5 {
                    "Fin de pluie imminente".to_string()
                } else if abs_min <= 15 {
                    format!("Fin dans ~{} min", abs_min)
                } else {
                    format!("Pluie pour ~{} min", abs_min)
                }
            }
            _ => "Pluie continue".to_string(),
        }
    } else {
        match forecast.minutes_to_change {
            Some(minutes) if minutes > 0 => {
                if minutes <= 5 {
                    "Pluie imminente".to_string()
                } else if minutes <= 15 {
                    format!("Pluie dans ~{} min", minutes)
                } else if minutes <= 30 {
                    format!("Pluie dans ~{} min", minutes)
                } else {
                    "Sec pour l'instant".to_string()
                }
            }
            _ => "Sec".to_string(),
        }
    };

    Ok(Json(ForecastResponse {
        timestamp: cache.date_str.clone(),
        current_rain_rate: forecast.current_rain_rate,
        is_raining: forecast.is_raining,
        sectors: forecast.sectors,
        minutes_to_change: forecast.minutes_to_change,
        distance_to_edge_km: forecast.distance_to_edge_km,
        status_text,
    }))
}
