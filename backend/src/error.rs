use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Meteo-France API error: {0}")]
    MeteoApi(String),

    #[error("HDF5 parsing error: {0}")]
    Hdf5(String),

    #[error("Projection error: {0}")]
    Projection(String),

    #[error("Radar data not yet available")]
    DataNotAvailable,

    #[error("Invalid bounding box")]
    InvalidBbox,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Config(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::MeteoApi(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            AppError::Hdf5(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::Projection(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::DataNotAvailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Radar data not yet available".to_string(),
            ),
            AppError::InvalidBbox => (
                StatusCode::BAD_REQUEST,
                "Invalid or out-of-bounds bounding box".to_string(),
            ),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let body = Json(json!({
            "error": message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::MeteoApi(err.to_string())
    }
}

impl From<hdf5::Error> for AppError {
    fn from(err: hdf5::Error) -> Self {
        AppError::Hdf5(err.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}
