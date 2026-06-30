use contour::ContourBuilder;
use geo::{Coord, LineString, MultiPolygon, Polygon, Simplify};
use ndarray::Array2;
use proj::Proj;

/// Line simplification tolerance in degrees
const SIMPLIFY_TOLERANCE: f64 = 0.0005;

/// Compute contours from a data subset at given thresholds
/// Returns contours as MultiPolygons in lat/lng coordinates, already simplified
/// row_offset and col_offset are used to adjust pixel positions to the full array
pub fn compute_subset_contours(
    subset: &Array2<f64>,
    thresholds: &[f64],
    proj: &Proj,
    row_offset: usize,
    col_offset: usize,
    x_origin: f64,
    y_origin: f64,
    x_scale: f64,
    y_scale: f64,
) -> Vec<(f64, MultiPolygon<f64>)> {
    thresholds
        .iter()
        .filter_map(|&threshold| {
            // Find raw contours at this threshold
            let raw_contours = find_contours(subset, threshold);

            // Convert each contour ring to a polygon in lat/lng coordinates
            let polygons: Vec<Polygon<f64>> = raw_contours
                .into_iter()
                .filter_map(|ring| {
                    pixel_ring_to_polygon(
                        &ring,
                        proj,
                        row_offset,
                        col_offset,
                        x_origin,
                        y_origin,
                        x_scale,
                        y_scale,
                    )
                })
                .collect();

            if polygons.is_empty() {
                return None;
            }

            // Create MultiPolygon and simplify
            let multi = MultiPolygon::new(polygons);
            let simplified = multi.simplify(SIMPLIFY_TOLERANCE);

            let rain_rate = threshold / 100.0;

            Some((rain_rate, simplified))
        })
        .collect()
}

/// Convert a pixel-coordinate ring to a lat/lng polygon
/// row_offset and col_offset adjust for subset positioning
fn pixel_ring_to_polygon(
    ring: &[(f64, f64)],
    proj: &Proj,
    row_offset: usize,
    col_offset: usize,
    x_origin: f64,
    y_origin: f64,
    x_scale: f64,
    y_scale: f64,
) -> Option<Polygon<f64>> {
    let coords: Vec<Coord<f64>> = ring
        .iter()
        .filter_map(|(row, col)| {
            // Adjust pixel coordinates by offset (subset position in full array)
            let full_row = row + row_offset as f64;
            let full_col = col + col_offset as f64;

            // Convert pixel (row, col) to projection coordinates
            let x = x_origin + full_col * x_scale;
            let y = y_origin - full_row * y_scale;

            // Convert to lat/lng
            proj.convert((x, y))
                .ok()
                .map(|(lon, lat)| Coord { x: lon, y: lat })
        })
        .collect();

    // Need at least 4 points for a valid polygon (3 unique + closing)
    if coords.len() >= 4 {
        Some(Polygon::new(LineString::new(coords), vec![]))
    } else {
        None
    }
}

/// Extract contours at given threshold using the contour crate
pub fn find_contours(data: &Array2<f64>, threshold: f64) -> Vec<Vec<(f64, f64)>> {
    let (rows, cols) = data.dim();

    // Replace NaN with 0 for contour finding
    let mut data_clean = Vec::with_capacity(rows * cols);
    for row in 0..rows {
        for col in 0..cols {
            let val = data[[row, col]];
            if val.is_nan() {
                data_clean.push(0.0);
            } else {
                data_clean.push(val);
            }
        }
    }

    // Build contours using the contour crate
    let contour_builder = ContourBuilder::new(cols as u32, rows as u32, false);
    let features = contour_builder.contours(&data_clean, &[threshold]);

    let mut result = Vec::new();

    for feature in features {
        // Get the geometry from the feature
        if let Some(geometry) = feature.geometry {
            // The geometry value contains the coordinates.
            // Note: this matches against `geojson-legacy` (geojson 0.13), not
            // our direct `geojson` 1.0 dependency, because the `contour` crate
            // (v0.1) is internally pinned to geojson ^0.13 and returns that
            // version's `Feature`/`Geometry`/`Value` types from `contours()`.
            if let geojson_legacy::Value::MultiPolygon(multi_polygon) = geometry.value {
                for polygon in multi_polygon {
                    // Each polygon has rings: first is exterior, rest are holes
                    for ring in polygon {
                        let coords: Vec<(f64, f64)> = ring
                            .iter()
                            .map(|coord| {
                                // GeoJSON coords are [x, y] = [col, row]
                                // We need (row, col) for our pixel_to_latlon
                                (coord[1], coord[0])
                            })
                            .collect();

                        if coords.len() >= 3 {
                            result.push(coords);
                        }
                    }
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_simple_contour() {
        // Create a simple gradient field
        let data = array![
            [0.0, 0.0, 0.0],
            [0.5, 0.5, 0.5],
            [1.0, 1.0, 1.0],
        ];

        let contours = find_contours(&data, 0.5);
        assert!(!contours.is_empty(), "Should find at least one contour");
    }

    #[test]
    fn test_no_contour_below_threshold() {
        let data = array![
            [0.1, 0.1],
            [0.1, 0.1],
        ];

        let contours = find_contours(&data, 0.5);
        assert!(contours.is_empty(), "Should find no contours");
    }

    #[test]
    fn test_uniform_high_values() {
        // When all values are uniformly above threshold, contour crate may still
        // produce boundary contours. The important thing is no crash.
        let data = array![
            [1.0, 1.0],
            [1.0, 1.0],
        ];

        let contours = find_contours(&data, 0.5);
        // Just verify it doesn't crash - behavior depends on contour crate internals
        let _ = contours;
    }
}
