package io.github.glandais.karoo.rainradar.datatype

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.Log
import android.widget.RemoteViews
import io.github.glandais.karoo.rainradar.R
import io.github.glandais.karoo.rainradar.RadarMapActivity
import io.github.glandais.karoo.rainradar.extension.KarooRainRadarExtension.Companion.TAG
import io.hammerhead.karooext.extension.DataTypeImpl
import io.hammerhead.karooext.internal.ViewEmitter
import io.hammerhead.karooext.models.OnLocationChanged
import io.hammerhead.karooext.models.StreamState
import io.hammerhead.karooext.models.ViewConfig
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.net.URL
import kotlin.math.cos

class RadarPreviewType(
    extension: String
) : DataTypeImpl(extension, TYPE_ID) {

    companion object {
        const val TYPE_ID = "radar-preview"

        // MVP: Hardcoded backend URL
        private const val BACKEND_URL = "http://192.168.50.72:8080"

        // Refresh interval (5 minutes)
        private const val REFRESH_INTERVAL_MS = 5 * 60 * 1000L

        // Preview tile size
        private const val PREVIEW_SIZE = 128

        // Radius for preview (km)
        private const val PREVIEW_RADIUS_KM = 30.0
    }

    private var viewJob: Job? = null
    private var currentLat = 48.85 // Default: Paris
    private var currentLng = 2.35

    override fun startView(
        context: Context,
        config: ViewConfig,
        emitter: ViewEmitter
    ) {
        Log.d(TAG, "RadarPreviewType startView called")

        viewJob = CoroutineScope(Dispatchers.IO).launch {
            while (isActive) {
                try {
                    // Calculate bounding box around current position
                    val bbox = calculateBbox(currentLat, currentLng, PREVIEW_RADIUS_KM)

                    // Build URL for radar tile
                    val url = "$BACKEND_URL/api/radar/tile" +
                            "?min_lat=${bbox.minLat}&max_lat=${bbox.maxLat}" +
                            "&min_lng=${bbox.minLng}&max_lng=${bbox.maxLng}" +
                            "&width=$PREVIEW_SIZE&height=$PREVIEW_SIZE"

                    Log.d(TAG, "Fetching radar preview: $url")

                    // Download and decode bitmap
                    val bitmap = downloadBitmap(url)

                    // Update RemoteViews on main thread
                    withContext(Dispatchers.Main) {
                        val remoteViews = RemoteViews(context.packageName, R.layout.radar_preview_view)

                        if (bitmap != null) {
                            remoteViews.setImageViewBitmap(R.id.radarPreviewImage, bitmap)
                        }

                        // Set click listener to open RadarMapActivity
                        val intent = Intent(context, RadarMapActivity::class.java).apply {
                            putExtra(RadarMapActivity.EXTRA_LAT, currentLat)
                            putExtra(RadarMapActivity.EXTRA_LNG, currentLng)
                            flags = Intent.FLAG_ACTIVITY_NEW_TASK
                        }

                        val pendingIntent = android.app.PendingIntent.getActivity(
                            context,
                            0,
                            intent,
                            android.app.PendingIntent.FLAG_UPDATE_CURRENT or android.app.PendingIntent.FLAG_IMMUTABLE
                        )
                        remoteViews.setOnClickPendingIntent(R.id.radarPreviewImage, pendingIntent)

                        emitter.updateView(remoteViews)
                        Log.d(TAG, "Radar preview updated")
                    }
                } catch (e: Exception) {
                    Log.e(TAG, "Error updating radar preview", e)
                }

                delay(REFRESH_INTERVAL_MS)
            }
        }

        emitter.setCancellable {
            Log.d(TAG, "RadarPreviewType view cancelled")
            viewJob?.cancel()
        }
    }

    override fun startStream(emitter: io.hammerhead.karooext.internal.Emitter<StreamState>) {
        // This data type doesn't stream numeric values, just displays a graphical view
        emitter.setCancellable { }
    }

    fun updateLocation(lat: Double, lng: Double) {
        currentLat = lat
        currentLng = lng
    }

    private fun downloadBitmap(urlString: String): Bitmap? {
        return try {
            val url = URL(urlString)
            val connection = url.openConnection()
            connection.connectTimeout = 10000
            connection.readTimeout = 30000
            connection.connect()

            val inputStream = connection.getInputStream()
            val bitmap = BitmapFactory.decodeStream(inputStream)
            inputStream.close()
            bitmap
        } catch (e: Exception) {
            Log.e(TAG, "Failed to download bitmap", e)
            null
        }
    }

    private data class BoundingBox(
        val minLat: Double,
        val maxLat: Double,
        val minLng: Double,
        val maxLng: Double
    )

    private fun calculateBbox(lat: Double, lng: Double, radiusKm: Double): BoundingBox {
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
}
