use serde::{Deserialize, Serialize};

/// Bounding box
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BBox {
    pub min_lat: f64,
    pub max_lat: f64,
    pub min_lng: f64,
    pub max_lng: f64,
}

/// Local maximum rain intensity point
#[derive(Debug, Serialize)]
pub struct LocalMaximum {
    pub lat: f64,
    pub lng: f64,
    pub rain_rate: f64, // mm/h
}

/// API response for radar endpoint
#[derive(Debug, Serialize)]
pub struct RadarResponse {
    pub timestamp: String,
    pub bbox: BBox,
    pub contours: Vec<ContourData>,
    pub local_maxima: Vec<LocalMaximum>,
    pub total_points: usize,
    pub contour_count: usize,
}

/// Single contour with encoded polyline
#[derive(Debug, Serialize)]
pub struct ContourData {
    pub rain_rate: f64,
    pub polyline: String,
    pub points: usize,
}

/// Query parameters for radar endpoint
#[derive(Debug, Deserialize)]
pub struct RadarQuery {
    pub min_lat: f64,
    pub max_lat: f64,
    pub min_lng: f64,
    pub max_lng: f64,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
}
