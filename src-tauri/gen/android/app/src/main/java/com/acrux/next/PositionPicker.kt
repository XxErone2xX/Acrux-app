package com.acrux.next

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.Point
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.widget.Button
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView

class PositionPicker(
    private val service: TapService,
    name: String,
    initial: Pair<Double, Double>?,
    private val done: (Pair<Double, Double>?) -> Unit,
) {
    private val manager = service.getSystemService(WindowManager::class.java)
    private val frame = FrameLayout(service)
    private val physical = Point().also {
        @Suppress("DEPRECATION")
        manager.defaultDisplay.getRealSize(it)
    }
    private val pointer = object : View(service) {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
        private val delta = service.resources.displayMetrics.density
        var xPos = (initial?.first?.coerceIn(0.0, .9999)?.times(physical.x) ?: physical.x * .5).toFloat()
        var yPos = (initial?.second?.coerceIn(0.0, .9999)?.times(physical.y) ?: physical.y * .5).toFloat()
        private var lastX = 0f
        private var lastY = 0f

        override fun onTouchEvent(event: MotionEvent): Boolean {
            if (event.actionMasked == MotionEvent.ACTION_MOVE) {
                xPos = (xPos + (event.rawX - lastX) * .6f).coerceIn(1f, (width - 1).coerceAtLeast(1).toFloat())
                yPos = (yPos + (event.rawY - lastY) * .6f).coerceIn(1f, (height - 1).coerceAtLeast(1).toFloat())
                invalidate()
            }
            lastX = event.rawX
            lastY = event.rawY
            return true
        }

        override fun onDraw(canvas: Canvas) {
            val arm = 19 * delta
            paint.style = Paint.Style.STROKE
            for ((color, line) in listOf(Color.BLACK to 5f, Color.WHITE to 2f)) {
                paint.color = color
                paint.strokeWidth = line * delta
                canvas.drawCircle(xPos, yPos, 12 * delta, paint)
                canvas.drawLine(xPos - arm, yPos, xPos + arm, yPos, paint)
                canvas.drawLine(xPos, yPos - arm, xPos, yPos + arm, paint)
            }
            paint.style = Paint.Style.FILL
            paint.color = Color.RED
            canvas.drawCircle(xPos, yPos, 2 * delta, paint)
        }
    }

    init {
        frame.addView(pointer, FrameLayout.LayoutParams(-1, -1))
        val controls = LinearLayout(service).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setBackgroundColor(0xee2b2d31.toInt())
        }
        controls.addView(TextView(service).apply {
            text = name
            setTextColor(Color.WHITE)
            setPadding(12, 4, 12, 4)
        }, LinearLayout.LayoutParams(0, -2, 1f))
        controls.addView(Button(service).apply {
            text = "확인"
            setOnClickListener {
                val origin = IntArray(2)
                pointer.getLocationOnScreen(origin)
                finish(Pair((origin[0] + pointer.xPos) / physical.x.toDouble(), (origin[1] + pointer.yPos) / physical.y.toDouble()))
            }
        })
        controls.addView(Button(service).apply {
            text = "취소"
            setOnClickListener { finish(null) }
        })
        frame.addView(controls, FrameLayout.LayoutParams(-1, -2, Gravity.BOTTOM))
        manager.addView(frame, WindowManager.LayoutParams(
            -1, -1, WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
            PixelFormat.TRANSLUCENT,
        ))
    }

    private fun finish(point: Pair<Double, Double>?) {
        remove()
        done(point)
    }

    fun remove() {
        if (frame.isAttachedToWindow) manager.removeViewImmediate(frame)
    }
}
