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
    private val physical = Point()
    private val pointer = object : View(service) {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
        private val delta = service.resources.displayMetrics.density
        var xPos = 0f
        var yPos = 0f
        private var positioned = false
        private var lastX = 0f
        private var lastY = 0f

        @Suppress("DEPRECATION")
        override fun onLayout(changed: Boolean, left: Int, top: Int, right: Int, bottom: Int) {
            super.onLayout(changed, left, top, right, bottom)
            manager.defaultDisplay.getRealSize(physical)
            if (width < 1 || height < 1 || physical.x < 1 || physical.y < 1) return
            if (!positioned) {
                val origin = IntArray(2)
                getLocationOnScreen(origin)
                xPos = PickerCoordinates.local(initial?.first, physical.x, origin[0], width)
                yPos = PickerCoordinates.local(initial?.second, physical.y, origin[1], height)
                positioned = true
            } else {
                xPos = PickerCoordinates.clamp(xPos, width)
                yPos = PickerCoordinates.clamp(yPos, height)
            }
        }

        override fun onTouchEvent(event: MotionEvent): Boolean {
            if (event.actionMasked == MotionEvent.ACTION_MOVE) {
                xPos = PickerCoordinates.clamp(xPos + (event.rawX - lastX) * .6f, width)
                yPos = PickerCoordinates.clamp(yPos + (event.rawY - lastY) * .6f, height)
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
                if (physical.x < 1 || physical.y < 1) return@setOnClickListener
                val origin = IntArray(2)
                pointer.getLocationOnScreen(origin)
                finish(Pair(
                    PickerCoordinates.normalized(pointer.xPos, origin[0], physical.x),
                    PickerCoordinates.normalized(pointer.yPos, origin[1], physical.y),
                ))
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

/** Physical-screen profile coordinates and the measured picker view use different origins. */
internal object PickerCoordinates {
    fun clamp(local: Float, extent: Int): Float = if (extent <= 1) 0f
        else local.coerceIn(1f, (extent - 1).toFloat())

    fun local(saved: Double?, physical: Int, origin: Int, extent: Int): Float {
        require(physical > 0)
        val point = saved?.takeIf { it.isFinite() }?.coerceIn(0.0, .9999) ?: .5
        return clamp((point * physical - origin).toFloat(), extent)
    }

    fun normalized(local: Float, origin: Int, physical: Int): Double {
        require(physical > 0)
        return ((origin.toDouble() + local) / physical).coerceIn(0.0, .9999)
    }
}
