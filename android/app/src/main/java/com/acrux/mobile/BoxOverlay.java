package com.acrux.mobile;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.PixelFormat;
import android.os.SystemClock;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.view.WindowManager;

import com.acrux.mobile.core.Fisher;
import com.acrux.mobile.core.I18n;
import com.acrux.mobile.core.Mask;
import com.acrux.mobile.core.TestBoxes;

import java.util.ArrayList;
import java.util.List;

/**
 * 낚시 테스트: 자동 보정된 칸을 로블록스 화면 위에 그림 (매크로가 실제로 보는 자리 — TestBoxes)
 *  + 릴링 때 읽은 내 위치 (흰 세모) · 구간 (초록 줄) · 누른 자리 (흰 네모) · 위쪽 가운데에 지금 상태
 *  · 터치는 그대로 아래 게임으로 감 · 그린 자리는 화면 캡처에서 지움 (Mask — 인식에 안 섞이게)
 */
final class BoxOverlay extends View {
    static BoxOverlay shown;
    static int gen;
    /** 릴링 중 읽은 것 (아무 스레드에서) — 바 (화면 px x, y, 폭, 높이) · 내 위치 · 구간 (바 안 px) */
    static volatile int[] reelBar, reelZone;
    static volatile Double reelMarker;
    /** 누른 자리 {x, y, 끝 시각 ms} (최근 몇 개) */
    static final long[][] taps = new long[4][];
    static int tapNext;
    /** 테스트가 끝났을 때 결과 한 줄 (끝난 뒤 잠깐 위쪽에 보여 줌) */
    static volatile String endMsg;
    static final long TAP_MS = 700, END_MS = 3500;

    final Paint fill = new Paint(), text = new Paint(Paint.ANTI_ALIAS_FLAG), back = new Paint();
    final float density;
    final int[] loc = new int[2];
    final Runnable onChange = this::invalidate;

