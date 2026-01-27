use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;

use crate::config::{get_api_key, METEO_API_BASE, ZONE, OBSERVATION, MAILLE};
use crate::error::AppError;

#[derive(Debug, Deserialize)]
struct ApiResponse {
    links: Vec<Link>,
}

#[derive(Debug, Deserialize)]
struct Link {
    href: String,
    validity_time: Option<String>,
}

/// Fetch the latest available radar timestamp from Meteo-France API
pub async fn fetch_latest_timestamp(client: &Client) -> Result<String, AppError> {
    let api_key = get_api_key().map_err(|e| AppError::Config(e.to_string()))?;
    let url = format!("{}/mosaiques/{}/observations/{}", METEO_API_BASE, ZONE, OBSERVATION);

    let response = client
        .get(&url)
        .header("apikey", &api_key)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(AppError::MeteoApi(format!(
            "API returned status: {}",
            response.status()
        )));
    }

    let data: ApiResponse = response.json().await?;

    // Find the link for our maille (500m)
    for link in data.links {
        if link.href.contains(&format!("maille={}", MAILLE)) {
            if let Some(validity_time) = link.validity_time {
                // Convert "2026-01-27T17:40:00Z" to "20260127T174000Z"
                let dt = DateTime::parse_from_rfc3339(&validity_time)
                    .map_err(|e| AppError::MeteoApi(format!("Invalid timestamp: {}", e)))?;
                let dt_utc: DateTime<Utc> = dt.into();
                return Ok(dt_utc.format("%Y%m%dT%H%M%SZ").to_string());
            }
        }
    }

    Err(AppError::MeteoApi(format!(
        "No radar data available for maille {}",
        MAILLE
    )))
}

/// Fetch radar HDF5 data from Meteo-France API
pub async fn fetch_radar_data(client: &Client) -> Result<Vec<u8>, AppError> {
    let api_key = get_api_key().map_err(|e| AppError::Config(e.to_string()))?;
    let url = format!(
        "{}/mosaiques/{}/observations/{}/produit",
        METEO_API_BASE, ZONE, OBSERVATION
    );

    let response = client
        .get(&url)
        .query(&[("maille", MAILLE.to_string())])
        .header("apikey", &api_key)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(AppError::MeteoApi(format!(
            "API returned status: {}",
            response.status()
        )));
    }

    Ok(response.bytes().await?.to_vec())
}
