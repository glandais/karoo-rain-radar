use contour::ContourBuilder;
use ndarray::Array2;

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
            // The geometry value contains the coordinates
            if let geojson::Value::MultiPolygon(multi_polygon) = geometry.value {
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
    fn test_no_contour_above_threshold() {
        let data = array![
            [1.0, 1.0],
            [1.0, 1.0],
        ];

        let contours = find_contours(&data, 0.5);
        assert!(contours.is_empty(), "Should find no contours when all above");
    }
}
