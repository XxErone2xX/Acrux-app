package com.acrux.next

import android.content.Context
import android.graphics.Canvas
import android.graphics.ColorFilter
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.Typeface
import android.graphics.drawable.Drawable
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.view.WindowManager
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import org.json.JSONObject
import kotlin.math.abs
import kotlin.math.roundToInt

class Overlay(private val service: TapService) {
    private val prefs = service.getSharedPreferences("overlay", Context.MODE_PRIVATE)
    private val wm = service.getSystemService(WindowManager::class.java)
    private var panel: Panel? = null
    private var menu: LinearLayout? = null
    private var picker: PositionPicker? = null
    private var recording: LinearLayout? = null
    private var inGame = false
    private var status: TextView? = null
    private var biome: TextView? = null
    private var run: TextView? = null
    private var lang = ""
    private val dictionary = mutableMapOf<String, String>()
    private val categories = listOf("낚시", "조이스틱", "기준 장소", "길", "판매")
    private val params = WindowManager.LayoutParams(
        WindowManager.LayoutParams.WRAP_CONTENT, WindowManager.LayoutParams.WRAP_CONTENT,
        WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
        WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
        PixelFormat.TRANSLUCENT,
    ).apply { gravity = Gravity.TOP or Gravity.START }

    private fun dp(n: Int) = (n * service.resources.displayMetrics.density).roundToInt()
    private fun t(s: String) = dictionary[s] ?: s

    private fun readLang() {
        lang = prefs.getString("lang", "ko") ?: "ko"
        dictionary.clear()
        val column = when (lang) {
            "en" -> 1
            "ja" -> 2
            else -> return
        }
        for (name in listOf("pc.tsv", "mobile.tsv")) {
            service.assets.open("i18n/$name").bufferedReader().useLines { lines ->
                lines.filter { it.isNotBlank() && !it.startsWith("#") }.forEach {
                    val columns = it.split('\t')
                    columns.getOrNull(column)?.takeIf(String::isNotBlank)?.let { translated ->
                        dictionary[columns[0]] = translated.replace("\\n", "\n")
                    }
                }
            }
        }
    }

    fun update(gameVisible: Boolean, appVisible: Boolean, reset: Boolean) {
        inGame = gameVisible
        val visible = prefs.getBoolean("visible", true) && (gameVisible || appVisible && !prefs.getBoolean("hideOnApp", true))
        if (!visible || lang != prefs.getString("lang", "ko")) remove()
        if (!visible) return
        if (panel == null) show()
        panel?.let { view ->
            view.alpha = prefs.getInt("alpha", 90) / 100f
            if (reset) {
                params.x = dp(12)
                params.y = dp(12)
                wm.updateViewLayout(view, params)
            }
        }
        refresh()
    }

    fun refresh() {
        val phase = Native.overlayPhase()
        status?.setTextIfChanged(t(if (Native.captureReady()) Native.message().let { if (it == "대기 중") "대기" else it } else "화면 캡처 꺼짐 · 앱에서 시작"))
        biome?.apply {
            val state = JSONObject(Native.biomeStatus())
            val enabled = state.getJSONObject("settings").optBoolean("enabled")
            setTextColor(if (!enabled) MUTED else if (state.optBoolean("watching")) GREEN else YELLOW)
        }
        if (phase == 3 && recording == null && inGame) showRecorder()
        if (phase != 3 && recording != null) { service.routes.stop(); recording?.let { panel?.removeView(it) }; recording = null }
        run?.apply {
            setTextIfChanged(t(if (phase > 0) "정지" else "시작"))
        }
    }

    private fun TextView.setTextIfChanged(value: String) {
        if (text.toString() != value) text = value
    }

    private fun show() {
        readLang()
        val view = Panel()
        val padding = dp(10)
        view.setPadding(padding, padding, padding, padding)
        view.background = Deco()
        val title = text("ACRUX MACRO", 12, FG, true)
        title.letterSpacing = 0.12f
        view.addView(title)
        status = text("대기", 11, MUTED).apply { maxWidth = dp(170) }
        view.addView(status, rowParams(2))
        val buttons = LinearLayout(service)
        val bio = button("바이옴", MUTED) {
            val error = Native.biomeToggle()
            feedback(error)
            if (error == null) LogService.refresh(service)
            refresh()
        }
        val start = button("시작", ACCENT) { operation(if (Native.overlayPhase() > 0) 0 else 2) }
        biome = bio
        run = start
        buttons.addView(bio)
        buttons.addView(start, rowParams().apply { leftMargin = dp(4) })
        buttons.addView(button("⚙", MUTED, ::toggleMenu), rowParams().apply { leftMargin = dp(4) })
        view.addView(buttons, rowParams(8))
        params.x = prefs.getInt("x", dp(12))
        params.y = prefs.getInt("y", dp(12))
        view.alpha = prefs.getInt("alpha", 90) / 100f
        panel = view
        try {
            wm.addView(view, params)
        } catch (error: RuntimeException) {
            panel = null
            status = null
            biome = null
            run = null
        }
    }

