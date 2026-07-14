use geo::{Coord, LineString, Polygon};
use ndarray::{s, Array2, ArrayView2};
use proj::Proj;

use crate::error::AppError;

/// Create a rectangular polygon from bounding box coordinates
pub fn create_bbox_polygon(min_lat: f64, max_lat: f64, min_lng: f64, max_lng: f64) -> Polygon<f64> {
    // Coords are (x, y) = (lng, lat)
    let coords = vec![
        Coord { x: min_lng, y: min_lat },
        Coord { x: max_lng, y: min_lat },
        Coord { x: max_lng, y: max_lat },
        Coord { x: min_lng, y: max_lat },
        Coord { x: min_lng, y: min_lat }, // Close the ring
    ];

    Polygon::new(LineString::new(coords), vec![])
}

/// Convert lat/lng bounding box to pixel array indices
/// Returns (min_row, max_row, min_col, max_col) clamped to array bounds
pub fn bbox_to_pixel_indices(
    min_lat: f64,
    max_lat: f64,
    min_lng: f64,
    max_lng: f64,
    projdef: &str,
    x_origin: f64,
    y_origin: f64,
    x_scale: f64,
    y_scale: f64,
    data_rows: usize,
    data_cols: usize,
) -> Result<(usize, usize, usize, usize), AppError> {
    // Create projection from WGS84 to radar coordinates
    let proj = Proj::new_known_crs("EPSG:4326", projdef, None)
        .map_err(|e| AppError::Projection(format!("Failed to create projection: {}", e)))?;

    // Transform all 4 corners to projection coordinates
    let corners = [
        (min_lng, min_lat),
        (min_lng, max_lat),
        (max_lng, min_lat),
        (max_lng, max_lat),
    ];

    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;

    for (lon, lat) in corners {
        let (x, y) = proj
            .convert((lon, lat))
            .map_err(|e| AppError::Projection(format!("Failed to convert corner: {}", e)))?;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    // Convert projection coordinates to pixel indices
    // row = (y_origin - y) / y_scale
    // col = (x - x_origin) / x_scale
    let min_col = ((min_x - x_origin) / x_scale).floor() as i64;
    let max_col = ((max_x - x_origin) / x_scale).ceil() as i64;
    let min_row = ((y_origin - max_y) / y_scale).floor() as i64; // Note: y is inverted
    let max_row = ((y_origin - min_y) / y_scale).ceil() as i64;

    // Clamp to array bounds
    let min_row = min_row.max(0) as usize;
    let max_row = (max_row as usize).min(data_rows);
    let min_col = min_col.max(0) as usize;
    let max_col = (max_col as usize).min(data_cols);

    // Ensure we have valid ranges
    if min_row >= max_row || min_col >= max_col {
        return Err(AppError::InvalidBbox);
    }

    Ok((min_row, max_row, min_col, max_col))
}

/// Convert a single pixel (row, col) to lat/lng coordinates
pub fn pixel_to_latlon(
    row: usize,
    col: usize,
    proj: &Proj,
    x_origin: f64,
    y_origin: f64,
    x_scale: f64,
    y_scale: f64,
) -> Option<(f64, f64)> {
    // Convert pixel to projection coordinates
    let x = x_origin + col as f64 * x_scale;
    let y = y_origin - row as f64 * y_scale;

    // Convert to lat/lng
    proj.convert((x, y))
        .ok()
        .map(|(lon, lat)| (lat, lon))
}

/// Local maximum with pixel position and value
pub struct PixelMaximum {
    pub row: usize,
    pub col: usize,
    pub value: f64,
}

/// Find local maxima in a 2D array using non-maximum suppression
/// Returns up to `max_count` maxima, sorted by value descending
/// `window_size` is the suppression window (should be odd, e.g., 7 for 3.5km radius at 500m resolution)
pub fn find_local_maxima(
    data: &ArrayView2<f64>,
    max_count: usize,
    window_size: usize,
) -> Vec<PixelMaximum> {
    let (rows, cols) = data.dim();
    if rows == 0 || cols == 0 {
        return Vec::new();
    }

    let half_window = window_size / 2;

    // Collect all valid values with their positions
    let mut candidates: Vec<(usize, usize, f64)> = Vec::new();

    for row in 0..rows {
        for col in 0..cols {
            let val = data[[row, col]];
            // Skip NaN and zero values
            if val.is_nan() || val <= 0.0 {
                continue;
            }

            // Check if this is a local maximum within the window
            let row_start = row.saturating_sub(half_window);
            let row_end = (row + half_window + 1).min(rows);
            let col_start = col.saturating_sub(half_window);
            let col_end = (col + half_window + 1).min(cols);

            let mut is_max = true;
            'outer: for r in row_start..row_end {
                for c in col_start..col_end {
                    if r == row && c == col {
                        continue;
                    }
                    let neighbor = data[[r, c]];
                    if !neighbor.is_nan() && neighbor > val {
                        is_max = false;
                        break 'outer;
                    }
                }
            }

            if is_max {
                candidates.push((row, col, val));
            }
        }
    }

    // Sort by value descending
    candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Apply non-maximum suppression to get distinct peaks
    let mut result: Vec<PixelMaximum> = Vec::new();
    let mut suppressed = vec![false; candidates.len()];

    for i in 0..candidates.len() {
        if suppressed[i] || result.len() >= max_count {
            continue;
        }

        let (row, col, value) = candidates[i];
        result.push(PixelMaximum { row, col, value });

        // Suppress nearby candidates
        for j in (i + 1)..candidates.len() {
            if suppressed[j] {
                continue;
            }
            let (r2, c2, _) = candidates[j];
            let row_diff = (row as i64 - r2 as i64).unsigned_abs() as usize;
            let col_diff = (col as i64 - c2 as i64).unsigned_abs() as usize;
            if row_diff <= half_window && col_diff <= half_window {
                suppressed[j] = true;
            }
        }
    }

    result
}

/// Extract a subset of the data array
pub fn extract_subset(
    data: &Array2<f64>,
    min_row: usize,
    max_row: usize,
    min_col: usize,
    max_col: usize,
) -> Array2<f64> {
    data.slice(s![min_row..max_row, min_col..max_col]).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_create_bbox_polygon() {
        let bbox = create_bbox_polygon(48.0, 49.0, 2.0, 3.0);
        assert_eq!(bbox.exterior().0.len(), 5); // 4 corners + closing point
    }

    #[test]
    fn test_find_local_maxima_simple() {
        let data = array![
            [0.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 5.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 10.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0],
        ];
        let view = data.view();
        let maxima = find_local_maxima(&view, 5, 3);

        assert_eq!(maxima.len(), 2);
        assert_eq!(maxima[0].value, 10.0);
        assert_eq!(maxima[0].row, 2);
        assert_eq!(maxima[0].col, 3);
        assert_eq!(maxima[1].value, 5.0);
    }

    #[test]
    fn test_find_local_maxima_with_nan() {
        let data = array![
            [f64::NAN, 5.0, f64::NAN],
            [5.0, 10.0, 5.0],
            [f64::NAN, 5.0, f64::NAN],
        ];
        let view = data.view();
        let maxima = find_local_maxima(&view, 5, 3);

        assert_eq!(maxima.len(), 1);
        assert_eq!(maxima[0].value, 10.0);
    }

}
