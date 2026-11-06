package com.acrux.mobile;

import android.accessibilityservice.AccessibilityService;
import android.accessibilityservice.GestureDescription;
import android.content.Intent;
import android.graphics.Path;
import android.graphics.PixelFormat;
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

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/**
 * 접근성 서비스 — 자동 낚시 · 이동의 터치를 대신 보냄 (dispatchGesture) + 로블록스 위에 떠 있는 조작 창
 * 화면 내용은 읽지 않음 (canRetrieveWindowContent = false · 화면 색은 화면 캡처로 봄)
 */
public class TapService extends AccessibilityService {
    static volatile TapService instance;

    WindowManager wm;
    LinearLayout panel, menu;
    WindowManager.LayoutParams lp;
    TextView status, calBtn, runBtn, gearBtn, arriveBtn;
    View picker;
    final Runnable refresh = this::refresh;
    // 지점 추가 (시간 재기)
    long measureAt;
    String measureName;
    boolean measureSell;

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
        Path p = new Path();
        p.moveTo(x, y);
        dispatchGesture(new GestureDescription.Builder().addStroke(new GestureDescription.StrokeDescription(p, 0, Math.max(1, ms))).build(), null, null);
    }

    /** 몸짓 하나를 보내고 끝날 때까지 기다림 */
    boolean send(GestureDescription g, long ms) {
        CountDownLatch done = new CountDownLatch(1);
        boolean ok = dispatchGesture(g, new GestureResultCallback() {
            @Override
            public void onCompleted(GestureDescription d) {
                done.countDown();
            }

            @Override
            public void onCancelled(GestureDescription d) {
                done.countDown();
            }
        }, null);
        if (!ok) return false;
        try {
            return done.await(ms + 1500, TimeUnit.MILLISECONDS);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return false;
        }
    }

    static float cl(float v, float hi) {
        return Math.max(1, Math.min(hi - 2, v));
    }

    void swipe(int x1, int y1, int x2, int y2, int ms) {
        int[] s = screen();
        Path p = new Path();
        p.moveTo(cl(x1, s[0]), cl(y1, s[1]));
        p.lineTo(cl(x2, s[0]), cl(y2, s[1]));
        send(new GestureDescription.Builder().addStroke(new GestureDescription.StrokeDescription(p, 0, Math.max(50, ms))).build(), ms);
    }

    /** 두 손가락: zoomOut = 모으기 (멀리 → 가까이) · 아니면 벌리기 */
    void pinch(int cx, int cy, int far, int near, boolean zoomOut, int ms) {
        int[] s = screen();
        int a = zoomOut ? far : near, b = zoomOut ? near : far;
        Path l = new Path(), r = new Path();
        l.moveTo(cl(cx - a, s[0]), cy);
        l.lineTo(cl(cx - b, s[0]), cy);
        r.moveTo(cl(cx + a, s[0]), cy);
        r.lineTo(cl(cx + b, s[0]), cy);
        send(new GestureDescription.Builder()
                .addStroke(new GestureDescription.StrokeDescription(l, 0, Math.max(50, ms)))
                .addStroke(new GestureDescription.StrokeDescription(r, 0, Math.max(50, ms))).build(), ms);
    }

    /** 조이스틱: (cx, cy) 에 손가락을 대고 방향마다 반지름 r 만큼 밀어 둔 채로 그 시간 동안 · 손가락은 맨 끝에만 뗌 */
    void stick(int cx, int cy, int r, double[][] segs, Runnable check) {
        List<float[]> parts = new ArrayList<>();          // {x, y, ms, 끌기(1) / 머물기(0)}
        float px = cx, py = cy;
        parts.add(new float[]{px, py, 60, 0});            // 누르기
        for (double[] sg : segs) {
            double a = Math.toRadians(sg[0]);
            float tx = (float) (cx + r * Math.sin(a)), ty = (float) (cy - r * Math.cos(a));
            parts.add(new float[]{tx, ty, 80, 1});
            long left = Math.max(0, (long) (sg[1] * 1000) - 80);
            while (left > 0) {                             // 0.4초씩 (그 사이에 멈춤 확인)
                long ms = Math.min(400, left);
                parts.add(new float[]{tx, ty, ms, 0});
                left -= ms;
            }
            px = tx;
            py = ty;
        }
        GestureDescription.StrokeDescription prev = null;
        float lx = cx, ly = cy;
        try {
            for (int i = 0; i < parts.size(); i++) {
                float[] q = parts.get(i);
                boolean last = i == parts.size() - 1;
                Path p = new Path();
                p.moveTo(lx, ly);
                if (q[3] == 1) p.lineTo(q[0], q[1]);
                long ms = Math.max(1, (long) q[2]);
                prev = prev == null ? new GestureDescription.StrokeDescription(p, 0, ms, !last) : prev.continueStroke(p, 0, ms, !last);
                if (!send(new GestureDescription.Builder().addStroke(prev).build(), ms)) {
                    Hub.log("조이스틱 몸짓이 끊김", 'd');
                    return;
                }
                lx = q[0];
                ly = q[1];
                if (!last) check.run();
            }
        } catch (RuntimeException e) {
            // 멈춤: 누른 손가락을 떼고 멈춤을 이어서 알림
            if (prev != null && prev.willContinue()) {
                Path p = new Path();
                p.moveTo(lx, ly);
                send(new GestureDescription.Builder().addStroke(prev.continueStroke(p, 0, 1, false)).build(), 1);
            }
            throw e;
        }
    }

    /** 코어(이동 · 판매)가 쓰는 터치 — 접근성 서비스가 켜져 있을 때만 */
    static final Mover.Gestures GESTURES = new Mover.Gestures() {
        @Override
        public void tap(int x, int y, int ms) {
            TapService t = instance;
            if (t == null) return;
            if (ms > 200) {
                Path p = new Path();
                p.moveTo(x, y);
                t.send(new GestureDescription.Builder().addStroke(new GestureDescription.StrokeDescription(p, 0, ms)).build(), ms);
            } else {
                t.tap(x, y, ms);
            }
        }

        @Override
        public void swipe(int x1, int y1, int x2, int y2, int ms) {
            TapService t = instance;
            if (t != null) t.swipe(x1, y1, x2, y2, ms);
        }

        @Override
        public void pinch(int cx, int cy, int far, int near, boolean zoomOut, int ms) {
            TapService t = instance;
            if (t != null) t.pinch(cx, cy, far, near, zoomOut, ms);
        }

        @Override
        public void stick(int cx, int cy, int r, double[][] segs, Runnable check) {
            TapService t = instance;
            if (t != null) t.stick(cx, cy, r, segs, check);
        }
    };

    // ---------------------------------------------------------------- 떠 있는 조작 창
    void showPanel() {
        if (panel != null) return;
        wm = (WindowManager) getSystemService(WINDOW_SERVICE);
        panel = new LinearLayout(this);
        panel.setOrientation(LinearLayout.VERTICAL);
        int pad = Ui.dp(this, 8);
        panel.setPadding(pad, pad, pad, pad);
        panel.setBackground(new Ui.Deco(this, 0xE62B2D31, Ui.LINE, Ui.WHITE, false));

        TextView title = Ui.text(this, "ACRUX  ✥", 12, Ui.FG, true);
        title.setPadding(0, 0, 0, Ui.dp(this, 2));
        status = Ui.text(this, "대기", 11, Ui.MUTED, false);
        status.setMaxWidth(Ui.dp(this, 170));
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        calBtn = small("보정", Ui.YELLOW, v -> toggle("cal"));
        runBtn = small("시작", Ui.ACCENT, v -> toggle("run"));
        gearBtn = small("⚙", Ui.MUTED, v -> toggleMenu());
        row.addView(calBtn, Ui.wrap());
        row.addView(runBtn, gap());
        row.addView(gearBtn, gap());
        arriveBtn = small("도착", Ui.GREEN, v -> arrived());
        arriveBtn.setVisibility(View.GONE);
        panel.addView(title, Ui.wrap());
        panel.addView(status, Ui.wrap());
        LinearLayout.LayoutParams rp = Ui.wrap();
        rp.topMargin = Ui.dp(this, 6);
        panel.addView(row, rp);
        LinearLayout.LayoutParams ap = Ui.wrap();
        ap.topMargin = Ui.dp(this, 6);
        panel.addView(arriveBtn, ap);

        lp = new WindowManager.LayoutParams(
                WindowManager.LayoutParams.WRAP_CONTENT, WindowManager.LayoutParams.WRAP_CONTENT,
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
                PixelFormat.TRANSLUCENT);
        lp.gravity = Gravity.TOP | Gravity.START;
        lp.x = Hub.prefs().getInt("panelX", Ui.dp(this, 12));
        lp.y = Hub.prefs().getInt("panelY", Ui.dp(this, 12));
        title.setOnTouchListener(new View.OnTouchListener() {
            float sx, sy;
            int ox, oy;

            @Override
            public boolean onTouch(View v, MotionEvent e) {
                switch (e.getActionMasked()) {
                    case MotionEvent.ACTION_DOWN:
                        sx = e.getRawX();
                        sy = e.getRawY();
                        ox = lp.x;
                        oy = lp.y;
                        return true;
                    case MotionEvent.ACTION_MOVE:
                        lp.x = ox + Math.round(e.getRawX() - sx);
                        lp.y = oy + Math.round(e.getRawY() - sy);
                        wm.updateViewLayout(panel, lp);
                        return true;
                    case MotionEvent.ACTION_UP:
                        Hub.prefs().edit().putInt("panelX", lp.x).putInt("panelY", lp.y).apply();
                        return true;
                    default:
                        return false;
                }
            }
        });
        try {
            wm.addView(panel, lp);
        } catch (RuntimeException e) {
            Hub.log("조작 창을 띄우지 못함: " + e, 'r');
            panel = null;
        }
        refresh();
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
        t.setOnClickListener(on);
        return t;
    }

    void toggle(String m) {
        if (Hub.running()) {
            Hub.stop();
            return;
        }
        String err = Hub.start(m);
        if (!err.isEmpty()) Hub.log(err, 'y');
        closeMenu();
        refresh();
    }

    // ---- ⚙ 메뉴: 위치 지정 · 지점 추가 · 판매 보정 · 테스트
    void toggleMenu() {
        if (menu != null) {
            closeMenu();
            return;
        }
        menu = new LinearLayout(this);
        menu.setOrientation(LinearLayout.VERTICAL);
        menu.addView(head("위치 지정"));
        for (String n : Hub.positionNames()) {
            boolean set = Hub.pos.containsKey(n);
            menu.addView(item((set ? "✓ " : "· ") + n, v -> pick(n, null)));
        }
        menu.addView(head("지점 추가 (터치이동 · 시간 재기)"));
        menu.addView(item("+ 낚시 장소 경로에", v -> addPoint(false)));
        menu.addView(item("+ 판매 장소 경로에", v -> addPoint(true)));
        menu.addView(head("판매"));
        menu.addView(item("판매 보정 (NPC 앞에서 누른 뒤 상호작용)", v -> toggle("sellcal")));
        menu.addView(item("판매 한 번 해 보기", v -> toggle("sell")));
        menu.addView(head("이동 테스트"));
        menu.addView(item("기준 장소 → 낚시 장소", v -> toggle("test_fish")));
        menu.addView(item("기준 장소 → 판매 장소", v -> toggle("test_sell")));
        ScrollView sv = new ScrollView(this);
        sv.addView(menu);
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(Ui.dp(this, 230), Math.min(Ui.dp(this, 260), screen()[1] / 2));
        p.topMargin = Ui.dp(this, 6);
        panel.addView(sv, p);
        menu.setTag(sv);
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

    /** 위치 지정: 로블록스 화면 위에 투명한 막을 띄우고, 누른 곳을 그 이름의 위치로 (done 이 있으면 저장 대신 넘김) */
    void pick(String name, java.util.function.Consumer<double[]> done) {
        closeMenu();
        if (picker != null) return;
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
        });
        f.addView(x, new FrameLayout.LayoutParams(FrameLayout.LayoutParams.WRAP_CONTENT, FrameLayout.LayoutParams.WRAP_CONTENT, Gravity.TOP | Gravity.END));
        f.setOnTouchListener((v, e) -> {
            if (e.getActionMasked() != MotionEvent.ACTION_UP) return true;
            int[] s = screen();
            double[] at = {Math.round(e.getRawX() / s[0] * 10000) / 10000.0, Math.round(e.getRawY() / s[1] * 10000) / 10000.0};
            removeView(picker);
            picker = null;
            if (done != null) {
                done.accept(at);
            } else {
                Hub.pos.put(name, at);
                Hub.save();
                Hub.log("'" + name + "' 위치 저장 (" + at[0] + ", " + at[1] + ")", 'g');
            }
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
        }
    }

    /** 지점 추가: 갈 곳을 누름 → 그 자리를 로블록스에도 눌러 걸어가게 함 (Tap to Move) → 도착하면 [도착] → 경로에 '터치이동 이름 초' 추가 */
    void addPoint(boolean sell) {
        String prefix = sell ? "판매" : "낚시";
        int n = 1;
        while (Hub.pos.containsKey(prefix + n)) n++;
        String name = prefix + n;
        pick(name, at -> {
            Hub.pos.put(name, at);
            Hub.save();
            int[] s = screen();
            Hub.main.postDelayed(() -> {
                tap((int) (at[0] * s[0]), (int) (at[1] * s[1]), 40);
                measureAt = System.nanoTime();
                measureName = name;
                measureSell = sell;
                arriveBtn.setVisibility(View.VISIBLE);
                Hub.state = name + " 까지 걷는 중 — 도착하면 [도착]";
                Hub.changed();
            }, 150);
        });
    }

    void arrived() {
        if (measureName == null) return;
        double sec = Math.round((System.nanoTime() - measureAt) / 1e7) / 100.0;
        Hub.appendRoute(measureSell, "터치이동 " + measureName + " " + sec);
        Hub.log("지점 추가: " + (measureSell ? "판매" : "낚시") + " 장소 경로에 '터치이동 " + measureName + " " + sec + "'", 'g');
        measureName = null;
        arriveBtn.setVisibility(View.GONE);
        Hub.state = "대기";
        Hub.changed();
    }

    void refresh() {
        if (panel == null) return;
        boolean run = Hub.running();
        String mode = Hub.mode;
        status.setText(CaptureService.active == null ? "화면 캡처 꺼짐 · 앱에서 시작" : Hub.state);
        calBtn.setText(run && "cal".equals(mode) ? "취소" : "보정");
        runBtn.setText(run && !"cal".equals(mode) ? "정지" : "시작");
        calBtn.setEnabled(!run || "cal".equals(mode));
        runBtn.setEnabled(!run || !"cal".equals(mode));
        calBtn.setAlpha(calBtn.isEnabled() ? 1f : 0.4f);
        runBtn.setAlpha(runBtn.isEnabled() ? 1f : 0.4f);
    }

    static String fmt(double v) {
        return String.format(Locale.ROOT, "%.2f", v);
    }
}
