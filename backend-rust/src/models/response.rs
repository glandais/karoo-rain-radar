use serde::{Deserialize, Serialize};

/// API response for radar endpoint
#[derive(Debug, Serialize)]
pub struct RadarResponse {
    pub timestamp: String,
    pub center: LatLng,
    pub radius_km: f64,
    pub contours: Vec<ContourData>,
    pub total_points: usize,
    pub contour_count: usize,
}

/// Latitude/longitude point
#[derive(Debug, Serialize, Deserialize)]
pub struct LatLng {
    pub lat: f64,
    pub lng: f64,
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
    pub lat: f64,
    pub lng: f64,
    #[serde(default = "default_radius")]
    pub radius: f64,
}

fn default_radius() -> f64 {
    30.0
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
}
