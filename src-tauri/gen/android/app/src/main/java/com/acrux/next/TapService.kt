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
import android.util.Log
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityWindowInfo

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
    private class GestureRun(val packet: IntArray) {
        val generation = packet[8]
        val id = packet[1]
        var elapsed = 0
        var submittedEnd = 0
        var strokes = arrayOfNulls<GestureDescription.StrokeDescription>(AgentTransport.fingers(packet))
        var completedStrokes: Array<GestureDescription.StrokeDescription?>? = null
        var releasing = false
    }
    private var gesture: GestureRun? = null
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
        if(Build.VERSION.SDK_INT>=33){serviceInfo=serviceInfo.apply{flags=flags or android.accessibilityservice.AccessibilityServiceInfo.FLAG_INPUT_METHOD_EDITOR}}
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
        val root = windows.firstOrNull { it.type == AccessibilityWindowInfo.TYPE_APPLICATION && it.isFocused }?.root
            ?: rootInActiveWindow
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
        if (!visible) gesture?.let(::release)
        CaptureService.wake()
    }

    fun input(packet: IntArray) {
        if (!AgentTransport.command(packet)) {
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
        if (packet[4] >= 4) {
            main.post {
                updateWindow()
                val size = screen()
                var success = false
                val launch = packet[4] == 6
                val allowed = if (launch) {
                    // Opening Roblox has no screen coordinates. Its own one-shot
                    // native claim permits only the already pending recovery URL.
                    Native.agentLaunchStart(generation, inputId, unlocked())
                } else {
                    Native.agentInputCurrent(generation,inputId) && canInput() && AgentTransport.viewport(packet, size[0], size[1])
                }
                if (allowed) {
                    success = try {
                        when (packet[4]) {
                            4 -> GameText.commit(this,AgentTransport.text(packet), packet[6] == 1)
                            5 -> when (packet[6]) {
                                0x0d -> GameText.submit(this)
                                0x1b -> performGlobalAction(GLOBAL_ACTION_BACK)
                                else -> false
                            }
                            6 -> {
                                val uri = android.net.Uri.parse(AgentTransport.text(packet))
                                val valid = uri.scheme == "roblox" || uri.scheme == "https" &&
                                    (uri.host == "roblox.com" || uri.host?.endsWith(".roblox.com") == true)
                                if (!valid || !unlocked() || !Native.inputCurrent(generation)) false else {
                                    startActivity(Intent(Intent.ACTION_VIEW, uri).setPackage(GAME_PACKAGE).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                                    true
                                }
                            }
                            else -> false
                        }
                    } catch (error: RuntimeException) {
                        Log.w("AcruxDUA", "platform input ${error.javaClass.simpleName}")
                        false
                    }
                }
                complete(generation, inputId, success)
            }
            return
        }
        val run = GestureRun(packet.copyOf())
        main.post {
            if (!agentInput.current(generation, inputId)) return@post
            gesture = run
            inputTimeout = Runnable { inputLog(run, "timeout"); release(run) }.also { main.postDelayed(it, run.packet[5].toLong() + 1500) }
            dispatchFragment(run)
        }
    }

    private fun allowed(run: GestureRun): Boolean {
        updateWindow()
        val geometry = screen()
        val blocked = when {
            current !== this -> "service"
            !canInput() -> "foreground"
            !Native.inputCurrent(run.generation) -> "generation"
            run.elapsed == 0 && !Native.agentInputCurrent(run.generation,run.id) -> "expired"
            run.releasing -> "releasing"
            !AgentTransport.viewport(run.packet, geometry[0], geometry[1]) -> "viewport"
            else -> null
        }
        if (blocked != null) { inputLog(run, "guard_$blocked"); return false }
        // Check the whole requested line, not just its endpoints, even after
        // the panel has moved during a continuous gesture.
        for (finger in run.strokes.indices) {
            val from = AgentTransport.position(run.packet, finger, 0)
            val to = AgentTransport.position(run.packet, finger, run.packet[5])
            if (overlay.intersects(from[0], from[1], to[0], to[1])) {
                inputLog(run, "guard_overlay")
                return false
            }
        }
        return true
    }

    private fun dispatchFragment(run: GestureRun) {
        if (!agentInput.current(run.generation, run.id) || run.releasing) return
        try {
            if (!allowed(run)) { release(run); return }
            val end = minOf(run.elapsed + 50, run.packet[5])
            val next = arrayOfNulls<GestureDescription.StrokeDescription>(run.strokes.size)
            val builder = GestureDescription.Builder()
            for (finger in next.indices) {
                val until = AgentTransport.fingerDuration(run.packet, finger)
                if (run.elapsed >= until) continue
                val fingerEnd = minOf(end, until)
                val from = AgentTransport.position(run.packet, finger, run.elapsed)
                val to = AgentTransport.position(run.packet, finger, fingerEnd)
                if (overlay.intersects(from[0], from[1], to[0], to[1])) {
                    inputLog(run, "guard_segment_overlay")
                    release(run)
                    return
                }
                val path = Path().apply { moveTo(from[0], from[1]); lineTo(to[0], to[1]) }
                val duration = (fingerEnd - run.elapsed).toLong()
                val stroke = run.strokes[finger]?.continueStroke(path, 0L, duration, fingerEnd < until)
                    ?: GestureDescription.StrokeDescription(path, 0L, duration, fingerEnd < until)
                next[finger] = stroke
                builder.addStroke(stroke)
            }
            val accepted = dispatchGesture(builder.build(), object : GestureResultCallback() {
                override fun onCompleted(description: GestureDescription) {
                    if (!agentInput.current(run.generation, run.id)) return
                    run.elapsed = end
                    run.completedStrokes = next
                    if (run.releasing) {
                        if (run.strokes.none { it?.willContinue() == true }) complete(run.generation, run.id, false)
                        return
                    }
                    if (end == run.packet[5]) complete(run.generation, run.id, allowed(run))
                    else dispatchFragment(run)
                }
                override fun onCancelled(description: GestureDescription) {
                    inputLog(run, "cancelled end=$end releasing=${run.releasing}")
                    // A cancellation callback does not prove Roblox's joystick
                    // state cleared. Try lifting the last completed contact once.
                    if (!run.releasing && agentInput.current(run.generation, run.id)) {
                        run.completedStrokes?.let {
                            run.strokes = it
                            run.submittedEnd = run.elapsed
                        }
                        release(run)
                    }
                }
            }, main)
            // Callbacks use this same main Handler, so publish only accepted
            // contacts before they can run. Failure must release the previous
            // actual contacts, never an unsubmitted continuation's ID.
            if (accepted) {
                run.strokes = next
                run.submittedEnd = end
            } else { inputLog(run, "dispatch_false end=$end"); release(run) }
        } catch (error: RuntimeException) {
            inputLog(run, "exception_${error.javaClass.simpleName}")
            release(run)
        }
    }

    private fun inputLog(run: GestureRun, event: String) {
        Log.i("AcruxDUA", "input kind=${run.packet[4]} elapsed=${run.elapsed} $event")
    }

    private fun release(run: GestureRun) {
        if (!agentInput.current(run.generation, run.id) || run.releasing) return
        run.releasing = true
        inputTimeout?.let(main::removeCallbacks)
        // Only continue existing contacts to lift them; cancellation must never
        // create a fresh tap in a different app or a later generation.
        val held = run.strokes.indices.filter { run.strokes[it]?.willContinue() == true }
        if (held.isEmpty()) {
            if (run.submittedEnd > run.elapsed) {
                // A final, noncontinuing fragment lifts by itself in <=50ms.
                // Wait for completion rather than acknowledging while it is down.
                inputTimeout = Runnable { inputLog(run, "release_timeout"); complete(run.generation, run.id, false) }.also { main.postDelayed(it, 250) }
            } else complete(run.generation, run.id, false)
            return
        }
        try {
            val builder = GestureDescription.Builder()
            for (finger in held) {
                val at = AgentTransport.position(run.packet, finger, run.submittedEnd)
                val to = AgentTransport.releasePosition(run.packet, at)
                val path = Path().apply { moveTo(at[0], at[1]); lineTo(to[0], to[1]) }
                builder.addStroke(run.strokes[finger]!!.continueStroke(path, 0L, 1L, false))
            }
            inputTimeout = Runnable { inputLog(run, "release_timeout"); complete(run.generation, run.id, false) }.also { main.postDelayed(it, 250) }
            if (!dispatchGesture(builder.build(), object : GestureResultCallback() {
                override fun onCompleted(description: GestureDescription) = complete(run.generation, run.id, false)
                override fun onCancelled(description: GestureDescription) = complete(run.generation, run.id, false)
            }, main)) { inputLog(run, "release_dispatch_false"); complete(run.generation, run.id, false) }
        } catch (error: RuntimeException) {
            inputLog(run, "release_exception_${error.javaClass.simpleName}")
            complete(run.generation, run.id, false)
        }
    }

    private fun complete(generation: Int, inputId: Int, success: Boolean) {
        if (!agentInput.finish(generation, inputId)) return
        gesture?.takeIf { it.generation == generation && it.id == inputId }?.let {
            if (!success || it.packet[4] != 0) inputLog(it, if (success) "ack_success" else "ack_failure")
        }
        inputTimeout?.let(main::removeCallbacks)
        inputTimeout = null
        if (gesture?.generation == generation && gesture?.id == inputId) gesture = null
        Native.agentInputResult(generation, inputId, success)
        CaptureService.wake()
    }

    fun safePoint(x: Float, y: Float) = canInput() && !overlay.contains(x, y)
    fun selectMacro(label: String, initial: List<Double>, region: Boolean, done: (List<Double>?) -> Unit) {
        overlay.selectMacro(label, initial, region, done)
    }
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
        gesture?.let(::release)
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
