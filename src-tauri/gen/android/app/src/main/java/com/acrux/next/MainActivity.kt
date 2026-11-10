package com.acrux.next

import android.app.Activity
import android.content.Intent
import android.media.projection.MediaProjectionConfig
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Bundle
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback

class MainActivity : TauriActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
            requestPermissions(arrayOf(android.Manifest.permission.POST_NOTIFICATIONS), 418)
        }
        LogService.refresh(this)
    }

    override fun onWebViewCreate(webView: WebView) {
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                webView.evaluateJavascript("window.acruxMobileBack?.() === true") { handled ->
                    if (handled != "true" && !isFinishing) {
                        isEnabled = false
                        onBackPressedDispatcher.onBackPressed()
                        isEnabled = true
                    }
                }
            }
        })
    }

    fun requestCapture() {
        val manager = getSystemService(MediaProjectionManager::class.java)
        val intent = if (Build.VERSION.SDK_INT >= 34) {
            manager.createScreenCaptureIntent(MediaProjectionConfig.createConfigForDefaultDisplay())
        } else {
            manager.createScreenCaptureIntent()
        }
        startActivityForResult(intent, CAPTURE_REQUEST)
    }

    @Deprecated("Activity result is forwarded to the Tauri activity")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == CAPTURE_REQUEST && resultCode == Activity.RESULT_OK && data != null) {
            startForegroundService(Intent(this, CaptureService::class.java).putExtra("projection", data))
        }
    }

    private companion object {
        const val CAPTURE_REQUEST = 417
    }
}
