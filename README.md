# Karoo Rain Radar

Rain radar overlay for Karoo GPS bike computers, using Meteo-France radar data.

## Architecture

```
[Meteo-France API] --> [Python Backend] --> [Karoo Extension]
      |                       |                     |
   HDF5 (2MB)          JSON (<100KB)         Polyline overlay
                              |
                     - Contour extraction
                     - Spatial filtering
                     - Google polyline encoding
```

The Karoo SDK only supports vector overlays (polylines), not raster tiles. This project extracts rain intensity contours from radar data and renders them as colored polylines on the map.

## Components

### Backend (`backend/`)

FastAPI server that processes Meteo-France radar data.

### Web Frontend (`backend/static/`)

Simple Leaflet-based web interface for testing and visualizing radar data.

### Karoo Extension (`karoo-rain-radar/`)

Android extension that fetches and displays rain contours on the Karoo map.

## Setup

### 1. Backend

```bash
cd backend
pip install -r requirements.txt
```

Create a `.env` file in the project root with your Meteo-France API key:
```
METEO_FRANCE_API_KEY=your_api_key_here
```

Run the server:
```bash
python radar_server.py
# Or with uvicorn for production:
uvicorn radar_server:app --host 0.0.0.0 --port 8080
```

Verify it works:
```bash
curl "http://localhost:8080/api/radar?lat=48.85&lng=2.35&radius=30"
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

2. Edit `RadarDrawService.kt` line 36 to set your backend URL:
```kotlin
private val backendUrl = "http://YOUR_SERVER_IP:8080/api/radar"
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

### GET /api/radar

Returns rain contours as encoded polylines.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| lat | float | Yes | Latitude of center point |
| lng | float | Yes | Longitude of center point |
| radius | float | No | Radius in km (default: 30, range: 5-100) |

**Response:**
```json
{
  "timestamp": "20260127T080000Z",
  "center": {"lat": 48.85, "lng": 2.35},
  "radius_km": 30,
  "contours": [
    {
      "level": "light",
      "color": "#4000FF00",
      "polyline": "encoded_polyline_string",
      "points": 42
    }
  ],
  "total_points": 1234,
  "contour_count": 43
}
```

### GET /api/health

Health check endpoint. Returns `{"status": "ok"}`.

## Configuration

### Rain Intensity Thresholds

The backend extracts contours at three intensity levels (in `radar_server.py`):

| Level | Threshold | Description |
|-------|-----------|-------------|
| light | 0.1 mm | Light rain/drizzle |
| moderate | 1.0 mm | Moderate rain |
| heavy | 4.0 mm | Heavy rain |

### Colors

Contour colors (ARGB format):

| Level | Color | Visual |
|-------|-------|--------|
| light | `#4000FF00` | Transparent green |
| moderate | `#60FFFF00` | Semi-transparent yellow |
| heavy | `#80FF0000` | Semi-transparent red |

### Refresh Interval

- Backend cache: 5 minutes (`CACHE_TTL`)
- Extension refresh: 5 minutes (`refreshIntervalMs`)

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
