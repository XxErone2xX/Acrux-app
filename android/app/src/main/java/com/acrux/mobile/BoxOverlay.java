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
import com.acrux.mobile.core.SellText;
import com.acrux.mobile.core.TestBoxes;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;

/**
 * 매크로가 보는 것 · 하는 것을 로블록스 화면 위에 그림 — 낚시 테스트 · 디버깅 모드 (모든 기능 · 바이옴 매크로)
 *  · 모든 기능: 위쪽 가운데 지금 상태 한 줄 · 누른 자리 (흰 꺾쇠) · 글자 읽기로 읽은 줄 (주황 꺾쇠) · 조이스틱 손가락 · 점프
 *  · 낚시 (자동 낚시 · 낚시 테스트 · 낚시 자동 보정): 보정된 칸 (매크로가 실제로 보는 자리 — TestBoxes · 지금 단계에 보는 칸만)
 *    · 릴링: 읽은 내 위치 (흰 세모) · 구간 (초록 줄) · 예측 (파란 세모 — 지금 누르면 반영될 때 내 위치) · 누름 기준 (노란 줄 — 예측이 이보다 왼쪽이면 누름)
 *    · 낚시 창에서 읽은 칸은 밝게 · 못 읽은 칸은 흐리게 (Fish 버튼 · 미니게임 바 · ◇) — 칸이 실제 창과 어긋났는지 보이게
 *    · 옆 칸 (낚시 칸들의 가로 범위 바깥): 이번 실행 결과 · 낚시 창에서 읽은 것 · 릴링 기록 · 릴링 표시 설명 · 바이옴
 *  · 위쪽 가운데는 한 줄만 (바로 아래가 결과창 제목 · X) · 이동 · 판매 · 보정 중엔 옆 칸을 안 그림 (대화 · 상점 글자를 읽는 자리를 가릴 수 있어서)
 *  · 터치는 그대로 아래 게임으로 감 · 그린 자리는 화면 캡처에서 지움 (Mask — 인식에 안 섞이게)
 */
final class BoxOverlay extends View {
    static BoxOverlay shown;
    static int gen;
    /** 실행이 막 끝나서 끝난 이유를 보여 주려는 중 (그 사이 sync 가 칸 창을 지우지 않게 · 일하는 스레드가 켬) */
    static volatile boolean ending;
    /** 칸 창에 보여 주는 기능 (Hub.mode · 끝난 뒤 결과를 보여 주는 동안에도 남김) */
    static volatile String runMode = "";
    /** 릴링 중 읽은 것 (아무 스레드에서) — 바 (화면 px x, y, 폭, 높이) · 내 위치 · 구간 (바 안 px) */
    static volatile int[] reelBar, reelZone;
    static volatile Double reelMarker;
    /** 릴링 조작 — 예측 · 누름 기준 (바 폭 비율 · NaN 없음) · 기록 (Fisher.Probe.pilot) · 지난 릴링 기록 */
    static volatile double reelPred = Double.NaN, reelAim = Double.NaN;
    static volatile double[] reelStats, lastStats;
    /** 누른 자리 {x, y, 끝 시각 ms} (최근 몇 개) */
    static final long[][] taps = new long[8][];
    static int tapNext;
    /** 글자 읽기 (OCR) 로 마지막에 읽은 줄들 {가운데 x, 가운데 y, 폭, 높이} (화면 비율) · 읽은 시각 (ms) */
    static volatile double[][] ocrBoxes;
    static volatile long ocrAt;
    /** 끝났을 때 결과 한 줄 · 색 (끝난 뒤 잠깐 위쪽에 보여 줌) */
    static volatile String endMsg;
    static volatile int endColor;
    static final long TAP_MS = 700, END_MS = 3500, OCR_MS = 1200;
    static final int C_TEXT = 0xFFFFFFFF, C_INFO = 0xFFC9CED5;
    /** Hub.changed 마다 칸 창을 상태에 맞춤 (TapService 가 등록) */
    static final Runnable SYNC = BoxOverlay::sync;

