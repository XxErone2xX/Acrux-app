package com.acrux.next

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.graphics.Path
import android.app.KeyguardManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.view.accessibility.AccessibilityEvent

class TapService : AccessibilityService() {
    @Volatile var gameVisible = false
        private set
    private val agentInput = AgentTransport.Input()
    val gesturePending: Boolean get() = agentInput.busy()
    private val main = Handler(Looper.getMainLooper())
    private lateinit var overlay: Overlay
    lateinit var routes: RouteDriver
        private set
    private var lastForeground: Boolean? = null
    private var inputTimeout: Runnable? = null
    @Volatile private var screenOff = false
    private var blocked = false
    private var screenReceiverRegistered = false
    private val screenReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            when (intent?.action) {
                Intent.ACTION_SCREEN_OFF -> screenOff = true
                Intent.ACTION_SCREEN_ON, Intent.ACTION_USER_PRESENT -> screenOff = false
            }
            updateWindow()
        }
    }
    private val update = object : Runnable {
        override fun run() {
            if (!Native.active()) routes.stop()
            updateWindow()
            main.postDelayed(this, 500)
        }
    }

    override fun onServiceConnected() {
        routes = RouteDriver(this)
        overlay = Overlay(this)
        current = this
        Native.accessibility(true)
        val filter = IntentFilter(Intent.ACTION_SCREEN_OFF).apply {
            addAction(Intent.ACTION_SCREEN_ON)
            addAction(Intent.ACTION_USER_PRESENT)
        }
        if (Build.VERSION.SDK_INT >= 33) registerReceiver(screenReceiver, filter, Context.RECEIVER_NOT_EXPORTED)
        else registerReceiver(screenReceiver, filter)
        screenReceiverRegistered = true
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
        if (!unlocked()) {
            gameVisible = false
            if (!blocked || Native.active()) {
                blocked = true
                // A locked display ends the task; unlocking never resumes input.
                Native.control(0)
                routes.stop()
            }
            foreground(false)
            overlay.update(false, false, reset)
            return
        }
        blocked = false
        val root = rootInActiveWindow
        val app = try { root?.packageName?.toString() } finally { root?.recycle() }
        gameVisible = app == GAME_PACKAGE
        foreground(gameVisible)
        overlay.update(gameVisible, app == packageName, reset)
    }

    private fun unlocked(): Boolean = !screenOff
        && getSystemService(PowerManager::class.java)?.isInteractive == true
        && getSystemService(KeyguardManager::class.java)?.isKeyguardLocked != true

    fun canInput(): Boolean = gameVisible && unlocked()

    private fun foreground(visible: Boolean) {
        if (lastForeground == visible) return
        lastForeground = visible
        Native.agentForeground(visible)
        CaptureService.wake()
    }

    fun input(packet: IntArray) {
        if (!AgentTransport.tap(packet)) {
            if (packet.size >= 9) Native.agentInputResult(packet[8], packet[1], false)
            CaptureService.wake()
            return
        }
        val generation = packet[8]
        val inputId = packet[1]
        if (!agentInput.begin(generation, inputId)) {
            if (!agentInput.current(generation, inputId)) Native.agentInputResult(generation, inputId, false)
            return
        }
        val width = packet[2]; val height = packet[3]
        val duration = packet[5].toLong()
        val x = packet[6].toFloat(); val y = packet[7].toFloat()
        main.post {
            if (!agentInput.current(generation, inputId)) return@post
            updateWindow()
            val geometry = screen()
            if (!Native.inputCurrent(generation) || !canInput() || current !== this
                || !AgentTransport.viewport(packet, geometry[0], geometry[1]) || overlay.contains(x, y)) {
                complete(generation, inputId, false)
                return@post
            }
            try {
                val path = Path().apply { moveTo(x, y) }
                inputTimeout = Runnable { complete(generation, inputId, false) }.also { main.postDelayed(it, duration + 1500) }
                val first = minOf(duration, 50L)
                val stroke = GestureDescription.StrokeDescription(path, 0L, first, first < duration)
                dispatchTap(stroke, duration - first, x, y, width, height, generation, inputId, true)
            } catch (_: RuntimeException) { complete(generation, inputId, false) }
        }
    }

    private fun dispatchTap(stroke: GestureDescription.StrokeDescription, left: Long, x: Float, y: Float,
                            width: Int, height: Int, generation: Int, inputId: Int, success: Boolean) {
        if (!agentInput.current(generation, inputId)) return
        try {
            val gesture = GestureDescription.Builder().addStroke(stroke).build()
            if (!dispatchGesture(gesture, object : GestureResultCallback() {
                override fun onCompleted(description: GestureDescription) {
                    if (!agentInput.current(generation, inputId)) return
                    val geometry = screen()
                    val keep = current === this@TapService && Native.inputCurrent(generation) && canInput()
                        && geometry[0] == width && geometry[1] == height
                    if (left == 0L || current !== this@TapService) {
                        complete(generation, inputId, success && keep)
                        return
                    }
                    try {
                        // A canceled long tap only lifts its existing finger; it never starts another tap.
                        val ms = if (keep) minOf(left, 50L) else 1L
                        val path = Path().apply { moveTo(x, y) }
                        val next = stroke.continueStroke(path, 0L, ms, keep && ms < left)
                        dispatchTap(next, if (keep) left - ms else 0L, x, y, width, height, generation, inputId, success && keep)
                    } catch (_: RuntimeException) { complete(generation, inputId, false) }
                }
                override fun onCancelled(description: GestureDescription) = complete(generation, inputId, false)
            }, main)) complete(generation, inputId, false)
        } catch (_: RuntimeException) { complete(generation, inputId, false) }
    }

    private fun complete(generation: Int, inputId: Int, success: Boolean) {
        if (!agentInput.finish(generation, inputId)) return
        inputTimeout?.let(main::removeCallbacks)
        inputTimeout = null
        Native.agentInputResult(generation, inputId, success)
        CaptureService.wake()
    }

    fun safePoint(x: Float, y: Float) = canInput() && !overlay.contains(x, y)
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

    override fun onInterrupt() {
        Native.control(0)
        routes.stop()
    }

    override fun onDestroy() {
        if (screenReceiverRegistered) {
            unregisterReceiver(screenReceiver)
            screenReceiverRegistered = false
        }
        if (current === this) {
            foreground(false)
            current = null
            Native.accessibility(false)
        }
        if (::routes.isInitialized) routes.stop()
        inputTimeout?.let(main::removeCallbacks)
        inputTimeout = null
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
