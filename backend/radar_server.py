"""
Rain Radar Backend Server for Karoo GPS
Fetches radar data from Meteo-France, extracts contours, and serves as polylines.
"""

import asyncio
import io
import os
import tempfile
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from functools import lru_cache
from typing import Optional

import h5py
import httpx
import numpy as np
import polyline
from fastapi import FastAPI, HTTPException, Query
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from fastapi.responses import FileResponse
from pyproj import Transformer
from shapely import Point, LineString, MultiLineString
from shapely.geometry.base import BaseGeometry
from skimage import measure

# Configuration
METEO_API_BASE = "https://public-api.meteofrance.fr/public/DPRadar/v1"
ZONE = "METROPOLE"
OBSERVATION = "LAME_D_EAU"  # Water depth (precipitation)
MAILLE = 500  # 500m resolution

# Rain intensity thresholds (mm/h equivalent from ACRR values)
# ACRR values are in 0.01 mm units, so we convert
THRESHOLDS = {
    "light": 10,      # 0.1 mm - light rain
    "moderate": 50,  # 0.5 mm - moderate rain
    "heavy": 100,     # 1.0 mm - heavy rain
}

# Colors (ARGB format for Karoo)
COLORS = {
    "light": "#00FF00",      # Green with transparency
    "moderate": "#FFFF00",   # Yellow with transparency
    "heavy": "#FF0000",      # Red with transparency
}

