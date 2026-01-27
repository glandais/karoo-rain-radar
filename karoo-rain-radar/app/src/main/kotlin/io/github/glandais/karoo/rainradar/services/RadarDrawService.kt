package io.github.glandais.karoo.rainradar.services

import android.graphics.Color
import android.util.Log
import io.github.glandais.karoo.rainradar.extension.KarooRainRadarExtension.Companion.TAG
import io.hammerhead.karooext.internal.Emitter
import io.hammerhead.karooext.models.HidePolyline
import io.hammerhead.karooext.models.MapEffect
import io.hammerhead.karooext.models.OnLocationChanged
import io.hammerhead.karooext.models.OnMapZoomLevel
import io.hammerhead.karooext.models.ShowPolyline
import io.ktor.client.*
import io.ktor.client.call.*
import io.ktor.client.engine.android.*
import io.ktor.client.plugins.*
import io.ktor.client.plugins.contentnegotiation.*
import io.ktor.client.request.*
import io.ktor.serialization.kotlinx.json.*
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.FlowPreview
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import androidx.core.graphics.toColorInt
import kotlin.math.cos
import kotlin.math.pow
import kotlin.math.roundToInt

@Serializable
data class RadarContour(
    val level: String,
    val color: String,
    val polyline: String,
    val points: Int
)

@Serializable
data class RadarResponse(
    val timestamp: String,
    val center: Center,
    val radius_km: Double,
    val contours: List<RadarContour>,
    val total_points: Int,
    val contour_count: Int
)

@Serializable
data class Center(
    val lat: Double,
    val lng: Double
)

data class BoundingBox(
    val minLat: Double,
    val maxLat: Double,
    val minLng: Double,
    val maxLng: Double
) {
    fun toEncodedPolyline(): String {
        // Create a rectangle polyline: SW -> NW -> NE -> SE -> SW (closed)
        val points = listOf(
            minLat to minLng, // SW
            maxLat to minLng, // NW
            maxLat to maxLng, // NE
            minLat to maxLng, // SE
            minLat to minLng  // SW (close the rectangle)
        )
        return RadarDrawService.encodePolyline(points)
    }
}

@OptIn(FlowPreview::class)
class RadarDrawService(private val karooSystem: KarooSystemServiceProvider) {

    // MVP: Hardcoded backend URL - change this to your server
    private val backendUrl = "http://192.168.50.72:8080/api/radar"

    // Refresh interval (5 minutes)
    private val refreshIntervalMs = 5 * 60 * 1000L

    private val json = Json {
        ignoreUnknownKeys = true
        isLenient = true
    }

    private val httpClient = HttpClient(Android) {
        install(ContentNegotiation) {
            json(json)
        }
        install(HttpTimeout) {
            requestTimeoutMillis = 30000
        }
    }

    companion object {
        private const val BBOX_POLYLINE_ID = "rain-radar-bbox"

        // Heuristic: radius in km based on zoom level
        // Zoom 8 (most zoomed out) -> ~100km radius
        // Zoom 18 (most zoomed in) -> ~1km radius
        // Using exponential decay: radius = baseRadius * 2^(baseZoom - zoomLevel)
        private const val BASE_ZOOM = 14.0
        private const val BASE_RADIUS_KM = 10.0
        private const val MIN_RADIUS_KM = 1.0
        private const val MAX_RADIUS_KM = 100.0

        fun calculateRadiusKm(zoomLevel: Double): Double {
            val radius = BASE_RADIUS_KM * 2.0.pow(BASE_ZOOM - zoomLevel) / 8.0
            return radius.coerceIn(MIN_RADIUS_KM, MAX_RADIUS_KM)
        }

        fun calculateBbox(lat: Double, lng: Double, radiusKm: Double): BoundingBox {
            // Approximate conversion: 1 degree latitude ~ 111km
            // 1 degree longitude ~ 111km * cos(latitude)
            val latDelta = radiusKm / 111.0
            val lngDelta = radiusKm / (111.0 * cos(Math.toRadians(lat)))

            return BoundingBox(
                minLat = lat - latDelta,
                maxLat = lat + latDelta,
                minLng = lng - lngDelta,
                maxLng = lng + lngDelta
            )
        }

        // Google Polyline encoding algorithm (precision 5)
        fun encodePolyline(points: List<Pair<Double, Double>>): String {
            val result = StringBuilder()
            var prevLat = 0
            var prevLng = 0

            for ((lat, lng) in points) {
                val latE5 = (lat * 1e5).roundToInt()
                val lngE5 = (lng * 1e5).roundToInt()

                result.append(encodeSignedNumber(latE5 - prevLat))
                result.append(encodeSignedNumber(lngE5 - prevLng))

                prevLat = latE5
                prevLng = lngE5
            }

            return result.toString()
        }

        private fun encodeSignedNumber(num: Int): String {
            var value = if (num < 0) (num shl 1).inv() else (num shl 1)
            val result = StringBuilder()

            while (value >= 0x20) {
                val chunk = (value and 0x1F) or 0x20
                result.append((chunk + 63).toChar())
                value = value shr 5
            }
            result.append((value + 63).toChar())

            return result.toString()
        }
    }

