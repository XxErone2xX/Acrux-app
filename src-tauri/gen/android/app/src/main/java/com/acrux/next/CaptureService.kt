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
import android.media.Image
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
import java.nio.ByteBuffer

class CaptureService : Service() {
    private lateinit var worker: HandlerThread
    private lateinit var handler: Handler
    private val lock = Any()
    private var projection: MediaProjection? = null
    private var display: VirtualDisplay? = null
    private var reader: ImageReader? = null
    private var readerGeneration = 0
    private var closing = false
    private var latest: Image? = null
    private var latestAt = 0L
    private var latestGeneration = 0
    private var consumedTimestamp = Long.MIN_VALUE
    private var packed: ByteBuffer? = null
    private var scheduledAt = Long.MAX_VALUE
    private var nextTickAt = 0L
    private var pendingObservation: IntArray? = null
    private var observationAt = 0L
    private var agentRunning = false
    private val pump = object : Runnable {
        override fun run() = synchronized(lock) {
            scheduledAt = Long.MAX_VALUE
            if (!closing) runAgent()
        }
    }
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
                readerGeneration = Native.generation()
                val output = output(metrics.widthPixels, metrics.heightPixels, readerGeneration)
                reader = output
                display = checkNotNull(active.createVirtualDisplay(
                    "Acrux", output.width, output.height, metrics.densityDpi,
                    DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR, output.surface, null, handler,
                ))
                Native.capture(true)
                health.reset(SystemClock.uptimeMillis())
                handler.post(watch)
                current = this
                schedule(0)
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

    private fun output(width: Int, height: Int, generation: Int): ImageReader =
        ImageReader.newInstance(width, height, PixelFormat.RGBA_8888, 3).also { output ->
            output.setOnImageAvailableListener({ source ->
                synchronized(lock) {
                    if (!closing && source === reader) receive(source, generation)
                }
            }, handler)
        }

    private fun resize(width: Int, height: Int, generation: Int = Native.generation()) = synchronized(lock) {
        val virtualDisplay = display ?: return@synchronized
        val sameSize = reader?.width == width && reader?.height == height
        if (closing || width < 1 || height < 1 || (sameSize && readerGeneration == generation)) {
            return@synchronized
        }
        val previous = reader
        dropImage()
        pendingObservation = null
        packed = null
        nextTickAt = 0L
        consumedTimestamp = Long.MIN_VALUE
        try {
            // A callback can be delayed across stop/restart. Drain the old queue,
            // then change producer surface instead of relabelling its buffered
            // images with the generation at callback time. Image timestamp
            // timebases are producer-defined, so uptime is not a safe boundary.
            previous?.acquireLatestImage()?.close()
            val next = output(width, height, generation)
            reader = next
            readerGeneration = generation
            try {
                if (!sameSize) virtualDisplay.resize(width, height, metrics().densityDpi)
                virtualDisplay.surface = next.surface
                sampledAt = 0L
                health.reset(SystemClock.uptimeMillis())
                schedule(0)
            } finally {
                previous?.close()
            }
        } catch (error: RuntimeException) {
            Log.e("Acrux", "Could not resize capture", error)
            stopSelf()
        }
    }

    private fun dropImage() {
        latest?.close()
        latest = null
        latestGeneration = 0
    }

    private fun schedule(delay: Long) {
        if (closing) return
        val now = SystemClock.uptimeMillis()
        var at = now + delay.coerceIn(0, 1000)
        if (latest != null) at = minOf(at, latestAt + AgentTransport.FRAME_TTL_MS)
        if (at >= scheduledAt) return
        handler.removeCallbacks(pump)
        scheduledAt = at
        handler.postAtTime(pump, at)
    }

    private fun receive(source: ImageReader, frameGeneration: Int) {
        val image = source.acquireLatestImage() ?: return
        val generation = Native.generation()
        if (frameGeneration != generation) {
            image.close()
            resize(source.width, source.height, generation)
            return
        }
        val now = SystemClock.uptimeMillis()
        health.frame(now)
        val visible = TapService.current?.canInput() == true
        if (!agentRunning || !visible || image.timestamp <= consumedTimestamp) {
            if (!visible) {
                dropImage()
                packed = null
            }
            image.close()
            return
        }
        if (now - sampledAt >= 1000) {
            sampledAt = now
            val plane = image.planes[0]
            health.sample(now, CaptureHealth.isBlack(plane.buffer, image.width, image.height, plane.rowStride, plane.pixelStride))
        }
        dropImage()
        latest = image
        latestAt = AgentTransport.captureAt(now,System.nanoTime(),image.timestamp)
        latestGeneration = frameGeneration
        // This callback only retains the newest image. The single pump owns JNI work.
        if (pendingObservation != null && now >= observationAt || now >= nextTickAt) schedule(0)
        else schedule((nextTickAt - now).coerceAtLeast(1))
    }

