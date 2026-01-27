package io.github.glandais.karoo.rainradar.services

import android.graphics.Color
import android.util.Log
import io.github.glandais.karoo.rainradar.extension.KarooRainRadarExtension.Companion.TAG
import io.hammerhead.karooext.internal.Emitter
import io.hammerhead.karooext.models.HidePolyline
import io.hammerhead.karooext.models.MapEffect
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
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import androidx.core.graphics.toColorInt

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

class RadarDrawService {

    // MVP: Hardcoded backend URL - change this to your server
    private val backendUrl = "http://192.168.50.72:8080/api/radar"

    // Refresh interval (5 minutes)
    private val refreshIntervalMs = 5 * 60 * 1000L

    // Radius around current position in km
    private val radiusKm = 50

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

    private var lastDrawnPolylines = setOf<String>()

    fun startJob(emitter: Emitter<MapEffect>): Job {
        return CoroutineScope(Dispatchers.IO).launch {
            Log.d(TAG, "RadarDrawService started")

            // Ticker flow that emits every refresh interval
            val tickerFlow = flow {
                while (true) {
                    emit(Unit)
                    delay(refreshIntervalMs)
                }
            }

            // Collect and update radar data
            tickerFlow.collect {
                try {
                    updateRadar(emitter)
                } catch (e: Exception) {
                    Log.e(TAG, "Failed to update radar: ${e.message}", e)
                }
            }
        }
    }

    private suspend fun updateRadar(emitter: Emitter<MapEffect>) {
        // For MVP, use a fixed position (Paris)
        // TODO: Get actual GPS position from Karoo
        val lat = 48.85
        val lng = 2.35

        Log.d(TAG, "Fetching radar data for ($lat, $lng) with radius $radiusKm km")

        try {
            val response: RadarResponse = httpClient.get(backendUrl) {
                parameter("lat", lat)
                parameter("lng", lng)
                parameter("radius", radiusKm)
            }.body()

            Log.d(TAG, "Received ${response.contour_count} contours, ${response.total_points} points")

            // Build new polylines
            val newPolylines = mutableSetOf<ShowPolyline>()

            for ((index, contour) in response.contours.withIndex()) {
                val color = parseColor(contour.color)
                val width = when (contour.level) {
                    "heavy" -> 8
                    "moderate" -> 6
                    else -> 4
                }

                val polyline = ShowPolyline(
                    id = "rain-${contour.level}-$index",
                    encodedPolyline = contour.polyline,
                    color = color,
                    width = width
                )
                newPolylines.add(polyline)
            }

            // Calculate diff
            val newIds = newPolylines.map { it.id }.toSet()
            val removedIds = lastDrawnPolylines - newIds

            // Remove old polylines
            for (id in removedIds) {
                emitter.onNext(HidePolyline(id))
            }

            // Add new polylines
            for (polyline in newPolylines) {
                emitter.onNext(polyline)
            }

            lastDrawnPolylines = newIds

            Log.d(TAG, "Updated map with ${newPolylines.size} polylines, removed ${removedIds.size}")

        } catch (e: Exception) {
            Log.e(TAG, "HTTP request failed: ${e.message}", e)
        }
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
