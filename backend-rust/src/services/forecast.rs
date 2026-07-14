use ndarray::Array2;
use proj::Proj;

use crate::error::AppError;

/// Rain intensity at a specific location
pub struct RainAtLocation {
    pub rain_rate: f64,       // mm/h at exact location
    pub is_raining: bool,     // Above threshold (0.2 mm/h)
}

/// Sample rain intensity around user's location for 12 sectors (like a clock)
/// Returns intensities for 12 sectors (30° each), representing rain around the user
pub struct RainForecastData {
    /// Rain rate at current location (mm/h)
    pub current_rain_rate: f64,
    /// Whether it's currently raining at user's location
    pub is_raining: bool,
    /// Rain intensities for 12 sectors around the user (0=North, clockwise)
    /// Each value is the max rain rate in that 30° sector within the sample radius
    pub sectors: [f64; 12],
    /// Minutes until rain change (positive = rain arriving, negative = rain ending)
    /// None if no change expected within analysis radius
    pub minutes_to_change: Option<i32>,
    /// Distance to nearest rain edge in km
    pub distance_to_edge_km: Option<f64>,
}

/// Minimum rain threshold (0.2 mm/h = 20 in 0.01mm units)
const RAIN_THRESHOLD: f64 = 20.0;

/// Default cloud/weather movement speed for time estimation (km/h)
const DEFAULT_WEATHER_SPEED_KMH: f64 = 30.0;

/// Get rain intensity at a specific lat/lng coordinate
pub fn get_rain_at_location(
    lat: f64,
    lng: f64,
    data: &Array2<f64>,
    projdef: &str,
    x_origin: f64,
    y_origin: f64,
    x_scale: f64,
    y_scale: f64,
) -> Result<RainAtLocation, AppError> {
    let (rows, cols) = data.dim();

    // Create projection from WGS84 to radar coordinates
    let proj = Proj::new_known_crs("EPSG:4326", projdef, None)
        .map_err(|e| AppError::Projection(format!("Failed to create projection: {}", e)))?;

    // Convert lat/lng to projection coordinates
    let (x, y) = proj
        .convert((lng, lat))
        .map_err(|e| AppError::Projection(format!("Failed to convert coordinates: {}", e)))?;

    // Convert to pixel indices
    let col = ((x - x_origin) / x_scale).round() as i64;
    let row = ((y_origin - y) / y_scale).round() as i64;

    // Check bounds
    if row < 0 || row >= rows as i64 || col < 0 || col >= cols as i64 {
        return Ok(RainAtLocation {
            rain_rate: 0.0,
            is_raining: false,
        });
    }

    let value = data[[row as usize, col as usize]];
    let rain_rate = if value.is_nan() { 0.0 } else { value / 100.0 }; // Convert to mm/h

    Ok(RainAtLocation {
        rain_rate,
        is_raining: value >= RAIN_THRESHOLD,
    })
}

