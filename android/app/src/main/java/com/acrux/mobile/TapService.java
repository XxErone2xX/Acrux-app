package com.acrux.mobile;

import android.accessibilityservice.AccessibilityService;
import android.accessibilityservice.GestureDescription;
import android.content.Context;
import android.content.Intent;
import android.graphics.Path;
import android.graphics.PixelFormat;
import android.os.SystemClock;
import android.hardware.display.DisplayManager;
import android.util.DisplayMetrics;
import android.view.Display;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowManager;
import android.view.accessibility.AccessibilityEvent;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

import com.acrux.mobile.core.Mover;
import com.acrux.mobile.core.Seller;

import java.util.List;
import java.util.Locale;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/**
 * 접근성 서비스 — 로블록스 위에 떠 있는 조작 창 + 모든 터치 (dispatchGesture · 루팅 · Shizuku 없음)
 * 자동 낚시 탭 · 이동 (조이스틱 · 화면 끌기 · 두 손가락 줌) · 판매 버튼
 * 화면 내용은 읽지 않음 (canRetrieveWindowContent = false · 화면 색은 화면 캡처로 봄)
 */
public class TapService extends AccessibilityService {
    static volatile TapService instance;

    WindowManager wm;
    LinearLayout panel, menu, body;
    WindowManager.LayoutParams lp;
    TextView status, calBtn, runBtn, gearBtn, arriveBtn;
    View picker;
    final Runnable refresh = this::refresh;

    @Override
    protected void onServiceConnected() {
        super.onServiceConnected();
        Hub.init(this);
        instance = this;
        Hub.listeners.add(refresh);
        showPanel();
        Hub.log("접근성 권한 켜짐 · 터치 준비됨", 'g');
        Hub.changed();
    }

    @Override
    public void onAccessibilityEvent(AccessibilityEvent event) {}

    @Override
    public void onInterrupt() {}

    @Override
    public boolean onUnbind(Intent intent) {
        cleanup();
        return super.onUnbind(intent);
    }

    @Override
    public void onDestroy() {
        cleanup();
        super.onDestroy();
    }

    void cleanup() {
        if (instance == this) instance = null;
        Hub.listeners.remove(refresh);
        Hub.stop();
        removeView(picker);
        picker = null;
        removeView(panel);
        panel = null;
        Hub.changed();
    }

    void removeView(View v) {
        if (v != null && wm != null) {
            try {
                wm.removeView(v);
            } catch (RuntimeException ignored) {
            }
        }
    }

    int[] screen() {
        DisplayManager dm = getSystemService(DisplayManager.class);
        DisplayMetrics m = new DisplayMetrics();
        dm.getDisplay(Display.DEFAULT_DISPLAY).getRealMetrics(m);
        return new int[]{m.widthPixels, m.heightPixels};
    }

    // ---------------------------------------------------------------- 터치
    /** (x, y) 를 ms 동안 누름 (화면 좌표 · 지금 화면 방향 기준) — 기다리지 않음 */
    void tap(int x, int y, int ms) {
        if (x < 0 || y < 0) return;
        letThrough(x, y);
        Path p = new Path();
        p.moveTo(x, y);
        dispatchGesture(new GestureDescription.Builder().addStroke(new GestureDescription.StrokeDescription(p, 0, Math.max(1, ms))).build(), null, null);
    }

    /** 한 손가락으로 (x0, y0) → (x1, y1) 끌기 (화면 돌리기) */
    void swipe(int x0, int y0, int x1, int y1, int ms) {
        letThrough(x0, y0);
        Path p = new Path();
        p.moveTo(x0, y0);
        p.lineTo(x1, y1);
        dispatchGesture(new GestureDescription.Builder().addStroke(new GestureDescription.StrokeDescription(p, 0, Math.max(1, ms))).build(), null, null);
    }