    fun remove() {
        picker?.remove()
        picker = null
        recording = null
        panel?.let { wm.removeView(it) }
        panel = null
        menu = null
        status = null
        biome = null
        run = null
    }

    fun contains(x: Float, y: Float): Boolean {
        val view = panel ?: return false
        if (picker != null) return true
        val origin = IntArray(2)
        view.getLocationOnScreen(origin)
        return x >= origin[0] && x < origin[0] + view.width && y >= origin[1] && y < origin[1] + view.height
    }

    private fun operation(operation: Int) {
        if (operation == 0 && Native.overlayPhase() == 3) { service.routes.arrive(); return }
        Native.control(operation)?.let { Toast.makeText(service, t(it), Toast.LENGTH_SHORT).show() }
        refresh()
    }

    private fun rowParams(top: Int = 0) = LinearLayout.LayoutParams(-2, -2).apply { topMargin = dp(top) }
    private fun text(label: String, size: Int, color: Int, bold: Boolean = false) = TextView(service).apply {
        text = t(label)
        textSize = size.toFloat()
        setTextColor(color)
        if (bold) typeface = Typeface.DEFAULT_BOLD
    }

    private fun button(label: String, color: Int, click: () -> Unit) = text(
        label, 12, if (color == ACCENT) WHITE else if (color == MUTED) FG else color, true,
    ).apply {
        gravity = Gravity.CENTER
        setPadding(dp(10), dp(6), dp(10), dp(6))
        setBackgroundColor(when (color) {
            ACCENT -> ACCENT
            MUTED -> FIELD
            else -> (color and 0x00ffffff) or 0x2e000000
        })
        setOnClickListener { click() }
    }

    private fun toggleMenu() {
        val view = panel ?: return
        menu?.let {
            view.removeView(it)
            menu = null
            return
        }
        showMenu(prefs.getInt("category", 0).coerceIn(categories.indices))
    }

