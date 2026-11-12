package com.acrux.next

import android.os.Handler
import android.os.Looper
import org.json.JSONArray
import org.json.JSONObject

/** Executes Android accessibility finger timelines and hands their data to Rust for storage. */
class RouteDriver(private val service: TapService) {
    private val main = Handler(Looper.getMainLooper())
    private var pad: RecPad? = null
    private var jumpReady = false
    @Volatile private var recorder: Thread? = null
    @Volatile private var finishing = false
    @Volatile private var playback: Thread? = null

    private fun configure() {
        val points = JSONObject(Native.navigation()).getJSONObject("positions")
        val center = points.getJSONArray("stick_center")
        val edge = points.getJSONArray("stick_edge")
        val screen = service.screen()
        Joy.sw = screen[0]
        Joy.sh = screen[1]
        val cx = center.getDouble(0) * screen[0]
        val cy = center.getDouble(1) * screen[1]
        Joy.at(cx, cy, Math.hypot(edge.getDouble(0) * screen[0] - cx, edge.getDouble(1) * screen[1] - cy))
        jumpReady = points.has("jump_btn")
        Joy.jx = -1f
        Joy.jy = -1f
        points.optJSONArray("jump_btn")?.let { jump ->
            Joy.jx = (jump.getDouble(0) * screen[0]).toFloat()
            Joy.jy = (jump.getDouble(1) * screen[1]).toFloat()
        }
        Joy.generation = Native.generation()
        Joy.start()
    }

    fun beginRecord() {
        if (pad != null || recorder?.isAlive == true) {
            throw IllegalStateException("이전 길 기록을 정리 중입니다. 잠깐 뒤 다시 시작하세요.")
        }
        configure()
        val input = RecPad()
        pad = input
        finishing = false
        val generation = Joy.generation
        Joy.pilot.traceStart()
        input.show()
        recorder = Thread({
            try {
                while (Native.inputCurrent(generation) && !Thread.currentThread().isInterrupted && !finishing) {
                    Joy.pilot.set(input.stick())
                    var jump = input.pollJump()
                    while (jump != 0) {
                        if (jumpReady) Joy.pilot.jump(jump > 0)
                        jump = input.pollJump()
                    }
                    Thread.sleep(8)
                }
            } catch (_: InterruptedException) {
            } finally {
                Joy.pilot.set(null)
                Joy.pilot.jump(false)
                Joy.settle(500)
                val events = Joy.pilot.traceEnd()
                if (finishing && Native.inputCurrent(generation)) {
                    val json = JSONArray()
                    events.forEach { event ->
                        json.put(JSONObject().put("k", event.k.toString()).put("t", event.t).put("dx", event.dx).put("dy", event.dy).put("a", event.a).put("b", event.b))
                    }
                    val error = Native.saveTrace(json.toString())
                    if (error != null) { Native.platformLog(error, 'r'); Native.control(0) }
                }
                main.post { input.hide(); if (pad === input) pad = null; recorder = null }
            }
        }, "acrux-route-record").also { it.start() }
    }

    fun arrive() { finishing = true }
    fun cancelRecord() {
        if (finishing) return
        // Invalidate the recording generation before its finally block can save.
        Native.control(0)
        stop()
    }
    fun stop() {
        recorder?.interrupt()
        playback?.interrupt()
        Joy.pilot.set(null)
        Joy.pilot.jump(false)
        Joy.userDownAt = 0
        pad?.hide()
        pad = null
    }

    fun play(packet: IntArray, done: (Boolean) -> Unit) {
        val screen = service.screen()
        Joy.sw = screen[0]
        Joy.sh = screen[1]
        Joy.at(packet[2].toDouble(), packet[3].toDouble(), Math.hypot((packet[4] - packet[2]).toDouble(), (packet[5] - packet[3]).toDouble()))
        Joy.jx = packet[6].toFloat()
        Joy.jy = packet[7].toFloat()
        Joy.generation = packet.last()
        val generation = packet.last()
        val events = (8 until packet.lastIndex step 5).map { at ->
            TraceEvent(packet[at].toChar(), packet[at + 1] / 10.0, packet[at + 2] / 1000.0, packet[at + 3] / 1000.0,
                packet[at + 4] and 1 != 0, packet[at + 4] and 2 != 0 && packet[6] >= 0)
        }
        Joy.start()
        playback = Thread({
            var success = true
            val priority = android.os.Process.getThreadPriority(android.os.Process.myTid())
            try {
                android.os.Process.setThreadPriority(android.os.Process.THREAD_PRIORITY_URGENT_DISPLAY)
                Joy.pilot.play(events) {
                    if (!Native.inputCurrent(generation) || !service.canInput() || Thread.currentThread().isInterrupted) throw IllegalStateException("길 재생 중지")
                }
            } catch (error: RuntimeException) {
                success = false
                if (Native.inputCurrent(generation)) Native.platformLog(error.message ?: "길 재생 실패", 'r')
            }
            finally {
                android.os.Process.setThreadPriority(priority)
                if (Joy.pilot.cuts > 0 && Native.inputCurrent(generation)) {
                    Native.platformLog("이동 중 터치가 ${Joy.pilot.cuts}번 끊김 — 다시 눌러서 이어 감 (+${Joy.pilot.lostMs / 1000.0}초)", 'y')
                }
                main.post { playback = null; done(success) }
            }
        }, "acrux-route-play").also { it.start() }
    }
}