/// Calculate rain forecast data for a location
/// Sample radius is in km
pub fn calculate_forecast(
    lat: f64,
    lng: f64,
    sample_radius_km: f64,
    data: &Array2<f64>,
    projdef: &str,
    x_origin: f64,
    y_origin: f64,
    x_scale: f64,
    y_scale: f64,
) -> Result<RainForecastData, AppError> {
    let (rows, cols) = data.dim();

    // Create projection
    let proj = Proj::new_known_crs("EPSG:4326", projdef, None)
        .map_err(|e| AppError::Projection(format!("Failed to create projection: {}", e)))?;

    // Get current location value
    let current = get_rain_at_location(
        lat, lng, data, projdef, x_origin, y_origin, x_scale, y_scale
    )?;

    // Convert center to projection coordinates
    let (center_x, center_y) = proj
        .convert((lng, lat))
        .map_err(|e| AppError::Projection(format!("Failed to convert center: {}", e)))?;

    // Sample radius in meters
    let radius_m = sample_radius_km * 1000.0;

    // Initialize sectors
    let mut sectors = [0.0f64; 12];

    // Sample at multiple distances and angles
    let distances = [0.25, 0.5, 0.75, 1.0]; // Fractions of radius
    let angles_per_sector = 5;

    for sector in 0..12 {
        let base_angle = (sector as f64) * 30.0; // 30° per sector
        let mut max_value = 0.0f64;

        for dist_frac in &distances {
            let dist = radius_m * dist_frac;

            for a in 0..angles_per_sector {
                // Sample within the sector
                let angle_deg = base_angle + (a as f64) * (30.0 / angles_per_sector as f64);
                let angle_rad = angle_deg.to_radians();

                // North is 0°, clockwise
                let dx = dist * angle_rad.sin();
                let dy = dist * angle_rad.cos();

                let sample_x = center_x + dx;
                let sample_y = center_y + dy;

                // Convert to pixel
                let col = ((sample_x - x_origin) / x_scale).round() as i64;
                let row = ((y_origin - sample_y) / y_scale).round() as i64;

                if row >= 0 && row < rows as i64 && col >= 0 && col < cols as i64 {
                    let value = data[[row as usize, col as usize]];
                    if !value.is_nan() && value > max_value {
                        max_value = value;
                    }
                }
            }
        }

        // Convert to mm/h
        sectors[sector] = max_value / 100.0;
    }

    // Find distance to nearest rain edge (transition point)
    let (distance_to_edge_km, minutes_to_change) = find_rain_edge(
        center_x,
        center_y,
        sample_radius_km,
        current.is_raining,
        data,
        x_origin,
        y_origin,
        x_scale,
        y_scale,
        rows,
        cols,
    );

    Ok(RainForecastData {
        current_rain_rate: current.rain_rate,
        is_raining: current.is_raining,
        sectors,
        minutes_to_change,
        distance_to_edge_km,
    })
}

/// Find the distance to the nearest rain edge (where rain state changes)
fn find_rain_edge(
    center_x: f64,
    center_y: f64,
    max_radius_km: f64,
    currently_raining: bool,
    data: &Array2<f64>,
    x_origin: f64,
    y_origin: f64,
    x_scale: f64,
    y_scale: f64,
    rows: usize,
    cols: usize,
) -> (Option<f64>, Option<i32>) {
    let max_radius_m = max_radius_km * 1000.0;
    let step_m = 500.0; // 500m steps
    let num_angles = 36; // Every 10°

    let mut min_distance = f64::MAX;

    for angle_idx in 0..num_angles {
        let angle_rad = (angle_idx as f64 * 10.0).to_radians();

        let mut distance = step_m;
        let mut prev_raining = currently_raining;

        while distance <= max_radius_m {
            let dx = distance * angle_rad.sin();
            let dy = distance * angle_rad.cos();

            let sample_x = center_x + dx;
            let sample_y = center_y + dy;

            let col = ((sample_x - x_origin) / x_scale).round() as i64;
            let row = ((y_origin - sample_y) / y_scale).round() as i64;

            if row >= 0 && row < rows as i64 && col >= 0 && col < cols as i64 {
                let value = data[[row as usize, col as usize]];
                let is_raining = !value.is_nan() && value >= RAIN_THRESHOLD;

                // Check for transition
                if is_raining != prev_raining {
                    if distance < min_distance {
                        min_distance = distance;
                    }
                    break;
                }
                prev_raining = is_raining;
            } else {
                // Out of bounds
                break;
            }

            distance += step_m;
        }
    }

    if min_distance < f64::MAX {
        let distance_km = min_distance / 1000.0;
        // Estimate time based on default weather speed
        let minutes = (distance_km / DEFAULT_WEATHER_SPEED_KMH * 60.0).round() as i32;
        // Positive if rain is coming (not currently raining), negative if rain ending
        let signed_minutes = if currently_raining { -minutes } else { minutes };
        (Some(distance_km), Some(signed_minutes))
    } else {
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    #[test]
    fn test_rain_threshold() {
        assert_eq!(RAIN_THRESHOLD, 20.0); // 0.2 mm/h
    }
}