    /**
     * 두 손가락 줌: (cx, cy) 위아래 d0/2 에서 d1/2 로 (d1 < d0 = 오므리기 = 줌 아웃)
     *  · 위아래로 놓음: 두 손가락이 조금 어긋나도 가로로는 안 움직여서 화면이 좌우로 안 돌아감 (세로로 기우는 건 뒤의 내려다보기가 다시 맞춤)
     *  · 두 손가락을 먼저 0.12초 가만히 대고 있다가 (로블록스가 '두 손가락' 으로 알아보게) 똑같이 오므림 — 한 손가락만 먼저 닿으면 화면 돌리기로 봄
     */
    void pinch(int cx, int cy, int d0, int d1, int ms) {
        letThrough(cx, cy);
        float ya = cy - d0 / 2f, yb = cy + d0 / 2f;
        GestureDescription.StrokeDescription a0 = new GestureDescription.StrokeDescription(line(cx, ya, cx, ya + 1), 0, 120, true);
        GestureDescription.StrokeDescription b0 = new GestureDescription.StrokeDescription(line(cx, yb, cx, yb - 1), 0, 120, true);
        GestureDescription.Builder g0 = new GestureDescription.Builder();
        g0.addStroke(a0);
        g0.addStroke(b0);
        dispatchGesture(g0.build(), new GestureResultCallback() {
            @Override
            public void onCompleted(GestureDescription gd) {
                GestureDescription.Builder g1 = new GestureDescription.Builder();
                g1.addStroke(a0.continueStroke(line(cx, ya + 1, cx, cy - d1 / 2f), 0, Math.max(1, ms), false));
                g1.addStroke(b0.continueStroke(line(cx, yb - 1, cx, cy + d1 / 2f), 0, Math.max(1, ms), false));
                dispatchGesture(g1.build(), null, null);
            }
        }, null);
    }

    static Path line(float x0, float y0, float x1, float y1) {
        Path p = new Path();
        p.moveTo(x0, y0);
        p.lineTo(x1, y1);
        return p;
    }

    // ---- 매크로의 터치 · 클릭이 Acrux 창에 걸리지 않게
    // 터치 · 마우스는 화면 맨 위 창이 받음 → Acrux 창이 Fish 버튼 위에 있으면 [보정] 이 누른 Fish 가 Acrux 창의 [시작] 으로 들어갔음
    volatile int[] rect;
    volatile long passUntil;
    volatile boolean passing;

    void updateRect() {
        View p = panel;
        if (p == null || p.getVisibility() != View.VISIBLE || !p.isAttachedToWindow()) {
            rect = null;
            return;
        }
        int[] at = new int[2];
        p.getLocationOnScreen(at);
        rect = new int[]{at[0], at[1], p.getWidth(), p.getHeight()};
    }

    /** 매크로가 (x, y) 를 누르기 전에: Acrux 창 위면 잠깐(마지막 입력 뒤 0.8초) 창이 터치를 안 받게 → 그대로 아래 게임으로 */
    void letThrough(int x, int y) {
        int[] r = rect;
        if (r == null || x < r[0] || y < r[1] || x >= r[0] + r[2] || y >= r[1] + r[3]) return;
        passUntil = SystemClock.uptimeMillis() + 800;
        if (passing) return;
        passing = true;
        if (android.os.Looper.myLooper() == android.os.Looper.getMainLooper()) {
            setTouchable(false);
            restoreLater();
            return;
        }
        CountDownLatch done = new CountDownLatch(1);
        Hub.main.post(() -> {
            setTouchable(false);
            restoreLater();
            done.countDown();
        });
        try {
            done.await(300, TimeUnit.MILLISECONDS);
        } catch (InterruptedException ignored) {
            Thread.currentThread().interrupt();
        }
        SystemClock.sleep(40);                       // 창 설정이 입력에 반영될 시간
    }

    void setTouchable(boolean on) {
        if (panel == null || lp == null) return;
        if (on) lp.flags &= ~WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE;
        else lp.flags |= WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE;
        try {
            wm.updateViewLayout(panel, lp);
        } catch (RuntimeException ignored) {
        }
        if (on) passing = false;
    }

    void restoreLater() {
        Hub.main.postDelayed(() -> {
            if (SystemClock.uptimeMillis() >= passUntil) setTouchable(true);
            else restoreLater();
        }, 200);
    }

