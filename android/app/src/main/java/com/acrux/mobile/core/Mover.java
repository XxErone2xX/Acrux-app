package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.function.BooleanSupplier;

/**
 * 이동 — 화면 터치만 씀 (루팅 · Shizuku 없음)
 *   1. 기준 장소 (스폰이 랜덤이라 어디서 시작하든 같은 곳으로 모음):
 *      재설정 (로블록스 메뉴 → 캐릭터 재설정 → 재설정 확인 · 탭) → 도감 열고 닫기 (화면 맞추기)
 *      → 조이스틱 W+A 3.5초 → A 3초 → A+W 2초 → 0.5초 쉼 → D 1초
 *      → 최대 줌 아웃 (두 손가락을 위아래로 오므리기) → 내려다보기 (화면을 아래로 끌기)
 *   2. 기준 장소 → 목적지: 플레이어가 조이스틱으로 직접 걸어간 길(Route) 을 그대로 다시 걸음
 *      길 기록: 기준 장소에 도착하면 '지금부터 기록' → 화면에서 조이스틱 손잡이를 매 프레임 읽음 → [도착]
 */
public final class Mover {
    /** 글자 읽기 (OCR): 화면의 비율 영역 [x1, y1, x2, y2] (null = 전체) → 덩어리 (위치는 화면 비율) */
    public interface Ocr {
        List<SellText.Box> read(double[] region);
    }

    /** 기준 장소로 가는 방법 */
    public static final class Cfg {
        public double resetWait = 3.5, waTime = 3.5, aTime = 3.0, awTime = 2.0, pauseTime = 0.5, dTime = 1.0, margin = 0.3;
        /** 내려다보기: 화면을 아래로 끄는 횟수 · 줌 아웃: 두 손가락 오므리는 횟수 */
        public int tiltSwipes = 3, zoomPinches = 4;
    }

    /** 공통 위치 (화면 비율 · ⚙ 에서 한 번 지정) */
    public static final String[][] BASE_KEYS = {
            {"menu_btn", "로블록스 메뉴 버튼 (왼쪽 위)"}, {"reset_btn", "캐릭터 재설정 버튼 (메뉴 안)"}, {"reset_confirm", "재설정 확인 버튼"},
            {"collection_pos", "도감 버튼"}, {"collection_close", "도감 Exit"}};
    public static final double RECORD_MAX = 300.0;
    /** 방금 리셋하고 안 움직였음 (리셋 쿨타임 때문에 또 리셋하지 않음) */
    static volatile boolean resetFresh = false;

    final Fisher.Screen scr;
    final Touch touch;
    final Ocr ocr;
    final Map<String, double[]> base;
    final Cfg cfg;
    final Fisher.Out out;
    final Runnable check;                 // 멈춤 확인 (멈췄으면 Fisher.Stopped)

    public Mover(Fisher.Screen scr, Touch touch, Ocr ocr, Map<String, double[]> base, Cfg cfg, Fisher.Out out, Runnable check) {
        this.scr = scr;
        this.touch = touch;
        this.ocr = ocr;
        this.base = base;
        this.cfg = cfg;
        this.out = out;
        this.check = check;
    }

    public static void markMoved() {
        resetFresh = false;
    }

    // ---- 도우미
    public void sleep(double sec) {
        long end = System.nanoTime() + (long) (Math.max(0, sec) * 1e9);
        while (true) {
            check.run();
            long left = end - System.nanoTime();
            if (left <= 0) return;
            try {
                Thread.sleep(Math.max(1, Math.min(30, left / 1_000_000)));
            } catch (InterruptedException e) {
                throw new Fisher.Stopped();
            }
        }
    }

    public int[] px(double[] pos) {
        return new int[]{(int) (pos[0] * scr.width()), (int) (pos[1] * scr.height())};
    }

    /** 게임 화면 버튼 누르기 (손가락 탭 · ms 동안) */
    public void tapUi(double[] pos, int ms) {
        int[] p = px(pos);
        touch.tap(p[0], p[1], ms);
        sleep(ms / 1000.0);
    }

    public void tapUi(double[] pos) {
        tapUi(pos, 150);
    }

