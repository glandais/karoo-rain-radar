use geo::{Buffer, MultiPolygon, Point};

/// Create a circular buffer around a point (for clipping)
/// Radius in km, converted to approximate degrees
pub fn create_circle(lat: f64, lon: f64, radius_km: f64) -> MultiPolygon<f64> {
    // Approximate degrees per km at given latitude
    let km_per_deg_lat = 111.0;
    let km_per_deg_lon = 111.0 * lat.to_radians().cos();

    // Average scale for the buffer (since buffer creates a circle in coordinate space)
    let avg_deg_per_km = 1.0 / ((km_per_deg_lat + km_per_deg_lon) / 2.0);
    let radius_deg = radius_km * avg_deg_per_km;

    // Create point and buffer it to form a circle
    let center = Point::new(lon, lat);
    center.buffer(radius_deg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_circle() {
        let circle = create_circle(48.85, 2.35, 10.0);
        assert!(!circle.0.is_empty(), "Circle should have at least one polygon");
        assert!(!circle.0[0].exterior().0.is_empty(), "Circle should have vertices");
    }
}
