use axum::{
    extract::{Query, State},
    Json,
};
use ndarray::s;
use proj::Proj;

use crate::config::{colors, thresholds};
use crate::error::AppError;
use crate::models::{ContourData, LatLng, RadarCache, RadarQuery, RadarResponse};
use crate::services::{
    clip_contour, create_circle, encode_polyline, find_contours,
    simplify_contour_dp,
};
use crate::state::AppState;

/// Convert lat/lon to pixel coordinates in the radar grid
fn latlon_to_pixel(cache: &RadarCache, lat: f64, lon: f64) -> Option<(f64, f64)> {
    let proj = Proj::new_known_crs("EPSG:4326", &cache.projdef, None).ok()?;
    let (x, y) = proj.convert((lon, lat)).ok()?;

    // Reverse of pixel_to_latlon:
    // x = x_origin + col * x_scale  =>  col = (x - x_origin) / x_scale
    // y = y_origin - row * y_scale  =>  row = (y_origin - y) / y_scale
    let col = (x - cache.x_origin) / cache.x_scale;
    let row = (cache.y_origin - y) / cache.y_scale;

    Some((row, col))
}

/// Convert pixel coordinates to lat/lon using provided projection
fn pixel_to_latlon_with_proj(cache: &RadarCache, proj: &Proj, row: f64, col: f64) -> Option<(f64, f64)> {
    let x = cache.x_origin + col * cache.x_scale;
    let y = cache.y_origin - row * cache.y_scale;

    match proj.convert((x, y)) {
        Ok((lon, lat)) => Some((lat, lon)),
        Err(_) => None,
    }
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

    // Create circular area around the center point
    let area = create_circle(lat, lng, radius);

    let mut result = RadarResponse {
        timestamp: cache.date_str.clone(),
        center: LatLng { lat, lng },
        radius_km: radius,
        contours: Vec::new(),
        total_points: 0,
        contour_count: 0,
    };

    // Convert center to pixel coordinates
    let (center_row, center_col) = latlon_to_pixel(cache, lat, lng)
        .ok_or_else(|| AppError::Internal("Failed to convert coordinates".to_string()))?;

    // Calculate pixel radius (approximate: 1 degree ~ 111 km, so radius_km / 111 degrees)
    // Then convert to pixels using scale (meters per pixel)
    let radius_meters = radius * 1000.0;
    let pixel_radius = radius_meters / cache.x_scale;

    // Add margin for contours that might extend slightly beyond
    let margin = pixel_radius * 0.2;
    let half_size = pixel_radius + margin;

    // Calculate bounds
    let (data_rows, data_cols) = cache.data.dim();
    let row_min = ((center_row - half_size).floor() as usize).max(0);
    let row_max = ((center_row + half_size).ceil() as usize + 1).min(data_rows);
    let col_min = ((center_col - half_size).floor() as usize).max(0);
    let col_max = ((center_col + half_size).ceil() as usize + 1).min(data_cols);

    // Extract subset of data
    let data_subset = cache.data.slice(s![row_min..row_max, col_min..col_max]).to_owned();

    // Create projection once for all coordinate conversions
    let proj = Proj::new_known_crs(&cache.projdef, "EPSG:4326", None)
        .map_err(|e| AppError::Projection(format!("Failed to create projection: {}", e)))?;

    let color_map = colors();
    let threshold_list = thresholds();

    for (level, threshold) in threshold_list {
        // Find contours at this threshold in the subset
        let contours = find_contours(&data_subset, threshold);

        for contour in contours {
            // Convert pixel coordinates to lat/lon (adjusting for subset offset)
            let coords: Vec<(f64, f64)> = contour
                .iter()
                .filter_map(|(row, col)| {
                    // Adjust coordinates back to full grid
                    let full_row = row + row_min as f64;
                    let full_col = col + col_min as f64;
                    pixel_to_latlon_with_proj(cache, &proj, full_row, full_col)
                })
                .collect();

            if coords.is_empty() {
                continue;
            }

            // Clip contour to area
            let clipped_segments = clip_contour(&coords, &area);

            for segment in clipped_segments {
                if segment.len() < 2 {
                    continue;
                }

                // Simplify contour to reduce size
                let simplified = simplify_contour_dp(&segment, 0.0005);

                if simplified.len() < 2 {
                    continue;
                }

                // Encode as Google polyline (precision 5)
                let encoded = encode_polyline(&simplified, 5);

                result.contours.push(ContourData {
                    level: level.to_string(),
                    color: color_map.get(level).unwrap_or(&"#0000FF").to_string(),
                    polyline: encoded,
                    points: simplified.len(),
                });

                result.total_points += simplified.len();
            }
        }
    }

    result.contour_count = result.contours.len();

    Ok(Json(result))
}