    private fun showMenu(category: Int) {
        val view = panel ?: return
        menu?.let { view.removeView(it) }
        prefs.edit().putInt("category", category).apply()
        val wrapper = LinearLayout(service).apply { orientation = LinearLayout.VERTICAL }
        val tabs = LinearLayout(service)
        categories.forEachIndexed { index, label ->
            val chip = button(label, MUTED) { showMenu(index) }.apply {
                setPadding(dp(8), dp(5), dp(8), dp(5))
                setTextColor(if (category == index) FG else MUTED)
                background = android.graphics.drawable.GradientDrawable().apply {
                    setColor(if (category == index) 0x295865f2 else PANEL)
                    setStroke(dp(1), if (category == index) ACCENT else LINE)
                }
            }
            tabs.addView(chip, rowParams().apply { if (index > 0) leftMargin = dp(4) })
        }
        wrapper.addView(HorizontalScrollView(service).apply { isHorizontalScrollBarEnabled = false; addView(tabs) })
        val body = LinearLayout(service).apply { orientation = LinearLayout.VERTICAL }
        val data = JSONObject(Native.navigation())
        val points = data.optJSONObject("positions") ?: JSONObject()
        fun heading(label: String) = body.addView(text(label, 11, MUTED, true), rowParams(7))
        fun item(label: String, action: (() -> Unit)? = null) {
            val entry = text(label, 12, FG).apply {
                setPadding(dp(6), dp(6), dp(6), dp(6))
                if (action != null) setOnClickListener { action() }
            }
            body.addView(entry, LinearLayout.LayoutParams(-1, -2))
        }
        fun position(key: String, label: String) {
            item((if (points.has(key)) "✓ " else "· ") + t(label)) { pick(key, label) }
        }
        fun route(sell: Boolean) {
            heading(if (sell) "물고기 판매 장소" else "낚시 장소")
            val count = data.optInt(if (sell) "sell" else "fish")
            item(if (count > 0) "✓ " + count + " " + t("조각 저장됨") else "· " + t("길 없음"))
            val row = LinearLayout(service)
            row.addView(button("길 기록", ACCENT) { task(if (sell) "recordSell" else "recordFish") })
            row.addView(button("지우기", MUTED) {
                feedback(Native.clearRoute(sell))
                showMenu(category)
            }, rowParams().apply { leftMargin = dp(4) })
            body.addView(row, rowParams(4))
        }
        when (category) {
            0 -> {
                heading("낚시")
                item("낚시 자동 보정") { closeMenu(); operation(1) }
                item("보정 · 테스트 · 길 기록 중에는 [정지] 로 멈춤")
            }
            1 -> {
                heading("조이스틱 (W · A · S · D)")
                item(if (points.has("stick_edge")) "✓ " + t("조이스틱 저장됨") else "· " + t("조이스틱 없음 — 로블록스 설정 → 이동 방식: 조이스틱"))
                item("조이스틱 찾기 (화면에서 자동으로)") { feedback("현재는 조이스틱을 직접 지정하세요.") }
                item("조이스틱 직접 지정 (가운데 → 가장자리)") {
                    pick("stick_center", "조이스틱 가운데") { pick("stick_edge", "조이스틱 가장자리") }
                }
                heading("점프")
                position("jump_btn", "점프 버튼 (오른쪽 아래)")
                heading("테스트")
                item("조이스틱 테스트 (W · A · S · D 0.8초씩)") { task("stickTest") }
                item("점프 테스트 (제자리 → 걸으면서)") { task("jumpTest") }
            }
            2 -> {
                heading("공통 위치 (기준 장소 맞추기)")
                BASE.forEach { (key, label) -> position(key, label) }
                heading("테스트")
                item("기준 장소로") { task("base") }
            }
            3 -> {
                route(false)
                route(true)
                heading("테스트")
                item("기준 장소 → 낚시 장소") { task("fish") }
                item("기준 장소 → 판매 장소") { task("shop") }
            }
            4 -> {
                heading("대화")
                position("talk_btn", "대화 버튼 (E) (판매 장소에 서서)")
                position("dialog_pos", "대화창")
                position("sell_fish_pos", "Sell Fish 버튼")
                heading("상점")
                item("판매 자동 보정 (NPC 앞에서 시작 → 대화 버튼은 직접)") { feedback("현재는 아래 판매 위치를 직접 지정하세요.") }
                item("  판매 위치 " + SELL.count { points.has(it.first) } + " / 6 저장됨")
                SELL.drop(2).forEach { (key, label) -> position(key, label) }
                heading("테스트")
                item("판매 한 번 해 보기") { task("sell") }
            }
        }
        val scroller = ScrollView(service).apply { addView(body) }
        wrapper.addView(scroller, LinearLayout.LayoutParams(-1, dp(260).coerceAtMost(service.resources.displayMetrics.heightPixels * 2 / 3 - dp(40))))
        menu = wrapper
        view.addView(wrapper, LinearLayout.LayoutParams(dp(250), -2).apply { topMargin = dp(6) })
    }

    private fun closeMenu() {
        menu?.let { panel?.removeView(it) }
        menu = null
    }

    private fun feedback(error: String?) {
        error?.let { Toast.makeText(service, t(it), Toast.LENGTH_SHORT).show() }
    }

    private fun task(name: String) {
        if (!inGame) {
            feedback("Roblox에서 실행하세요.")
            return
        }
        val error = Native.task(name)
        feedback(error)
        if (error == null) closeMenu()
        refresh()
    }

    private fun pick(key: String, label: String, saved: (() -> Unit)? = null) {
        if (!inGame || Native.active() || picker != null) {
            feedback("Roblox에서 실행 중인 작업을 멈춘 뒤 위치를 지정하세요.")
            return
        }
        val view = panel ?: return
        val points = JSONObject(Native.navigation()).optJSONObject("positions")
        val at = points?.optJSONArray(key)?.let { Pair(it.getDouble(0), it.getDouble(1)) }
        view.visibility = View.GONE
        try {
            picker = PositionPicker(service, t(label) + " — " + t("화면을 끌어서 포인터를 옮기세요"), at) { point ->
                picker = null
                view.visibility = View.VISIBLE
                if (point != null) {
                    val error = Native.setPoint(key, point.first, point.second)
                    feedback(error)
                    if (error == null && saved != null) saved()
                    else showMenu(prefs.getInt("category", 0))
                }
            }
        } catch (error: RuntimeException) {
            view.visibility = View.VISIBLE
            feedback("위치 지정 창을 띄우지 못했습니다.")
        }
    }

