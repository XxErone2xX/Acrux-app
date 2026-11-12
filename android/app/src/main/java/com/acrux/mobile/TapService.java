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
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

import com.acrux.mobile.core.Mover;
import com.acrux.mobile.core.Seller;

import java.util.ArrayList;
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
    /** root = 떠 있는 창 (오른쪽 끝) · slide = [도착] + 손잡이 + 패널 (열고 닫을 때 같이 미끄러짐) · panel = 오른쪽에서 나오는 세로 패널 */
    LinearLayout root, slide, panel, menu, body;
    WindowManager.LayoutParams lp;
    TextView status, bioBtn, runBtn, gearBtn, arriveBtn, handle;
    /** 패널이 열려 있나 · 패널 폭 (px) */
    boolean open;
    int drawerW;
    int bioCol;
    View picker;
    final Runnable refresh = this::refresh;

    // ---- 설정 안내 (로블록스 화면 위) — 앱 밖일 때 Acrux 창 옆에 안내 카드 · 눌러야 할 곳을 파랗게 강조
    /** 강조할 수 있는 곳들 (gear · cal · run · cat:분류 · pos:위치 · act:실행) */
    final java.util.Map<String, View> targets = new java.util.HashMap<>();
    View lit;
    FrameLayout guideWin;
    WindowManager.LayoutParams guideLp;
    GuideCard fcard;
    final Runnable guideView = this::applyGuide;

    <T extends View> T reg(String key, T v) {
        targets.put(key, v);
        return v;
    }

    void applyGuide() {
        boolean menuOpen = menu != null;
        String cat = Hub.prefs().getString("menuCat", "stick");
        String tg = Guide.cur == null ? null : Guide.target(menuOpen, cat);
        View v = tg == null ? null : open ? targets.get(tg) : handle;   // 패널이 닫혀 있으면 먼저 손잡이를 강조
        if (v != null && !v.isAttachedToWindow()) v = null;
        if (v != lit) {
            if (lit != null) lit.setForeground(null);
            if (v != null) v.setForeground(new Ui.Deco(this, 0x405865F2, Ui.ACCENT, 0xFFFFFFFF, true, 7, 2));
            lit = v;
        }
        boolean show = Guide.cur != null && !Hub.appVisible && !Hub.running() && picker == null;
        if (show) {
            if (guideWin == null) makeGuideWin();
            if (guideWin == null) return;
            guideWin.setVisibility(View.VISIBLE);
            fcard.bind(menuOpen, cat);
        } else if (guideWin != null) {
            guideWin.setVisibility(View.GONE);
        }
    }

    /** 안내 카드 창 (열린 패널 왼쪽 위 · 위쪽 줄을 잡고 끌어서 옮김 · 카드 밖은 게임이 그대로 눌림) */
    void makeGuideWin() {
        int[] sc = screen();
        int w = Math.min(Ui.dp(this, 330), sc[0] / 2);
        fcard = new GuideCard(this, null, 0.38f, true);
        guideWin = new FrameLayout(this);
        guideWin.addView(fcard.card, new FrameLayout.LayoutParams(w, FrameLayout.LayoutParams.WRAP_CONTENT));
        guideLp = new WindowManager.LayoutParams(w, WindowManager.LayoutParams.WRAP_CONTENT,
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL
                        | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
                PixelFormat.TRANSLUCENT);
        guideLp.gravity = Gravity.TOP | Gravity.START;
        // 패널이 오른쪽에서 나오니까 기본 자리는 열린 패널의 왼쪽 (예전 자리 'guideX' 는 패널과 겹쳐서 새 이름으로)
        guideLp.x = Hub.prefs().getInt("guideX2", Math.max(0, sc[0] - w - drawerW - Ui.dp(this, 26 + 12)));
        guideLp.y = Hub.prefs().getInt("guideY2", Ui.dp(this, 12));
        View drag = fcard.card.findViewWithTag("head");
        float[] st = new float[4];
        View.OnTouchListener mover = (v, e) -> {
            switch (e.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                    st[0] = e.getRawX();
                    st[1] = e.getRawY();
                    st[2] = guideLp.x;
                    st[3] = guideLp.y;
                    return true;
                case MotionEvent.ACTION_MOVE:
                    guideLp.x = (int) (st[2] + e.getRawX() - st[0]);
                    guideLp.y = (int) (st[3] + e.getRawY() - st[1]);
                    try {
                        wm.updateViewLayout(guideWin, guideLp);
                    } catch (RuntimeException ignored) {
                    }
                    return true;
                case MotionEvent.ACTION_UP:
                    Hub.prefs().edit().putInt("guideX2", guideLp.x).putInt("guideY2", guideLp.y).apply();
                    return true;
                default:
                    return true;
            }
        };
        if (drag != null) drag.setOnTouchListener(mover);
        fcard.badge.setOnTouchListener(mover);
        try {
            wm.addView(guideWin, guideLp);
        } catch (RuntimeException e) {
            Hub.log("안내 창을 띄우지 못함: " + e, 'r');
            guideWin = null;
        }
    }

    @Override
    protected void onServiceConnected() {
        super.onServiceConnected();
        Hub.init(this);
        instance = this;
        Hub.listeners.add(refresh);
        Guide.views.add(guideView);
        Biome.resume(this);
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
        Guide.views.remove(guideView);
        removeView(guideWin);
        guideWin = null;
        Hub.stop();
        removeView(picker);
        picker = null;
        removeView(root);
        root = null;
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
        x = Math.max(0, x);                                     // 화면 밖이면 화면 끝을 누름 (음수 좌표는 안드로이드가 거절)
        y = Math.max(0, y);
        letThrough(x, y);
        Path p = new Path();
        p.moveTo(x, y);
        dispatchGesture(new GestureDescription.Builder().addStroke(new GestureDescription.StrokeDescription(p, 0, Math.max(1, ms))).build(), null, null);
    }

    /** 한 손가락으로 (x0, y0) → (x1, y1) 끌기 (화면 돌리기) */
    void swipe(int x0, int y0, int x1, int y1, int ms) {
        letThrough(x0, y0);
        Path p = new Path();
        p.moveTo(Math.max(0, x0), Math.max(0, y0));
        p.lineTo(Math.max(0, x1), Math.max(0, y1));
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
        p.moveTo(Math.max(0, x0), Math.max(0, y0));             // 음수 좌표는 안드로이드가 거절 ('Path bounds must not be negative')
        p.lineTo(Math.max(0, x1), Math.max(0, y1));
        return p;
    }

    // ---- 매크로의 터치 · 클릭이 Acrux 창에 걸리지 않게
    // 터치 · 마우스는 화면 맨 위 창이 받음 → Acrux 창이 Fish 버튼 위에 있으면 [보정] 이 누른 Fish 가 Acrux 창의 [시작] 으로 들어갔음
    volatile int[] rect;
    volatile long passUntil;
    volatile boolean passing;

    void updateRect() {
        View p = root;
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
        if (root == null || lp == null) return;
        if (on) lp.flags &= ~WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE;
        else lp.flags |= WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE;
        try {
            wm.updateViewLayout(root, lp);
        } catch (RuntimeException ignored) {
        }
        if (on) passing = false;
    }

    /** 이동하는 동안 계속 통과 (조이스틱을 새로 누를 때마다 창 설정을 바꾸느라 0.04초씩 늦어지지 않게) */
    volatile boolean passKeep;

    void restoreLater() {
        Hub.main.postDelayed(() -> {
            if (!passKeep && SystemClock.uptimeMillis() >= passUntil) setTouchable(true);
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

    /**
     * 떠 있는 조작 창 — 화면 오른쪽 가운데의 손잡이 · 누르거나 왼쪽으로 밀면 오른쪽에서 위아래로 꽉 찬 패널이 나옴
     *  · 닫힘: 손잡이만 (게임 화면을 거의 안 가림) · 열림: 손잡이 + 패널 (⚙ 메뉴도 패널 안에서 펼쳐짐)
     *  · 손잡이를 다시 누르거나 · 패널을 오른쪽으로 밀면 닫힘 · 매크로 · 보정 · 테스트를 시작하면 자동으로 닫힘 (화면 캡처에 안 찍히게)
     *  · 손잡이 글자 색: 파랑 = 대기 · 초록 = 도는 중
     */
    void showPanel() {
        if (root != null) return;
        wm = (WindowManager) getSystemService(WINDOW_SERVICE);
        int[] sc = screen();
        drawerW = Math.max(Ui.dp(this, 200), Math.min(Ui.dp(this, 280), sc[0] * 45 / 100));
        root = new LinearLayout(this);
        root.setOrientation(LinearLayout.HORIZONTAL);
        root.setGravity(Gravity.CENTER_VERTICAL | Gravity.END);
        slide = new LinearLayout(this);
        slide.setOrientation(LinearLayout.HORIZONTAL);
        slide.setGravity(Gravity.CENTER_VERTICAL);
        root.addView(slide, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.MATCH_PARENT));

        // 길 기록 중 [도착] — 패널이 닫혀 있어도 손잡이 옆에 보임 (걸어가는 동안 패널이 화면을 가리지 않게)
        arriveBtn = small("도착", Ui.GREEN, v -> {
            Hub.arrived = true;
            Hub.log("도착", 'g');
        });
        arriveBtn.setVisibility(View.GONE);
        LinearLayout.LayoutParams ap = Ui.wrap();
        ap.rightMargin = Ui.dp(this, 6);
        slide.addView(arriveBtn, ap);

        handle = Ui.text(this, "‹", 20, Ui.ACCENT, true);
        handle.setGravity(Gravity.CENTER);
        handle.setBackground(new Ui.Deco(this, 0xF22B2D31, Ui.LINE, Ui.WHITE, true, 10, 2));
        handle.setOnTouchListener(new View.OnTouchListener() {
            final int slop = android.view.ViewConfiguration.get(TapService.this).getScaledTouchSlop();
            float hx, hy;
            boolean moved;

            @Override
            public boolean onTouch(View v, MotionEvent e) {
                switch (e.getActionMasked()) {
                    case MotionEvent.ACTION_DOWN:
                        hx = e.getRawX();
                        hy = e.getRawY();
                        moved = false;
                        return true;
                    case MotionEvent.ACTION_MOVE: {
                        float dx = e.getRawX() - hx, dy = e.getRawY() - hy;
                        if (!moved && Math.abs(dx) > slop * 2 && Math.abs(dx) > Math.abs(dy)) {
                            moved = true;
                            setDrawer(dx < 0);                          // 왼쪽으로 밀면 열림 · 오른쪽으로 밀면 닫힘
                        }
                        return true;
                    }
                    case MotionEvent.ACTION_UP:
                        if (!moved) {
                            v.performClick();
                            setDrawer(!open);
                        }
                        return true;
                    default:
                        return true;
                }
            }
        });
        slide.addView(handle, new LinearLayout.LayoutParams(Ui.dp(this, 26), Ui.dp(this, 88)));

        panel = new SwipePanel(this);
        panel.setOrientation(LinearLayout.VERTICAL);
        int pad = Ui.dp(this, 12);
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
        status.setMaxWidth(drawerW - pad * 2);            // 긴 이유는 줄바꿈
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        // 바이옴 매크로 켜기 · 끄기 (꺼짐 = 회색 · 켜짐 + 감시 중 = 초록 · 켜짐 + 연결 안 됨 = 노랑) — 낚시 자동 보정은 ⚙ → 낚시
        bioCol = Ui.MUTED;
        bioBtn = reg("biome", small("바이옴 OFF", Ui.MUTED, v -> toggleBiome()));
        runBtn = reg("run", small("시작", Ui.ACCENT, v -> toggle("run")));
        gearBtn = reg("gear", small("⚙", Ui.MUTED, v -> toggleMenu()));
        row.addView(bioBtn, Ui.wrap());
        row.addView(runBtn, gap());
        row.addView(gearBtn, gap());
        panel.addView(head, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        LinearLayout.LayoutParams sp = Ui.wrap();
        sp.topMargin = Ui.dp(this, 2);
        body.addView(status, sp);
        LinearLayout.LayoutParams rp = Ui.wrap();
        rp.topMargin = Ui.dp(this, 8);
        body.addView(row, rp);
        panel.addView(body, Ui.wrap());
        panel.setVisibility(View.GONE);
        slide.addView(panel, new LinearLayout.LayoutParams(drawerW, LinearLayout.LayoutParams.MATCH_PARENT));
        open = false;

        lp = new WindowManager.LayoutParams(
                WindowManager.LayoutParams.WRAP_CONTENT, WindowManager.LayoutParams.WRAP_CONTENT,
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL
                        | WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
                PixelFormat.TRANSLUCENT);
        lp.gravity = Gravity.END | Gravity.CENTER_VERTICAL;
        lp.x = 0;
        lp.y = 0;
        if (android.os.Build.VERSION.SDK_INT >= 28) {
            lp.layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
        }
        applyAlphaTo();
        root.getViewTreeObserver().addOnGlobalLayoutListener(this::updateRect);
        try {
            wm.addView(root, lp);
        } catch (RuntimeException e) {
            Hub.log("조작 창을 띄우지 못함: " + e, 'r');
            root = null;
            panel = null;
        }
        updateVisible();
        refresh();
    }

    /**
     * 패널 열기 · 닫기 (0.18초 동안 오른쪽에서 미끄러짐)
     * 열 때: 창을 위아래로 꽉 차게 키운 뒤 손잡이 + 패널을 패널 폭만큼 오른쪽에 두었다가 제자리로
     * 닫을 때: 오른쪽으로 밀어낸 뒤 패널을 숨기고 창을 손잡이 크기로 줄임 (닫힌 동안 게임 화면을 거의 안 가림)
     */
    void setDrawer(boolean o) {
        if (root == null || panel == null || o == open) return;
        open = o;
        slide.animate().cancel();
        handle.setText(o ? "›" : "‹");
        if (o) {
            panel.setVisibility(View.VISIBLE);
            lp.height = WindowManager.LayoutParams.MATCH_PARENT;
            try {
                wm.updateViewLayout(root, lp);
            } catch (RuntimeException ignored) {
            }
            slide.setTranslationX(drawerW);
            slide.animate().translationX(0).setDuration(180).withEndAction(this::updateRect).start();
        } else {
            closeMenu();
            slide.animate().translationX(drawerW).setDuration(160).withEndAction(() -> {
                if (open || root == null) return;
                panel.setVisibility(View.GONE);
                slide.setTranslationX(0);
                lp.height = WindowManager.LayoutParams.WRAP_CONTENT;
                try {
                    wm.updateViewLayout(root, lp);
                } catch (RuntimeException ignored) {
                }
                updateRect();
            }).start();
        }
        Hub.main.post(this::applyGuide);
    }

    /** 패널: 오른쪽으로 밀면 닫힘 (짧게 누르면 그대로 버튼 · ⚙ 메뉴 안에서 위아래로 밀면 메뉴 스크롤) */
    final class SwipePanel extends LinearLayout {
        final int slop = android.view.ViewConfiguration.get(getContext()).getScaledTouchSlop();
        float sx, sy;
        boolean swiping;

        SwipePanel(Context c) {
            super(c);
        }

        boolean right(MotionEvent e) {
            float dx = e.getRawX() - sx, dy = e.getRawY() - sy;
            return dx > slop * 3 && dx > Math.abs(dy) * 1.5f;
        }

        @Override
        public boolean onInterceptTouchEvent(MotionEvent e) {
            switch (e.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                    sx = e.getRawX();
                    sy = e.getRawY();
                    swiping = false;
                    return false;
                case MotionEvent.ACTION_MOVE:
                    if (!swiping && right(e)) swiping = true;
                    return swiping;
                default:
                    return swiping;
            }
        }

        @Override
        public boolean onTouchEvent(MotionEvent e) {
            switch (e.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                    sx = e.getRawX();
                    sy = e.getRawY();
                    swiping = false;
                    return true;
                case MotionEvent.ACTION_MOVE:
                    if (!swiping && right(e)) {
                        swiping = true;
                        setDrawer(false);
                    }
                    return true;
                default:
                    swiping = false;
                    return true;
            }
        }
    }

    /** 보이기 · 숨기기: 앱의 [UI 끄기] 로 꺼져 있거나, Acrux 앱을 보고 있고 '앱을 볼 때 숨기기' 가 켜져 있으면 숨김 */
    void updateVisible() {
        Hub.main.post(() -> {
            if (root == null) return;
            boolean on = Hub.prefs().getBoolean("panelOn", true) && !(Hub.appVisible && Hub.prefs().getBoolean("hideOnApp", true));
            root.setVisibility(on ? View.VISIBLE : View.GONE);
            if (!on) setDrawer(false);
            root.post(TapService.this::updateRect);
        });
    }

    /** 언어를 바꿨을 때: 창을 지금 언어로 다시 만듦 (열려 있었으면 다시 엶) */
    void rebuildPanel() {
        Hub.main.post(() -> {
            if (root == null) return;
            boolean was = open;
            closeMenu();
            removeView(root);
            root = null;
            panel = null;
            open = false;
            showPanel();
            if (was) setDrawer(true);
        });
    }

    void applyAlpha() {
        Hub.main.post(this::applyAlphaTo);
    }

    void applyAlphaTo() {
        if (root != null) root.setAlpha(Hub.prefs().getInt("panelAlpha", 90) / 100f);
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
        // 패널이 열려 있으면 먼저 닫고 창이 손잡이 크기로 줄어든 뒤에 시작 (열린 패널이 화면 캡처에 찍히거나 낚시 창을 가린다고 잘못 알리지 않게)
        if (open) {
            setDrawer(false);
            Hub.main.postDelayed(() -> startMode(m), 260);
            return;
        }
        startMode(m);
    }

    void startMode(String m) {
        if (Hub.running()) return;
        updateRect();
        String err = Hub.start(m);
        if (!err.isEmpty()) {
            // 못 하는 이유: 기록 칸만이 아니라 떠 있는 창 · 화면 알림에도 (게임 화면에선 기록 칸이 안 보임)
            Hub.log(err, 'y');
            Hub.state = err;
            Hub.changed();
            android.widget.Toast.makeText(this, com.acrux.mobile.core.I18n.t(err), android.widget.Toast.LENGTH_LONG).show();
            // 쓰려는 기능에 설정이 덜 됐으면 그 기능의 설정 안내 (처음 [시작] 을 누른 때 등)
            if (TapService.instance == null || CaptureService.active == null) Guide.featureOn("ready");
            else if (m.equals("run") && !Hub.cfg.ready()) Guide.featureOn("fish");
            else if (m.equals("run") && Guide.moveNeeded()) Guide.featureOn("move");   // 자동 낚시는 이동 · 판매 설정까지 필요
        }
        closeMenu();
        refresh();
    }

    /** 미니 UI 의 [바이옴]: 바이옴 매크로 켜기 · 끄기 (앱의 바이옴 칸 스위치와 같음) */
    void toggleBiome() {
        boolean on = !Biome.on();
        Biome.set(this, on);
        String msg = on ? (Biome.hooks().isEmpty() ? "바이옴 매크로 켜짐 — 웹후크는 앱의 바이옴 칸에서 입력" : "바이옴 매크로 켜짐") : "바이옴 매크로 꺼짐";
        android.widget.Toast.makeText(this, com.acrux.mobile.core.I18n.t(msg), android.widget.Toast.LENGTH_SHORT).show();
        refresh();
    }

    // ---- ⚙ 메뉴 — 위쪽 분류 (낚시 · 조이스틱 · 기준 장소 · 길 · 판매) 를 눌러서 그 분류만 보임 · 마지막에 본 분류를 기억
    static final String[][] CATS = {{"fish", "낚시"}, {"stick", "조이스틱"}, {"base", "기준 장소"}, {"route", "길"}, {"sell", "판매"}};
    LinearLayout catBody;
    final List<TextView> catTabs = new ArrayList<>();

    void toggleMenu() {
        if (menu != null) {
            closeMenu();
            return;
        }
        setDrawer(true);                                         // 메뉴는 패널 안에서 펼쳐짐 (안내 중 다시 열 때도)
        menu = new LinearLayout(this);
        menu.setOrientation(LinearLayout.VERTICAL);
        HorizontalScrollView hs = new HorizontalScrollView(this);
        hs.setHorizontalScrollBarEnabled(false);
        LinearLayout tabs = new LinearLayout(this);
        tabs.setOrientation(LinearLayout.HORIZONTAL);
        catTabs.clear();
        for (int i = 0; i < CATS.length; i++) {
            String key = CATS[i][0];
            TextView c = reg("cat:" + key, Ui.chip(this, CATS[i][1], v -> showCat(key)));
            c.setTextSize(12);
            LinearLayout.LayoutParams lp = Ui.wrap();
            if (i > 0) lp.leftMargin = Ui.dp(this, 4);
            tabs.addView(c, lp);
            catTabs.add(c);
        }
        hs.addView(tabs);
        catBody = new LinearLayout(this);
        catBody.setOrientation(LinearLayout.VERTICAL);
        ScrollView sv = new ScrollView(this);
        sv.addView(catBody);
        menu.addView(hs);
        // 패널 남은 높이를 메뉴가 다 씀 (위아래로 꽉 찬 패널 · 길면 스크롤)
        menu.addView(sv, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f));
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f);
        p.topMargin = Ui.dp(this, 8);
        panel.addView(menu, p);
        menu.setTag(menu);
        showCat(Hub.prefs().getString("menuCat", "fish"));
    }

    void showCat(String key) {
        if (catBody == null) return;
        Hub.prefs().edit().putString("menuCat", key).apply();
        for (int i = 0; i < CATS.length; i++) Ui.chipOn(this, catTabs.get(i), CATS[i][0].equals(key));
        LinearLayout m = catBody;
        m.removeAllViews();
        Hub.main.post(this::applyGuide);
        switch (key) {
            case "fish":
                m.addView(head("낚시 보정"));
                m.addView(item(Hub.cfg.ready() ? "✓ 보정됨" : "· 보정 필요 — 낚시 자리에서 Fish 버튼이 보일 때", null));
                m.addView(reg("act:cal", item((Hub.cfg.ready() ? "낚시 자동 보정 다시 하기" : "낚시 자동 보정") + " (Fish 를 한 번 던져 창 위치를 맞춤)", v -> toggle("cal"))));
                break;
            case "base":
                m.addView(head("공통 위치 (기준 장소 맞추기)"));
                for (String[] k : Mover.BASE_KEYS) m.addView(posItem(k[0], k[1], ""));
                m.addView(head("테스트"));
                m.addView(reg("act:base", item("기준 장소로", v -> toggle("base"))));
                break;
            case "route":
                place(m, false);
                place(m, true);
                m.addView(head("테스트"));
                m.addView(reg("act:fish", item("기준 장소 → 낚시 장소", v -> toggle("fish"))));
                m.addView(reg("act:sellgo", item("기준 장소 → 판매 장소", v -> toggle("sellgo"))));
                break;
            case "sell": {
                m.addView(head("대화"));
                for (String[] k : Seller.TALK) m.addView(posItem(k[0], k[1], " (판매 장소에 서서)"));
                m.addView(head("상점"));
                m.addView(reg("act:sellcal", item("판매 자동 보정 (NPC 앞에서 시작 → 대화 버튼은 직접)", v -> toggle("sellcal"))));
                int set = 0;
                for (String[] k : Seller.KEYS) if (Hub.pos.containsKey(k[0])) set++;
                m.addView(item("  판매 위치 " + set + " / " + Seller.KEYS.length + " 저장됨", null));
                for (String[] k : new String[][]{{"first_fish_pos", "첫 번째 물고기 칸"}, {"shop_close_pos", "상점 닫기 X"}}) {
                    m.addView(posItem(k[0], k[1], " 직접 지정 (상점을 연 채로)"));
                }
                m.addView(head("테스트"));
                m.addView(reg("act:sell", item("판매 한 번 해 보기", v -> toggle("sell"))));
                break;
            }
            default:
                m.addView(head("조이스틱 (W · A · S · D)"));
                m.addView(item(Hub.stick != null ? "✓ 조이스틱 저장됨 (눌러서 위치 보기)" : "· 조이스틱 없음 — 로블록스 설정 → 이동 방식: 조이스틱", Hub.stick != null ? v -> {
                    closeMenu();
                    Hub.showStick(2500);
                } : null));
                m.addView(reg("act:stickfind", item("조이스틱 찾기 (화면에서 자동으로)", v -> toggle("stickfind"))));
                m.addView(item("조이스틱 직접 지정 (가운데 → 가장자리)", v -> pickStick()));
                m.addView(head("점프"));
                for (String[] k : Mover.JUMP_KEYS) m.addView(posItem(k[0], k[1], " — 길 기록에서 점프"));
                m.addView(head("테스트"));
                m.addView(reg("act:sticktest", item("조이스틱 테스트 (W · A · S · D 0.8초씩)", v -> toggle("sticktest"))));
                m.addView(reg("act:jumptest", item("점프 테스트 (제자리 → 걸으면서)", v -> toggle("jumptest"))));
                break;
        }
    }

    /** 위치 하나 (✓ 저장됨 · · 없음) → 누르면 포인터로 지정 */
    TextView posItem(String key, String label, String tail) {
        return reg("pos:" + key, item((Hub.pos.containsKey(key) ? "✓ " : "· ") + label + tail, v -> pickSave(key, label)));
    }

    /** 길 기록: 기준 장소로 간 뒤 '지금부터 기록' → WASD 패드로 걸어가서 [도착] */
    void place(LinearLayout m, boolean sell) {
        com.acrux.mobile.core.Route r = sell ? Hub.sellRoute : Hub.fishRoute;
        String name = sell ? "물고기 판매 장소" : "낚시 장소";
        m.addView(head(name + " 길"));
        m.addView(item(r.isEmpty() ? "· 기록 없음" : "✓ 기록됨 · " + fmt(r.seconds()) + "초 · " + r.parts() + "조각 · 점프 " + r.jumps() + "번", null));
        m.addView(reg(sell ? "act:rec_sell" : "act:rec_fish", item((r.isEmpty() ? "● 길 기록" : "● 다시 기록") + " (기준 장소로 간 뒤 WASD 패드로 걸어감 → [도착])", v -> toggle(sell ? "rec_sell" : "rec_fish"))));
    }

    TextView head(String s) {
        TextView t = Ui.text(this, s, 11, Ui.MUTED, true);
        t.setPadding(Ui.dp(this, 2), Ui.dp(this, 8), 0, Ui.dp(this, 2));
        return t;
    }

    void closeMenu() {
        if (menu == null || panel == null) return;
        panel.removeView((View) menu.getTag());
        menu = null;
        catBody = null;
        Hub.main.post(this::applyGuide);
    }

    void pickSave(String key, String label) {
        pick(label, Hub.pos.get(key), at -> {
            if (at == null) return;
            Hub.pos.put(key, at);
            Hub.save();
            Hub.log("'" + label + "' 위치 저장 (" + at[0] + ", " + at[1] + ")", 'g');
            if (Guide.cur != null) Hub.main.post(() -> {        // 설정 안내 중이면 메뉴를 다시 열어 둠 (다음 위치로 바로)
                if (menu == null) toggleMenu();
            });
        });
    }

    /** 조이스틱 직접 지정: 가운데를 누르고 → 바깥 판 가장자리를 누름 */
    void pickStick() {
        double[] st = Hub.stick;
        pick("조이스틱 가운데", st, c -> {
            if (c == null) return;
            int[] s0 = screen();
            double er = st != null ? st[2] * s0[1] : s0[1] * 0.1;
            Hub.main.post(() -> pick("조이스틱 바깥 판 가장자리", new double[]{c[0] + er / s0[0], c[1]}, e -> {
                if (e == null) return;
                int[] s = screen();
                double r = Math.hypot((e[0] - c[0]) * s[0], (e[1] - c[1]) * s[1]);
                if (r < 5) {
                    Hub.log("가운데와 가장자리가 너무 가까움 — 다시", 'y');
                    return;
                }
                double[] st2 = Hub.fitStick(new double[]{c[0], c[1], r / s[1]}, s[0], s[1]);
                if (st2[2] * s[1] < r - 1) Hub.log("조이스틱 판이 화면 밖으로 나가서 반지름을 화면 안까지로 줄임 (" + Math.round(r) + " → " + Math.round(st2[2] * s[1]) + "px)", 'y');
                if (st2[2] * s[1] < 5) {
                    Hub.log("조이스틱 가운데가 화면 끝에 너무 붙어 있음 — 다시", 'y');
                    return;
                }
                r = st2[2] * s[1];
                Hub.stick = st2;
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

    void pick(String name, java.util.function.Consumer<double[]> done) {
        pick(name, null, done);
    }

    /**
     * 위치 지정: 로블록스 화면 위에 포인터 (십자 · 동그라미) 를 띄움 → 화면 아무 데나 대고 끌면 포인터가 끈 만큼 움직임 (손가락 자리로 가지 않음)
     *  · 손가락이 포인터를 가리지 않아서 정확하게 맞출 수 있음 · 끈 거리의 0.6 배만 움직임 (세밀하게)
     *  · [확인] 으로 저장 · [취소] · 처음 자리 = 저장된 자리 (없으면 화면 가운데) · 화면 비율로 넘김 · 취소하면 null
     */
    void pick(String name, double[] init, java.util.function.Consumer<double[]> done) {
        setDrawer(false);                                        // 패널이 그 자리 게임 화면을 가리지 않게
        closeMenu();
        if (picker != null) {
            done.accept(null);
            return;
        }
        int[] s = screen();
        FrameLayout f = new FrameLayout(this);
        init = Hub.fitPos(init);
        PointerView pv = new PointerView(this, init != null ? (float) (init[0] * s[0]) : s[0] / 2f, init != null ? (float) (init[1] * s[1]) : s[1] / 2f);
        f.addView(pv, new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        LinearLayout bar = new LinearLayout(this);
        bar.setOrientation(LinearLayout.HORIZONTAL);
        bar.setGravity(Gravity.CENTER_VERTICAL);
        bar.setBackgroundColor(0xE61E1F22);
        bar.setPadding(Ui.dp(this, 12), Ui.dp(this, 6), Ui.dp(this, 6), Ui.dp(this, 6));
        TextView label = Ui.text(this, "'" + name + "' — 화면을 끌어서 포인터를 옮기세요", 13, Ui.WHITE, true);
        bar.addView(label, Ui.wrap());
        java.util.function.Consumer<double[]> finish = at -> {
            removeView(picker);
            picker = null;
            done.accept(at);
        };
        TextView ok = small("확인", Ui.GREEN, v -> {
            int[] loc = new int[2];
            pv.getLocationOnScreen(loc);
            finish.accept(Hub.fitPos(new double[]{Math.round((pv.px + loc[0]) / s[0] * 10000) / 10000.0, Math.round((pv.py + loc[1]) / s[1] * 10000) / 10000.0}));
        });
        TextView no = small("취소", Ui.MUTED, v -> finish.accept(null));
        bar.addView(ok, gap());
        bar.addView(no, gap());
        FrameLayout.LayoutParams bp = new FrameLayout.LayoutParams(FrameLayout.LayoutParams.WRAP_CONTENT, FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.TOP | Gravity.CENTER_HORIZONTAL);
        bp.topMargin = Ui.dp(this, 10);
        f.addView(bar, bp);
        // 포인터가 위쪽 막대 근처면 막대를 아래로 (포인터를 가리지 않게)
        pv.onMove = () -> {
            boolean top = pv.py < s[1] * 0.25f;
            int g = (top ? Gravity.BOTTOM : Gravity.TOP) | Gravity.CENTER_HORIZONTAL;
            if (bp.gravity != g) {
                bp.gravity = g;
                bp.topMargin = top ? 0 : Ui.dp(this, 10);
                bp.bottomMargin = top ? Ui.dp(this, 10) : 0;
                bar.setLayoutParams(bp);
            }
        };
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
            pv.post(pv.onMove);
        } catch (RuntimeException e) {
            Hub.log("위치 지정 창을 띄우지 못함: " + e, 'r');
            done.accept(null);
        }
    }

    /** 위치 지정 포인터 — 끌면 끈 만큼 (0.6 배) 움직임 · 검정 테두리 + 흰 선 + 빨간 점이라 어떤 배경에서도 잘 보임 */
    static final class PointerView extends View {
        static final float SPEED = 0.6f;
        float px, py, lx, ly;
        int id = -1;
        Runnable onMove;
        final android.graphics.Paint dark = new android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG),
                light = new android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG),
                dot = new android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG);
        final float u;

        PointerView(Context c, float x, float y) {
            super(c);
            px = x;
            py = y;
            u = c.getResources().getDisplayMetrics().density;
            dark.setStyle(android.graphics.Paint.Style.STROKE);
            dark.setStrokeWidth(4.5f * u);
            dark.setColor(0xFF000000);
            light.setStyle(android.graphics.Paint.Style.STROKE);
            light.setStrokeWidth(2f * u);
            light.setColor(0xFFFFFFFF);
            dot.setColor(0xFFFF3B3B);
        }

        @Override
        protected void onDraw(android.graphics.Canvas c) {
            c.drawColor(0x22000000);
            float r = 18 * u, gap = 5 * u, arm = 46 * u;
            for (android.graphics.Paint pt : new android.graphics.Paint[]{dark, light}) {
                c.drawCircle(px, py, r, pt);
                c.drawLine(px - arm, py, px - gap, py, pt);
                c.drawLine(px + gap, py, px + arm, py, pt);
                c.drawLine(px, py - arm, px, py - gap, pt);
                c.drawLine(px, py + gap, px, py + arm, pt);
            }
            c.drawCircle(px, py, 2.2f * u, dot);
        }

        @Override
        public boolean onTouchEvent(MotionEvent e) {
            switch (e.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                case MotionEvent.ACTION_POINTER_DOWN: {
                    int i = e.getActionIndex();         // 마지막에 댄 손가락으로 끌기
                    id = e.getPointerId(i);
                    lx = e.getX(i);
                    ly = e.getY(i);
                    break;
                }
                case MotionEvent.ACTION_MOVE: {
                    int i = e.findPointerIndex(id);
                    if (i < 0) break;
                    float x = e.getX(i), y = e.getY(i);
                    px = Math.max(0, Math.min(getWidth() - 1, px + (x - lx) * SPEED));
                    py = Math.max(0, Math.min(getHeight() - 1, py + (y - ly) * SPEED));
                    lx = x;
                    ly = y;
                    invalidate();
                    if (onMove != null) onMove.run();
                    break;
                }
                case MotionEvent.ACTION_POINTER_UP: {
                    if (e.getPointerId(e.getActionIndex()) == id) {
                        int j = e.getActionIndex() == 0 ? 1 : 0;
                        id = e.getPointerId(j);
                        lx = e.getX(j);
                        ly = e.getY(j);
                    }
                    break;
                }
                default:
                    id = -1;
                    break;
            }
            return true;
        }
    }

    void refresh() {
        if (root == null || panel == null) return;
        boolean run = Hub.running();
        String mode = Hub.mode;
        Ui.set(status, CaptureService.active == null ? "화면 캡처 꺼짐 · 앱에서 시작" : Hub.state);
        boolean bio = Biome.on();
        Ui.set(bioBtn, bio ? "바이옴 ON" : "바이옴 OFF");
        int col = !bio ? Ui.MUTED : Biome.watching ? Ui.GREEN : Ui.YELLOW;
        if (col != bioCol) {
            bioCol = col;
            Ui.paintButton(this, bioBtn, col);
        }
        Ui.set(runBtn, run ? "정지" : "시작");            // 보정 · 테스트 · 기록 중에도 [정지] 로 멈춤
        arriveBtn.setVisibility(Hub.measuring ? View.VISIBLE : View.GONE);
        handle.setTextColor(run ? Ui.GREEN : Ui.ACCENT);
    }

    static String fmt(double v) {
        return String.format(Locale.ROOT, "%.2f", v);
    }
}
