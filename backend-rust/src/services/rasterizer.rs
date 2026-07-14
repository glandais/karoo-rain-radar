use image::{ImageBuffer, ImageEncoder, Rgba, RgbaImage};
use ndarray::Array2;

/// Rain intensity thresholds and corresponding RGBA colors
/// Values are in 0.01mm units (same as ACRR data)
/// Color format: (R, G, B, A)
const COLOR_PALETTE: &[(f64, (u8, u8, u8, u8))] = &[
    // threshold, (R, G, B, A)
    (20.0, (173, 216, 230, 128)),  // 0.2 mm/h - Light blue, alpha 0.5
    (50.0, (0, 255, 0, 153)),      // 0.5 mm/h - Green, alpha 0.6
    (100.0, (255, 255, 0, 179)),   // 1.0 mm/h - Yellow, alpha 0.7
    (200.0, (255, 165, 0, 204)),   // 2.0 mm/h - Orange, alpha 0.8
    (500.0, (255, 0, 0, 230)),     // 5.0 mm/h - Red, alpha 0.9
];

/// Convert a rain intensity value (in 0.01mm units) to RGBA color
fn intensity_to_rgba(value: f64) -> Rgba<u8> {
    if value.is_nan() || value < COLOR_PALETTE[0].0 {
        // Below threshold or NaN - fully transparent
        return Rgba([0, 0, 0, 0]);
    }

    // Find the appropriate color based on intensity
    let mut color = COLOR_PALETTE[COLOR_PALETTE.len() - 1].1;

    for (threshold, rgba) in COLOR_PALETTE.iter() {
        if value < *threshold {
            break;
        }
        color = *rgba;
    }

    Rgba([color.0, color.1, color.2, color.3])
}

/// Rasterize radar data subset to an RGBA image
///
/// # Arguments
/// * `data` - 2D array of rain intensity values (in 0.01mm units)
/// * `width` - Target image width
/// * `height` - Target image height
///
/// # Returns
/// An RgbaImage with the radar data rendered
pub fn rasterize_radar_data(data: &Array2<f64>, width: u32, height: u32) -> RgbaImage {
    let (data_rows, data_cols) = data.dim();

    if data_rows == 0 || data_cols == 0 {
        return ImageBuffer::new(width, height);
    }

    let mut img: RgbaImage = ImageBuffer::new(width, height);

    // Calculate scale factors for mapping image pixels to data pixels
    let x_scale = data_cols as f64 / width as f64;
    let y_scale = data_rows as f64 / height as f64;

    for y in 0..height {
        for x in 0..width {
            // Map image pixel to data pixel using bilinear interpolation
            let data_x = x as f64 * x_scale;
            let data_y = y as f64 * y_scale;

            let value = bilinear_interpolate(data, data_x, data_y, data_rows, data_cols);
            let color = intensity_to_rgba(value);

            img.put_pixel(x, y, color);
        }
    }

    img
}

/// Bilinear interpolation for sampling radar data
fn bilinear_interpolate(
    data: &Array2<f64>,
    x: f64,
    y: f64,
    rows: usize,
    cols: usize,
) -> f64 {
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(cols - 1);
    let y1 = (y0 + 1).min(rows - 1);

    let x_frac = x - x0 as f64;
    let y_frac = y - y0 as f64;

    let v00 = data[[y0, x0]];
    let v10 = data[[y0, x1]];
    let v01 = data[[y1, x0]];
    let v11 = data[[y1, x1]];

    // Handle NaN values - treat as 0
    let v00 = if v00.is_nan() { 0.0 } else { v00 };
    let v10 = if v10.is_nan() { 0.0 } else { v10 };
    let v01 = if v01.is_nan() { 0.0 } else { v01 };
    let v11 = if v11.is_nan() { 0.0 } else { v11 };

    // Bilinear interpolation
    let v0 = v00 * (1.0 - x_frac) + v10 * x_frac;
    let v1 = v01 * (1.0 - x_frac) + v11 * x_frac;

    v0 * (1.0 - y_frac) + v1 * y_frac
}

/// Encode an RGBA image to PNG bytes
pub fn encode_png(img: &RgbaImage) -> Vec<u8> {
    let mut buffer = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buffer);

    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .expect("Failed to encode PNG");

    buffer
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_intensity_to_rgba_transparent() {
        let color = intensity_to_rgba(0.0);
        assert_eq!(color, Rgba([0, 0, 0, 0]));
    }

    #[test]
    fn test_intensity_to_rgba_light_blue() {
        let color = intensity_to_rgba(25.0); // 0.25 mm/h
        assert_eq!(color, Rgba([173, 216, 230, 128]));
    }

    #[test]
    fn test_intensity_to_rgba_red() {
        let color = intensity_to_rgba(600.0); // 6 mm/h
        assert_eq!(color, Rgba([255, 0, 0, 230]));
    }

    #[test]
    fn test_rasterize_simple() {
        let data = array![
            [0.0, 50.0],
            [100.0, 500.0]
        ];
        let img = rasterize_radar_data(&data, 2, 2);
        assert_eq!(img.width(), 2);
        assert_eq!(img.height(), 2);
    }

    #[test]
    fn test_encode_png() {
        let img = ImageBuffer::from_fn(4, 4, |_, _| Rgba([255, 0, 0, 255]));
        let png_bytes = encode_png(&img);

        // PNG magic bytes
        assert_eq!(&png_bytes[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }
}
