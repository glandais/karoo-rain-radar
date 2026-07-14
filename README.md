# Karoo Rain Radar

Rain radar overlay for Karoo GPS bike computers, using Meteo-France radar data.

## Architecture

```
[Meteo-France API] --> [Rust Backend] --> [Karoo Extension]
      |                      |                    |
   HDF5 (2MB)         PNG tile / polylines   Rain overlay
                             |
                    - Contour extraction
                    - Spatial filtering (bbox)
                    - PNG tile rasterization
                    - Google polyline encoding
```

This project extracts rain intensity from Meteo-France radar data and serves it to
the Karoo extension either as a PNG overlay tile or as encoded contour polylines.

## Components

### Backend (`backend/`)

Rust (Axum) server that processes Meteo-France radar data. See
[`backend/README.md`](backend/README.md) for full build and run instructions.

### Web Frontend (`backend/static/`)

Simple Leaflet-based web interface for testing and visualizing radar data.

### Karoo Extension (`karoo-rain-radar/`)

Android extension that fetches and displays the rain overlay on the Karoo map.

## Setup

### 1. Backend

See [`backend/README.md`](backend/README.md) for prerequisites (HDF5 + PROJ)
and details. Quick start:

Create a `.env` file with your Meteo-France API key:
```
METEO_FRANCE_API_KEY=your_api_key_here
RUST_LOG=info
```

Run the server:
```bash
cd backend
cargo run --release
```

Verify it works:
```bash
curl "http://localhost:8080/api/radar?min_lat=48.5&max_lat=49.2&min_lng=2.0&max_lng=2.7"
```

Open http://localhost:8080 in a browser to use the web frontend.

### 2. Karoo Extension

#### Prerequisites

- Android Studio or command-line build tools
- GitHub account with personal access token (for karoo-ext package)

#### Configuration

1. Edit `karoo-rain-radar/local.properties`:
```properties
gpr.user=YOUR_GITHUB_USERNAME
gpr.key=YOUR_GITHUB_TOKEN
```

Get a token from https://github.com/settings/tokens with `read:packages` scope.

2. Set your backend URL in the extension sources (`RadarPreviewType.kt` /
   `RadarDrawService.kt`):
```kotlin
private const val BACKEND_URL = "http://YOUR_SERVER_IP:8080"
```

#### Build

```bash
cd karoo-rain-radar
./gradlew assembleDebug
```

The APK will be at `app/build/outputs/apk/debug/app-debug.apk`

#### Install

```bash
adb install app/build/outputs/apk/debug/app-debug.apk
```

## API Reference

All spatial queries use a bounding box (`min_lat`, `max_lat`, `min_lng`, `max_lng`).

### GET /api/radar

Returns rain contours as encoded polylines, clipped to the bounding box.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| min_lat | float | Yes | South edge of the bounding box |
| max_lat | float | Yes | North edge of the bounding box |
| min_lng | float | Yes | West edge of the bounding box |
| max_lng | float | Yes | East edge of the bounding box |

**Response:**
```json
{
  "timestamp": "20260127T080000Z",
  "bbox": {"min_lat": 48.5, "max_lat": 49.2, "min_lng": 2.0, "max_lng": 2.7},
  "contours": [
    {
      "rain_rate": 0.2,
      "polyline": "encoded_polyline_string",
      "points": 42
    }
  ],
  "local_maxima": [
    {"lat": 48.85, "lng": 2.35, "rain_rate": 1.4}
  ],
  "total_points": 1234,
  "contour_count": 43
}
```

### GET /api/radar/tile

Returns the rain overlay as a transparent PNG image for the bounding box.

**Parameters:** `min_lat`, `max_lat`, `min_lng`, `max_lng` (required), plus optional
`width` / `height` (default 256, max 2048).

### GET /api/health

Health check endpoint. Returns `{"status": "ok"}`.

## Configuration

### Rain Intensity Thresholds

The backend extracts contours at five ACRR thresholds (in `backend/src/routes/radar.rs`),
in 0.01 mm units — `20, 40, 70, 100, 150` — corresponding to 0.2 / 0.4 / 0.7 / 1.0 / 1.5 mm/h.

### Refresh Interval

- Backend polling: every ~10 s, new data expected ~7 min after each radar timestamp
- Extension refresh: driven by location/zoom updates

## Meteo-France API

This project uses the [DonneesPubliquesRadar API](https://portail-api.meteofrance.fr/).

- **Zone**: METROPOLE (mainland France)
- **Product**: ACRR (Accumulated Rain Rate)
- **Resolution**: 500m grid
- **Update frequency**: Every 5 minutes
- **Coverage**: France (~38°N to 54°N, ~10°W to 18°E)

To get an API key, register at https://portail-api.meteofrance.fr/

## Limitations

- Coverage limited to France (Meteo-France data)
- Karoo SDK limitation: polylines only, no filled polygons
- Response size must stay under 100KB (HTTP limit on Karoo)

## Future Improvements

- [ ] Settings screen to configure backend URL
- [ ] Use actual GPS position instead of hardcoded coordinates
- [ ] Local data caching on Karoo
- [ ] Rain-on-route alerts
- [ ] Data field widget showing rain intensity

## License

MIT
