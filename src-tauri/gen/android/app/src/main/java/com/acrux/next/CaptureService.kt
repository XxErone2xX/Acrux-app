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
import android.os.SystemClock
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
    private val health = CaptureHealth()
    private var sampledAt = 0L
    private val watch = object : Runnable {
        override fun run() {
            synchronized(lock) {
                if (closing) return@synchronized
                val problem = health.warning(SystemClock.uptimeMillis(), Native.active() && TapService.current?.canInput() == true)
                if (problem != null) {
                    val message = "$problem — 다른 화면 녹화 앱이 켜져 있으면 끄고, 앱에서 [화면 캡처 시작] 을 다시 눌러 주세요"
                    Native.platformLog(message, 'r')
                    Handler(mainLooper).post {
                        android.widget.Toast.makeText(this@CaptureService, LogService.tr(this@CaptureService, message), android.widget.Toast.LENGTH_LONG).show()
                    }
                }
                handler.postDelayed(this, 1000)
            }
        }
    }

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
                health.reset(SystemClock.uptimeMillis())
                handler.post(watch)
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
                previousFrame = 0L
                sampledAt = 0L
                health.reset(SystemClock.uptimeMillis())
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
            val now = SystemClock.uptimeMillis()
            health.frame(now)
            if (!Native.active()) return
            val tapper = TapService.current
            if (tapper?.canInput() != true) {
                Native.noTarget()
                return
            }
            val plane = image.planes[0]
            if (now - sampledAt >= 1000) {
                sampledAt = now
                health.sample(now, CaptureHealth.isBlack(plane.buffer, image.width, image.height, plane.rowStride, plane.pixelStride))
            }
            if (tapper.gesturePending) return
            // A display can produce more frames than the reel loop consumes.
            if (image.timestamp - previousFrame < 8_000_000L) return
            previousFrame = image.timestamp
            val tap = Native.frame(plane.buffer, image.width, image.height, plane.rowStride, plane.pixelStride)
            if (tap != null) tapper.input(tap)
        }
    }

    override fun onDestroy() {
        synchronized(lock) {
            closing = true
            if (::handler.isInitialized) handler.removeCallbacks(watch)
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
