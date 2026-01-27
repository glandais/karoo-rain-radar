use geo::MultiPolygon;

/// Pre-computed contour at a specific rain threshold
pub struct PrecomputedContour {
    /// Rain rate
    pub rain_rate: f64,
    /// Pre-computed polygons in lat/lng coordinates, already simplified
    pub polygons: MultiPolygon<f64>,
}

/// Cached radar data with pre-computed contours
pub struct RadarCache {
    /// Radar data timestamp string (e.g., "20260127T143500Z")
    pub date_str: String,
    /// Pre-computed contours for each rain threshold
    pub contours: Vec<PrecomputedContour>,
}

// Implement Send and Sync since MultiPolygon is Send+Sync
unsafe impl Send for RadarCache {}
unsafe impl Sync for RadarCache {}
