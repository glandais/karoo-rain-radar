use ndarray::Array2;

/// Cached radar data with raw ACRR values for on-demand contour computation
pub struct RadarCache {
    /// Radar data timestamp string (e.g., "20260127T143500Z")
    pub date_str: String,
    /// Raw ACRR array (rain intensity in 0.01 mm units)
    pub data: Array2<f64>,
    /// Projection definition string (PROJ format)
    pub projdef: String,
    /// X origin in projection coordinates
    pub x_origin: f64,
    /// Y origin in projection coordinates
    pub y_origin: f64,
    /// X scale (meters per pixel)
    pub x_scale: f64,
    /// Y scale (meters per pixel)
    pub y_scale: f64,
}

// Array2<f64> is Send+Sync, so RadarCache is too
unsafe impl Send for RadarCache {}
unsafe impl Sync for RadarCache {}
