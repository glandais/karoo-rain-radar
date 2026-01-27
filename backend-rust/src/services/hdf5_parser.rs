use hdf5::File;
use ndarray::Array2;
use proj::Proj;
use std::io::Write;
use tempfile::NamedTempFile;

use crate::error::AppError;

/// Intermediate parsed radar data before contour computation
pub struct ParsedRadarData {
    /// Radar data array (rain intensity in 0.01 mm units)
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
    /// Radar data timestamp string (e.g., "20260127T143500Z")
    pub date_str: String,
}

/// Parse HDF5 radar data and extract projection info
pub fn parse_hdf5(data: &[u8]) -> Result<ParsedRadarData, AppError> {
    // Write data to a temporary file (hdf5 crate requires a file path)
    let mut tmp_file = NamedTempFile::new()
        .map_err(|e| AppError::Hdf5(format!("Failed to create temp file: {}", e)))?;
    tmp_file
        .write_all(data)
        .map_err(|e| AppError::Hdf5(format!("Failed to write temp file: {}", e)))?;
    let tmp_path = tmp_file.path();

    // Open HDF5 file
    let file = File::open(tmp_path)?;

    // Extract radar data
    let dataset = file.dataset("dataset1/data1/data")?;
    let raw_data: Array2<u16> = dataset.read()?;

    // Get data attributes for conversion
    let what_group = file.group("dataset1/data1/what")?;
    let gain: f64 = what_group.attr("gain")?.read_scalar()?;
    let offset: f64 = what_group.attr("offset")?.read_scalar()?;
    let nodata: u16 = what_group.attr("nodata")?.read_scalar()?;
    let undetect: u16 = what_group.attr("undetect")?.read_scalar()?;

    // Convert to physical values (ACRR in 0.01 mm)
    let shape = raw_data.dim();
    let mut data_array = Array2::<f64>::zeros(shape);
    for ((row, col), &val) in raw_data.indexed_iter() {
        if val == nodata {
            data_array[[row, col]] = f64::NAN;
        } else if val == undetect {
            data_array[[row, col]] = 0.0;
        } else {
            // Convert to 0.01 mm units (multiply by 100 since gain gives mm)
            data_array[[row, col]] = (val as f64 * gain + offset) * 100.0;
        }
    }

    // Get projection info
    let where_group = file.group("where")?;

    // Read projdef - may be stored as bytes or string
    let projdef: String = read_string_attr(&where_group, "projdef")?;
    let x_scale: f64 = where_group.attr("xscale")?.read_scalar()?;
    let y_scale: f64 = where_group.attr("yscale")?.read_scalar()?;

    // Get UL corner coordinates with defaults
    let ul_lat: f64 = where_group
        .attr("UL_lat")
        .and_then(|a| a.read_scalar())
        .unwrap_or(53.67);
    let ul_lon: f64 = where_group
        .attr("UL_lon")
        .and_then(|a| a.read_scalar())
        .unwrap_or(-9.965);

    // Validate projection by creating one temporarily
    let proj_from_wgs84 = Proj::new_known_crs("EPSG:4326", &projdef, None)
        .map_err(|e| AppError::Projection(format!("Failed to create projection: {}", e)))?;

    // Compute origin in projection coordinates
    let (x_origin, y_origin) = proj_from_wgs84
        .convert((ul_lon, ul_lat))
        .map_err(|e| AppError::Projection(format!("Failed to convert UL corner: {}", e)))?;

    // Get date info
    let what_root = file.group("what")?;
    let date_str = read_string_attr(&what_root, "date")?;
    let time_str = read_string_attr(&what_root, "time")?;

    Ok(ParsedRadarData {
        data: data_array,
        projdef,
        x_origin,
        y_origin,
        x_scale,
        y_scale,
        date_str: format!("{}T{}Z", date_str, time_str),
    })
}

/// Read a string attribute that may be stored as bytes or a fixed-length string
fn read_string_attr(group: &hdf5::Group, name: &str) -> Result<String, AppError> {
    let attr = group.attr(name)?;

    // Try reading as String first
    if let Ok(s) = attr.read_scalar::<hdf5::types::VarLenUnicode>() {
        return Ok(s.to_string());
    }

    // Try reading as fixed-length string
    if let Ok(s) = attr.read_scalar::<hdf5::types::FixedAscii<256>>() {
        return Ok(s.to_string());
    }

    // Try reading raw bytes and converting
    if let Ok(bytes) = attr.read_raw::<u8>() {
        let s = String::from_utf8_lossy(&bytes);
        return Ok(s.trim_end_matches('\0').to_string());
    }

    Err(AppError::Hdf5(format!("Failed to read attribute '{}' as string", name)))
}
