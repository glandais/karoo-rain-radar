use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, Response, StatusCode},
};
use serde::Deserialize;

use crate::error::AppError;
use crate::services::{bbox_to_pixel_indices, encode_png, extract_subset, rasterize_radar_data};
use crate::state::AppState;

/// Query parameters for the tile endpoint
#[derive(Debug, Deserialize)]
pub struct TileQuery {
    pub min_lat: f64,
    pub max_lat: f64,
    pub min_lng: f64,
    pub max_lng: f64,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
}

fn default_width() -> u32 {
    256
}

fn default_height() -> u32 {
    256
}

/// GET /api/radar/tile - Get radar data as PNG image
///
/// Query parameters:
/// - min_lat, max_lat, min_lng, max_lng: Bounding box coordinates
/// - width, height: Output image dimensions (default: 256x256)
///
/// Returns:
/// - PNG image with transparent background and rain overlay
pub async fn get_tile(
    State(state): State<AppState>,
    Query(query): Query<TileQuery>,
) -> Result<Response<Body>, AppError> {
    // Validate dimensions
    let width = query.width.clamp(1, 2048);
    let height = query.height.clamp(1, 2048);

    // Get cached radar data
    let cache_guard = state.radar_cache.read().await;
    let cache = cache_guard.as_ref().ok_or(AppError::DataNotAvailable)?;

    let (data_rows, data_cols) = cache.data.dim();

    // Convert bbox to pixel indices
    let (min_row, max_row, min_col, max_col) = bbox_to_pixel_indices(
        query.min_lat,
        query.max_lat,
        query.min_lng,
        query.max_lng,
        &cache.projdef,
        cache.x_origin,
        cache.y_origin,
        cache.x_scale,
        cache.y_scale,
        data_rows,
        data_cols,
    )?;

    // Extract data subset
    let subset = extract_subset(&cache.data, min_row, max_row, min_col, max_col);

    // Rasterize to image
    let img = rasterize_radar_data(&subset, width, height);

    // Encode to PNG
    let png_bytes = encode_png(&img);

    // Build response with appropriate headers
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header(header::CACHE_CONTROL, "public, max-age=60") // Cache for 1 minute
        .header(
            header::CONTENT_DISPOSITION,
            "inline; filename=\"radar.png\"",
        )
        .body(Body::from(png_bytes))
        .map_err(|e| AppError::Internal(format!("Failed to build response: {}", e)))?;

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_dimensions() {
        assert_eq!(default_width(), 256);
        assert_eq!(default_height(), 256);
    }
}