    private fun runAgent() {
        val now = SystemClock.uptimeMillis()
        val generation = Native.generation()
        reader?.let { source ->
            if (readerGeneration != generation) {
                resize(source.width, source.height, generation)
            }
        }
        if (latest != null && !AgentTransport.fresh(latestAt, now, latestGeneration, generation)) dropImage()
        agentRunning = Native.active()
        if (!agentRunning) {
            pendingObservation = null
            packed = null
            nextTickAt = 0L
            dropImage()
            schedule(100)
            return
        }
        val source = reader ?: return
        val tapper = TapService.current
        if (tapper?.canInput() != true) { dropImage(); packed = null }
        try {
            var observation = pendingObservation
            if (observation != null && (!Native.inputCurrent(observation[1]) || tapper?.canInput() != true)) {
                pendingObservation = null
                observation = null
                dropImage()
                nextTickAt = now
            }
            if (observation == null) {
                if (now < nextTickAt) { schedule(nextTickAt - now); return }
                val plan = Native.agentTick(source.width, source.height)
                if (plan.size < 3) throw IllegalArgumentException("화면 관측 계획이 올바르지 않습니다.")
                when (plan[0]) {
                    0 -> {
                        nextTickAt = now + plan[2].toLong().coerceIn(1, 1000)
                        schedule(nextTickAt - now)
                        return
                    }
                    1 -> {
                        if (plan.size !in 8..9 || !AgentTransport.region(source.width, source.height, plan[3], plan[4], plan[5], plan[6])) {
                            throw IllegalArgumentException("화면 관측 영역이 올바르지 않습니다.")
                        }
                        pendingObservation = plan
                        observationAt = now + plan[2].toLong().coerceIn(0, 1000)
                        observation = plan
                    }
                    2 -> { dispatch(plan); nextTickAt = 0; schedule(16); return }
                    else -> throw IllegalArgumentException("화면 관측 계획이 올바르지 않습니다.")
                }
            }
            val request = observation ?: return
            if (now < observationAt) { schedule(observationAt - now); return }
            if (tapper?.gesturePending == true) { schedule(16); return }
            val image = latest
            if (image == null) { schedule(16); return }
            if (latestAt < observationAt || !AgentTransport.fresh(latestAt, now, latestGeneration, request[1])) {
                dropImage()
                schedule(16)
                return
            }
            latest = null
            pendingObservation = null
            nextTickAt = 0L
            try {
                if (image.width != source.width || image.height != source.height || !Native.inputCurrent(request[1]) || tapper?.canInput() != true) return
                val plane = image.planes[0]
                if (!plane.buffer.isDirect) throw IllegalArgumentException("화면 관측 데이터가 올바르지 않습니다.")
                val x = request[3]; val y = request[4]; val width = request[5]; val height = request[6]
                if (request.size == 9 && request[8] == 1) {
                    val receivedAt = latestAt
                    val words = FrameText.read(image, x, y, width, height)
                    // Cold OCR initialization may finish after the screen changed.
                    // Its result warms the recognizer but cannot drive an input.
                    if (!AgentTransport.fresh(receivedAt, SystemClock.uptimeMillis(), latestGeneration, request[1])
                        || !Native.agentText(request[1], request[7], words)) return
                }
                consumedTimestamp = image.timestamp
                val full = x == 0 && y == 0 && width == image.width && height == image.height
                val buffer: ByteBuffer
                val row: Int
                val pixel: Int
                if (full) {
                    // Keep the Image open for this synchronous JNI call; no full-frame copy.
                    buffer = plane.buffer.duplicate().apply { position(0) }
                    row = plane.rowStride
                    pixel = plane.pixelStride
                } else {
                    val size = AgentTransport.copySize(image.width, image.height, x, y, width, height)
                    buffer = packed?.takeIf { it.capacity() >= size } ?: ByteBuffer.allocateDirect(size).also { packed = it }
                    AgentTransport.copy(plane.buffer, image.width, image.height, plane.rowStride, plane.pixelStride, x, y, width, height, buffer)
                    row = width * 4
                    pixel = 4
                }
                val age=SystemClock.uptimeMillis()-latestAt
                if(age !in 0 until AgentTransport.FRAME_TTL_MS)return
                val input = Native.agentObserve(buffer, image.width, image.height, row, pixel, x, y, width, height, request[1], request[7],age.toInt())
                if (input != null) dispatch(input)
            } finally { image.close(); schedule(1) }
        } catch (error: RuntimeException) {
            pendingObservation = null
            dropImage()
            Native.platformLog(error.message ?: "화면 관측 실패", 'r')
            Native.control(0)
            schedule(100)
        }
    }

    private fun dispatch(packet: IntArray) {
        val tapper = TapService.current
        if (tapper == null) {
            if (packet.size >= 9) Native.agentInputResult(packet[8], packet[1], false)
        } else tapper.input(packet)
    }

    override fun onDestroy() {
        synchronized(lock) {
            closing = true
            if (current === this) current = null
            dropImage()
            pendingObservation = null
            packed = null
            if (::handler.isInitialized) {
                handler.removeCallbacks(watch)
                handler.removeCallbacks(pump)
            }
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

    companion object {
        @Volatile private var current: CaptureService? = null
        fun wake() {
            val service = current ?: return
            service.handler.post {
                synchronized(service.lock) {
                    if (!service.closing) {
                        service.nextTickAt = 0L
                        service.schedule(0)
                    }
                }
            }
        }
        const val RESULT_OK = -1
        const val CHANNEL = "acrux-capture"
        const val STOP = "com.acrux.next.STOP_CAPTURE"
    }
}
