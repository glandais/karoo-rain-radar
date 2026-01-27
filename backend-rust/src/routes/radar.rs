use axum::{
    extract::{Query, State},
    Json,
};
use geo::BooleanOps;
use proj::Proj;

use crate::error::AppError;
use crate::models::{BBox, ContourData, LocalMaximum, RadarQuery, RadarResponse};
use crate::services::{
    bbox_to_pixel_indices, compute_subset_contours, create_bbox_polygon,
    encode_polyline, extract_subset, find_local_maxima, pixel_to_latlon,
};
use crate::state::AppState;

/// Fixed ACRR thresholds in 0.01mm units
/// Corresponding to 0.2, 0.4, 0.7, 1.0, 1.5 mm/h
const THRESHOLDS: [f64; 5] = [20.0, 40.0, 70.0, 100.0, 150.0];

/// Minimum threshold for local maxima (first contour level)
const MIN_THRESHOLD: f64 = 20.0;

/// Maximum number of local maxima to return
const MAX_LOCAL_MAXIMA: usize = 5;

/// Window size for local maxima detection (7 pixels = 3.5km at 500m resolution)
const MAXIMA_WINDOW_SIZE: usize = 7;

/// GET /api/radar - Get rain radar contours clipped to bounding box
pub async fn get_radar(
    State(state): State<AppState>,
    Query(query): Query<RadarQuery>,
) -> Result<Json<RadarResponse>, AppError> {
    let bbox = BBox {
        min_lat: query.min_lat,
        max_lat: query.max_lat,
        min_lng: query.min_lng,
        max_lng: query.max_lng,
    };

    // Get cached radar data
    let cache_guard = state.radar_cache.read().await;
    let cache = cache_guard.as_ref().ok_or(AppError::DataNotAvailable)?;

    let (data_rows, data_cols) = cache.data.dim();

    // Convert bbox to pixel indices
    let (min_row, max_row, min_col, max_col) = bbox_to_pixel_indices(
        bbox.min_lat,
        bbox.max_lat,
        bbox.min_lng,
        bbox.max_lng,
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
    let subset_view = subset.view();

    // Create projection for coordinate conversion
    let proj = Proj::new_known_crs(&cache.projdef, "EPSG:4326", None)
        .map_err(|e| AppError::Projection(format!("Failed to create projection: {}", e)))?;

    // Find local maxima (only those above the minimum contour threshold)
    let pixel_maxima = find_local_maxima(&subset_view, MAX_LOCAL_MAXIMA, MAXIMA_WINDOW_SIZE);

    let local_maxima: Vec<LocalMaximum> = pixel_maxima
        .into_iter()
        .filter(|pm| pm.value >= MIN_THRESHOLD)
        .filter_map(|pm| {
            // Convert subset pixel to full array pixel
            let full_row = min_row + pm.row;
            let full_col = min_col + pm.col;

            pixel_to_latlon(
                full_row,
                full_col,
                &proj,
                cache.x_origin,
                cache.y_origin,
                cache.x_scale,
                cache.y_scale,
            )
            .map(|(lat, lng)| LocalMaximum {
                lat,
                lng,
                rain_rate: pm.value / 100.0, // Convert to mm/h
            })
        })
        .collect();

    // Compute contours at fixed thresholds
    let contours_data = compute_subset_contours(
        &subset,
        &THRESHOLDS,
        &proj,
        min_row,
        min_col,
        cache.x_origin,
        cache.y_origin,
        cache.x_scale,
        cache.y_scale,
    );

    // Create clip polygon from bbox
    let clip_polygon = create_bbox_polygon(bbox.min_lat, bbox.max_lat, bbox.min_lng, bbox.max_lng);

    let mut result = RadarResponse {
        timestamp: cache.date_str.clone(),
        bbox: bbox.clone(),
        contours: Vec::new(),
        local_maxima,
        total_points: 0,
        contour_count: 0,
    };

    for (rain_rate, multi_polygon) in contours_data {
        // Clip to bbox using geo::BooleanOps
        let clipped = multi_polygon.intersection(&clip_polygon);

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
                    rain_rate,
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
