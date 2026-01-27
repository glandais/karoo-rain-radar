use geo::{Coord, LineString, Point, Polygon};

/// Create a circular polygon around a point (for clipping)
/// Radius in km, approximated in degrees
pub fn create_circle(lat: f64, lon: f64, radius_km: f64) -> Polygon<f64> {
    // Approximate degrees per km at given latitude
    let km_per_deg_lat = 111.0;
    let km_per_deg_lon = 111.0 * lat.to_radians().cos();

    // Average scale for the buffer
    let avg_deg_per_km = 1.0 / ((km_per_deg_lat + km_per_deg_lon) / 2.0);
    let radius_deg = radius_km * avg_deg_per_km;

    // Create circle with 32 segments
    let num_segments = 32;
    let center = Point::new(lon, lat);
    let mut coords = Vec::with_capacity(num_segments + 1);

    for i in 0..=num_segments {
        let angle = 2.0 * std::f64::consts::PI * (i as f64) / (num_segments as f64);
        let x = center.x() + radius_deg * angle.cos();
        let y = center.y() + radius_deg * angle.sin();
        coords.push(Coord { x, y });
    }

    Polygon::new(LineString::new(coords), vec![])
}

/// Clip a contour (lat/lon coords) to a polygon area
/// Returns multiple segments if the contour crosses the boundary
pub fn clip_contour(coords: &[(f64, f64)], area: &Polygon<f64>) -> Vec<Vec<(f64, f64)>> {
    if coords.len() < 2 {
        return Vec::new();
    }

    // Check which points are inside the area and create segments
    let mut segments = Vec::new();
    let mut current_segment: Vec<(f64, f64)> = Vec::new();

    for (lat, lon) in coords {
        let point = Point::new(*lon, *lat);
        let inside = geo::algorithm::Contains::contains(area, &point);

        if inside {
            current_segment.push((*lat, *lon));
        } else {
            if current_segment.len() >= 2 {
                segments.push(std::mem::take(&mut current_segment));
            }
            current_segment.clear();
        }
    }

    // Don't forget the last segment
    if current_segment.len() >= 2 {
        segments.push(current_segment);
    }

    segments
}

/// Douglas-Peucker simplification (alternative to VW)
pub fn simplify_contour_dp(coords: &[(f64, f64)], tolerance: f64) -> Vec<(f64, f64)> {
    if coords.len() < 3 {
        return coords.to_vec();
    }

    fn perpendicular_distance(point: &(f64, f64), line_start: &(f64, f64), line_end: &(f64, f64)) -> f64 {
        let dx = line_end.0 - line_start.0;
        let dy = line_end.1 - line_start.1;

        if dx.abs() < 1e-10 && dy.abs() < 1e-10 {
            let pdx = point.0 - line_start.0;
            let pdy = point.1 - line_start.1;
            return (pdx * pdx + pdy * pdy).sqrt();
        }

        let line_len_sq = dx * dx + dy * dy;
        let t = ((point.0 - line_start.0) * dx + (point.1 - line_start.1) * dy) / line_len_sq;
        let t = t.clamp(0.0, 1.0);

        let proj_x = line_start.0 + t * dx;
        let proj_y = line_start.1 + t * dy;

        let pdx = point.0 - proj_x;
        let pdy = point.1 - proj_y;
        (pdx * pdx + pdy * pdy).sqrt()
    }

    fn rdp(points: &[(f64, f64)], epsilon: f64, result: &mut Vec<(f64, f64)>) {
        if points.len() < 2 {
            return;
        }

        let first = &points[0];
        let last = &points[points.len() - 1];

        // Find point with maximum distance
        let mut max_dist = 0.0;
        let mut max_idx = 0;

        for (i, point) in points.iter().enumerate().skip(1).take(points.len() - 2) {
            let dist = perpendicular_distance(point, first, last);
            if dist > max_dist {
                max_dist = dist;
                max_idx = i;
            }
        }

        if max_dist > epsilon {
            // Recursively simplify
            rdp(&points[..=max_idx], epsilon, result);
            result.pop(); // Remove duplicate point
            rdp(&points[max_idx..], epsilon, result);
        } else {
            // Just add endpoints
            result.push(*first);
            result.push(*last);
        }
    }

    let mut result = Vec::new();
    rdp(coords, tolerance, &mut result);

    // Remove consecutive duplicates
    result.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-10 && (a.1 - b.1).abs() < 1e-10);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_circle() {
        let circle = create_circle(48.85, 2.35, 10.0);
        assert!(!circle.exterior().0.is_empty());
    }

}
