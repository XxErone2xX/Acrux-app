package com.acrux.next

import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.PixelFormat
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import org.json.JSONObject

internal class DebugOverlay(private val service: TapService) {
    private val manager = service.getSystemService(WindowManager::class.java)
    private val ink = Paint().apply {
        color = 0xffff0dc5.toInt()
        style = Paint.Style.STROKE
        strokeWidth = 2f
    }
    @Volatile var visible = false
        private set
    private var geometry: String? = null
    private var data: JSONObject? = null
    private val drawable = object : View(service) {
        private val origin = IntArray(2)

        override fun onDraw(canvas: Canvas) {
            val snapshot = data ?: return
            getLocationOnScreen(origin)
            val screen = service.screen()
            fun x(value: Double) = (value * screen[0] - origin[0]).toFloat()
            fun y(value: Double) = (value * screen[1] - origin[1]).toFloat()
            val boxes = snapshot.getJSONArray("regions")
            for (i in 0 until boxes.length()) {
                val bounds = boxes.getJSONArray(i)
                canvas.drawRect(x(bounds.getDouble(0)), y(bounds.getDouble(1)),
                    x(bounds.getDouble(2)), y(bounds.getDouble(3)), ink)
            }
            val points = snapshot.getJSONArray("points")
            for (i in 0 until points.length()) {
                val point = points.getJSONArray(i)
                val cx = x(point.getDouble(0))
                val cy = y(point.getDouble(1))
                canvas.drawLine(cx - 7, cy, cx + 7, cy, ink)
                canvas.drawLine(cx, cy - 7, cx, cy + 7, ink)
            }
        }
    }

    fun update(snapshot: String?) {
        if (geometry == snapshot) return
        geometry = snapshot
        data = snapshot?.let(::JSONObject)
        if (data == null) {
            if (drawable.isAttachedToWindow) manager.removeViewImmediate(drawable)
            visible = false
            return
        }
        if (!drawable.isAttachedToWindow) {
            try {
                manager.addView(drawable, WindowManager.LayoutParams(
                    -1, -1, WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                    WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                        WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN, PixelFormat.TRANSLUCENT,
                ).apply { gravity = Gravity.TOP or Gravity.START })
                visible = true
            } catch (_: RuntimeException) {
                geometry = null
            }
        }
        drawable.invalidate()
    }
}
