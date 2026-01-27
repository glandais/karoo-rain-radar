use axum::{
    extract::{Query, State},
    Json,
};
use geo::BooleanOps;
use serde::Serialize;

use crate::error::AppError;
use crate::models::{ContourData, LatLng, RadarQuery, RadarResponse};
use crate::services::{create_circle, encode_polyline};
use crate::state::AppState;

/// Response for the all-contours endpoint
#[derive(Debug, Serialize)]
pub struct RadarAllResponse {
    pub timestamp: String,
    pub contours: Vec<ContourData>,
    pub total_points: usize,
    pub contour_count: usize,
}

/// GET /api/radar - Get rain radar contours as encoded polylines
pub async fn get_radar(
    State(state): State<AppState>,
    Query(query): Query<RadarQuery>,
) -> Result<Json<RadarResponse>, AppError> {
    // Validate query parameters
    let lat = query.lat;
    let lng = query.lng;
    let radius = query.radius.clamp(5.0, 100.0);

    // Get cached radar data
    let cache_guard = state.radar_cache.read().await;
    let cache = cache_guard.as_ref().ok_or(AppError::DataNotAvailable)?;

    // Create circular clip area using geo::Buffer
    let clip_circle = create_circle(lat, lng, radius);

    let mut result = RadarResponse {
        timestamp: cache.date_str.clone(),
        center: LatLng { lat, lng },
        radius_km: radius,
        contours: Vec::new(),
        total_points: 0,
        contour_count: 0,
    };

    for precomputed in &cache.contours {
        // Intersect pre-computed polygons with clip circle using geo::BooleanOps
        let clipped = precomputed.polygons.intersection(&clip_circle);

        // Encode each polygon's exterior ring
        for polygon in clipped.0.iter() {
            let ring = polygon.exterior();

            // Skip tiny polygons
            if ring.0.len() < 4 {
                continue;
            }

            // Convert to (lat, lon) format for polyline encoding
            let coords: Vec<(f64, f64)> = ring.0.iter().map(|c| (c.y, c.x)).collect();

            let encoded = encode_polyline(&coords, 5);
            if !encoded.is_empty() {
                let points = coords.len();
                result.contours.push(ContourData {
                    rain_rate: precomputed.rain_rate,
                    polyline: encoded,
                    points,
                });
                result.total_points += points;
            }
        }
    }

    result.contour_count = result.contours.len();

    Ok(Json(result))
}

/// GET /api/radar/all - Get all pre-computed radar contours (no clipping)
pub async fn get_radar_all(
    State(state): State<AppState>,
) -> Result<Json<RadarAllResponse>, AppError> {
    // Get cached radar data
    let cache_guard = state.radar_cache.read().await;
    let cache = cache_guard.as_ref().ok_or(AppError::DataNotAvailable)?;

    let mut result = RadarAllResponse {
        timestamp: cache.date_str.clone(),
        contours: Vec::new(),
        total_points: 0,
        contour_count: 0,
    };

    for precomputed in &cache.contours {
        // Encode each polygon's exterior ring without clipping
        for polygon in precomputed.polygons.0.iter() {
            let ring = polygon.exterior();

            // Skip tiny polygons
            if ring.0.len() < 4 {
                continue;
            }

            // Convert to (lat, lon) format for polyline encoding
            let coords: Vec<(f64, f64)> = ring.0.iter().map(|c| (c.y, c.x)).collect();

            let encoded = encode_polyline(&coords, 5);
            if !encoded.is_empty() {
                let points = coords.len();
                result.contours.push(ContourData {
                    rain_rate: precomputed.rain_rate,
                    polyline: encoded,
                    points,
                });
                result.total_points += points;
            }
        }
    }

    result.contour_count = result.contours.len();

    Ok(Json(result))
}