    // ---------------------------------------------------------------- 떠 있는 조작 창
    /** Acrux 창이 낚시 창 쪽 (화면 가운데 아래) 을 가리나 — 화면 캡처에 같이 찍혀서 인식이 틀릴 수 있음 */
    boolean coversGame() {
        int[] r = rect, s = screen();
        if (r == null) return false;
        int gx1 = s[0] / 4, gx2 = s[0] * 3 / 4, gy1 = s[1] * 2 / 5, gy2 = s[1];
        int ix = Math.min(r[0] + r[2], gx2) - Math.max(r[0], gx1), iy = Math.min(r[1] + r[3], gy2) - Math.max(r[1], gy1);
        return ix > r[2] / 4 && iy > r[3] / 4;
    }

    void showPanel() {
        if (panel != null) return;
        wm = (WindowManager) getSystemService(WINDOW_SERVICE);
        panel = new DragPanel(this);
        panel.setOrientation(LinearLayout.VERTICAL);
        int pad = Ui.dp(this, 10);
        panel.setPadding(pad, pad, pad, pad);
        // 얇은 테두리 + 네 모서리 ㄱ (앱의 카드와 같음)
        panel.setBackground(new Ui.Deco(this, 0xF22B2D31, Ui.LINE, Ui.WHITE, true, 10, 2));

        // 제목 줄
        LinearLayout head = new LinearLayout(this);
        head.setOrientation(LinearLayout.HORIZONTAL);
        head.setGravity(Gravity.CENTER_VERTICAL);
        TextView title = Ui.text(this, "ACRUX MACRO", 12, Ui.FG, true);
        title.setLetterSpacing(0.12f);
        head.addView(title, Ui.wrap());
        body = new LinearLayout(this);
        body.setOrientation(LinearLayout.VERTICAL);
        status = Ui.text(this, "대기", 11, Ui.MUTED, false);
        status.setMaxWidth(Ui.dp(this, 240));            // 긴 이유는 줄바꿈
        status.setMaxWidth(Ui.dp(this, 170));
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        calBtn = small("보정", Ui.YELLOW, v -> toggle("cal"));
        runBtn = small("시작", Ui.ACCENT, v -> toggle("run"));
        gearBtn = small("⚙", Ui.MUTED, v -> toggleMenu());
        row.addView(calBtn, Ui.wrap());
        row.addView(runBtn, gap());
        row.addView(gearBtn, gap());
        arriveBtn = small("도착", Ui.GREEN, v -> {
            Hub.arrived = true;
            Hub.log("도착", 'g');
        });
        arriveBtn.setVisibility(View.GONE);
        panel.addView(head, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        LinearLayout.LayoutParams sp = Ui.wrap();
        sp.topMargin = Ui.dp(this, 2);
        body.addView(status, sp);
        LinearLayout.LayoutParams rp = Ui.wrap();
        rp.topMargin = Ui.dp(this, 8);
        body.addView(row, rp);
        LinearLayout.LayoutParams ap = Ui.wrap();
        ap.topMargin = Ui.dp(this, 6);
        body.addView(arriveBtn, ap);
        panel.addView(body, Ui.wrap());

        lp = new WindowManager.LayoutParams(
                WindowManager.LayoutParams.WRAP_CONTENT, WindowManager.LayoutParams.WRAP_CONTENT,
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
                PixelFormat.TRANSLUCENT);
        lp.gravity = Gravity.TOP | Gravity.START;
        lp.x = Hub.prefs().getInt("panelX", Ui.dp(this, 12));
        lp.y = Hub.prefs().getInt("panelY", Ui.dp(this, 12));
        applyAlphaTo();
        panel.getViewTreeObserver().addOnGlobalLayoutListener(this::updateRect);
        try {
            wm.addView(panel, lp);
        } catch (RuntimeException e) {
            Hub.log("조작 창을 띄우지 못함: " + e, 'r');
            panel = null;
        }
        updateVisible();
        refresh();
    }

    /** 어디를 잡고 끌어도 창이 움직임 (짧게 누르면 그대로 버튼 · ⚙ 메뉴 안에서 위아래로 밀면 메뉴 스크롤) */
    final class DragPanel extends LinearLayout {
        final int slop = android.view.ViewConfiguration.get(getContext()).getScaledTouchSlop();
        float sx, sy;
        int ox, oy;
        boolean dragging, inMenu;

        DragPanel(Context c) {
            super(c);
        }

        void begin(MotionEvent e) {
            sx = e.getRawX();
            sy = e.getRawY();
            ox = lp.x;
            oy = lp.y;
            dragging = false;
            View m = menu != null ? (View) menu.getTag() : null;
            inMenu = false;
            if (m != null) {
                int[] at = new int[2];
                m.getLocationOnScreen(at);
                inMenu = sx >= at[0] && sx <= at[0] + m.getWidth() && sy >= at[1] && sy <= at[1] + m.getHeight();
            }
        }

        boolean far(MotionEvent e) {
            float dx = e.getRawX() - sx, dy = e.getRawY() - sy;
            if (Math.hypot(dx, dy) <= slop) return false;
            return !inMenu || Math.abs(dx) > Math.abs(dy) * 1.5f;
        }

        void move(MotionEvent e) {
            lp.x = ox + Math.round(e.getRawX() - sx);
            lp.y = oy + Math.round(e.getRawY() - sy);
            try {
                wm.updateViewLayout(this, lp);
            } catch (RuntimeException ignored) {
            }
            post(TapService.this::updateRect);
        }

        @Override
        public boolean onInterceptTouchEvent(MotionEvent e) {
            switch (e.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                    begin(e);
                    return false;
                case MotionEvent.ACTION_MOVE:
                    if (!dragging && far(e)) dragging = true;
                    return dragging;
                default:
                    return dragging;
            }
        }

        @Override
        public boolean onTouchEvent(MotionEvent e) {
            switch (e.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                    begin(e);
                    return true;
                case MotionEvent.ACTION_MOVE:
                    if (!dragging && far(e)) dragging = true;
                    if (dragging) move(e);
                    return true;
                case MotionEvent.ACTION_UP:
                case MotionEvent.ACTION_CANCEL:
                    if (dragging) Hub.prefs().edit().putInt("panelX", lp.x).putInt("panelY", lp.y).apply();
                    dragging = false;
                    return true;
                default:
                    return true;
            }
        }
    }

    /** 보이기 · 숨기기: 앱의 [UI 끄기] 로 꺼져 있거나, Acrux 앱을 보고 있고 '앱을 볼 때 숨기기' 가 켜져 있으면 숨김 */
    void updateVisible() {
        Hub.main.post(() -> {
            if (panel == null) return;
            boolean on = Hub.prefs().getBoolean("panelOn", true) && !(Hub.appVisible && Hub.prefs().getBoolean("hideOnApp", true));
            panel.setVisibility(on ? View.VISIBLE : View.GONE);
            if (!on) closeMenu();
            panel.post(TapService.this::updateRect);
        });
    }

    /** 언어를 바꿨을 때: 창을 지금 언어로 다시 만듦 (자리는 그대로) */
    void rebuildPanel() {
        Hub.main.post(() -> {
            if (panel == null) return;
            closeMenu();
            removeView(panel);
            panel = null;
            showPanel();
        });
    }

    void resetPanelPos() {
        Hub.prefs().edit().putInt("panelX", Ui.dp(this, 12)).putInt("panelY", Ui.dp(this, 12)).apply();
        Hub.main.post(() -> {
            if (panel == null || lp == null) return;
            lp.x = Ui.dp(this, 12);
            lp.y = Ui.dp(this, 12);
            try {
                wm.updateViewLayout(panel, lp);
            } catch (RuntimeException ignored) {
            }
        });
    }

    void applyAlpha() {
        Hub.main.post(this::applyAlphaTo);
    }

    void applyAlphaTo() {
        if (panel != null) panel.setAlpha(Hub.prefs().getInt("panelAlpha", 90) / 100f);
    }

    LinearLayout.LayoutParams gap() {
        LinearLayout.LayoutParams g = Ui.wrap();
        g.leftMargin = Ui.dp(this, 6);
        return g;
    }

    TextView small(String s, int color, View.OnClickListener on) {
        TextView b = Ui.button(this, s, color, on);
        b.setTextSize(13);
        b.setPadding(Ui.dp(this, 12), Ui.dp(this, 6), Ui.dp(this, 12), Ui.dp(this, 6));
        return b;
    }

    TextView item(String s, View.OnClickListener on) {
        TextView t = Ui.text(this, s, 12, Ui.FG, false);
        t.setPadding(Ui.dp(this, 6), Ui.dp(this, 7), Ui.dp(this, 6), Ui.dp(this, 7));
        if (on != null) t.setOnClickListener(on);
        else t.setTextColor(Ui.MUTED);
        return t;
    }

    void toggle(String m) {
        if (Hub.running()) {
            Hub.stop();
            return;
        }
        String err = Hub.start(m);
        if (!err.isEmpty()) {
            // 못 하는 이유: 기록 칸만이 아니라 떠 있는 창 · 화면 알림에도 (게임 화면에선 기록 칸이 안 보임)
            Hub.log(err, 'y');
            Hub.state = err;
            Hub.changed();
            android.widget.Toast.makeText(this, com.acrux.mobile.core.I18n.t(err), android.widget.Toast.LENGTH_LONG).show();
        }
        closeMenu();
        refresh();
    }

    // ---- ⚙ 메뉴 (PC 버전 이동 · 판매 탭과 같은 순서)
    void toggleMenu() {
        if (menu != null) {
            closeMenu();
            return;
        }
        menu = new LinearLayout(this);
        menu.setOrientation(LinearLayout.VERTICAL);
        menu.addView(head("조이스틱 (W · A · S · D)"));
        menu.addView(item(Hub.stick != null ? "✓ 조이스틱 저장됨 (눌러서 위치 보기)" : "· 조이스틱 없음 — 로블록스 설정 → 이동 방식: 조이스틱", Hub.stick != null ? v -> {
            closeMenu();
            Hub.showStick(2500);
        } : null));
        menu.addView(item("조이스틱 찾기 (화면에서 자동으로)", v -> toggle("stickfind")));
        menu.addView(item("조이스틱 직접 지정 (가운데 → 가장자리)", v -> pickStick()));
        menu.addView(item("조이스틱 테스트 (W · A · S · D 0.8초씩)", v -> toggle("sticktest")));
        for (String[] k : Mover.JUMP_KEYS) {
            menu.addView(item((Hub.pos.containsKey(k[0]) ? "✓ " : "· ") + k[1] + " — 길 기록에서 점프", v -> pickSave(k[0], k[1])));
        }
        menu.addView(item("점프 테스트 (제자리 → 걸으면서)", v -> toggle("jumptest")));
        menu.addView(head("공통 위치 (기준 장소 맞추기)"));
        for (String[] k : Mover.BASE_KEYS) {
            menu.addView(item((Hub.pos.containsKey(k[0]) ? "✓ " : "· ") + k[1], v -> pickSave(k[0], k[1])));
        }
        place(false);
        place(true);
        menu.addView(head("판매"));
        for (String[] k : Seller.TALK) {
            menu.addView(item((Hub.pos.containsKey(k[0]) ? "✓ " : "· ") + k[1] + " (판매 장소에 서서)", v -> pickSave(k[0], k[1])));
        }
        menu.addView(item("판매 자동 보정 (NPC 앞에서 시작 → 대화 버튼은 직접)", v -> toggle("sellcal")));
        int set = 0;
        for (String[] k : Seller.KEYS) if (Hub.pos.containsKey(k[0])) set++;
        menu.addView(item("  판매 위치 " + set + " / " + Seller.KEYS.length + " 저장됨", null));
        for (String[] k : new String[][]{{"first_fish_pos", "첫 번째 물고기 칸"}, {"shop_close_pos", "상점 닫기 X"}}) {
            menu.addView(item((Hub.pos.containsKey(k[0]) ? "✓ " : "· ") + k[1] + " 직접 지정 (상점을 연 채로)", v -> pickSave(k[0], k[1])));
        }
        menu.addView(item("판매 한 번 해 보기", v -> toggle("sell")));
        menu.addView(head("이동 테스트"));
        menu.addView(item("기준 장소로", v -> toggle("base")));
        menu.addView(item("기준 장소 → 낚시 장소", v -> toggle("fish")));
        menu.addView(item("기준 장소 → 판매 장소", v -> toggle("sellgo")));
        ScrollView sv = new ScrollView(this);
        sv.addView(menu);
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(Ui.dp(this, 240), Math.min(Ui.dp(this, 300), screen()[1] * 2 / 3));
        p.topMargin = Ui.dp(this, 6);
        panel.addView(sv, p);
        menu.setTag(sv);
    }

    /** 길 기록: 기준 장소로 간 뒤 '지금부터 기록' → 조이스틱으로 직접 걸어가서 [도착] */
    void place(boolean sell) {
        com.acrux.mobile.core.Route r = sell ? Hub.sellRoute : Hub.fishRoute;
        String name = sell ? "물고기 판매 장소" : "낚시 장소";
        menu.addView(head(name + " 길"));
        menu.addView(item(r.isEmpty() ? "· 기록 없음" : "✓ 기록됨 · " + fmt(r.seconds()) + "초 · " + r.segs.size() + "조각 · 점프 " + r.jumps() + "번", null));
        menu.addView(item((r.isEmpty() ? "● 길 기록" : "● 다시 기록") + " (기준 장소로 간 뒤 WASD 패드로 걸어감 → [도착])", v -> toggle(sell ? "rec_sell" : "rec_fish")));
    }

    TextView head(String s) {
        TextView t = Ui.text(this, s, 11, Ui.MUTED, true);
        t.setPadding(Ui.dp(this, 2), Ui.dp(this, 8), 0, Ui.dp(this, 2));
        return t;
    }

    void closeMenu() {
        if (menu == null) return;
        panel.removeView((View) menu.getTag());
        menu = null;
    }

    void pickSave(String key, String label) {
        pick(label, at -> {
            if (at == null) return;
            Hub.pos.put(key, at);
            Hub.save();
            Hub.log("'" + label + "' 위치 저장 (" + at[0] + ", " + at[1] + ")", 'g');
        });
    }

    /** 조이스틱 직접 지정: 가운데를 누르고 → 바깥 판 가장자리를 누름 */
    void pickStick() {
        pick("조이스틱 가운데", c -> {
            if (c == null) return;
            Hub.main.post(() -> pick("조이스틱 바깥 판 가장자리", e -> {
                if (e == null) return;
                int[] s = screen();
                double r = Math.hypot((e[0] - c[0]) * s[0], (e[1] - c[1]) * s[1]);
                if (r < 5) {
                    Hub.log("가운데와 가장자리가 너무 가까움 — 다시", 'y');
                    return;
                }
                Hub.stick = new double[]{c[0], c[1], r / s[1]};
                Hub.save();
                Hub.log("조이스틱 저장: 가운데 (" + Math.round(c[0] * s[0]) + ", " + Math.round(c[1] * s[1]) + ") · 반지름 " + Math.round(r) + "px", 'g');
                showRing(Hub.stick, 2500);
            }));
        });
    }

    /** (x, y) 에 작은 초록 점을 ms 동안 (점프 탭 자리 보여 주기) */
    void showDot(int x, int y, long ms) {
        int[] s = screen();
        double r = Math.max(10, s[1] * 0.012);
        showRing(new double[]{(double) x / s[0], (double) y / s[1], r / s[1]}, ms);
    }

    /** 조이스틱 자리에 초록 원을 ms 동안 그려 줌 (터치는 그대로 아래로) */
    void showRing(double[] st, long ms) {
        int[] s = screen();
        float cx = (float) (st[0] * s[0]), cy = (float) (st[1] * s[1]), r = (float) (st[2] * s[1]);
        View v = new View(this) {
            final android.graphics.Paint pt = new android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG);

            @Override
            protected void onDraw(android.graphics.Canvas c) {
                pt.setStyle(android.graphics.Paint.Style.STROKE);
                pt.setStrokeWidth(Math.max(3, r / 20f));
                pt.setColor(0xFF3BA55D);
                c.drawCircle(cx, cy, r, pt);
                c.drawLine(cx - r * 0.2f, cy, cx + r * 0.2f, cy, pt);
                c.drawLine(cx, cy - r * 0.2f, cx, cy + r * 0.2f, pt);
            }
        };
        WindowManager.LayoutParams p = new WindowManager.LayoutParams(
                WindowManager.LayoutParams.MATCH_PARENT, WindowManager.LayoutParams.MATCH_PARENT,
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE
                        | WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
                PixelFormat.TRANSLUCENT);
        p.gravity = Gravity.TOP | Gravity.START;
        if (android.os.Build.VERSION.SDK_INT >= 28) {
            p.layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
        }
        try {
            wm.addView(v, p);
            Hub.main.postDelayed(() -> removeView(v), ms);
        } catch (RuntimeException e) {
            Hub.log("원을 못 그림: " + e, 'r');
        }
    }