    private fun showRecorder() {
        val view = panel ?: return
        closeMenu()
        val pad = LinearLayout(service).apply { orientation = LinearLayout.VERTICAL }
        try { service.routes.beginRecord() }
        catch (error: RuntimeException) { feedback(error.message); Native.control(0); return }
        pad.addView(button("도착", ACCENT) { service.routes.arrive() }, rowParams(6))
        view.addView(pad, LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(6) })
        recording = pad
    }

    private inner class Panel : LinearLayout(service) {
        private val slop = ViewConfiguration.get(service).scaledTouchSlop
        private var startX = 0f
        private var startY = 0f
        private var x = 0
        private var y = 0
        private var moving = false
        private var inMenu = false
        init { orientation = VERTICAL; isClickable = true }

        override fun onInterceptTouchEvent(event: MotionEvent): Boolean {
            if (event.actionMasked == MotionEvent.ACTION_DOWN) {
                startX = event.rawX
                startY = event.rawY
                x = params.x
                y = params.y
                moving = false
                inMenu = recording != null || menu?.let { event.y >= it.top } == true
            }
            if (!inMenu && event.actionMasked == MotionEvent.ACTION_MOVE && dragged(event)) {
                moving = true
                move(event)
                return true
            }
            return false
        }

        private fun dragged(event: MotionEvent) = abs(event.rawX - startX) > slop || abs(event.rawY - startY) > slop

        private fun move(event: MotionEvent) {
            val metrics = resources.displayMetrics
            params.x = (x + (event.rawX - startX).toInt()).coerceIn(0, (metrics.widthPixels - width).coerceAtLeast(0))
            params.y = (y + (event.rawY - startY).toInt()).coerceIn(0, (metrics.heightPixels - height).coerceAtLeast(0))
            wm.updateViewLayout(this, params)
        }

        override fun onTouchEvent(event: MotionEvent): Boolean {
            if (!inMenu && event.actionMasked == MotionEvent.ACTION_MOVE && (moving || dragged(event))) {
                moving = true
                move(event)
                return true
            }
            if (moving && event.actionMasked in listOf(MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL)) {
                prefs.edit().putInt("x", params.x).putInt("y", params.y).apply()
                moving = false
                return true
            }
            return super.onTouchEvent(event)
        }
    }

    private inner class Deco : Drawable() {
        private val paint = Paint()
        override fun draw(canvas: Canvas) {
            val left = bounds.left.toFloat()
            val top = bounds.top.toFloat()
            val right = bounds.right.toFloat()
            val bottom = bounds.bottom.toFloat()
            paint.color = 0xf22b2d31.toInt()
            canvas.drawRect(left, top, right, bottom, paint)
            paint.color = LINE
            paint.style = Paint.Style.STROKE
            paint.strokeWidth = dp(1).toFloat()
            canvas.drawRect(left + .5f, top + .5f, right - .5f, bottom - .5f, paint)
            paint.style = Paint.Style.FILL
            paint.color = WHITE
            val arm = dp(10).toFloat()
            val stroke = dp(2).toFloat()
            for (y in listOf(top, bottom - stroke)) {
                canvas.drawRect(left, y, left + arm, y + stroke, paint)
                canvas.drawRect(right - arm, y, right, y + stroke, paint)
            }
            for (x in listOf(left, right - stroke)) {
                canvas.drawRect(x, top, x + stroke, top + arm, paint)
                canvas.drawRect(x, bottom - arm, x + stroke, bottom, paint)
            }
        }
        override fun setAlpha(alpha: Int) = Unit
        override fun setColorFilter(filter: ColorFilter?) = Unit
        @Deprecated("Required by Drawable")
        override fun getOpacity() = PixelFormat.TRANSLUCENT
    }

    private companion object {
        const val PANEL = 0xff2b2d31.toInt()
        const val FIELD = 0xff383a40.toInt()
        const val LINE = 0xff3f4147.toInt()
        const val FG = 0xffdbdee1.toInt()
        const val MUTED = 0xff949ba4.toInt()
        const val ACCENT = 0xff5865f2.toInt()
        const val GREEN = 0xff3ba55d.toInt()
        const val YELLOW = 0xfff0b232.toInt()
        const val WHITE = 0xffffffff.toInt()
        val BASE = listOf(
            "menu_btn" to "로블록스 메뉴 버튼 (왼쪽 위)", "reset_btn" to "캐릭터 재설정 버튼 (메뉴 안)",
            "reset_confirm" to "재설정 확인 버튼", "collection_pos" to "도감 버튼", "collection_close" to "도감 Exit",
        )
        val SELL = listOf(
            "dialog_pos" to "대화창", "sell_fish_pos" to "Sell Fish 버튼",
            "first_fish_pos" to "첫 번째 물고기 칸", "sell_all_pos" to "Sell All 버튼",
            "confirm_sell_pos" to "확인 Sell 버튼", "shop_close_pos" to "상점 닫기 X",
        )
    }
}