app = FastAPI(title="Rain Radar API", version="1.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)

# Serve static files
STATIC_DIR = os.path.join(os.path.dirname(__file__), "static")
if os.path.exists(STATIC_DIR):
    app.mount("/static", StaticFiles(directory=STATIC_DIR), name="static")


@dataclass
class RadarCache:
    """Cached radar data with timestamp."""
    data: np.ndarray
    transformer: Transformer
    x_origin: float
    y_origin: float
    x_scale: float
    y_scale: float
    timestamp: float
    date_str: str


# Global cache for radar data
_radar_cache: Optional[RadarCache] = None
_cache_lock = asyncio.Lock()
_download_task: Optional[asyncio.Task] = None

# Meteo-France updates every 5 minutes, we add 10 seconds margin
DOWNLOAD_DELAY_SECONDS = 5 * 60 + 10


def get_api_key() -> str:
    """Get API key from environment."""
    key = os.environ.get("METEO_FRANCE_API_KEY")
    if not key:
        # Try loading from .env file
        env_path = os.path.join(os.path.dirname(__file__), "..", ".env")
        if os.path.exists(env_path):
            with open(env_path) as f:
                for line in f:
                    if line.startswith("METEO_FRANCE_API_KEY="):
                        key = line.split("=", 1)[1].strip()
                        break
    if not key:
        raise RuntimeError("METEO_FRANCE_API_KEY not set")
    return key


async def fetch_latest_timestamp() -> str:
    """Fetch the latest available radar timestamp from Meteo-France API."""
    api_key = get_api_key()
    url = f"{METEO_API_BASE}/mosaiques/{ZONE}/observations/{OBSERVATION}"

    async with httpx.AsyncClient(timeout=10.0) as client:
        response = await client.get(url, headers={"apikey": api_key})
        response.raise_for_status()
        data = response.json()

        # Find the link for our maille (500m)
        for link in data.get("links", []):
            if f"maille={MAILLE}" in link.get("href", ""):
                validity_time = link.get("validity_time")
                if validity_time:
                    # Convert "2026-01-27T17:40:00Z" to "20260127T174000Z"
                    dt = datetime.fromisoformat(validity_time.replace("Z", "+00:00"))
                    return dt.strftime("%Y%m%dT%H%M%SZ")

        raise RuntimeError("No radar data available for maille " + str(MAILLE))


async def fetch_radar_data() -> bytes:
    """Fetch latest radar HDF5 data from Meteo-France API."""
    api_key = get_api_key()
    url = f"{METEO_API_BASE}/mosaiques/{ZONE}/observations/{OBSERVATION}/produit"

    async with httpx.AsyncClient(timeout=30.0) as client:
        response = await client.get(
            url,
            params={"maille": MAILLE},
            headers={"apikey": api_key}
        )
        response.raise_for_status()
        return response.content


def parse_hdf5(data: bytes) -> RadarCache:
    """Parse HDF5 radar data and extract projection info."""
    with tempfile.NamedTemporaryFile(suffix=".h5", delete=False) as tmp:
        tmp.write(data)
        tmp_path = tmp.name

    try:
        with h5py.File(tmp_path, "r") as f:
            # Extract radar data
            raw_data = f["dataset1/data1/data"][:]

            # Get data attributes for conversion
            gain = f["dataset1/data1/what"].attrs["gain"]
            offset = f["dataset1/data1/what"].attrs["offset"]
            nodata = f["dataset1/data1/what"].attrs["nodata"]
            undetect = f["dataset1/data1/what"].attrs["undetect"]

            # Convert to physical values (ACRR in 0.01 mm)
            data_array = raw_data.astype(np.float32)
            data_array[raw_data == nodata] = np.nan
            data_array[raw_data == undetect] = 0
            data_array = data_array * gain + offset
            # Convert to 0.01 mm units (multiply by 100 since gain gives mm)
            data_array = data_array * 100

            # Get projection info
            where_attrs = dict(f["where"].attrs)
            projdef = where_attrs["projdef"]
            if isinstance(projdef, bytes):
                projdef = projdef.decode()

            x_scale = where_attrs["xscale"]
            y_scale = where_attrs["yscale"]
            x_size = where_attrs["xsize"]
            y_size = where_attrs["ysize"]

            # Create transformer from stereographic to WGS84
            transformer = Transformer.from_crs(projdef, "EPSG:4326", always_xy=True)

            # For ODIM HDF5, the grid origin (pixel 0,0) corresponds to the UL corner
            # We need to compute the projection coordinates of the UL corner
            # Using the stored corner coordinates from the file
            ul_lat = where_attrs.get("UL_lat", 53.67)
            ul_lon = where_attrs.get("UL_lon", -9.965)
            fwd_transformer = Transformer.from_crs("EPSG:4326", projdef, always_xy=True)
            x_origin, y_origin = fwd_transformer.transform(ul_lon, ul_lat)

            # Get date info
            date_str = f["what"].attrs["date"]
            time_str = f["what"].attrs["time"]
            if isinstance(date_str, bytes):
                date_str = date_str.decode()
            if isinstance(time_str, bytes):
                time_str = time_str.decode()

            return RadarCache(
                data=data_array,
                transformer=transformer,
                x_origin=x_origin,
                y_origin=y_origin,
                x_scale=x_scale,
                y_scale=y_scale,
                timestamp=time.time(),
                date_str=f"{date_str}T{time_str}Z"
            )
    finally:
        os.unlink(tmp_path)


def pixel_to_latlon(cache: RadarCache, row: int, col: int) -> tuple[float, float]:
    """Convert pixel coordinates to lat/lon."""
    # ODIM HDF5 grid: pixel (0,0) is upper-left corner
    # X increases with column from origin, Y decreases with row from origin
    x = cache.x_origin + col * cache.x_scale
    y = cache.y_origin - row * cache.y_scale

    # Transform to WGS84
    lon, lat = cache.transformer.transform(x, y)
    return lat, lon


def clip_contour(coords: list[tuple[float, float]], area: BaseGeometry) -> list[list[tuple[float, float]]]:
    """Clip a contour to a Shapely geometry (circle, box, etc.)."""
    if not coords or len(coords) < 2:
        return []

    # Convert coords (lat, lon) to (lon, lat) for Shapely
    line_coords = [(lon, lat) for lat, lon in coords]
    line = LineString(line_coords)

    # Intersect
    clipped = line.intersection(area)

    if clipped.is_empty:
        return []

    # Convert back to list of segments
    segments = []
    if isinstance(clipped, LineString):
        # Single segment - convert back to (lat, lon)
        segment = [(lat, lon) for lon, lat in clipped.coords]
        if len(segment) >= 2:
            segments.append(segment)
    elif isinstance(clipped, MultiLineString):
        # Multiple segments
        for geom in clipped.geoms:
            segment = [(lat, lon) for lon, lat in geom.coords]
            if len(segment) >= 2:
                segments.append(segment)

    return segments


def create_circle(lat: float, lon: float, radius_km: float) -> BaseGeometry:
    """Create a circular area around a point. Radius in km, approximated in degrees."""
    # Approximate degrees per km at given latitude
    km_per_deg_lat = 111.0
    km_per_deg_lon = 111.0 * np.cos(np.radians(lat))

    # Average scale for the buffer (elliptical approximation)
    avg_deg_per_km = 1.0 / ((km_per_deg_lat + km_per_deg_lon) / 2)
    radius_deg = radius_km * avg_deg_per_km

    # Create circle: Point(lon, lat) because Shapely uses (x, y) = (lon, lat)
    return Point(lon, lat).buffer(radius_deg, resolution=32)


def extract_contours(cache: RadarCache, threshold: float, area: Optional[BaseGeometry] = None) -> list[list[tuple[float, float]]]:
    """Extract contours at given threshold, clipped to area."""
    data = cache.data

    # Replace NaN with 0 for contour finding (NaN causes issues)
    data_clean = np.nan_to_num(data, nan=0.0)

    # Find contours using marching squares
    contours = measure.find_contours(data_clean, threshold)

    result = []
    for contour in contours:
        # Convert pixel coordinates to lat/lon
        coords = []
        for row, col in contour:
            lat, lon = pixel_to_latlon(cache, row, col)
            coords.append((lat, lon))

        if not coords:
            continue

        # Clip contour to area if provided
        if area:
            clipped_segments = clip_contour(coords, area)
            for segment in clipped_segments:
                if len(segment) >= 2:
                    result.append(segment)
        else:
            if len(coords) >= 3:
                result.append(coords)

    return result


def simplify_contour(coords: list[tuple[float, float]], tolerance: float = 0.0005) -> list[tuple[float, float]]:
    """Simplify contour using Douglas-Peucker algorithm."""
    if len(coords) < 3:
        return coords

    # Use numpy for efficient computation
    points = np.array(coords)

    def perpendicular_distance(point, line_start, line_end):
        if np.allclose(line_start, line_end):
            return np.linalg.norm(point - line_start)

        line_vec = line_end - line_start
        point_vec = point - line_start
        line_len = np.linalg.norm(line_vec)
        line_unitvec = line_vec / line_len
        proj_length = np.dot(point_vec, line_unitvec)
        proj_length = max(0, min(line_len, proj_length))
        proj_point = line_start + proj_length * line_unitvec
        return np.linalg.norm(point - proj_point)

    def rdp(points, epsilon):
        if len(points) < 3:
            return points

        # Find point with maximum distance
        dmax = 0
        index = 0
        end = len(points) - 1

        for i in range(1, end):
            d = perpendicular_distance(points[i], points[0], points[end])
            if d > dmax:
                dmax = d
                index = i

        # If max distance is greater than epsilon, recursively simplify
        if dmax > epsilon:
            left = rdp(points[:index + 1], epsilon)
            right = rdp(points[index:], epsilon)
            return np.vstack([left[:-1], right])
        else:
            return np.array([points[0], points[end]])

    simplified = rdp(points, tolerance)
    return [tuple(p) for p in simplified]


def parse_radar_timestamp(date_str: str) -> datetime:
    """Parse radar timestamp from format 'YYYYMMDDTHHMMSSZ' to datetime."""
    # Format: "20260127T143500Z"
    return datetime.strptime(date_str, "%Y%m%dT%H%M%SZ").replace(tzinfo=timezone.utc)


def calculate_next_poll_time(date_str: str) -> float:
    """Calculate timestamp when we should start polling for new data."""
    radar_time = parse_radar_timestamp(date_str)
    # New data expected at radar_time + 7 minutes
    return radar_time.timestamp() + 7 * 60


async def check_and_download():
    """Check if new data is available and download it."""
    global _radar_cache

    log_prefix = f"[{datetime.now(timezone.utc).isoformat()}]"
    now = time.time()

    # If we have cached data, check if it's time to poll
    if _radar_cache is not None:
        next_poll_time = calculate_next_poll_time(_radar_cache.date_str)
        if now < next_poll_time:
            # Not yet time to poll
            return

    try:
        # Check latest available timestamp
        latest_timestamp = await fetch_latest_timestamp()

        current_timestamp = _radar_cache.date_str if _radar_cache else None

        if current_timestamp is None or latest_timestamp != current_timestamp:
            # New data available, download it
            print(f"{log_prefix} New data available: {latest_timestamp}")

            async with _cache_lock:
                data = await fetch_radar_data()
                _radar_cache = parse_hdf5(data)
                print(f"{log_prefix} Radar data downloaded: {_radar_cache.date_str}")

                next_poll = calculate_next_poll_time(_radar_cache.date_str)
                wait_seconds = next_poll - time.time()
                print(f"{log_prefix} Next poll in {wait_seconds:.0f} seconds")
        else:
            print(f"{log_prefix} Waiting for new data (current: {current_timestamp})...")

    except Exception as e:
        print(f"{log_prefix} Error: {e}")


async def ticker_loop():
    """Run check_and_download every 10 seconds."""
    while True:
        try:
            await check_and_download()
        except Exception as e:
            print(f"[{datetime.now(timezone.utc).isoformat()}] Ticker error: {e}")
        await asyncio.sleep(10)


async def get_cached_radar() -> RadarCache:
    """Get radar data from cache."""
    if _radar_cache is None:
        raise HTTPException(status_code=503, detail="Radar data not yet available")
    return _radar_cache


@app.on_event("startup")
async def startup_event():
    """Start the ticker loop on startup."""
    global _download_task
    _download_task = asyncio.create_task(ticker_loop())


@app.on_event("shutdown")
async def shutdown_event():
    """Cancel ticker loop on shutdown."""
    global _download_task
    if _download_task:
        _download_task.cancel()
        try:
            await _download_task
        except asyncio.CancelledError:
            pass


@app.get("/api/radar")
async def get_radar(
    lat: float = Query(..., description="Latitude of center point"),
    lng: float = Query(..., description="Longitude of center point"),
    radius: float = Query(30, description="Radius in km", ge=5, le=100)
):
    """
    Get rain radar contours as encoded polylines.

    Returns contours for three rain intensity levels:
    - light: > 0.1 mm/h
    - moderate: > 1.0 mm/h
    - heavy: > 4.0 mm/h
    """
    cache = await get_cached_radar()

    # Create circular area around the center point
    area = create_circle(lat, lng, radius)

    result = {
        "timestamp": cache.date_str,
        "center": {"lat": lat, "lng": lng},
        "radius_km": radius,
        "contours": []
    }

    total_points = 0

    for level, threshold in THRESHOLDS.items():
        contours = extract_contours(cache, threshold, area)

        for coords in contours:
            # Simplify contour to reduce size
            simplified = simplify_contour(coords)

            if len(simplified) < 2:
                continue

            # Encode as Google polyline (precision 5)
            encoded = polyline.encode(simplified, 5)

            result["contours"].append({
                "level": level,
                "color": COLORS[level],
                "polyline": encoded,
                "points": len(simplified)
            })

            total_points += len(simplified)

    result["total_points"] = total_points
    result["contour_count"] = len(result["contours"])

    return result


@app.get("/api/health")
async def health():
    """Health check endpoint."""
    return {"status": "ok"}


@app.get("/")
async def root():
    """Serve the web frontend."""
    index_path = os.path.join(STATIC_DIR, "index.html")
    if os.path.exists(index_path):
        return FileResponse(index_path)
    return {
        "name": "Rain Radar API",
        "version": "1.0",
        "endpoints": {
            "/api/radar": "Get rain contours (params: lat, lng, radius)",
            "/api/health": "Health check"
        }
    }


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8080)