    /** 위치 지정: 로블록스 화면 위에 투명한 막을 띄우고 누른 곳을 넘김 (화면 비율) · 취소하면 null */
    void pick(String name, java.util.function.Consumer<double[]> done) {
        closeMenu();
        if (picker != null) {
            done.accept(null);
            return;
        }
        FrameLayout f = new FrameLayout(this);
        f.setBackgroundColor(0x33000000);
        TextView banner = Ui.text(this, "'" + name + "' 위치를 누르세요   ·   취소: 오른쪽 위 ✕", 14, Ui.WHITE, true);
        banner.setBackgroundColor(0xCC1E1F22);
        banner.setPadding(Ui.dp(this, 14), Ui.dp(this, 8), Ui.dp(this, 14), Ui.dp(this, 8));
        FrameLayout.LayoutParams bp = new FrameLayout.LayoutParams(FrameLayout.LayoutParams.WRAP_CONTENT, FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.TOP | Gravity.CENTER_HORIZONTAL);
        bp.topMargin = Ui.dp(this, 10);
        f.addView(banner, bp);
        TextView x = Ui.text(this, "✕", 20, Ui.WHITE, true);
        x.setPadding(Ui.dp(this, 16), Ui.dp(this, 8), Ui.dp(this, 16), Ui.dp(this, 8));
        x.setOnClickListener(v -> {
            removeView(picker);
            picker = null;
            done.accept(null);
        });
        f.addView(x, new FrameLayout.LayoutParams(FrameLayout.LayoutParams.WRAP_CONTENT, FrameLayout.LayoutParams.WRAP_CONTENT, Gravity.TOP | Gravity.END));
        f.setOnTouchListener((v, e) -> {
            if (e.getActionMasked() != MotionEvent.ACTION_UP) return true;
            int[] s = screen();
            double[] at = {Math.round(e.getRawX() / s[0] * 10000) / 10000.0, Math.round(e.getRawY() / s[1] * 10000) / 10000.0};
            removeView(picker);
            picker = null;
            done.accept(at);
            return true;
        });
        WindowManager.LayoutParams p = new WindowManager.LayoutParams(
                WindowManager.LayoutParams.MATCH_PARENT, WindowManager.LayoutParams.MATCH_PARENT,
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN
                        | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
                PixelFormat.TRANSLUCENT);
        p.gravity = Gravity.TOP | Gravity.START;
        if (android.os.Build.VERSION.SDK_INT >= 28) {
            p.layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
        }
        try {
            wm.addView(f, p);
            picker = f;
        } catch (RuntimeException e) {
            Hub.log("위치 지정 창을 띄우지 못함: " + e, 'r');
            done.accept(null);
        }
    }

    void refresh() {
        if (panel == null) return;
        boolean run = Hub.running();
        String mode = Hub.mode;
        Ui.set(status, CaptureService.active == null ? "화면 캡처 꺼짐 · 앱에서 시작" : Hub.state);
        Ui.set(calBtn, run && "cal".equals(mode) ? "취소" : "보정");
        Ui.set(runBtn, run && !"cal".equals(mode) ? "정지" : "시작");
        calBtn.setEnabled(!run || "cal".equals(mode));
        runBtn.setEnabled(!run || !"cal".equals(mode));
        calBtn.setAlpha(calBtn.isEnabled() ? 1f : 0.4f);
        arriveBtn.setVisibility(Hub.measuring ? View.VISIBLE : View.GONE);
        runBtn.setAlpha(runBtn.isEnabled() ? 1f : 0.4f);
    }

    static String fmt(double v) {
        return String.format(Locale.ROOT, "%.2f", v);
    }
}
