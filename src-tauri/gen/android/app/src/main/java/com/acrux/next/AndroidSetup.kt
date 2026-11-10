package com.acrux.next

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import java.nio.ByteBuffer

@InvokeArg
class OverlayOptions {
    var visible = true
    var hideOnApp = true
    var alpha = 90
    var lang = "ko"
    var reset = false
}

@InvokeArg
class BiomeAction { var action = "refresh" }

@TauriPlugin
class AndroidSetup(private val activity: Activity) : Plugin(activity) {
    @Command
    fun biome(invoke: Invoke) {
        val args = invoke.parseArgs(BiomeAction::class.java)
        activity.runOnUiThread {
            try { LogService.action(activity, args.action); invoke.resolve() }
            catch (error: RuntimeException) { invoke.reject(error.toString()) }
        }
    }
    @Command
    fun biomeTransport(invoke: Invoke) {
        invoke.resolve(app.tauri.plugin.JSObject().apply {
            put("connection", LogService.adbState)
            put("testOutput", LogService.testOutput)
            put("testUntil", LogService.testUntil)
        })
    }

    @Command
    fun accessibility(invoke: Invoke) {
        activity.runOnUiThread {
            try {
                activity.startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS))
                invoke.resolve()
            } catch (error: RuntimeException) {
                invoke.reject(error.toString())
            }
        }
    }

    @Command
    fun capture(invoke: Invoke) {
        activity.runOnUiThread {
            try {
                (activity as MainActivity).requestCapture()
                invoke.resolve()
            } catch (error: RuntimeException) {
                invoke.reject(error.toString())
            }
        }
    }

    @Command
    fun stopCapture(invoke: Invoke) {
        activity.stopService(Intent(activity, CaptureService::class.java))
        invoke.resolve()
    }

    @Command
    fun appInfo(invoke: Invoke) = open(invoke, Intent(
        Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", activity.packageName, null),
    ))

    @Command
    fun donate(invoke: Invoke) = open(invoke, Intent(Intent.ACTION_VIEW, Uri.parse("https://ko-fi.com/RG_Acrux")))

    private fun open(invoke: Invoke, intent: Intent) {
        activity.runOnUiThread {
            try {
                activity.startActivity(intent)
                invoke.resolve()
            } catch (error: RuntimeException) {
                invoke.reject(error.toString())
            }
        }
    }

    @Command
    fun overlay(invoke: Invoke) {
        val args = invoke.parseArgs(OverlayOptions::class.java)
        activity.runOnUiThread {
            val prefs = activity.getSharedPreferences("overlay", Activity.MODE_PRIVATE)
            val editor = prefs.edit()
                .putBoolean("visible", args.visible)
                .putBoolean("hideOnApp", args.hideOnApp)
                .putInt("alpha", args.alpha.coerceIn(40, 100))
                .putString("lang", args.lang)
            if (args.reset) editor.remove("x").remove("y")
            editor.apply()
            if (!args.visible && Native.active()) Native.control(0)
            TapService.refresh(args.reset)
            invoke.resolve()
        }
    }

}

object Native {
    init {
        System.loadLibrary("acrux_lib")
    }

    @JvmStatic external fun biomeStatus(): String
    @JvmStatic external fun biomeToggle(): String?
    @JvmStatic external fun biomeLine(line: String, source: Int, snapshot: Boolean)
    @JvmStatic external fun biomeConnection(ready: Boolean, message: String)
    @JvmStatic external fun platformLog(message: String, color: Char)
    @JvmStatic external fun capture(ready: Boolean)
    @JvmStatic external fun accessibility(ready: Boolean)
    @JvmStatic external fun active(): Boolean
    @JvmStatic external fun inputCurrent(generation: Int): Boolean
    @JvmStatic external fun overlayPhase(): Int
    @JvmStatic external fun captureReady(): Boolean
    @JvmStatic external fun noTarget()
    @JvmStatic external fun inputRejected()
    @JvmStatic external fun message(): String
    @JvmStatic external fun task(task: String): String?
    @JvmStatic external fun setPoint(key: String, x: Double, y: Double): String?
    @JvmStatic external fun navigation(): String
    @JvmStatic external fun saveTrace(trace: String): String?
    @JvmStatic external fun generation(): Int
    @JvmStatic external fun clearRoute(sell: Boolean): String?
    @JvmStatic external fun control(operation: Int): String?
    @JvmStatic external fun frame(buffer: ByteBuffer, width: Int, height: Int, rowStride: Int, pixelStride: Int): IntArray?
}
