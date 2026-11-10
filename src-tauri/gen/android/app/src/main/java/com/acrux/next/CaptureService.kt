package com.acrux.next

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.PixelFormat
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.media.ImageReader
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.util.DisplayMetrics
import android.util.Log
import android.view.Display

class CaptureService : Service() {
    private lateinit var worker: HandlerThread
    private lateinit var handler: Handler
    private val lock = Any()
    private var projection: MediaProjection? = null
    private var display: VirtualDisplay? = null
    private var reader: ImageReader? = null
    private var closing = false
    private var previousFrame = 0L

    private val resizeListener = object : DisplayManager.DisplayListener {
        override fun onDisplayAdded(id: Int) = Unit
        override fun onDisplayRemoved(id: Int) = Unit
        override fun onDisplayChanged(id: Int) {
            if (id != Display.DEFAULT_DISPLAY) return
            val metrics = metrics()
            resize(metrics.widthPixels, metrics.heightPixels)
        }
    }

    private val projectionCallback = object : MediaProjection.Callback() {
        override fun onStop() {
            stopSelf()
        }

        override fun onCapturedContentResize(width: Int, height: Int) {
            resize(width, height)
        }
    }

    override fun onBind(intent: Intent?) = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == STOP) {
            stopSelf()
            return START_NOT_STICKY
        }
        if (projection != null) return START_NOT_STICKY
        val token = if (Build.VERSION.SDK_INT >= 33) {
            intent?.getParcelableExtra("projection", Intent::class.java)
        } else {
            @Suppress("DEPRECATION")
            intent?.getParcelableExtra<Intent>("projection")
        }
        if (token == null) {
            stopSelf()
            return START_NOT_STICKY
        }

        try {
            val nm = getSystemService(NotificationManager::class.java)
            nm.createNotificationChannel(NotificationChannel(CHANNEL, "Acrux 화면 캡처", NotificationManager.IMPORTANCE_LOW))
            val stop = PendingIntent.getService(
                this, 0, Intent(this, CaptureService::class.java).setAction(STOP), PendingIntent.FLAG_IMMUTABLE,
            )
            val notification = Notification.Builder(this, CHANNEL)
                .setSmallIcon(R.mipmap.ic_launcher)
                .setContentTitle("Acrux")
                .setContentText("Roblox 화면 캡처 중")
                .addAction(Notification.Action.Builder(null, "캡처 종료", stop).build())
                .setOngoing(true)
                .build()
            if (Build.VERSION.SDK_INT >= 29) {
                startForeground(1, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION)
            } else {
                startForeground(1, notification)
            }

            worker = HandlerThread("acrux-capture").also { it.start() }
            handler = Handler(worker.looper)
            val active = getSystemService(MediaProjectionManager::class.java)
                .getMediaProjection(RESULT_OK, token) ?: error("No screen capture permission")
            synchronized(lock) {
                projection = active
                active.registerCallback(projectionCallback, handler)
                val metrics = metrics()
                val output = output(metrics.widthPixels, metrics.heightPixels)
                reader = output
                display = checkNotNull(active.createVirtualDisplay(
                    "Acrux", output.width, output.height, metrics.densityDpi,
                    DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR, output.surface, null, handler,
                ))
                Native.capture(true)
                TapService.refresh()
            }
            if (Build.VERSION.SDK_INT < 34) {
                getSystemService(DisplayManager::class.java).registerDisplayListener(resizeListener, handler)
            }
        } catch (error: RuntimeException) {
            Log.e("Acrux", "Could not start screen capture", error)
            stopSelf()
        }
        return START_NOT_STICKY
    }

    @Suppress("DEPRECATION")
    private fun metrics(): DisplayMetrics = DisplayMetrics().also {
        getSystemService(DisplayManager::class.java).getDisplay(Display.DEFAULT_DISPLAY).getRealMetrics(it)
    }

    private fun output(width: Int, height: Int): ImageReader =
        ImageReader.newInstance(width, height, PixelFormat.RGBA_8888, 2).also { output ->
            output.setOnImageAvailableListener({ source ->
                synchronized(lock) {
                    if (!closing && source === reader) read(source)
                }
            }, handler)
        }

    private fun resize(width: Int, height: Int) = synchronized(lock) {
        val virtualDisplay = display ?: return@synchronized
        if (closing || width < 1 || height < 1 || (reader?.width == width && reader?.height == height)) {
            return@synchronized
        }
        val previous = reader
        try {
            val next = output(width, height)
            reader = next
            try {
                virtualDisplay.resize(width, height, metrics().densityDpi)
                virtualDisplay.surface = next.surface
            } finally {
                previous?.close()
            }
        } catch (error: RuntimeException) {
            Log.e("Acrux", "Could not resize capture", error)
            stopSelf()
        }
    }

    private fun read(source: ImageReader) {
        source.acquireLatestImage()?.use { image ->
            if (!Native.active()) return
            val tapper = TapService.current
            if (tapper?.gameVisible != true) {
                Native.noTarget()
                return
            }
            if (tapper.gesturePending) return
            // A display can produce more frames than the reel loop consumes.
            if (image.timestamp - previousFrame < 8_000_000L) return
            previousFrame = image.timestamp
            val plane = image.planes[0]
            val tap = Native.frame(plane.buffer, image.width, image.height, plane.rowStride, plane.pixelStride)
            if (tap != null) tapper.input(tap)
        }
    }

    override fun onDestroy() {
        synchronized(lock) {
            closing = true
            Native.capture(false)
            display?.release()
            display = null
            reader?.close()
            reader = null
            projection?.unregisterCallback(projectionCallback)
            projection?.stop()
            projection = null
        }
        if (Build.VERSION.SDK_INT < 34) {
            getSystemService(DisplayManager::class.java).unregisterDisplayListener(resizeListener)
        }
        if (::worker.isInitialized) worker.quitSafely()
        TapService.refresh()
        super.onDestroy()
    }

    private companion object {
        const val RESULT_OK = -1
        const val CHANNEL = "acrux-capture"
        const val STOP = "com.acrux.next.STOP_CAPTURE"
    }
}
