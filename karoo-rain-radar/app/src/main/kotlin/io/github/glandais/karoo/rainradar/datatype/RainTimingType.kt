package io.github.glandais.karoo.rainradar.datatype

import android.content.Context
import android.graphics.Color
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
import kotlin.math.abs

/**
 * Data field showing time until rain arrives or ends
 * Displays a simple text-based field with timing information
 */
class RainTimingType(
    extension: String
) : DataTypeImpl(extension, TYPE_ID) {

    companion object {
        const val TYPE_ID = "rain-timing"

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
        Log.d(TAG, "RainTimingType startView called")

        viewJob = CoroutineScope(Dispatchers.IO).launch {
            while (isActive) {
                try {
                    // Fetch forecast data
                    val url = "$BACKEND_URL/api/rain/forecast" +
                            "?lat=$currentLat&lng=$currentLng" +
                            "&radius_km=$FORECAST_RADIUS_KM"

                    Log.d(TAG, "Fetching rain timing: $url")

                    val response = fetchForecast(url)

                    // Update RemoteViews on main thread
                    withContext(Dispatchers.Main) {
                        val remoteViews = RemoteViews(context.packageName, R.layout.rain_timing_view)

                        if (response != null) {
                            updateTimingView(remoteViews, response)
                        } else {
                            // No data available
                            remoteViews.setTextViewText(R.id.timingValue, "--")
                            remoteViews.setTextViewText(R.id.timingDescription, "No data")
                        }

                        emitter.updateView(remoteViews)
                        Log.d(TAG, "Rain timing updated")
                    }
                } catch (e: Exception) {
                    Log.e(TAG, "Error updating rain timing", e)
                }

                delay(REFRESH_INTERVAL_MS)
            }
        }

        emitter.setCancellable {
            Log.d(TAG, "RainTimingType view cancelled")
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

    private fun updateTimingView(remoteViews: RemoteViews, response: ForecastResponse) {
        val minutes = response.minutes_to_change

        if (response.is_raining) {
            // Currently raining - show when it will end
            if (minutes != null && minutes < 0) {
                val absMinutes = abs(minutes)
                when {
                    absMinutes <= 5 -> {
                        remoteViews.setTextViewText(R.id.timingValue, "<5")
                        remoteViews.setTextViewText(R.id.timingDescription, "min fin")
                    }
                    absMinutes <= 60 -> {
                        remoteViews.setTextViewText(R.id.timingValue, "$absMinutes")
                        remoteViews.setTextViewText(R.id.timingDescription, "min fin")
                    }
                    else -> {
                        remoteViews.setTextViewText(R.id.timingValue, ">60")
                        remoteViews.setTextViewText(R.id.timingDescription, "min fin")
                    }
                }
                // Set text color to indicate rain ending soon
                remoteViews.setTextColor(R.id.timingValue, Color.rgb(0, 150, 0)) // Green
            } else {
                // Rain continues indefinitely within range
                remoteViews.setTextViewText(R.id.timingValue, "Pluie")
                remoteViews.setTextViewText(R.id.timingDescription, "continue")
                remoteViews.setTextColor(R.id.timingValue, Color.rgb(0, 100, 200)) // Blue
            }
        } else {
            // Currently dry - show when rain will arrive
            if (minutes != null && minutes > 0) {
                when {
                    minutes <= 5 -> {
                        remoteViews.setTextViewText(R.id.timingValue, "<5")
                        remoteViews.setTextViewText(R.id.timingDescription, "min pluie")
                        remoteViews.setTextColor(R.id.timingValue, Color.rgb(255, 100, 0)) // Orange
                    }
                    minutes <= 15 -> {
                        remoteViews.setTextViewText(R.id.timingValue, "$minutes")
                        remoteViews.setTextViewText(R.id.timingDescription, "min pluie")
                        remoteViews.setTextColor(R.id.timingValue, Color.rgb(255, 165, 0)) // Orange
                    }
                    minutes <= 30 -> {
                        remoteViews.setTextViewText(R.id.timingValue, "$minutes")
                        remoteViews.setTextViewText(R.id.timingDescription, "min pluie")
                        remoteViews.setTextColor(R.id.timingValue, Color.rgb(200, 200, 0)) // Yellow
                    }
                    minutes <= 60 -> {
                        remoteViews.setTextViewText(R.id.timingValue, "$minutes")
                        remoteViews.setTextViewText(R.id.timingDescription, "min pluie")
                        remoteViews.setTextColor(R.id.timingValue, Color.rgb(100, 150, 100)) // Light green
                    }
                    else -> {
                        remoteViews.setTextViewText(R.id.timingValue, ">60")
                        remoteViews.setTextViewText(R.id.timingDescription, "min pluie")
                        remoteViews.setTextColor(R.id.timingValue, Color.rgb(0, 150, 0)) // Green
                    }
                }
            } else {
                // No rain expected within range
                remoteViews.setTextViewText(R.id.timingValue, "Sec")
                remoteViews.setTextViewText(R.id.timingDescription, "")
                remoteViews.setTextColor(R.id.timingValue, Color.rgb(0, 150, 0)) // Green
            }
        }
    }
}