    fun startJob(emitter: Emitter<MapEffect>): Job {
        return CoroutineScope(Dispatchers.IO).launch {
            Log.d(TAG, "RadarDrawService started")

            // Default values
            var currentLat = 48.85 // Paris
            var currentLng = 2.35
            var currentZoom = 14.0

            // Location flow
            val locationFlow = karooSystem.stream<OnLocationChanged>()
                .onStart {
                    Log.d(TAG, "Starting location stream with default position")
                    emit(OnLocationChanged(currentLat, currentLng, null))
                }

            // Zoom level flow
            val zoomFlow = karooSystem.stream<OnMapZoomLevel>()
                .onStart {
                    Log.d(TAG, "Starting zoom stream with default zoom")
                    emit(OnMapZoomLevel(currentZoom))
                }

            // Combine location and zoom changes with debounce to avoid rapid updates
            combine(locationFlow, zoomFlow) { location, zoom ->
                Triple(location.lat, location.lng, zoom.zoomLevel)
            }
            .distinctUntilChanged()
            .debounce(500) // Wait 500ms after last change before updating
            .collect { (lat, lng, zoomLevel) ->
                currentLat = lat
                currentLng = lng
                currentZoom = zoomLevel

                Log.d(TAG, "Location/zoom update: ($lat, $lng) @ zoom $zoomLevel")

                updateBbox(emitter, lat, lng, zoomLevel)
            }
        }
    }

    private fun updateBbox(emitter: Emitter<MapEffect>, lat: Double, lng: Double, zoomLevel: Double) {
        val radiusKm = calculateRadiusKm(zoomLevel)
        val bbox = calculateBbox(lat, lng, radiusKm)

        Log.d(TAG, "Bbox: radius=${radiusKm}km, bounds=[${bbox.minLat},${bbox.minLng}]-[${bbox.maxLat},${bbox.maxLng}]")

        // Note: ShowPolyline with same ID will update the existing polyline
        // No need to HidePolyline first

        // Draw new bbox
        val encodedPolyline = bbox.toEncodedPolyline()
        // Use ARGB format with full alpha (0xFF) for maximum visibility
        val bboxColor = Color.argb(255, 255, 0, 0) // Fully opaque red
        val polyline = ShowPolyline(
            id = BBOX_POLYLINE_ID,
            encodedPolyline = encodedPolyline,
            color = bboxColor,
            width = 15
        )

        Log.d(TAG, "Bbox color: $bboxColor (0x${Integer.toHexString(bboxColor)})")

        emitter.onNext(polyline)

        Log.d(TAG, "Bbox polyline drawn: $encodedPolyline")
    }

    private fun parseColor(colorStr: String): Int {
        return try {
            // Handle ARGB format like "#4000FF00"
            if (colorStr.startsWith("#") && colorStr.length == 9) {
                val alpha = colorStr.substring(1, 3).toInt(16)
                val red = colorStr.substring(3, 5).toInt(16)
                val green = colorStr.substring(5, 7).toInt(16)
                val blue = colorStr.substring(7, 9).toInt(16)
                Color.argb(alpha, red, green, blue)
            } else {
                colorStr.toColorInt()
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to parse color: $colorStr", e)
            Color.BLUE
        }
    }
}
