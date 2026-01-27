package io.github.glandais.karoo.rainradar

import android.annotation.SuppressLint
import android.app.Activity
import android.os.Bundle
import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.WebChromeClient
import android.webkit.WebView
import android.webkit.WebViewClient
import io.github.glandais.karoo.rainradar.extension.KarooRainRadarExtension.Companion.TAG

class RadarMapActivity : Activity() {

    companion object {
        const val EXTRA_LAT = "lat"
        const val EXTRA_LNG = "lng"

        // MVP: Hardcoded backend URL - same as RadarDrawService
        private const val BACKEND_URL = "http://192.168.50.72:8080"
    }

    private lateinit var webView: WebView

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Get initial position from intent or use default (Paris)
        val lat = intent.getDoubleExtra(EXTRA_LAT, 48.85)
        val lng = intent.getDoubleExtra(EXTRA_LNG, 2.35)

        // Create WebView programmatically
        webView = WebView(this)
        setContentView(webView)

        // Configure WebView
        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            allowFileAccess = true
            loadWithOverviewMode = true
            useWideViewPort = true
            builtInZoomControls = false
            displayZoomControls = false
        }

        // Add JavaScript interface for back button
        webView.addJavascriptInterface(WebAppInterface(), "Android")

        // Set WebView clients
        webView.webViewClient = object : WebViewClient() {
            override fun onPageFinished(view: WebView?, url: String?) {
                Log.d(TAG, "WebView page loaded: $url")
            }
        }

        webView.webChromeClient = object : WebChromeClient() {
            override fun onConsoleMessage(
                message: String?,
                lineNumber: Int,
                sourceID: String?
            ) {
                Log.d(TAG, "WebView console: $message [$sourceID:$lineNumber]")
            }
        }

        // Load the map page with position parameters
        val mapUrl = "$BACKEND_URL/static/map.html?lat=$lat&lng=$lng&zoom=10"
        Log.d(TAG, "Loading map URL: $mapUrl")
        webView.loadUrl(mapUrl)
    }

    // JavaScript interface for communication from web page
    inner class WebAppInterface {
        @JavascriptInterface
        fun close() {
            runOnUiThread {
                finish()
            }
        }

        @JavascriptInterface
        fun log(message: String) {
            Log.d(TAG, "JS: $message")
        }
    }

    override fun onBackPressed() {
        if (webView.canGoBack()) {
            webView.goBack()
        } else {
            super.onBackPressed()
        }
    }

    override fun onDestroy() {
        webView.destroy()
        super.onDestroy()
    }
}