    final Paint fill = new Paint(), text = new Paint(Paint.ANTI_ALIAS_FLAG), back = new Paint(), small = new Paint(Paint.ANTI_ALIAS_FLAG);
    final float density, smallPx;
    final int[] loc = new int[2];
    final Runnable onChange = this::invalidate;
    /** 실행 중: 읽은 것 · 조이스틱 · 글자 읽기가 바뀌는 걸 보이게 다시 그림 (하나만 걸어 둠) */
    final Runnable tick = this::invalidate;

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
        smallPx = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, 10, c.getResources().getDisplayMetrics());
        small.setTextSize(smallPx);
        small.setFakeBoldText(true);
    }

    // ---------------------------------------------------------------- 띄우기 · 지우기 (메인 스레드)
    /** 칸 창이 있어야 하나: 낚시 테스트 중 · 디버깅 모드 + (아무 기능이나 실행 중 · 바이옴 매크로 켜짐) */
    static boolean wanted() {
        boolean run = Hub.running();
        return run && "fishtest".equals(Hub.mode) || Hub.debug && (run || Biome.on());
    }

    /** 칸 창을 상태에 맞춤 — 끝난 이유를 보여 주는 동안은 그대로 */
    static void sync() {
        String m = Hub.mode;
        if (Hub.running() && !m.isEmpty()) runMode = m;
        if (wanted()) {
            if (shown == null) show();
        } else if (shown != null && endMsg == null && !ending) {
            hide();
        }
    }

    /** 새 실행 시작 (Hub.start) — 전 실행의 결과 · 릴링 · 누른 자리를 지우고 상태에 맞춤 */
    static void started() {
        String m = Hub.mode;
        if (!m.isEmpty()) runMode = m;
        if (shown != null) reset();
        sync();
    }

    static void show() {
        TapService ts = TapService.instance;
        if (ts == null || ts.wm == null) return;
        reset();
        if (shown != null) return;
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

    /** 새로 그리기 시작 (전 실행의 결과 · 릴링 · 글자 읽기 · 누른 자리를 지움) */
    static void reset() {
        gen++;
        endMsg = null;
        ending = false;
        reelBar = reelZone = null;
        reelMarker = null;
        reelPred = reelAim = Double.NaN;
        reelStats = lastStats = null;
        ocrBoxes = null;
        synchronized (taps) {
            Arrays.fill(taps, null);
        }
        if (shown != null) shown.invalidate();
    }

    /** 실행 끝: 결과 한 줄을 잠깐 보여 준 뒤 지움 (디버깅 모드 + 바이옴 매크로가 켜져 있으면 바이옴 상태로 돌아감) · 안 떠 있으면 아무것도 안 함 */
    static void end(String msg, int color) {
        ending = false;
        if (shown == null) return;
        endMsg = msg;
        endColor = color;
        reelBar = null;
        shown.invalidate();
        int g = gen;
        Hub.main.postDelayed(() -> {
            if (g != gen) return;
            endMsg = null;
            if (!wanted()) hide();
            else if (shown != null) shown.invalidate();
        }, END_MS);
    }

    static void hide() {
        BoxOverlay v = shown;
        shown = null;
        ending = false;
        if (v == null) return;
        gen++;
        v.removeCallbacks(v.tick);
        Hub.listeners.remove(v.onChange);
        TapService ts = TapService.instance;
        if (ts != null) ts.removeView(v);
        Mask.clear();
    }

    // ---------------------------------------------------------------- 매크로가 넘기는 것 (아무 스레드에서)
    /** 누른 자리 */
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

    /** 글자 읽기 (OCR) 결과 — 읽은 줄마다 칸 (화면 비율) */
    static void ocr(List<SellText.Box> boxes) {
        double[][] a = new double[boxes.size()][];
        for (int i = 0; i < a.length; i++) {
            SellText.Box b = boxes.get(i);
            a[i] = new double[]{b.x, b.y, b.w, b.h};
        }
        ocrBoxes = a;
        ocrAt = SystemClock.uptimeMillis();
        BoxOverlay v = shown;
        if (v != null) {
            v.postInvalidateOnAnimation();
            v.postInvalidateDelayed(OCR_MS + 30);
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
        public void pilot(double pred, double target, double[] s) {
            reelPred = pred;
            reelAim = target;
            reelStats = s;
        }

        @Override
        public void reelEnd() {
            reelBar = null;
            reelPred = reelAim = Double.NaN;
            lastStats = reelStats;
            reelStats = null;
            BoxOverlay v = shown;
            if (v != null) v.postInvalidateOnAnimation();
        }
    };

    // ---------------------------------------------------------------- 그리기
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
        long now = SystemClock.uptimeMillis();
        Fisher f = Hub.fisher;
        String em = endMsg;
        boolean running = Hub.running();
        // 보여 줄 기능: 도는 기능 · 끝난 결과를 보여 주는 동안은 끝난 기능 · 아무것도 안 돌면 "" (바이옴 매크로만)
        String mode = running ? Hub.mode : em != null ? runMode : "";
        boolean test = mode.equals("fishtest"), cal = mode.equals("cal");
        boolean fishing = f != null && (mode.equals("run") || test || cal);
        int phase = !fishing ? 4 : em != null ? 0 : f.phase;
        // 낚시 창에서 읽은 것 (대기 · 모름 단계에서 계속 읽음) → 읽은 칸은 밝게 · 못 읽은 칸은 흐리게
        boolean look = fishing && !cal && f.debug && em == null && (phase == 0 || phase == 1);
        String btn = fishing ? f.seenBtn : null;
        boolean barSeen = fishing && f.seenBar;
        int[] dia = fishing ? f.seenDia : null;
        if (fishing) {
            for (TestBoxes.Box b : TestBoxes.of(Hub.cfg, W, H, phase)) {
                if (cal && b.kind.equals("learned")) continue;     // 보정 중: 전에 기억한 X 자리는 안 그림 (지금 창과 다를 수 있음)
                int col = look && !seen(b.kind, btn, barSeen, dia) ? dim(b.color) : b.color;
                fill.setColor(col);
                for (int[] r : TestBoxes.strips(b, t, g)) rect(c, r, fill, drawn);
                if (b.labelAt == TestBoxes.L_NONE || b.label.isEmpty()) continue;
                String l = I18n.t(b.label);
                int[] lr = TestBoxes.labelRect(b, (int) Math.ceil(text.measureText(l)), th, pad, t, g, W, H);
                label(c, lr, l, col, pad, fm, drawn);
            }
        }
        // 릴링: 구간 · 누름 기준 · 예측 · 내 위치 (바 아래 · 릴링 때 잡는 칸 밖)
        int[] bar = reelBar;
        if (fishing && bar != null) {
            int[][] mk = TestBoxes.reelMarks(bar, reelMarker, reelZone, reelPred, reelAim, t);
            if (mk[1] != null) {
                fill.setColor(TestBoxes.C_ZONE);
                rect(c, mk[1], fill, drawn);
            }
            if (mk[3] != null) {
                fill.setColor(TestBoxes.C_AIM);
                rect(c, mk[3], fill, drawn);
            }
            if (mk[2] != null) {
                fill.setColor(TestBoxes.C_PRED);
                for (int[] r : TestBoxes.triangle(mk[2], t)) rect(c, r, fill, drawn);
            }
            if (mk[0] != null) {
                fill.setColor(TestBoxes.C_MARK);
                for (int[] r : TestBoxes.triangle(mk[0], t)) rect(c, r, fill, drawn);
            }
        }
        // 글자 읽기 (판매 · 이동): 읽은 줄마다 둘레 꺾쇠 (글자 위에는 안 그림 · 읽은 뒤 잠깐)
        double[][] ob = ocrBoxes;
        if (running && ob != null && now - ocrAt < OCR_MS) {
            fill.setColor(TestBoxes.C_OCR);
            int og = Math.max(2, t);
            for (double[] o : ob) {
                int x0 = (int) ((o[0] - o[2] / 2) * W), y0 = (int) ((o[1] - o[3] / 2) * H), x1 = (int) ((o[0] + o[2] / 2) * W), y1 = (int) ((o[1] + o[3] / 2) * H);
                for (int[] r : TestBoxes.ocrMark(x0, y0, x1, y1, og, Math.max(2, t - 1))) rect(c, r, fill, drawn);
            }
        }
        // 조이스틱을 누르고 있으면 판 (네모) + 손가락 자리 · 점프 버튼을 누르고 있으면 그 자리
        boolean moving = false;
        if (running) {
            double[] fg = Joy.finger(), jf = Joy.jumpFinger();
            double jr = Joy.r;
            if (fg != null && jr > 0) {
                fill.setColor(TestBoxes.C_STICK);
                for (int[] r : TestBoxes.stickRing(Joy.cx, Joy.cy, jr, t)) rect(c, r, fill, drawn);
                fill.setColor(TestBoxes.C_MARK);
                rect(c, TestBoxes.finger(fg[0], fg[1], Math.max(4, (int) (jr * 0.12))), fill, drawn);
                moving = true;
            }
            if (jf != null) {
                fill.setColor(TestBoxes.C_STICK);
                for (int[] r : TestBoxes.tapMark((int) jf[0], (int) jf[1], H, t)) rect(c, r, fill, drawn);
                moving = true;
            }
        }
        // 누른 자리
        fill.setColor(TestBoxes.C_TAP);
        synchronized (taps) {
            for (long[] tp : taps) {
                if (tp == null || tp[2] < now) continue;
                for (int[] r : TestBoxes.tapMark((int) tp[0], (int) tp[1], H, t)) rect(c, r, fill, drawn);
            }
        }
        // 위쪽 가운데: 지금 상태 · 끝났으면 결과 (한 줄)
        String st;
        int stc = C_TEXT;
        if (em != null) {
            st = I18n.f(test ? "낚시 테스트 끝 — {0}" : "디버깅 끝 — {0}", I18n.t(em));
            stc = endColor;
        } else if (running) {
            st = I18n.f(test ? "낚시 테스트 · {0}" : "디버깅 · {0}", I18n.t(Hub.state));
        } else {
            st = I18n.f("디버깅 · 바이옴 매크로 · {0}", biome());
        }
        int tw = (int) Math.ceil(text.measureText(st));
        int maxW = W - pad * 8;
        if (tw > maxW) {
            st = st.substring(0, Math.max(1, text.breakText(st, true, maxW - text.measureText("…"), null))) + "…";
            tw = (int) Math.ceil(text.measureText(st));
        }
        int sw = tw + pad * 4, sh = th + pad * 2, sx = (W - sw) / 2, sy = Math.max(pad, Math.round(6 * density));
        label(c, new int[]{sx, sy, sx + sw, sy + sh}, st, stc, pad * 2, fm, drawn);
        // 옆 칸 (낚시만): 결과 · 읽은 것 · 릴링 · 바이옴 — 이동 · 판매 (단계 4) · 보정 중엔 안 그림
        if (fishing && !cal && phase != 4) side(c, lines(f, em, test, phase, btn, barSeen, dia), W, H, t, g, pad, th, drawn);
        c.restore();
        Mask.set(drawn.toArray(new int[0][]));
        removeCallbacks(tick);
        if (running && em == null) postDelayed(tick, moving ? 66 : 150);
        else if (em == null && Biome.waiting()) postDelayed(tick, 1000);    // 바이옴 안전 모드 남은 초
    }

    /** 바이옴 매크로 상태 한 줄 (지금 바이옴 포함) */
    static String biome() {
        if (Biome.waiting()) return I18n.t("안전 모드 " + Biome.safeLeft() + "초");
        String cur = Biome.current;
        return I18n.t(Biome.status) + (cur != null && !cur.isEmpty() ? " · " + cur : "");
    }

    /** 옆 칸의 줄들 — 줄마다 {글자, 색, 글자, 색, …} (짧게 · 옆 칸 폭이 좁아서) */
    List<Object[]> lines(Fisher f, String em, boolean test, int phase, String btn, boolean barSeen, int[] dia) {
        List<Object[]> out = new ArrayList<>();
        if (!test) out.add(new Object[]{I18n.f("성공 {0} · 쓰레기 {1} · 실패 {2} (놓침 {3})", f.success, f.junk, f.fail, f.missed), C_TEXT});
        if (em != null) return out;
        double[] rs = reelStats;
        if (reelBar != null && rs != null) {
            out.add(new Object[]{I18n.f("구간 안 {0}% · 터치 {1}번", Math.round(rs[1]), Math.round(rs[2])), C_INFO});
            out.add(new Object[]{I18n.f("지연 {0}초 · 화면 {1}fps", d2(rs[0]), Math.round(rs[3])), C_INFO});
            out.add(new Object[]{"▲ " + I18n.t("내 위치"), TestBoxes.C_MARK});
            out.add(new Object[]{"▲ " + I18n.t("예측"), TestBoxes.C_PRED});
            out.add(new Object[]{"| " + I18n.t("누름 기준"), TestBoxes.C_AIM});
            out.add(new Object[]{"— " + I18n.t("구간"), TestBoxes.C_ZONE});
            return out;
        }
        if (f.debug && (phase == 0 || phase == 1)) {
            String b = btn == null ? I18n.t("안 보임") : btn.equals("fish") ? "Fish" : btn.equals("exit") ? "Exit" : btn;
            out.add(new Object[]{I18n.f("Fish 버튼: {0}", b), btn != null ? TestBoxes.C_FISH : C_INFO});
            out.add(new Object[]{I18n.f("미니게임 바: {0}", I18n.t(barSeen ? "보임" : "안 보임")), barSeen ? TestBoxes.C_BAR : C_INFO});
            if (dia != null) {
                String d = I18n.t(dia[0] >= 6 && dia[1] >= 6 ? "두 자리 다" : dia[1] >= 6 ? "미니게임 자리" : dia[0] >= 6 ? "대기 자리" : "안 보임");
                out.add(new Object[]{"◇: " + d, dia[0] >= 6 || dia[1] >= 6 ? TestBoxes.C_DIA : C_INFO});
            }
        }
        double[] ls = lastStats;
        if (ls != null) {
            if (ls[7] > 0) {
                out.add(new Object[]{I18n.f("지난 릴링: 구간 안 {0}% · 터치 {1}번", Math.round(ls[1]), Math.round(ls[2])), C_INFO});
                out.add(new Object[]{I18n.f("지연 {0}초 · 화면 {1}fps", d2(ls[0]), Math.round(ls[3])), C_INFO});
            } else {
                out.add(new Object[]{I18n.t("지난 릴링: 미니게임 아님 (내 위치가 안 움직임)"), TestBoxes.C_REEL});
                out.add(new Object[]{I18n.f("바 화면 {0}장 · 내 위치 읽음 {1}장 · 구간 읽음 {2}장", Math.round(ls[4]), Math.round(ls[5]), Math.round(ls[6])), C_INFO});
            }
        }
        if (Biome.on()) out.add(new Object[]{I18n.f("바이옴 매크로: {0}", biome()), C_INFO});
        return out;
    }

    static String d2(double v) {
        return String.format(Locale.ROOT, "%.2f", v);
    }

    /** 읽은 칸인지 (낚시 창에서 Fish 버튼 · 미니게임 바 · ◇ 를 읽었나 · 다른 칸은 늘 밝게) */
    static boolean seen(String kind, String btn, boolean bar, int[] dia) {
        switch (kind) {
            case "fish":
                return btn != null;
            case "bar":
                return bar;
            case "dia_idle":
                return dia != null && dia[0] >= 6;
            case "dia_reel":
                return dia != null && dia[1] >= 6;
            default:
                return true;
        }
    }

    /** 흐린 색 (반투명) */
    static int dim(int c) {
        return (c & 0x00FFFFFF) | 0x66000000;
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

    /**
     * 옆 칸: 여러 줄 (줄마다 글자 조각 + 색) 을 바탕 하나에 — 지울 자리도 하나
     *  (줄마다 따로 그리면 붙은 칸끼리 바깥 칸을 못 찾아서 화면 캡처에서 다 안 지워질 수 있음)
     *  · 자리: 낚시 칸들의 가로 범위 바깥 (TestBoxes.sideRange) · 화면 높이 36% 부터 (오른쪽 위 순위표 아래 · 줄 수가 바뀌어도 위치는 그대로)
     *  · 폭이 모자라면 글자를 줄이고 그래도 길면 … 로 자름 · 너무 좁으면 안 그림
     */
    void side(Canvas c, List<Object[]> lines, int W, int H, int t, int g, int pad, int th, List<int[]> drawn) {
        int n = lines.size();
        if (n == 0) return;
        int[] range = TestBoxes.sideRange(Hub.cfg, W, H, t, g, pad, th, Math.max(pad * 3, Math.round(10 * density)),
                l -> (int) Math.ceil(text.measureText(I18n.t(l))));
        int room = range[1] - range[0] - pad * 4;
        if (room < Math.round(90 * density)) return;
        small.setTextSize(smallPx);
        float w = widest(lines);
        if (w > room) small.setTextSize(Math.max(smallPx * 0.75f, smallPx * room / w));
        Paint.FontMetrics fm = small.getFontMetrics();
        int lh = (int) Math.ceil(fm.descent - fm.ascent), gapL = Math.max(1, pad / 2);
        w = 0;
        for (int i = 0; i < n; i++) {
            Object[] l = lines.get(i);
            float sum = 0;
            for (int k = 0; k < l.length; k += 2) sum += small.measureText((String) l[k]);
            if (sum > room) {
                StringBuilder sb = new StringBuilder();
                for (int k = 0; k < l.length; k += 2) sb.append((String) l[k]);
                String one = sb.toString();
                one = one.substring(0, Math.max(1, small.breakText(one, true, room - small.measureText("…"), null))) + "…";
                l = new Object[]{one, l[1]};
                lines.set(i, l);
                sum = small.measureText(one);
            }
            w = Math.max(w, sum);
        }
        int bw = (int) Math.ceil(w) + pad * 4, bh = n * lh + (n - 1) * gapL + pad * 2;
        boolean right = range[0] > W / 2;
        int x = right ? range[1] - bw : range[0], y = Math.max(pad, Math.min(H - bh - pad, (int) (H * 0.36)));
        c.drawRect(x, y, x + bw, y + bh, back);
        float base = y + pad - fm.ascent;
        for (int i = 0; i < n; i++) {
            Object[] l = lines.get(i);
            float xx = x + pad * 2;
            for (int k = 0; k < l.length; k += 2) {
                String seg = (String) l[k];
                small.setColor((Integer) l[k + 1]);
                c.drawText(seg, xx, base, small);
                xx += small.measureText(seg);
            }
            base += lh + gapL;
        }
        drawn.add(new int[]{x - 2, y - 2, x + bw + 2, y + bh + 2});
    }

    float widest(List<Object[]> lines) {
        float w = 0;
        for (Object[] l : lines) {
            float sum = 0;
            for (int k = 0; k < l.length; k += 2) sum += small.measureText((String) l[k]);
            w = Math.max(w, sum);
        }
        return w;
    }
}
