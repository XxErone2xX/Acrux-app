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
import android.view.KeyEvent;
import android.os.SystemClock;
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
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.Locale;

/**
 * 접근성 서비스 — 로블록스 위에 떠 있는 조작 창 + 터치 (자동 낚시 · 클릭 → 탭) + 접근성 입력기 (키보드 방식 키 · 안드로이드 13+)
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

    /** 몸짓 하나를 보내고 끝날 때까지 기다림 · 다른 몸짓에 끊기면 false */
    boolean send(GestureDescription g, long ms) {
        CountDownLatch done = new CountDownLatch(1);
        boolean[] ok = {false};
        boolean sent = dispatchGesture(g, new GestureResultCallback() {
            @Override
            public void onCompleted(GestureDescription d) {
                ok[0] = true;
                done.countDown();
            }

            @Override
            public void onCancelled(GestureDescription d) {
                done.countDown();
            }
        }, null);
        if (!sent) return false;
        try {
            return done.await(ms + 1500, TimeUnit.MILLISECONDS) && ok[0];
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return false;
        }
    }

    static float cl(float v, float hi) {
        return Math.max(1, Math.min(hi - 2, v));
    }

    // ---- 마우스 대신 터치 (KeyPc.Touch)
    /** 누르고 ms 뒤에 뗌 · 끝날 때까지 기다림 (일하는 스레드에서) */
    void tapWait(int x, int y, int ms) {
        int[] s = screen();
        Path p = new Path();
        p.moveTo(cl(x, s[0]), cl(y, s[1]));
        send(new GestureDescription.Builder().addStroke(new GestureDescription.StrokeDescription(p, 0, Math.max(1, ms))).build(), ms);
    }

    public void swipe(int x1, int y1, int x2, int y2, int ms) {
        int[] s = screen();
        Path p = new Path();
        p.moveTo(cl(x1, s[0]), cl(y1, s[1]));
        p.lineTo(cl(x2, s[0]), cl(y2, s[1]));
        send(new GestureDescription.Builder().addStroke(new GestureDescription.StrokeDescription(p, 0, Math.max(50, ms))).build(), ms);
    }

    /** 접근성 입력기 (안드로이드 13+) 로 키 보내기 — 로블록스에 입력 연결이 있을 때만 보내짐 */
    boolean imeKey(KeyEvent e) {
        if (android.os.Build.VERSION.SDK_INT < 33) return false;
        android.accessibilityservice.InputMethod im = getInputMethod();
        if (im == null) return false;
        android.accessibilityservice.InputMethod.AccessibilityInputConnection ic = im.getCurrentInputConnection();
        if (ic == null) return false;
        ic.sendKeyEvent(e);
        return true;
    }

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
        arriveBtn = small("도착", Ui.GREEN, v -> {
            Hub.arrived = true;
            Hub.log("도착", 'g');
        });
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
        if (!err.isEmpty()) Hub.log(err, 'y');
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
        menu.addView(head("키보드 방식"));
        menu.addView(item(Kb.status(), null));
        menu.addView(item("키 테스트 (W 를 1초 누름 → 캐릭터가 앞으로 가면 성공)", v -> toggle("keytest")));
        menu.addView(head("공통 위치 (기준 장소 맞추기)"));
        for (String[] k : Mover.BASE_KEYS) {
            menu.addView(item((Hub.pos.containsKey(k[0]) ? "✓ " : "· ") + k[1], v -> pickSave(k[0], k[1])));
        }
        place(false);
        place(true);
        menu.addView(head("판매"));
        menu.addView(item("판매 자동 보정 (NPC 앞에서 시작 → E 는 직접)", v -> toggle("sellcal")));
        int set = 0;
        for (String[] k : Seller.KEYS) if (Hub.pos.containsKey(k[0])) set++;
        menu.addView(item("  판매 위치 " + set + " / " + Seller.KEYS.length + " 저장됨", null));
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

    void place(boolean sell) {
        List<Mover.Point> pts = sell ? Hub.sellPts : Hub.fishPts;
        String key = sell ? "sell" : "fish";
        menu.addView(head((sell ? "물고기 판매 장소" : "낚시 장소") + " · 지점 " + pts.size() + "개"));
        for (int i = 0; i < pts.size(); i++) {
            Mover.Point pt = pts.get(i);
            int j = i;
            String t = pt.time == null ? "시간 없음" : fmt(pt.time) + "초";
            menu.addView(item("  " + (i + 1) + "번 · " + t + " — 다시 재기", v -> toggle("measure_" + key + ":" + j)));
        }
        menu.addView(item("+ 지점 추가 (기준 장소부터 걸어감)", v -> toggle("addpt_" + key)));
        if (!pts.isEmpty()) {
            menu.addView(item("− 마지막 지점 지우기", v -> {
                pts.remove(pts.size() - 1);
                Hub.save();
                Hub.log((sell ? "판매" : "낚시") + " 장소 마지막 지점 지움", 'd');
                closeMenu();
            }));
        }
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
        status.setText(CaptureService.active == null ? "화면 캡처 꺼짐 · 앱에서 시작" : Hub.state);
        calBtn.setText(run && "cal".equals(mode) ? "취소" : "보정");
        runBtn.setText(run && !"cal".equals(mode) ? "정지" : "시작");
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