    /** 조이스틱 놓기 */
    public void releaseKeys() {
        try {
            touch.stick(null);
        } catch (RuntimeException ignored) {
        }
    }

    // ---- 미리 확인
    public String baseMissing() {
        List<String> miss = new ArrayList<>();
        if (touch.stickAt() == null) miss.add("조이스틱");
        for (String[] k : BASE_KEYS) if (!base.containsKey(k[0])) miss.add(k[1]);
        return miss.isEmpty() ? null : "먼저 지정 필요: " + String.join(", ", miss) + " (Acrux 창 → ⚙)";
    }

    // ---- 1. 기준 장소
    /** W A S D → 조이스틱 방향 · sec 초 동안 밀고 있음 (멈추면 바로 뗌) */
    void hold(String[] keys, double sec) {
        boolean w = false, a = false, s = false, d = false;
        for (String k : keys) {
            if (k.equals("w")) w = true;
            else if (k.equals("a")) a = true;
            else if (k.equals("s")) s = true;
            else if (k.equals("d")) d = true;
        }
        double[] v = Stick.dir(w, a, s, d);
        if (v == null || sec <= 0) return;
        markMoved();
        try {
            touch.stick(v);
            sleep(sec);
        } finally {
            touch.stick(null);
        }
    }

    /**
     * 최대 줌 아웃 → 내려다보기
     *  · 줌 아웃: 화면 가운데에서 두 손가락(위아래)을 오므리기 여러 번 — 화면이 좌우로 돌지 않게 세로로만
     *  · 내려다보기: 화면 오른쪽 가운데를 위 → 아래로 여러 번 끌기 (줌 아웃 중에 위아래로 조금 기운 것도 여기서 다시 맞춰짐)
     */
    void lookDown() {
        int W = scr.width(), H = scr.height();
        for (int i = 0; i < cfg.zoomPinches; i++) {
            touch.pinch((int) (W * 0.55), (int) (H * 0.5), (int) (H * 0.6), (int) (H * 0.06), 300);
            sleep(0.6);
        }
        for (int i = 0; i < cfg.tiltSwipes; i++) {
            touch.swipe((int) (W * 0.62), (int) (H * 0.22), (int) (W * 0.62), (int) (H * 0.80), 250);
            sleep(0.4);
        }
    }

    /** 도감 버튼 → 도감 Exit (각 0.5초) — 도감을 열고 닫으면 카메라가 캐릭터 뒤로 맞춰짐 */
    public void alignCamera() {
        String m = baseMissing();
        if (m != null) throw new RuntimeException(m);
        tapUi(base.get("collection_pos"));
        sleep(0.5);
        tapUi(base.get("collection_close"));
        sleep(0.5);
    }

    /** 재설정: 로블록스 메뉴 → 캐릭터 재설정 → 재설정 확인 (탭 · 0.8초 간격) */
    void reset() {
        releaseKeys();
        sleep(0.2);
        if (resetFresh) {
            out.log("방금 리셋함 — 리셋 건너뜀", 'd');
            return;
        }
        out.state("기준 장소로 이동 · 재설정 (메뉴)");
        tapUi(base.get("menu_btn"));
        sleep(0.8);
        out.state("기준 장소로 이동 · 재설정 (캐릭터 재설정)");
        tapUi(base.get("reset_btn"));
        sleep(0.8);
        out.state("기준 장소로 이동 · 재설정 (확인)");
        tapUi(base.get("reset_confirm"));
        out.log("재설정 누름 (메뉴 → 캐릭터 재설정 → 확인)", 'd');
        resetFresh = true;
        sleep(cfg.resetWait);
    }

    /** 재설정 → 도감 열고 닫기 → W+A → A → A+W → 쉼 → D → 줌 아웃 + 내려다보기 */
    public void goBase() {
        String m = baseMissing();
        if (m != null) throw new RuntimeException(m);
        reset();
        out.state("기준 장소로 이동 · 카메라 정렬 (도감)");
        alignCamera();
        out.state("기준 장소로 이동 · 걷기 (W+A → A → A+W → 쉼 → D)");
        hold(new String[]{"w", "a"}, cfg.waTime);
        hold(new String[]{"a"}, cfg.aTime);
        hold(new String[]{"a", "w"}, cfg.awTime);
        sleep(cfg.pauseTime);
        hold(new String[]{"d"}, cfg.dTime);
        sleep(0.5);
        out.state("기준 장소로 이동 · 줌 아웃 + 내려다보기");
        lookDown();
        sleep(0.3);
    }

