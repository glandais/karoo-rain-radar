/// Encode coordinates as Google Polyline with given precision
/// Coordinates are in (lat, lon) format
pub fn encode_polyline(coords: &[(f64, f64)], precision: u32) -> String {
    if coords.is_empty() {
        return String::new();
    }

    // Use the polyline crate
    let line: geo::LineString<f64> = coords
        .iter()
        .map(|(lat, lon)| geo::Coord { x: *lon, y: *lat })
        .collect();

    polyline::encode_coordinates(line.coords().cloned(), precision)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_coords() {
        let encoded = encode_polyline(&[], 5);
        assert!(encoded.is_empty());
    }
}
