package io.github.glandais.karoo.rainradar.datatype

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.util.Log
import android.widget.RemoteViews
import io.github.glandais.karoo.rainradar.R
import io.github.glandais.karoo.rainradar.extension.KarooRainRadarExtension.Companion.TAG
import io.hammerhead.karooext.extension.DataTypeImpl
import io.hammerhead.karooext.internal.Emitter
import io.hammerhead.karooext.internal.ViewEmitter
import io.hammerhead.karooext.models.StreamState
import io.hammerhead.karooext.models.ViewConfig
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.net.URL
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.sin

/**
 * Data field showing a circular rain forecast visualization
 * Similar to Garmin watches rain display
 */
class RainForecastType(
    extension: String
) : DataTypeImpl(extension, TYPE_ID) {

    companion object {
        const val TYPE_ID = "rain-forecast"

        // MVP: Hardcoded backend URL
        private const val BACKEND_URL = "http://192.168.50.72:8080"

        // Refresh interval (1 minute)
        private const val REFRESH_INTERVAL_MS = 60 * 1000L

        // Forecast radius in km
        private const val FORECAST_RADIUS_KM = 30.0
    }

    private var viewJob: Job? = null
    private var currentLat = 48.85 // Default: Paris
    private var currentLng = 2.35

    @Serializable
    data class ForecastResponse(
        val timestamp: String,
        val current_rain_rate: Double,
        val is_raining: Boolean,
        val sectors: List<Double>,
        val minutes_to_change: Int? = null,
        val distance_to_edge_km: Double? = null,
        val status_text: String
    )

    override fun startView(
        context: Context,
        config: ViewConfig,
        emitter: ViewEmitter
    ) {
        Log.d(TAG, "RainForecastType startView called")

        viewJob = CoroutineScope(Dispatchers.IO).launch {
            while (isActive) {
                try {
                    // Fetch forecast data
                    val url = "$BACKEND_URL/api/rain/forecast" +
                            "?lat=$currentLat&lng=$currentLng" +
                            "&radius_km=$FORECAST_RADIUS_KM"

                    Log.d(TAG, "Fetching rain forecast: $url")

                    val response = fetchForecast(url)

                    // Generate circular visualization
                    val bitmap = if (response != null) {
                        generateCircleBitmap(response, 128)
                    } else {
                        generateEmptyBitmap(128)
                    }

                    // Update RemoteViews on main thread
                    withContext(Dispatchers.Main) {
                        val remoteViews = RemoteViews(context.packageName, R.layout.rain_forecast_view)
                        remoteViews.setImageViewBitmap(R.id.rainCircleImage, bitmap)
                        remoteViews.setTextViewText(
                            R.id.rainStatusText,
                            response?.status_text ?: "No data"
                        )

                        emitter.updateView(remoteViews)
                        Log.d(TAG, "Rain forecast updated")
                    }
                } catch (e: Exception) {
                    Log.e(TAG, "Error updating rain forecast", e)
                }

                delay(REFRESH_INTERVAL_MS)
            }
        }

        emitter.setCancellable {
            Log.d(TAG, "RainForecastType view cancelled")
            viewJob?.cancel()
        }
    }

    override fun startStream(emitter: Emitter<StreamState>) {
        // This data type doesn't stream numeric values
        emitter.setCancellable { }
    }

    fun updateLocation(lat: Double, lng: Double) {
        currentLat = lat
        currentLng = lng
    }

    private fun fetchForecast(urlString: String): ForecastResponse? {
        return try {
            val url = URL(urlString)
            val connection = url.openConnection()
            connection.connectTimeout = 10000
            connection.readTimeout = 30000
            connection.connect()

            val inputStream = connection.getInputStream()
            val jsonText = inputStream.bufferedReader().readText()
            inputStream.close()

            Json { ignoreUnknownKeys = true }.decodeFromString<ForecastResponse>(jsonText)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to fetch forecast", e)
            null
        }
    }

    /**
     * Generate a circular bitmap showing rain intensity in 12 sectors
     * North is at the top, sectors go clockwise
     */
    private fun generateCircleBitmap(response: ForecastResponse, size: Int): Bitmap {
        val bitmap = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)

        val centerX = size / 2f
        val centerY = size / 2f
        val radius = size / 2f - 4f

        val paint = Paint().apply {
            style = Paint.Style.FILL
            isAntiAlias = true
        }

        // Draw 12 sectors (30 degrees each)
        // Sector 0 = North (top), clockwise
        val sectors = response.sectors
        val rectF = RectF(
            centerX - radius,
            centerY - radius,
            centerX + radius,
            centerY + radius
        )

        for (i in 0 until 12) {
            val rainRate = if (i < sectors.size) sectors[i] else 0.0

            // Calculate start angle (0° is at 3 o'clock in Android, so offset by -90°)
            // Sector 0 should be at top (12 o'clock), which is -90° from 3 o'clock
            val startAngle = -90f + (i * 30f)

            // Get color based on rain intensity
            paint.color = getRainColor(rainRate)

            canvas.drawArc(rectF, startAngle, 30f, true, paint)
        }

        // Draw concentric circles for distance indication
        paint.style = Paint.Style.STROKE
        paint.strokeWidth = 1f
        paint.color = Color.argb(80, 0, 0, 0)

        for (i in 1..3) {
            val circleRadius = radius * i / 3f
            canvas.drawCircle(centerX, centerY, circleRadius, paint)
        }

        // Draw center dot (current position)
        paint.style = Paint.Style.FILL
        paint.color = if (response.is_raining) Color.WHITE else Color.BLACK
        canvas.drawCircle(centerX, centerY, 4f, paint)

        // Draw direction markers
        paint.color = Color.argb(150, 0, 0, 0)
        paint.textSize = 10f
        paint.textAlign = Paint.Align.CENTER

        // N marker at top
        canvas.drawText("N", centerX, 12f, paint)

        return bitmap
    }

    private fun generateEmptyBitmap(size: Int): Bitmap {
        val bitmap = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)

        val paint = Paint().apply {
            style = Paint.Style.FILL
            color = Color.LTGRAY
            isAntiAlias = true
        }

        val centerX = size / 2f
        val centerY = size / 2f
        val radius = size / 2f - 4f

        canvas.drawCircle(centerX, centerY, radius, paint)

        // Draw "?" in center
        paint.color = Color.DKGRAY
        paint.textSize = 24f
        paint.textAlign = Paint.Align.CENTER
        canvas.drawText("?", centerX, centerY + 8f, paint)

        return bitmap
    }

    /**
     * Get color for rain intensity (mm/h)
     * Uses same thresholds as backend
     */
    private fun getRainColor(rainRate: Double): Int {
        return when {
            rainRate < 0.1 -> Color.argb(60, 200, 200, 200)   // Very light gray - dry
            rainRate < 0.2 -> Color.argb(128, 173, 216, 230)  // Light blue
            rainRate < 0.4 -> Color.argb(153, 100, 149, 237)  // Cornflower blue
            rainRate < 0.7 -> Color.argb(179, 0, 255, 0)      // Green
            rainRate < 1.0 -> Color.argb(204, 255, 255, 0)    // Yellow
            rainRate < 1.5 -> Color.argb(230, 255, 165, 0)    // Orange
            else -> Color.argb(255, 255, 0, 0)                 // Red
        }
    }
}