    // ---- 2. 기록한 길 따라가기
    public void walk(String name, Route r) {
        if (r == null || r.isEmpty()) throw new RuntimeException(name + " 길을 먼저 기록해야 함 (Acrux 창 → ⚙ → " + name + " 길 기록)");
        markMoved();
        out.state(name + " 가는 중 (" + Math.round(r.seconds()) + "초)");
        try {
            for (Route.Seg g : r.segs) {
                touch.stick(g.idle() ? null : new double[]{g.dx, g.dy});
                sleep(g.sec);
            }
        } finally {
            touch.stick(null);
        }
        sleep(cfg.margin);
    }

    public void goPlace(String name, Route r) {
        goBase();
        walk(name, r);
        out.log(name + " 도착", 'g');
    }

    // ---- 3. 길 기록
    /**
     * 기준 장소에 간 뒤 플레이어가 조이스틱으로 직접 걸어가는 동안 조이스틱 손잡이를 매 프레임 읽어서 길로 저장 · arrived 가 true 가 되면 끝
     */
    public Route record(String name, BooleanSupplier arrived) {
        double[] st = touch.stickAt();
        if (st == null) throw new RuntimeException("먼저 조이스틱 찾기 (Acrux 창 → ⚙ → 조이스틱 찾기)");
        goBase();
        int x0 = (int) (st[0] - st[2] * 1.6), y0 = (int) (st[1] - st[2] * 1.6), sz = (int) (st[2] * 3.2);
        out.state(name + " 길 기록 중 — 조이스틱으로 걸어가서 도착하면 [도착]");
        out.log(name + " 길 기록 시작 — 지금부터 조이스틱 움직임을 저장 · 도착하면 Acrux 창의 [도착]", 'c');
        Route.Recorder rec = new Route.Recorder();
        double t0 = now();
        double[][] last = new double[3][];
        int frames = 0, seen = 0;
        while (!arrived.getAsBoolean()) {
            check.run();
            double t = now() - t0;
            if (t > RECORD_MAX) throw new RuntimeException((int) RECORD_MAX + "초 안에 [도착]을 안 누름 — 기록 취소");
            Img im = scr.grab(x0, y0, sz, sz);
            double[] v = null;
            if (im != null && im.w > 0) {
                // 잘린 화면 안에서의 판 가운데 (화면 가장자리에서 잘렸을 수 있음)
                double[] k = Stick.knob(im, st[0] - Math.max(0, x0), st[1] - Math.max(0, y0), st[2]);
                frames++;
                if (k != null) {
                    v = new double[]{k[0], k[1]};
                    if (Math.hypot(k[0], k[1]) >= 0.2) seen++;
                }
            }
            // 3프레임 중 2프레임이 같은 쪽이면 그 값 (한 프레임 튀는 건 무시)
            last[0] = last[1];
            last[1] = last[2];
            last[2] = Route.quant(v);
            rec.add(t, steady(last));
            Touch.nap(25);
        }
        rec.add(now() - t0, null);
        Route r = rec.done();
        out.log(name + " 길 기록 끝: " + r.segs.size() + "조각 · " + Math.round(r.seconds() * 10) / 10.0 + "초 (프레임 " + frames + " · 움직임 " + seen + ")",
                r.isEmpty() ? 'y' : 'g');
        if (r.isEmpty()) throw new RuntimeException("조이스틱 움직임을 못 읽음 — 조이스틱 찾기를 다시 하거나, 조이스틱을 끝까지 밀어서 걸어 주세요");
        return r;
    }

    static double[] steady(double[][] l) {
        if (l[0] == null || l[1] == null) return l[2];
        if (same(l[1], l[2]) || same(l[0], l[2])) return l[2];
        if (same(l[0], l[1])) return l[1];
        return l[2];
    }

    static boolean same(double[] a, double[] b) {
        return a != null && b != null && a[0] == b[0] && a[1] == b[1];
    }

    static double now() {
        return System.nanoTime() / 1e9;
    }
}
