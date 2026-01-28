package io.github.glandais.karoo.rainradar.extension

import android.util.Log
import io.github.glandais.karoo.rainradar.datatype.RadarPreviewType
import io.github.glandais.karoo.rainradar.datatype.RainForecastType
import io.github.glandais.karoo.rainradar.datatype.RainTimingType
import io.github.glandais.karoo.rainradar.services.KarooSystemServiceProvider
import io.github.glandais.karoo.rainradar.services.RadarDrawService
import io.hammerhead.karooext.extension.KarooExtension
import io.hammerhead.karooext.internal.Emitter
import io.hammerhead.karooext.models.MapEffect

class KarooRainRadarExtension : KarooExtension("karoo-rain-radar", "1.0.0") {
    companion object {
        const val TAG = "karoo-rain-radar"
    }

    private lateinit var karooSystemProvider: KarooSystemServiceProvider
    private lateinit var radarDrawService: RadarDrawService

    override val types by lazy {
        listOf(
            RadarPreviewType(extension),
            RainForecastType(extension),
            RainTimingType(extension)
        )
    }

    override fun startMap(emitter: Emitter<MapEffect>) {
        Log.d(TAG, "Starting rain radar map overlay")

        val job = radarDrawService.startJob(emitter)

        emitter.setCancellable {
            Log.d(TAG, "Stopping rain radar map overlay")
            job.cancel()
        }
    }

    override fun onCreate() {
        super.onCreate()
        Log.d(TAG, "Rain Radar extension created")

        karooSystemProvider = KarooSystemServiceProvider(applicationContext)
        radarDrawService = RadarDrawService(karooSystemProvider)
    }

    override fun onDestroy() {
        Log.d(TAG, "Rain Radar extension destroyed")
        super.onDestroy()
    }
}
