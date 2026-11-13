package com.acrux.next

import android.content.Context
import android.graphics.Canvas
import android.graphics.ColorFilter
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.Typeface
import android.graphics.drawable.Drawable
import android.os.Handler
import android.os.Looper
import android.util.Log
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
    private val main = Handler(Looper.getMainLooper())
    @Volatile private var macroRequest = false
    @Volatile private var actionRevision = 0
    private var inGame = false
    private var status: TextView? = null
    private var biome: TextView? = null
    private var run: TextView? = null
    private var lang = ""
    private val dictionary = mutableMapOf<String, String>()
    private val categories = listOf("낚시", "조이스틱", "기준 장소", "길", "판매", "매크로")
    private val params = WindowManager.LayoutParams(
        WindowManager.LayoutParams.WRAP_CONTENT, WindowManager.LayoutParams.WRAP_CONTENT,
        WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
        WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL
            or WindowManager.LayoutParams.FLAG_SECURE,
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
        status?.apply {
            val message = t(if (Native.captureReady()) Native.message().let { if (it == "대기 중") "대기" else it } else "화면 캡처 꺼짐 · 앱에서 시작")
            if (text.toString() != message) {
                Log.i("AcruxDUA", message)
                text = message
            }
        }
        biome?.apply {
            val state = JSONObject(Native.biomeStatus())
            val enabled = state.getJSONObject("settings").optBoolean("enabled")
            setTextColor(if (!enabled) MUTED else if (state.optBoolean("watching")) GREEN else YELLOW)
        }
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
        ++actionRevision
        macroRequest = false
        picker?.remove()
        picker = null
        panel?.let { wm.removeView(it) }
        panel = null
        menu = null
        status = null
        biome = null
        run = null
    }

    fun contains(x: Float, y: Float): Boolean {
        return intersects(x, y, x, y)
    }

    fun intersects(x0: Float, y0: Float, x1: Float, y1: Float): Boolean {
        val view = panel ?: return false
        if (picker != null) return true
        if (params.flags and WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE != 0) return false
        val origin = IntArray(2)
        view.getLocationOnScreen(origin)
        return AgentTransport.intersects(x0, y0, x1, y1, origin[0], origin[1], origin[0] + view.width, origin[1] + view.height)
    }

    fun routePassThrough(packet: IntArray) {
        val view = panel ?: return
        if (picker != null) return
        val origin = IntArray(2)
        view.getLocationOnScreen(origin)
        val cx = packet[2].toDouble()
        val cy = packet[3].toDouble()
        val radius = Math.hypot((packet[4] - packet[2]).toDouble(), (packet[5] - packet[3]).toDouble()) * Joy.PUSH
        val nearestX = cx.coerceIn(origin[0].toDouble(), (origin[0] + view.width).toDouble())
        val nearestY = cy.coerceIn(origin[1].toDouble(), (origin[1] + view.height).toDouble())
        val coversStick = Math.hypot(cx - nearestX, cy - nearestY) <= radius
        val coversJump = packet[6] >= 0 && contains(packet[6].toFloat(), packet[7].toFloat())
        if (!coversStick && !coversJump) return
        params.flags = params.flags or WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE
        panel?.let { wm.updateViewLayout(it, params) }
    }

    fun restoreTouch() {
        if (params.flags and WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE == 0) return
        params.flags = params.flags and WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE.inv()
        panel?.let { wm.updateViewLayout(it, params) }
    }

    private fun operation(operation: Int) {
        if (operation == 0) { ++actionRevision; macroRequest = false }
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
        fun diagnostic(label: String, action: String, vararg required: String) {
            val ready = inGame && required.all(points::has)
            body.addView(button(label, MUTED) {
                if (!inGame) { feedback("Roblox에서 실행하세요."); return@button }
                if (Native.active()) { feedback("실행 중인 작업을 먼저 중지하세요."); return@button }
                val size = service.screen()
                val error = Native.navigationAction(action, size[0], size[1])
                feedback(error)
                if (error == null) closeMenu()
                refresh()
            }.apply { isEnabled = ready; if (!ready) alpha = 0.6f }, rowParams(4))
        }
        fun nativeAction(label: String, action: String, payload: JSONObject = JSONObject()) {
            item(label) { macroAction(action, payload) }
        }
        fun destination(name: String, destination: String) {
            heading(name)
            nativeAction("이 장소로 이동", "go", JSONObject().put("destination", destination))
            nativeAction("현재 장소 기억", "waypoint_mark", JSONObject().put("destination", destination))
        }
        when (category) {
            0 -> {
                heading("낚시")
                item("낚시 자동 보정") { closeMenu(); operation(1) }
                item("선택한 기능과 이동 · 판매를 DUA가 제어합니다.")
                nativeAction("낚시 중지", "mfish_stop")
            }
            1 -> {
                heading("조이스틱 (W · A · S · D)")
                item(if (points.has("stick_edge")) "✓ " + t("조이스틱 저장됨") else "· " + t("조이스틱 없음 — 로블록스 설정 → 이동 방식: 조이스틱"))
                item("조이스틱 직접 지정 (가운데 → 가장자리)") {
                    pick("stick_center", "조이스틱 가운데") { pick("stick_edge", "조이스틱 가장자리") }
                }
                heading("점프")
                position("jump_btn", "점프 버튼 (오른쪽 아래)")
                heading("테스트")
                diagnostic("앞으로", "forward", "stick_center", "stick_edge")
                diagnostic("뒤로", "back", "stick_center", "stick_edge")
                diagnostic("왼쪽", "left", "stick_center", "stick_edge")
                diagnostic("오른쪽", "right", "stick_center", "stick_edge")
                diagnostic("점프", "jump", "jump_btn")
                diagnostic("이동하며 점프", "moving_jump", "stick_center", "stick_edge", "jump_btn")
            }
            2 -> {
                heading("공통 위치 (기준 장소 맞추기)")
                BASE.forEach { (key, label) -> position(key, label) }
                heading("테스트")
                nativeAction("기준 장소로 이동", "move_base")
                nativeAction("현재 장소 기억", "waypoint_mark", JSONObject().put("destination", "base"))
            }
            3 -> {
                item("현재 위치와 지형을 확인해 경로를 계산합니다.")
                destination("낚시 장소", "coast")
                destination("물고기 판매 장소", "sell")
                destination("포션 제작 장소", "craft")
                destination("메모리 매치 장소", "match")
                nativeAction("목적지 관측 취소", "waypoint_cancel")
                item("이동 정지") { operation(0) }
                heading("화면 기반 이동 입력 확인")
                diagnostic("카메라 위로", "camera_up")
                diagnostic("줌 아웃", "zoom_out")
                diagnostic("왼쪽", "left", "stick_center", "stick_edge")
                diagnostic("오른쪽", "right", "stick_center", "stick_edge")
                diagnostic("앞으로", "forward", "stick_center", "stick_edge")
                diagnostic("뒤로", "back", "stick_center", "stick_edge")
                diagnostic("점프", "jump", "jump_btn")
                diagnostic("이동하며 점프", "moving_jump", "stick_center", "stick_edge", "jump_btn")
            }
            4 -> {
                heading("대화")
                position("talk_btn", "대화 버튼 (E) (판매 장소에 서서)")
                position("dialog_pos", "대화창")
                position("sell_fish_pos", "Sell Fish 버튼")
                heading("상점")
                nativeAction("판매 자동 보정", "sell_autocal")
                item(t("판매 위치 {0} / 6 저장됨").replace("{0}", SELL.count { points.has(it.first) }.toString()))
                SELL.drop(2).forEach { (key, label) -> position(key, label) }
                heading("테스트")
                nativeAction("판매 테스트", "sell_test")
            }
            5 -> {
                heading("개별 기능 테스트")
                item("기능 선택과 상세 설정은 앱의 홈 · 설정에서 변경합니다.")
                nativeAction("CYBERSPACE 팝핑 테스트", "mpop_test", JSONObject().put("biome", "CYBERSPACE"))
                nativeAction("GLITCHED 팝핑 테스트", "mpop_test", JSONObject().put("biome", "GLITCHED"))
                nativeAction("DREAMSPACE 팝핑 테스트", "mpop_test", JSONObject().put("biome", "DREAMSPACE"))
                nativeAction("주기 아이템 테스트", "mitem_test")
                nativeAction("블러드 레인 테스트", "mblood_test")
                nativeAction("상인 채팅 확인", "mmerch_check")
                nativeAction("Mari 구매 테스트", "mmerch_buy_test", JSONObject().put("name", "Mari"))
                nativeAction("Jester 구매 테스트", "mmerch_buy_test", JSONObject().put("name", "Jester"))
                nativeAction("상점 자동 보정", "mmerch_autocal")
                nativeAction("포션 제작 테스트", "mcraft_test")
                nativeAction("제작 창 자동 보정", "mcraft_autocal")
                nativeAction("메모리 매치 테스트", "mmatch_test")
                nativeAction("메모리 매치 알림 보정", "mmatch_autocal")
                nativeAction("카드 판 보정", "mmatch_boardcal")
                nativeAction("인벤토리 OCR 확인", "mpop_ocr_test")
                heading("접속 · 복귀")
                nativeAction("내 서버 접속", "ret_join")
                nativeAction("복귀 테스트", "ret_test")
                nativeAction("Play 테스트", "play_test")
                nativeAction("자동 복귀 취소", "crash_cancel")
                item("모두 중지") { operation(0) }
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

    private fun macroAction(action: String, payload: JSONObject = JSONObject()) {
        val cancelling = action.endsWith("_stop") || action == "waypoint_cancel" || action == "crash_cancel"
        if (!inGame && !cancelling) { feedback("Roblox에서 실행하세요."); return }
        if (macroRequest && !cancelling) { feedback("요청을 처리하고 있습니다. 잠시 기다려 주세요."); return }
        if (cancelling) ++actionRevision
        val revision = actionRevision
        val generation = Native.generation()
        payload.put("_generation", generation)
        macroRequest = true
        closeMenu()
        Thread({
            val error = try {
                if (revision != actionRevision || generation != Native.generation()) "이전 실행 요청이 취소됐습니다."
                else Native.macroAction(action, payload.toString())
            } catch (error: RuntimeException) { error.message ?: "작업 요청을 처리하지 못했습니다." }
            main.post {
                if (revision == actionRevision) {
                    macroRequest = false
                    feedback(error)
                }
                refresh()
            }
        }, "acrux-overlay-action").start()
    }

    fun selectMacro(label: String, initial: List<Double>, region: Boolean, done: (List<Double>?) -> Unit) {
        if (!inGame || Native.active() || picker != null) { done(null); return }
        val view = panel
        view?.visibility = View.GONE
        fun finish(pos: List<Double>?) { picker = null; view?.visibility = View.VISIBLE; done(pos) }
        fun second(first: Pair<Double, Double>) {
            val at = if (initial.size == 4) Pair(initial[2], initial[3]) else null
            picker = PositionPicker(service, label + " — " + t("오른쪽 아래"), at) { last ->
                picker = null
                if (last == null) finish(null)
                else finish(listOf(minOf(first.first,last.first),minOf(first.second,last.second),maxOf(first.first,last.first),maxOf(first.second,last.second)))
            }
        }
        try {
            val at = if (initial.size >= 2) Pair(initial[0], initial[1]) else null
            picker = PositionPicker(service, label + (if (region) " — " + t("왼쪽 위") else ""), at) { first ->
                picker = null
                if (first == null) finish(null)
                else if (region) second(first)
                else finish(listOf(first.first,first.second))
            }
        } catch (error: RuntimeException) { finish(null) }
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
                inMenu = menu?.let { event.y >= it.top } == true
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
        const val RED = 0xffed4245.toInt()
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
