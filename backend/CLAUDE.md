# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rain radar backend for GPS bike computers. Fetches rain radar data from Meteo-France API, processes HDF5 files, and serves rain contours as Google Polylines via REST API.

## Build & Run Commands

```bash
# Build (requires HDF5 and PROJ libraries)
cargo build --release

# Run locally (macOS with Homebrew - handles HDF5 path)
./start.sh

# Run directly
METEO_FRANCE_API_KEY=your_key cargo run --release

# Run tests
cargo test

# Run single test
cargo test test_name

# Docker
docker-compose up --build
```

**macOS Prerequisites:**
```bash
brew install hdf5@1.10 proj
export HDF5_DIR=/opt/homebrew/opt/hdf5@1.10
```

## Environment Setup

Create `.env` file:
```
METEO_FRANCE_API_KEY=your_jwt_token  # Required - from https://portail-api.meteofrance.fr/
RUST_LOG=info                         # Optional
```

## Architecture

```
src/
├── main.rs          # Server setup, background ticker (polls every 10s)
├── config.rs        # API URLs, rain thresholds, polling intervals
├── state.rs         # AppState with Arc<RwLock<Option<RadarCache>>>
├── error.rs         # AppError enum → HTTP status mapping
├── routes/
│   ├── radar.rs     # GET /api/radar - main endpoint
│   └── health.rs    # GET /api/health
├── models/
│   ├── radar_cache.rs   # Cached radar data + projection info
│   └── response.rs      # API response types, RadarQuery params
└── services/
    ├── meteo_france.rs    # API client (timestamp + HDF5 download)
    ├── hdf5_parser.rs     # Parse HDF5 → RadarCache
    ├── contour.rs         # Marching squares contour extraction
    ├── geometry.rs        # Circle creation, contour clipping, Douglas-Peucker
    └── polyline_encoder.rs
```

## Data Flow

1. **Background ticker** polls Meteo-France every 10s, downloads new HDF5 data when available
2. **On API request** (`/api/radar?lat=48.85&lng=2.35&radius=30`):
   - Extract data subset around query point
   - Find contours at rain thresholds (light=10, moderate=50, heavy=100 in 0.01mm units)
   - Convert pixel coords → lat/lng via PROJ
   - Clip to circular area, simplify with Douglas-Peucker
   - Encode as Google Polyline (precision 5)

## Key Patterns

- **Coordinate transformation**: Radar data uses Lambert projection. Use `Proj::new_known_crs()` for pixel↔latlon conversion
- **Contour extraction**: Uses `contour` crate's marching squares, outputs GeoJSON MultiPolygon
- **Error handling**: `AppError` implements `IntoResponse` for automatic HTTP status codes
- **State sharing**: `AppState` wrapped in `Arc` for handler access

## API Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /api/radar?lat=&lng=&radius=` | Rain contours as polylines (radius: 5-100km) |
| `GET /api/health` | Health check |
| `GET /` | Static web UI (Leaflet map) |

## Configuration Constants (config.rs)

- Polling interval: 10 seconds
- Rain thresholds: 10 (light/green), 50 (moderate/yellow), 100 (heavy/red)
- Grid resolution: 500m (maille)
- Line simplification tolerance: 0.0005 degrees
