# Rain Radar Backend

Rain radar backend for GPS bike computers. Fetches rain radar data from Meteo-France, processes HDF5 files, and serves rain contours as Google Polylines.

## Features

- Real-time rain radar data from Meteo-France (updated every ~7 minutes)
- Contour extraction at three intensity levels (light, moderate, heavy)
- Google Polyline encoding for compact transmission
- Circular area clipping around query point
- Web UI with Leaflet map for visualization

## Prerequisites

### macOS (Homebrew)

```bash
brew install hdf5@1.10 proj
```

### Debian/Ubuntu

```bash
apt-get install libhdf5-dev libproj-dev
```

## Setup

1. Get an API key from [Meteo-France API Portal](https://portail-api.meteofrance.fr/)

2. Create a `.env` file:
   ```
   METEO_FRANCE_API_KEY=your_jwt_token
   RUST_LOG=info
   ```

3. Build and run:
   ```bash
   # macOS with Homebrew
   ./start.sh

   # Or directly
   export HDF5_DIR=/opt/homebrew/opt/hdf5@1.10  # macOS only
   cargo run --release
   ```

4. Open http://localhost:8080 for the web UI

## API

### GET /api/radar

Returns rain contours as encoded polylines.

**Parameters:**
| Name | Type | Default | Description |
|------|------|---------|-------------|
| `lat` | float | required | Latitude (WGS84) |
| `lng` | float | required | Longitude (WGS84) |
| `radius` | int | 30 | Radius in km (5-100) |

**Example:**
```bash
curl "http://localhost:8080/api/radar?lat=48.8566&lng=2.3522&radius=30"
```

**Response:**
```json
{
  "timestamp": "2024-01-15T12:30:00Z",
  "contours": [
    {
      "level": "light",
      "color": "#00FF00",
      "polyline": "encoded_polyline_string"
    },
    {
      "level": "moderate",
      "color": "#FFFF00",
      "polyline": "encoded_polyline_string"
    },
    {
      "level": "heavy",
      "color": "#FF0000",
      "polyline": "encoded_polyline_string"
    }
  ]
}
```

### GET /api/health

Health check endpoint.

## Docker

```bash
docker-compose up --build
```

Or build manually:
```bash
docker build -t rain-radar .
docker run -p 8080:8080 -e METEO_FRANCE_API_KEY=your_key rain-radar
```

## Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────┐
│  Meteo-France   │────▶│  Background      │────▶│  In-Memory  │
│  API (HDF5)     │     │  Ticker (10s)    │     │  Cache      │
└─────────────────┘     └──────────────────┘     └──────┬──────┘
                                                        │
┌─────────────────┐     ┌──────────────────┐           │
│  Client Request │────▶│  /api/radar      │◀──────────┘
│  (lat, lng, r)  │     │  Handler         │
└─────────────────┘     └────────┬─────────┘
                                 │
                    ┌────────────┼────────────┐
                    ▼            ▼            ▼
              ┌──────────┐ ┌──────────┐ ┌──────────┐
              │ Contour  │ │ Coord    │ │ Polyline │
              │ Extract  │ │ Transform│ │ Encode   │
              └──────────┘ └──────────┘ └──────────┘
```

## License

MIT