    BoxOverlay(Context c) {
        super(c);
        density = c.getResources().getDisplayMetrics().density;
        fill.setStyle(Paint.Style.FILL);
        fill.setAntiAlias(false);
        back.setStyle(Paint.Style.FILL);
        back.setAntiAlias(false);
        back.setColor(0xD9101214);
        text.setTextSize(TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, 11, c.getResources().getDisplayMetrics()));
        text.setFakeBoldText(true);
    }

    /** 낚시 테스트 시작: 칸 그리기 (메인 스레드) */
    static void show() {
        TapService ts = TapService.instance;
        if (ts == null || ts.wm == null) return;
        gen++;
        endMsg = null;
        reelBar = reelZone = null;
        reelMarker = null;
        synchronized (taps) {
            for (int i = 0; i < taps.length; i++) taps[i] = null;
        }
        if (shown != null) {
            shown.invalidate();
            return;
        }
        BoxOverlay v = new BoxOverlay(ts);
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
            ts.wm.addView(v, p);
            shown = v;
            Hub.listeners.add(v.onChange);
        } catch (RuntimeException e) {
            Hub.log("칸을 화면에 못 그림: " + e, 'r');
        }
    }

    /** 낚시 테스트 끝: 결과 한 줄을 잠깐 보여 주고 지움 (메인 스레드) */
    static void end(String msg) {
        if (shown == null) return;
        endMsg = msg;
        reelBar = null;
        shown.invalidate();
        int g = gen;
        Hub.main.postDelayed(() -> {
            if (g == gen) hide();
        }, END_MS);
    }

    static void hide() {
        BoxOverlay v = shown;
        shown = null;
        if (v == null) return;
        Hub.listeners.remove(v.onChange);
        TapService ts = TapService.instance;
        if (ts != null) ts.removeView(v);
        Mask.clear();
    }

    /** 누른 자리 (아무 스레드에서) */
    static void tapped(int x, int y) {
        synchronized (taps) {
            taps[tapNext] = new long[]{x, y, SystemClock.uptimeMillis() + TAP_MS};
            tapNext = (tapNext + 1) % taps.length;
        }
        BoxOverlay v = shown;
        if (v != null) {
            v.postInvalidateOnAnimation();
            v.postInvalidateDelayed(TAP_MS + 30);
        }
    }

    /** 릴링 중 읽은 것 → 다음 화면에 다시 그림 */
    static final Fisher.Probe PROBE = new Fisher.Probe() {
        @Override
        public void reel(int x, int y, int w, int h, Double marker, int[] zone) {
            reelBar = new int[]{x, y, w, h};
            reelMarker = marker;
            reelZone = zone == null ? null : zone.clone();
            BoxOverlay v = shown;
            if (v != null) v.postInvalidateOnAnimation();
        }

        @Override
        public void reelEnd() {
            reelBar = null;
            BoxOverlay v = shown;
            if (v != null) v.postInvalidateOnAnimation();
        }
    };

    @Override
    protected void onDraw(Canvas c) {
        TapService ts = TapService.instance;
        if (ts == null) return;
        int[] s = ts.screen();
        int W = s[0], H = s[1], t = TestBoxes.stroke(density), g = TestBoxes.gap(density), pad = Math.max(2, Math.round(2 * density));
        getLocationOnScreen(loc);
        c.save();
        c.translate(-loc[0], -loc[1]);                         // 화면 px 그대로 (캡처 · 매크로가 쓰는 자리)
        List<int[]> drawn = new ArrayList<>();
        Paint.FontMetrics fm = text.getFontMetrics();
        int th = (int) Math.ceil(fm.descent - fm.ascent);
        Fisher f = Hub.fisher;
        for (TestBoxes.Box b : TestBoxes.of(Hub.cfg, W, H, f != null && endMsg == null ? f.phase : 0)) {
            fill.setColor(b.color);
            for (int[] r : TestBoxes.strips(b, t, g)) rect(c, r, fill, drawn);
            if (b.labelAt == TestBoxes.L_NONE || b.label.isEmpty()) continue;
            String l = I18n.t(b.label);
            int[] lr = TestBoxes.labelRect(b, (int) Math.ceil(text.measureText(l)), th, pad, t, g, W, H);
            label(c, lr, l, b.color, pad, fm, drawn);
        }
        // 릴링: 내 위치 · 구간 (바 아래)
        int[] bar = reelBar;
        if (bar != null) {
            int[][] mk = TestBoxes.reelMarks(bar, reelMarker, reelZone, t);
            if (mk[1] != null) {
                fill.setColor(TestBoxes.C_ZONE);
                rect(c, mk[1], fill, drawn);
            }
            if (mk[0] != null) {
                int[] m = mk[0];
                fill.setColor(TestBoxes.C_MARK);
                // 바 쪽을 가리키는 세모 (안티에일리어싱 없이 계단으로 · 지울 자리가 정확하게)
                int steps = Math.max(2, (m[3] - m[1]) / Math.max(1, t));
                for (int i = 0; i < steps; i++) {
                    int y0 = m[1] + (m[3] - m[1]) * i / steps, y1 = m[1] + (m[3] - m[1]) * (i + 1) / steps;
                    int cx = (m[0] + m[2]) / 2, half = Math.max(1, (m[2] - m[0]) / 2 * (i + 1) / steps);
                    rect(c, new int[]{cx - half, y0, cx + half + 1, y1}, fill, drawn);
                }
            }
        }
        // 누른 자리
        long now = SystemClock.uptimeMillis();
        fill.setColor(TestBoxes.C_TAP);
        synchronized (taps) {
            for (long[] tp : taps) {
                if (tp == null || tp[2] < now) continue;
                for (int[] r : TestBoxes.tapMark((int) tp[0], (int) tp[1], H, t)) rect(c, r, fill, drawn);
            }
        }
        // 위쪽 가운데: 지금 상태 · 끝났으면 결과
        String em = endMsg;
        String st = em != null ? I18n.f("낚시 테스트 끝 — {0}", I18n.t(em)) : I18n.f("낚시 테스트 · {0}", I18n.t(Hub.state));
        int tw = (int) Math.ceil(text.measureText(st));
        int maxW = W - pad * 8;
        if (tw > maxW) {
            st = st.substring(0, Math.max(1, text.breakText(st, true, maxW - text.measureText("…"), null))) + "…";
            tw = (int) Math.ceil(text.measureText(st));
        }
        int sw = tw + pad * 4, sh = th + pad * 2, sx = (W - sw) / 2, sy = Math.max(pad, Math.round(6 * density));
        // 한 판을 끝냈으면 초록 · 칸 문제로 끝났으면 노랑
        boolean done = em != null && (em.equals("성공") || em.equals("쓰레기") || em.equals("실패"));
        label(c, new int[]{sx, sy, sx + sw, sy + sh}, st, em == null ? 0xFFFFFFFF : done ? TestBoxes.C_FISH : TestBoxes.C_REEL, pad * 2, fm, drawn);
        c.restore();
        Mask.set(drawn.toArray(new int[0][]));
    }

    /** 직사각형 하나 그리고 지울 자리로 넣음 (1 px 넉넉히) */
    static void rect(Canvas c, int[] r, Paint p, List<int[]> drawn) {
        if (r[2] <= r[0] || r[3] <= r[1]) return;
        c.drawRect(r[0], r[1], r[2], r[3], p);
        drawn.add(new int[]{r[0] - 1, r[1] - 1, r[2] + 1, r[3] + 1});
    }

    /** 글자 (어두운 바탕 위) — 글자 가장자리는 부드럽게 그려져서 지울 자리를 2 px 넉넉히 */
    void label(Canvas c, int[] r, String s, int color, int padX, Paint.FontMetrics fm, List<int[]> drawn) {
        c.drawRect(r[0], r[1], r[2], r[3], back);
        text.setColor(color);
        float base = r[1] + (r[3] - r[1] - (fm.descent - fm.ascent)) / 2f - fm.ascent;
        c.drawText(s, r[0] + padX, base, text);
        drawn.add(new int[]{r[0] - 2, r[1] - 2, r[2] + 2, r[3] + 2});
    }
}
