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
    /**
     * 창 두 개 — 크기를 바꾸면 기기가 크기 변화를 따로 그려서 패널이 위에서부터 늘어나 보였음 → 두 창 다 크기는 고정
     *  · hwin = 닫힌 동안의 손잡이 (+ 길 기록 중 [도착]) · 오른쪽 가운데 · 작음
     *  · root = 패널 창 (손잡이 + 패널 · 위아래 꽉 참) · 닫힌 동안은 안 보이고 터치도 통과 · 열고 닫을 땐 안의 slide 만 미끄러짐
     */
    LinearLayout root, hwin, slide, panel;
    WindowManager.LayoutParams lp, hlp;
    TextView status, runBtn, arriveBtn;
    /** 손잡이 — 닫힌 창의 것 · 열린 패널 왼쪽의 것 (모양 · 자리는 같음) */
    HandleView handle, handleIn;
    /** 패널이 열려 있나 · 패널 폭 (px) */
    boolean open;
    int drawerW;
    View picker;
    final Runnable refresh = this::refresh;

    // ---- 설정 안내 (로블록스 화면 위) — 앱 밖일 때 Acrux 창 옆에 안내 카드 · 눌러야 할 곳을 파랗게 강조
    /** 강조할 수 있는 곳들 (지금 페이지의 것만: run · biome · feat:기능 · back · pos:위치 · act:실행) */
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
        String tg = guideTarget();
        View v = tg == null ? null : open ? targets.get(tg) : handle;   // 패널이 닫혀 있으면 먼저 손잡이를 강조
        if (v != null && !v.isAttachedToWindow()) v = null;
        if (v != lit) {
            if (lit instanceof HandleView) ((HandleView) lit).setLit(false);
            else if (lit != null) lit.setForeground(null);
            if (v instanceof HandleView) ((HandleView) v).setLit(true);           // 손잡이는 사다리꼴 모양 그대로 파랗게
            else if (v != null) v.setForeground(new Ui.Deco(this, 0x334C8DFF, C_ACC, 0, true, 0, 0));
            lit = v;
        }
        boolean show = Guide.cur != null && !Hub.appVisible && !Hub.running() && picker == null && !locked();
        if (show) {
            if (guideWin == null) makeGuideWin();
            if (guideWin == null) return;
            guideWin.setVisibility(View.VISIBLE);
            fcard.bind(open ? tg : null);
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
        android.content.IntentFilter f = new android.content.IntentFilter(Intent.ACTION_SCREEN_OFF);
        f.addAction(Intent.ACTION_SCREEN_ON);
        f.addAction(Intent.ACTION_USER_PRESENT);
        try {
            registerReceiver(screenRx, f);
            screenRxOn = true;
        } catch (RuntimeException e) {
            Hub.log("화면 잠금 감지를 못 켬: " + e, 'y');
        }
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
        if (screenRxOn) {
            screenRxOn = false;
            try {
                unregisterReceiver(screenRx);
            } catch (RuntimeException ignored) {
            }
        }
        removeView(guideWin);
        guideWin = null;
        Hub.stop();
        removeView(picker);
        picker = null;
        removeView(root);
        removeView(hwin);
        root = null;
        hwin = null;
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
        View p = open ? root : hwin;
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
        if (root == null || lp == null || hwin == null) return;
        touch(hwin, hlp, on);
        touch(root, lp, on && open);                     // 패널 창은 열려 있을 때만 터치를 받음
        if (on) passing = false;
    }

    void touch(View v, WindowManager.LayoutParams p, boolean on) {
        int f = on ? p.flags & ~WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE : p.flags | WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE;
        if (f == p.flags) return;
        p.flags = f;
        try {
            wm.updateViewLayout(v, p);
        } catch (RuntimeException ignored) {
        }
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
     *  · 닫힘: 손잡이만 (게임 화면을 거의 안 가림) · 열림: 손잡이 + 패널 (홈 · 기능 설정 페이지)
     *  · 손잡이를 다시 누르거나 · 패널을 오른쪽으로 밀면 닫힘 · 매크로 · 보정 · 테스트를 시작하면 자동으로 닫힘 (화면 캡처에 안 찍히게)
     *  · 손잡이 = 사다리꼴 (화면 쪽이 넓음) + 끝에 세로선 · 선 색: 회색 = 대기 · 파랑 = 도는 중
     */
    void showPanel() {
        if (root != null) return;
        wm = (WindowManager) getSystemService(WINDOW_SERVICE);
        int[] sc = screen();
        drawerW = Math.max(Ui.dp(this, 200), Math.min(Ui.dp(this, 280), sc[0] * 45 / 100));
        int hw = Ui.dp(this, 22), hh = Ui.dp(this, 92);

        // ---- 닫힌 손잡이 창
        hwin = new LinearLayout(this);
        hwin.setOrientation(LinearLayout.HORIZONTAL);
        hwin.setGravity(Gravity.CENTER_VERTICAL);
        // 길 기록 중 [도착] — 패널이 닫혀 있어도 손잡이 옆에 보임 (걸어가는 동안 패널이 화면을 가리지 않게)
        arriveBtn = small("도착", Ui.GREEN, v -> {
            Hub.arrived = true;
            Hub.log("도착", 'g');
        });
        arriveBtn.setVisibility(View.GONE);
        LinearLayout.LayoutParams ap = Ui.wrap();
        ap.rightMargin = Ui.dp(this, 6);
        hwin.addView(arriveBtn, ap);
        handle = new HandleView(this);
        handle.setOnTouchListener(handleTouch());
        hwin.addView(handle, new LinearLayout.LayoutParams(hw, hh));
        hlp = new WindowManager.LayoutParams(
                WindowManager.LayoutParams.WRAP_CONTENT, WindowManager.LayoutParams.WRAP_CONTENT,
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
                PixelFormat.TRANSLUCENT);
        hlp.gravity = Gravity.END | Gravity.CENTER_VERTICAL;

        // ---- 패널 창 (손잡이 + 패널 · 크기 고정 · 닫힌 동안 안 보이고 터치 통과)
        root = new LinearLayout(this);
        root.setOrientation(LinearLayout.HORIZONTAL);
        slide = new LinearLayout(this);
        slide.setOrientation(LinearLayout.HORIZONTAL);
        slide.setGravity(Gravity.CENTER_VERTICAL);
        root.addView(slide, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.MATCH_PARENT));
        handleIn = new HandleView(this);
        handleIn.setOnTouchListener(handleTouch());
        slide.addView(handleIn, new LinearLayout.LayoutParams(hw, hh));

        panel = new SwipePanel(this);
        panel.setOrientation(LinearLayout.VERTICAL);
        buildPanel();
        slide.addView(panel, new LinearLayout.LayoutParams(drawerW, LinearLayout.LayoutParams.MATCH_PARENT));
        open = false;
        slide.setTranslationX(drawerW);                    // 닫힘: 패널은 화면 밖 · 손잡이는 닫힌 손잡이와 같은 자리
        root.setVisibility(View.INVISIBLE);

        // 상태 표시줄 · 내비게이션 바 안쪽으로 (로블록스 전체 화면에선 바가 없어서 위아래 끝까지)
        lp = new WindowManager.LayoutParams(
                hw + drawerW, WindowManager.LayoutParams.MATCH_PARENT,
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL
                        | WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE,
                PixelFormat.TRANSLUCENT);
        lp.gravity = Gravity.END | Gravity.TOP;
        applyAlphaTo();
        hwin.getViewTreeObserver().addOnGlobalLayoutListener(this::updateRect);
        root.getViewTreeObserver().addOnGlobalLayoutListener(this::updateRect);
        try {
            wm.addView(root, lp);
            wm.addView(hwin, hlp);
        } catch (RuntimeException e) {
            Hub.log("조작 창을 띄우지 못함: " + e, 'r');
            removeView(root);
            removeView(hwin);
            root = null;
            hwin = null;
            panel = null;
        }
        updateVisible();
        refresh();
    }

    /** 손잡이 터치: 누르면 열고 닫기 · 왼쪽으로 밀면 열림 · 오른쪽으로 밀면 닫힘 */
    View.OnTouchListener handleTouch() {
        return new View.OnTouchListener() {
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
                            setDrawer(dx < 0);
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
        };
    }

    /**
     * 패널 열기 · 닫기 — 창 크기는 그대로 · 안의 손잡이 + 패널만 미끄러짐 (열 때 0.22초 감속 · 닫을 때 0.18초 가속)
     * 열 때: 닫힌 손잡이를 숨기는 순간 패널 창의 손잡이가 같은 자리에 보이고 → 그대로 왼쪽으로
     * 닫을 때: 오른쪽 끝까지 밀어낸 뒤 패널 창을 숨기고 닫힌 손잡이를 다시 보임
     */
    void setDrawer(boolean o) {
        if (root == null || panel == null || hwin == null || o == open) return;
        open = o;
        slide.animate().cancel();
        if (o) {
            refresh();                                           // 실행 시간 · 상태를 열자마자 지금 값으로
            root.setVisibility(View.VISIBLE);
            touch(root, lp, true);
            handle.setVisibility(View.INVISIBLE);
            slide.animate().translationX(0).setDuration(220).setInterpolator(new android.view.animation.DecelerateInterpolator(1.6f))
                    .withEndAction(this::updateRect).start();
        } else {
            slide.animate().translationX(drawerW).setDuration(180).setInterpolator(new android.view.animation.AccelerateInterpolator(1.4f))
                    .withEndAction(() -> {
                        if (open || root == null) return;
                        handle.setVisibility(View.VISIBLE);
                        root.setVisibility(View.INVISIBLE);
                        touch(root, lp, false);
                        updateRect();
                    }).start();
        }
        updateRect();
        Hub.main.post(this::applyGuide);
    }

    /** 손잡이 — 사다리꼴 (화면 쪽 오른변이 길고 왼변이 짧음) + 왼쪽 끝 가까이 세로선 하나 (ㄱ 장식 · 화살표 없음) */
    static final class HandleView extends View {
        final android.graphics.Paint fill = new android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG),
                edge = new android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG),
                bar = new android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG);
        final android.graphics.Path shape = new android.graphics.Path();
        final float u;
        int barColor = C_MUT;
        boolean lit;

        HandleView(Context c) {
            super(c);
            u = c.getResources().getDisplayMetrics().density;
            edge.setStyle(android.graphics.Paint.Style.STROKE);
            edge.setStrokeWidth(u);
            bar.setStrokeWidth(3 * u);
            bar.setStrokeCap(android.graphics.Paint.Cap.ROUND);
        }

        void setBar(int c) {
            if (c == barColor) return;
            barColor = c;
            invalidate();
        }

        void setLit(boolean on) {
            if (on == lit) return;
            lit = on;
            invalidate();
        }

        @Override
        protected void onDraw(android.graphics.Canvas c) {
            float w = getWidth(), h = getHeight(), inset = h * 0.16f;
            shape.reset();
            shape.moveTo(w, 0);                      // 오른쪽 위 (화면 · 패널 쪽 · 긴 변)
            shape.lineTo(0, inset);                  // 왼쪽 위 (짧은 변)
            shape.lineTo(0, h - inset);
            shape.lineTo(w, h);
            shape.close();
            fill.setColor(lit ? 0xF51B2F55 : C_BG);
            c.drawPath(shape, fill);
            edge.setColor(lit ? C_ACC : C_LINE);
            c.drawPath(shape, edge);
            float x = Math.max(4 * u, w * 0.38f), len = (h - inset * 2) * 0.42f;
            bar.setColor(lit ? 0xFFFFFFFF : barColor);
            c.drawLine(x, h / 2 - len / 2, x, h / 2 + len / 2, bar);
        }
    }

    /** 패널: 오른쪽으로 밀면 닫힘 (짧게 누르면 그대로 버튼 · 위아래로 밀면 목록 스크롤) */
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
            if (root == null || hwin == null) return;
            boolean on = Hub.prefs().getBoolean("panelOn", true) && !(Hub.appVisible && Hub.prefs().getBoolean("hideOnApp", true)) && !locked();
            if (!on) setDrawer(false);
            hwin.setVisibility(on ? View.VISIBLE : View.GONE);
            hwin.post(TapService.this::updateRect);
        });
    }

    /** 화면이 꺼졌거나 잠금 화면이면 true — 그동안 Acrux 창 · 안내 창을 숨김 (잠금 화면 위에 안 뜨게) */
    boolean locked() {
        android.os.PowerManager pm = (android.os.PowerManager) getSystemService(POWER_SERVICE);
        android.app.KeyguardManager km = (android.app.KeyguardManager) getSystemService(KEYGUARD_SERVICE);
        return (pm != null && !pm.isInteractive()) || (km != null && km.isKeyguardLocked());
    }

    /** 화면 꺼짐 · 켜짐 · 잠금 해제 때 다시 확인 */
    final android.content.BroadcastReceiver screenRx = new android.content.BroadcastReceiver() {
        @Override
        public void onReceive(Context c, Intent i) {
            updateVisible();
            Hub.main.post(TapService.this::applyGuide);
        }
    };
    boolean screenRxOn;

    /** 언어를 바꿨을 때: 창을 지금 언어로 다시 만듦 (열려 있었으면 다시 엶) */
    void rebuildPanel() {
        Hub.main.post(() -> {
            if (root == null) return;
            boolean was = open;
            removeView(root);
            removeView(hwin);
            root = null;
            hwin = null;
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
        float a = Hub.prefs().getInt("panelAlpha", 90) / 100f;
        if (root != null) root.setAlpha(a);
        if (hwin != null) hwin.setAlpha(a);
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

    // ---- 미니 UI — 홈 (매크로 시작 · 바이옴 매크로 · 기능 목록 · 낚시 통계는 앱에서만) + 기능마다 설정 페이지 (‹ 로 돌아감)
    //  · 위 줄 (Acrux · 상태 · 실행 시간) 은 어느 페이지에서나 그대로
    //  · 기능 = PAGES 에 한 줄 + 설정 페이지 하나 (새 기능도 같은 방식으로 추가) · 안내의 분류 이름 = 페이지 id
    static final int C_BG = 0xF516191C, C_P2 = 0xFF1C2024, C_LINE = 0xFF262B30, C_FG = 0xFFE6E8EA, C_MUT = 0xFF8A939B,
            C_DIM = 0xFF5B636B, C_ACC = 0xFF4C8DFF, C_RED = 0xFFFF5A4E, C_WARN = 0xFFFFB020;

    /** 페이지: id · 위 페이지 · 이름 */
    static final String[][] PAGES = {
            {"fish", "home", "낚시"},                            // 낚시 보정 + 길 기록 + 판매 (한 기능)
            {"move", "home", "이동"},
            {"stick", "move", "조이스틱"},
            {"base", "move", "기준 장소"},
    };

    static String parentOf(String id) {
        for (String[] p : PAGES) if (p[0].equals(id)) return p[1];
        return null;
    }

    static String pageName(String id) {
        for (String[] p : PAGES) if (p[0].equals(id)) return p[2];
        return id;
    }

    /** 기능 줄의 상태 한 줄 (준비가 덜 됐으면 주황) */
    String featState(String id) {
        switch (id) {
            case "fish":
                return !Hub.cfg.ready() ? "보정 필요" : routes() < 2 ? "길 기록 필요" : sellState();
            case "move":
                return moveReady() + " / 2 준비됨";
            case "stick":
                return Hub.stick != null ? "저장됨" : "없음";
            case "base":
                return count(Mover.BASE_KEYS) + " / " + Mover.BASE_KEYS.length + " 저장됨";
            default:
                return "";
        }
    }

    static String sellState() {
        return Guide.sellReady() ? "준비됨" : count(Seller.TALK) < Seller.TALK.length ? "대화 버튼 없음" : "판매 자동 보정 필요";
    }

    boolean featOk(String id) {
        switch (id) {
            case "fish":
                return Hub.cfg.ready() && routes() == 2 && Guide.sellReady();
            case "move":
                return moveReady() == 2;
            case "stick":
                return Hub.stick != null;
            case "base":
                return Guide.baseReady();
            default:
                return true;
        }
    }

    int moveReady() {
        int n = 0;
        for (String id : new String[]{"stick", "base"}) if (featOk(id)) n++;
        return n;
    }

    static int count(String[][] keys) {
        int n = 0;
        for (String[] k : keys) if (Hub.pos.containsKey(k[0])) n++;
        return n;
    }

    static int routes() {
        return (Hub.fishRoute.isEmpty() ? 0 : 1) + (Hub.sellRoute.isEmpty() ? 0 : 1);
    }

    String page = "home";
    LinearLayout pageBox;
    TextView dot, clock, bioState;
    Toggle bioSw;
    /** 지금 페이지에서 상태가 바뀌면 다시 쓸 것들 */
    final List<Runnable> live = new ArrayList<>();
    long runT0;
    boolean wasRun;
    int runCol;
    final Runnable tick = this::refresh;

    /** 패널 안 (위 줄 · 페이지) */
    void buildPanel() {
        int pad = Ui.dp(this, 14);
        panel.setPadding(pad, Ui.dp(this, 12), pad, Ui.dp(this, 12));
        panel.setBackground(new Ui.Deco(this, C_BG, C_LINE, 0, true, 0, 0));
        LinearLayout head = hrow();
        head.addView(Ui.text(this, "Acrux", 15, C_FG, true), new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f));
        dot = Ui.text(this, "●", 9, C_DIM, false);
        clock = mono("", 11, C_MUT);
        LinearLayout.LayoutParams cp = Ui.wrap();
        cp.leftMargin = Ui.dp(this, 6);
        head.addView(dot, Ui.wrap());
        head.addView(clock, cp);
        panel.addView(head, mw());
        pageBox = new LinearLayout(this);
        pageBox.setOrientation(LinearLayout.VERTICAL);
        LinearLayout.LayoutParams pp = new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f);
        pp.topMargin = Ui.dp(this, 8);
        panel.addView(pageBox, pp);
        showPage(page);
    }

    /** 페이지 바꾸기 (패널이 닫혀 있어도 다음에 열면 그 페이지) */
    void showPage(String id) {
        if (pageBox == null) return;
        if (!id.equals("home") && parentOf(id) == null) id = "home";
        page = id;
        pageBox.removeAllViews();
        live.clear();
        targets.clear();
        if (id.equals("home")) homePage(pageBox);
        else subPage(pageBox, id);
        refresh();
        Hub.main.post(this::applyGuide);
    }

    void homePage(LinearLayout box) {
        status = Ui.text(this, "대기", 11, C_MUT, false);
        status.setMaxLines(2);
        status.setEllipsize(android.text.TextUtils.TruncateAt.END);
        box.addView(status, mw());
        // 매크로 시작 · 정지 (크게)
        runBtn = reg("run", Ui.text(this, "매크로 시작", 14, 0xFFFFFFFF, true));
        runBtn.setGravity(Gravity.CENTER);
        runCol = 0;
        runBtn.setOnClickListener(v -> toggle("run"));
        LinearLayout.LayoutParams rp = new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, Ui.dp(this, 44));
        rp.topMargin = Ui.dp(this, 8);
        box.addView(runBtn, rp);
        // 바이옴 매크로 (그 밑에 작게 · 줄 아무 데나 누르면 켜고 끔)
        LinearLayout br = reg("biome", hrow());
        br.setPadding(0, Ui.dp(this, 7), 0, Ui.dp(this, 7));
        br.setBackground(press());
        br.setOnClickListener(v -> toggleBiome());
        br.addView(Ui.text(this, "바이옴 매크로", 11, C_FG, false), Ui.wrap());
        bioState = Ui.text(this, "꺼짐", 10, C_DIM, false);
        bioState.setSingleLine(true);
        bioState.setEllipsize(android.text.TextUtils.TruncateAt.END);
        LinearLayout.LayoutParams bp = new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f);
        bp.leftMargin = Ui.dp(this, 8);
        bp.rightMargin = Ui.dp(this, 8);
        br.addView(bioState, bp);
        bioSw = new Toggle(this);
        br.addView(bioSw, new LinearLayout.LayoutParams(Ui.dp(this, 28), Ui.dp(this, 14)));
        LinearLayout.LayoutParams brp = mw();
        brp.topMargin = Ui.dp(this, 2);
        box.addView(br, brp);
        box.addView(section("기능"), mw());
        LinearLayout list = scroll(box);
        for (String[] p : PAGES) if (p[1].equals("home")) featRow(list, p[0]);
    }

    void subPage(LinearLayout box, String id) {
        String up = parentOf(id);
        LinearLayout back = reg("back", hrow());
        back.setPadding(0, Ui.dp(this, 2), 0, Ui.dp(this, 8));
        back.setBackground(press());
        back.setOnClickListener(v -> showPage(up));
        LinearLayout.LayoutParams ap = Ui.wrap();
        ap.rightMargin = Ui.dp(this, 8);
        back.addView(Ui.text(this, "‹", 18, C_MUT, false), ap);
        back.addView(Ui.text(this, pageName(id), 13, C_FG, true), Ui.wrap());
        box.addView(back, mw());
        box.addView(hair(), hairLp());
        LinearLayout m = scroll(box);
        switch (id) {
            case "fish":
                pageFish(m);
                break;
            case "stick":
                pageStick(m);
                break;
            case "base":
                pageBase(m);
                break;
            default:                                            // 묶음 페이지 = 아래 기능 목록
                for (String[] p : PAGES) if (p[1].equals(id)) featRow(m, p[0]);
                break;
        }
    }

    /** 낚시 = 보정 + 길 기록 (낚시 장소 · 판매 장소) + 판매 — 자동 낚시 하나에 다 쓰임 */
    void pageFish(LinearLayout m) {
        m.addView(section("낚시 보정"), mw());
        info(m, () -> Hub.cfg.ready() ? "보정 완료" : "보정 필요 — 낚시 자리에서 Fish 버튼이 보일 때", () -> Hub.cfg.ready());
        act(m, "act:cal", "낚시 자동 보정", "Fish 를 한 번 던져 창 위치를 맞춤", true, v -> toggle("cal"));
        for (boolean sell : new boolean[]{false, true}) {
            m.addView(section((sell ? "물고기 판매 장소" : "낚시 장소") + " 길"), mw());
            info(m, () -> {
                com.acrux.mobile.core.Route r = sell ? Hub.sellRoute : Hub.fishRoute;
                return r.isEmpty() ? "기록 없음" : "기록됨 · " + fmt(r.seconds()) + "초 · " + r.parts() + "조각 · 점프 " + r.jumps() + "번";
            }, () -> !(sell ? Hub.sellRoute : Hub.fishRoute).isEmpty());
            act(m, sell ? "act:rec_sell" : "act:rec_fish", "길 기록", "기준 장소로 간 뒤 WASD 패드로 걸어감 → [도착]", true, v -> toggle(sell ? "rec_sell" : "rec_fish"));
        }
        m.addView(section("판매 · 대화"), mw());
        for (String[] k : Seller.TALK) pos(m, k[0], k[1], "판매 장소에 서서");
        m.addView(section("판매 · 상점"), mw());
        act(m, "act:sellcal", "판매 자동 보정", "NPC 앞에서 시작 → 대화 버튼은 직접", true, v -> toggle("sellcal"));
        info(m, () -> "판매 위치 " + count(Seller.KEYS) + " / " + Seller.KEYS.length + " 저장됨", () -> count(Seller.KEYS) == Seller.KEYS.length);
        for (String[][] k : new String[][][]{{{"first_fish_pos", "첫 번째 물고기 칸"}}, {{"shop_close_pos", "상점 닫기 X"}}}) {
            pos(m, k[0][0], k[0][1], "직접 지정 · 상점을 연 채로");
        }
        m.addView(section("테스트"), mw());
        act(m, "act:sell", "판매 한 번 해 보기", null, false, v -> toggle("sell"));
    }

    void pageStick(LinearLayout m) {
        m.addView(section("조이스틱 (W · A · S · D)"), mw());
        val(m, null, "조이스틱", "로블록스 설정 → 이동 방식: 조이스틱", () -> Hub.stick != null, v -> {
            if (Hub.stick == null) return;
            setDrawer(false);
            Hub.showStick(2500);
        });
        act(m, "act:stickfind", "조이스틱 찾기", "화면에서 자동으로", true, v -> toggle("stickfind"));
        act(m, null, "조이스틱 직접 지정", "가운데 → 가장자리", false, v -> pickStick());
        m.addView(section("점프"), mw());
        for (String[] k : Mover.JUMP_KEYS) pos(m, k[0], k[1], "길 기록에서 점프");
        m.addView(section("테스트"), mw());
        act(m, "act:sticktest", "조이스틱 테스트", "W · A · S · D 0.8초씩", false, v -> toggle("sticktest"));
        act(m, "act:jumptest", "점프 테스트", "제자리 → 걸으면서", false, v -> toggle("jumptest"));
    }

    void pageBase(LinearLayout m) {
        // VIP 가 있으면 걸음이 빨라서 W + A 를 짧게 누름 (기본 = VIP 있음 · 2.5초 · 없으면 3.5초)
        LinearLayout vr = reg("vip", hrow());
        vr.setPadding(0, Ui.dp(this, 8), 0, Ui.dp(this, 8));
        vr.setBackground(press());
        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);
        col.addView(Ui.text(this, "VIP 있음", 12.5f, C_FG, false), Ui.wrap());
        TextView vs = Ui.text(this, "", 10, C_DIM, false);
        col.addView(vs, Ui.wrap());
        vr.addView(col, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f));
        Toggle vt = new Toggle(this);
        vr.addView(vt, new LinearLayout.LayoutParams(Ui.dp(this, 28), Ui.dp(this, 14)));
        vr.setOnClickListener(v -> {
            Hub.move.vip = !Hub.move.vip;
            Hub.save();
            Hub.changed();
        });
        live.add(() -> {
            vt.set(Hub.move.vip);
            Ui.set(vs, "W + A " + fmt(Hub.move.wa()) + "초");
        });
        m.addView(vr, mw());
        m.addView(hair(), hairLp());
        m.addView(section("공통 위치 (기준 장소 맞추기)"), mw());
        for (String[] k : Mover.BASE_KEYS) pos(m, k[0], k[1], null);
        m.addView(section("테스트"), mw());
        act(m, "act:base", "기준 장소로", null, false, v -> toggle("base"));
        act(m, "act:fish", "기준 장소 → 낚시 장소", null, false, v -> toggle("fish"));
        act(m, "act:sellgo", "기준 장소 → 판매 장소", null, false, v -> toggle("sellgo"));
    }

    // ---- 줄 만들기

    /** 기능 줄: 이름 + 상태 한 줄 + › (누르면 그 페이지) */
    void featRow(LinearLayout list, String id) {
        LinearLayout r = reg("feat:" + id, hrow());
        r.setPadding(0, Ui.dp(this, 9), 0, Ui.dp(this, 9));
        r.setBackground(press());
        r.setOnClickListener(v -> showPage(id));
        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);
        col.addView(Ui.text(this, pageName(id), 13, C_FG, true), Ui.wrap());
        TextView s = Ui.text(this, featState(id), 10.5f, C_MUT, false);
        col.addView(s, Ui.wrap());
        r.addView(col, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f));
        r.addView(chev("›"), Ui.wrap());
        live.add(() -> {
            Ui.set(s, featState(id));
            s.setTextColor(featOk(id) ? C_MUT : C_WARN);
        });
        list.addView(r, mw());
        list.addView(hair(), hairLp());
    }

    /** 위치 줄: 이름 (+ 설명) · 저장됨 / 없음 · 누르면 포인터로 지정 */
    void pos(LinearLayout m, String key, String label, String hint) {
        val(m, "pos:" + key, label, hint, () -> Hub.pos.containsKey(key), v -> pickSave(key, label));
    }

    void val(LinearLayout m, String key, String label, String hint, java.util.function.BooleanSupplier ok, View.OnClickListener on) {
        LinearLayout r = hrow();
        if (key != null) reg(key, r);
        r.setPadding(0, Ui.dp(this, 8), 0, Ui.dp(this, 8));
        r.setBackground(press());
        r.setOnClickListener(on);
        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);
        col.addView(Ui.text(this, label, 12.5f, C_FG, false), Ui.wrap());
        if (hint != null) col.addView(Ui.text(this, hint, 10, C_DIM, false), Ui.wrap());
        r.addView(col, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f));
        TextView v = Ui.text(this, "", 11, C_DIM, false);
        LinearLayout.LayoutParams vp = Ui.wrap();
        vp.leftMargin = Ui.dp(this, 8);
        r.addView(v, vp);
        r.addView(chev("›"), Ui.wrap());
        live.add(() -> {
            boolean y = ok.getAsBoolean();
            Ui.set(v, y ? "저장됨" : "없음");
            v.setTextColor(y ? C_ACC : C_DIM);
        });
        m.addView(r, mw());
        m.addView(hair(), hairLp());
    }

    /** 실행 줄 (테두리 상자): 주요 = 파란 테두리 · 그 밖 = 회색 테두리 */
    void act(LinearLayout m, String key, String title, String hint, boolean primary, View.OnClickListener on) {
        LinearLayout r = hrow();
        if (key != null) reg(key, r);
        r.setPadding(Ui.dp(this, 10), Ui.dp(this, 8), Ui.dp(this, 10), Ui.dp(this, 8));
        android.graphics.drawable.StateListDrawable d = new android.graphics.drawable.StateListDrawable();
        d.addState(new int[]{android.R.attr.state_pressed}, new Ui.Deco(this, primary ? 0x334C8DFF : 0x14FFFFFF, primary ? C_ACC : C_MUT, 0, true, 0, 0));
        d.addState(new int[0], new Ui.Deco(this, 0, primary ? C_ACC : C_LINE, 0, true, 0, 0));
        r.setBackground(d);
        r.setOnClickListener(on);
        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);
        col.addView(Ui.text(this, title, 12.5f, primary ? C_ACC : C_FG, true), Ui.wrap());
        if (hint != null) col.addView(Ui.text(this, hint, 10, C_DIM, false), Ui.wrap());
        r.addView(col, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f));
        r.addView(chev("▸"), Ui.wrap());
        LinearLayout.LayoutParams p = mw();
        p.topMargin = Ui.dp(this, 6);
        m.addView(r, p);
    }

    /** 상태 한 줄 (됐으면 파랑 · 아니면 주황) */
    void info(LinearLayout m, java.util.function.Supplier<String> s, java.util.function.BooleanSupplier ok) {
        TextView t = Ui.text(this, "", 11, C_MUT, false);
        t.setPadding(0, Ui.dp(this, 2), 0, Ui.dp(this, 2));
        live.add(() -> {
            Ui.set(t, s.get());
            t.setTextColor(ok.getAsBoolean() ? C_ACC : C_WARN);
        });
        m.addView(t, mw());
    }

    TextView section(String s) {
        TextView t = Ui.text(this, s, 10, C_DIM, true);
        t.setPadding(0, Ui.dp(this, 12), 0, Ui.dp(this, 4));
        return t;
    }

    TextView chev(String s) {
        TextView t = Ui.text(this, s, 15, C_DIM, false);
        t.setPadding(Ui.dp(this, 8), 0, Ui.dp(this, 2), 0);
        return t;
    }

    TextView mono(String s, float sp, int color) {
        TextView t = Ui.text(this, s, sp, color, false);
        t.setTypeface(android.graphics.Typeface.MONOSPACE);
        return t;
    }

    LinearLayout hrow() {
        LinearLayout r = new LinearLayout(this);
        r.setOrientation(LinearLayout.HORIZONTAL);
        r.setGravity(Gravity.CENTER_VERTICAL);
        return r;
    }

    /** 남은 높이를 다 쓰는 스크롤 칸 (길면 위아래로 밀어서) */
    LinearLayout scroll(LinearLayout box) {
        ScrollView sv = new ScrollView(this);
        sv.setVerticalScrollBarEnabled(false);
        sv.setOverScrollMode(View.OVER_SCROLL_NEVER);
        LinearLayout m = new LinearLayout(this);
        m.setOrientation(LinearLayout.VERTICAL);
        sv.addView(m, new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.WRAP_CONTENT));
        box.addView(sv, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f));
        return m;
    }

    View hair() {
        View v = new View(this);
        v.setBackgroundColor(C_LINE);
        return v;
    }

    LinearLayout.LayoutParams hairLp() {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, Math.max(1, Ui.dp(this, 0.6f)));
    }

    static LinearLayout.LayoutParams mw() {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
    }

    static android.graphics.drawable.Drawable press() {
        android.graphics.drawable.StateListDrawable d = new android.graphics.drawable.StateListDrawable();
        d.addState(new int[]{android.R.attr.state_pressed}, new android.graphics.drawable.ColorDrawable(0x14FFFFFF));
        d.addState(new int[0], new android.graphics.drawable.ColorDrawable(0));
        return d;
    }

    /** 작은 켜기 · 끄기 (각진 모양 · 켜짐 = 파랑) */
    static final class Toggle extends View {
        final android.graphics.Paint p = new android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG);
        final float u;
        boolean on;

        Toggle(Context c) {
            super(c);
            u = c.getResources().getDisplayMetrics().density;
        }

        void set(boolean v) {
            if (v == on) return;
            on = v;
            invalidate();
        }

        @Override
        protected void onDraw(android.graphics.Canvas c) {
            float w = getWidth(), h = getHeight(), i = 2 * u, k = h - i * 2;
            p.setColor(on ? C_ACC : C_LINE);
            c.drawRect(0, 0, w, h, p);
            p.setColor(on ? 0xFFFFFFFF : C_MUT);
            float x = on ? w - i - k : i;
            c.drawRect(x, i, x + k, i + k, p);
        }
    }

    /** 안내가 강조할 곳 — 그 항목의 페이지면 항목 · 위 페이지면 그쪽 기능 줄 · 아니면 ‹ (돌아가기) */
    String guideTarget() {
        Guide.Step s = Guide.cur == null ? null : Guide.step();
        if (s == null || s.target == null) return null;
        String want = s.cat == null ? "home" : s.cat;
        if (want.equals(page)) return s.target;
        for (String c = want; c != null && !c.equals("home"); c = parentOf(c)) {
            if (page.equals(parentOf(c))) return "feat:" + c;
        }
        return "back";
    }

    void pickSave(String key, String label) {
        pick(label, Hub.pos.get(key), at -> {
            if (at == null) return;
            Hub.pos.put(key, at);
            Hub.save();
            Hub.log("'" + label + "' 위치 저장 (" + at[0] + ", " + at[1] + ")", 'g');
            if (Guide.cur != null) Hub.main.post(() -> setDrawer(true));   // 설정 안내 중이면 패널을 다시 열어 둠 (같은 페이지 · 다음 위치로 바로)
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
        if (root == null || panel == null || pageBox == null) return;
        boolean run = Hub.running();
        if (run && !wasRun) runT0 = SystemClock.elapsedRealtime();
        wasRun = run;
        boolean cap = CaptureService.active != null;
        // 위 줄: 점 (파랑 = 도는 중 · 주황 = 화면 캡처 꺼짐 · 회색 = 대기) + 실행 시간
        dot.setTextColor(run ? C_ACC : !cap ? C_WARN : C_DIM);
        if (run) {
            long t = (SystemClock.elapsedRealtime() - runT0) / 1000;
            String c = String.format(Locale.ROOT, "%02d:%02d:%02d", t / 3600, t / 60 % 60, t % 60);
            if (!c.contentEquals(clock.getText())) clock.setText(c);
        } else {
            Ui.set(clock, cap ? "대기" : "캡처 꺼짐");
        }
        Hub.main.removeCallbacks(tick);
        if (run && open) Hub.main.postDelayed(tick, 1000);
        if (status != null) Ui.set(status, !cap ? "화면 캡처 꺼짐 · 앱에서 시작" : Hub.state);
        if (runBtn != null) {
            boolean macro = !run || "run".equals(Hub.mode);
            Ui.set(runBtn, !run ? "매크로 시작" : macro ? "매크로 정지" : "정지");   // 보정 · 테스트 · 기록 중에도 [정지] 로 멈춤
            int col = run ? C_RED : C_ACC;
            if (col != runCol) {
                runCol = col;
                android.graphics.drawable.StateListDrawable d = new android.graphics.drawable.StateListDrawable();
                d.addState(new int[]{android.R.attr.state_pressed}, new Ui.Deco(this, (col & 0x00FFFFFF) | 0xB3000000, 0, 0, false, 0, 0));
                d.addState(new int[0], new Ui.Deco(this, col, 0, 0, false, 0, 0));
                runBtn.setBackground(d);
            }
        }
        if (bioState != null) {
            boolean bio = Biome.on();
            Ui.set(bioState, bio ? Biome.status : "꺼짐");
            bioState.setTextColor(!bio ? C_DIM : Biome.watching ? C_ACC : C_WARN);
            bioSw.set(bio);
        }
        for (Runnable r : live) r.run();
        arriveBtn.setVisibility(Hub.measuring ? View.VISIBLE : View.GONE);
        handle.setBar(run ? C_ACC : C_MUT);
        handleIn.setBar(run ? C_ACC : C_MUT);
    }

    static String fmt(double v) {
        return String.format(Locale.ROOT, "%.2f", v);
    }
}
