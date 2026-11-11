package com.acrux.next

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.graphics.Path
import android.os.Handler
import android.os.Looper
import android.view.accessibility.AccessibilityEvent

class TapService : AccessibilityService() {
    @Volatile var gameVisible = false
        private set
    @Volatile var gesturePending = false
        private set
    private val main = Handler(Looper.getMainLooper())
    private lateinit var overlay: Overlay
    lateinit var routes: RouteDriver
        private set
    private var generation = 0
    private val update = object : Runnable {
        override fun run() {
            if (!Native.active()) routes.stop()
            overlay.refresh()
            main.postDelayed(this, 500)
        }
    }

    override fun onServiceConnected() {
        routes = RouteDriver(this)
        overlay = Overlay(this)
        current = this
        Native.accessibility(true)
        updateWindow()
        main.post(update)
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent) {
        if (event.eventType == AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED
            || event.eventType == AccessibilityEvent.TYPE_WINDOWS_CHANGED) updateWindow()
    }

    @Suppress("DEPRECATION")
    private fun updateWindow(reset: Boolean = false) {
        if (!::overlay.isInitialized) return
        val root = rootInActiveWindow
        val foreground = try { root?.packageName?.toString() } finally { root?.recycle() }
        gameVisible = foreground == GAME_PACKAGE
        if (!gameVisible) Native.noTarget()
        overlay.update(gameVisible, foreground == packageName, reset)
    }

    fun input(packet: IntArray) {
        gesturePending = true
        main.post {
            generation = packet.last()
            if (!Native.inputCurrent(generation)) {
                finished()
                return@post
            }
            updateWindow()
            if (!gameVisible || current !== this) {
                gesturePending = false
                Native.noTarget()
                return@post
            }
            if (packet[0] == 4) overlay.routePassThrough(packet)
            val coordinatesEnd = if (packet[0] == 4) 8 else packet.lastIndex
            if ((2 until coordinatesEnd step 2).any { packet[it] >= 0 && overlay.contains(packet[it].toFloat(), packet[it + 1].toFloat()) }) {
                rejected()
                return@post
            }
            val ms = packet[1].toLong()
            val x = packet[2].toFloat()
            val y = packet[3].toFloat()
            when (packet[0]) {
                0 -> send(listOf(stroke(x, y, x, y, ms)), ::finished)
                1 -> send(listOf(stroke(x, y, packet[4].toFloat(), packet[5].toFloat(), ms)), ::finished)
                2 -> send(listOf(
                    stroke(x, y, packet[4].toFloat(), packet[5].toFloat(), ms),
                    stroke(packet[6].toFloat(), packet[7].toFloat(), packet[8].toFloat(), packet[9].toFloat(), ms),
                ), ::finished)
                3 -> stick(packet, ms)
                4 -> routes.play(packet) { success ->
                    overlay.restoreTouch()
                    if (!Native.inputCurrent(packet.last()) || success) finished() else rejected()
                }
                else -> rejected()
            }
        }
    }

    fun safePoint(x: Float, y: Float) = !overlay.contains(x, y)
    @Suppress("DEPRECATION")
    fun screen(): IntArray {
        val metrics = android.util.DisplayMetrics()
        getSystemService(android.view.WindowManager::class.java).defaultDisplay.getRealMetrics(metrics)
        return intArrayOf(metrics.widthPixels, metrics.heightPixels)
    }
    fun addPad(view: android.view.View, params: android.view.WindowManager.LayoutParams) = getSystemService(android.view.WindowManager::class.java).addView(view, params)
    fun removePad(view: android.view.View) {
        if (view.isAttachedToWindow) getSystemService(android.view.WindowManager::class.java).removeView(view)
    }

    private fun path(x: Float, y: Float, nextX: Float, nextY: Float) = Path().apply {
        val bounds = screen()
        moveTo(x.coerceIn(0f, (bounds[0] - 1).toFloat()), y.coerceIn(0f, (bounds[1] - 1).toFloat()))
        if (x != nextX || y != nextY) lineTo(nextX.coerceIn(0f, (bounds[0] - 1).toFloat()), nextY.coerceIn(0f, (bounds[1] - 1).toFloat()))
    }

    private fun stroke(x: Float, y: Float, nextX: Float, nextY: Float, duration: Long, hold: Boolean = false) =
        GestureDescription.StrokeDescription(path(x, y, nextX, nextY), 0L, duration, hold)

    private fun stick(packet: IntArray, ms: Long) {
        val x = packet[4].toFloat()
        val y = packet[5].toFloat()
        val jumping = packet[6] >= 0
        val first = minOf(ms, if (jumping) 150L else 90L)
        val start = stroke(packet[2].toFloat(), packet[3].toFloat(), x, y, first, first < ms)
        val strokes = if (jumping) {
            val jx = packet[6].toFloat()
            val jy = packet[7].toFloat()
            listOf(start, stroke(jx, jy, jx, jy, first))
        } else listOf(start)
        send(strokes) {
            if (first == ms) finished()
            else holdStick(start, x, y, ms - first)
        }
    }

    private fun holdStick(previous: GestureDescription.StrokeDescription, x: Float, y: Float, left: Long) {
        val keep = gameVisible && Native.inputCurrent(generation) && current === this
        val ms = if (keep) minOf(left, 120L) else 1L
        val next = previous.continueStroke(path(x, y, x, y), 0L, ms, keep && ms < left)
        send(listOf(next)) {
            if (!keep || left <= ms) finished()
            else holdStick(next, x, y, left - ms)
        }
    }

    private fun send(strokes: List<GestureDescription.StrokeDescription>, done: () -> Unit) {
        try {
            val builder = GestureDescription.Builder()
            strokes.forEach(builder::addStroke)
            if (!dispatchGesture(builder.build(), object : GestureResultCallback() {
                override fun onCompleted(description: GestureDescription) = done()
                override fun onCancelled(description: GestureDescription) = rejected()
            }, main)) rejected()
        } catch (error: RuntimeException) {
            rejected()
        }
    }

    private fun finished() {
        gesturePending = false
    }

    private fun rejected() {
        overlay.restoreTouch()
        finished()
        if (Native.inputCurrent(generation)) Native.inputRejected()
    }

    override fun onInterrupt() {
        Native.control(0)
        routes.stop()
    }

    override fun onDestroy() {
        if (current === this) {
            current = null
            Native.accessibility(false)
        }
        if (::routes.isInitialized) routes.stop()
        main.removeCallbacks(update)
        if (::overlay.isInitialized) overlay.remove()
        super.onDestroy()
    }

    companion object {
        private const val GAME_PACKAGE = "com.roblox.client"
        @Volatile var current: TapService? = null
            private set

        fun refresh(reset: Boolean = false) {
            current?.let { service -> service.main.post { service.updateWindow(reset) } }
        }
    }
}
