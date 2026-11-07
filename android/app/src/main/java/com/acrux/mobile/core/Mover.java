package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * 이동 — PC 버전 move.py 를 그대로 옮김 (낚시 · 판매에 쓰는 부분)
 * 기준 장소에서 화면의 한 점을 눌러(Click to Move · 우클릭) 원하는 장소로 걸어감
 * 외부 앱이라 게임 안 좌표를 모르므로, 매번 '같은 화면'을 만든 뒤 그 화면의 같은 점을 누름:
 *   1. 기준 장소: 리셋(Esc → R → Enter) → / · 채팅 · 도감 열고 닫기 · / · Enter (카메라 정렬)
 *      → W 1초 → W+A 7초 → A 1초 (0.75초 뒤부터 W 0.25초 같이) → S+D 0.25초 → 우클릭 드래그(위에서 내려다보기) + O 2.5초 (최대 줌)
 *   2. 장소마다 지점 목록: [누를 곳, 걸리는 시간] — 지점이 여러 개면 앞 지점에 도착한 화면에서 다음 지점을 누름
 *   3. 걸리는 시간은 직접 잼: 누른 순간부터 [도착]을 누를 때까지
 */
public final class Mover {
    /** 글자 읽기 (OCR): 화면의 비율 영역 [x1, y1, x2, y2] (null = 전체) → 덩어리 (위치는 화면 비율) */
    public interface Ocr {
        List<SellText.Box> read(double[] region);
    }

    /** 기준 장소로 가는 방법 (PC MOVE_DEFAULT 와 같은 이름 · 같은 기본값) */
    public static final class Cfg {
        public double resetWait = 3.5, wTime = 1.0, waTime = 7.0, aTime = 1.0, w2Time = 0.25, sdTime = 0.25, oTime = 2.5, margin = 0.3;
        public int tiltPx = 800;
        public String button = "right";          // 이동할 곳을 누를 마우스 버튼 (Click to Move)
    }

    /** 장소의 지점: 기준 장소 화면에서 누를 곳 [x, y] · 걸린 시간 (초 · 아직 안 쟀으면 null) */
    public static final class Point {
        public double[] pos;
        public Double time;

        public Point(double[] pos, Double time) {
            this.pos = pos;
            this.time = time;
        }
    }

    public static final String[][] BASE_KEYS = {{"chat_pos", "채팅 버튼"}, {"collection_pos", "도감 버튼"}, {"collection_close", "도감 Exit"}};
    public static final String[] MOVE_KEYS = {"w", "a", "s", "d", "o", "space", "shift", "e"};
    public static final double MEASURE_MAX = 180.0;
    /** 방금 리셋하고 안 움직였음 (PC macro.reset_fresh · 리셋 쿨타임 때문에 또 리셋하지 않음) */
    static volatile boolean resetFresh = false;

    final Fisher.Screen scr;
    final Pc pc;
    final Ocr ocr;
    final Map<String, double[]> base;     // 공통 위치 (채팅 · 도감 · 도감 Exit)
    final Cfg cfg;
    final Fisher.Out out;
    final Runnable check;                 // 멈춤 확인 (멈췄으면 Fisher.Stopped)

    public Mover(Fisher.Screen scr, Pc pc, Ocr ocr, Map<String, double[]> base, Cfg cfg, Fisher.Out out, Runnable check) {
        this.scr = scr;
        this.pc = pc;
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

    /** PC Mover._click */
    public void click(double[] pos, String button) {
        if (button.equals(cfg.button)) markMoved();          // 갈 곳 클릭 (Click to Move)
        int[] p = px(pos);
        pc.click(p[0], p[1], button);
    }

    public void click(double[] pos) {
        click(pos, "left");
    }

    /** PC Seller._press: 게임 버튼 누르기 — 로블록스 버튼이 마우스를 인식하도록 근처로 옮겼다가 누름 (바로 누르면 씹힐 때가 있음) */
    public void press(double[] pos) {
        int[] p = px(pos);
        pc.moveTo(p[0] + 3, p[1] + 3);
        sleep(0.05);
        pc.click(p[0], p[1], "left");
    }

    public void releaseKeys() {
        for (String k : MOVE_KEYS) {
            try {
                pc.keyUp(k);
            } catch (RuntimeException ignored) {
            }
        }
        try {
            pc.button("right", false);               // 화면 돌리던 우클릭이 눌린 채 남지 않게
        } catch (RuntimeException ignored) {
        }
    }

    // ---- 미리 확인
    public String baseMissing() {
        List<String> miss = new ArrayList<>();
        for (String[] k : BASE_KEYS) if (!base.containsKey(k[0])) miss.add(k[1]);
        return miss.isEmpty() ? null : "먼저 지정 필요: " + String.join(", ", miss) + " (Acrux 창 → ⚙ → 공통 위치)";
    }

    /** 장소 지점들(upto 앞까지) 중 위치 · 시간이 없는 것 → 안내 글 또는 null */
    public static String pathMissing(String name, List<Point> pts, Integer upto) {
        int n = upto != null ? Math.min(upto, pts.size()) : pts.size();
        for (int i = 0; i < n; i++) {
            Point pt = pts.get(i);
            if (pt.pos == null) return name + " · " + (i + 1) + "번 지점 위치가 없음";
            if (pt.time == null) return name + " · " + (i + 1) + "번 지점 시간을 먼저 재야 함";
        }
        return null;
    }

    // ---- 1. 기준 장소
    /** key 를 sec 초 동안 계속 누르고, 끝나기 extraSec 초 전부터 extra 도 같이 누름 (key 는 중간에 안 뗌) */
    void holdWith(String key, double sec, String extra, double extraSec) {
        markMoved();
        sec = Math.max(0, sec);
        extraSec = Math.max(0, extraSec);
        double start = now(), end = start + Math.max(sec, extraSec), extraAt = start + Math.max(0, sec - extraSec);
        List<String> keys = new ArrayList<>();
        keys.add(key);
        try {
            pc.keyDown(key);
            while (now() < end) {
                check.run();
                if (!keys.contains(extra) && now() >= extraAt) {
                    keys.add(extra);
                    pc.keyDown(extra);
                }
                double until = !keys.contains(extra) ? Math.min(end, extraAt) : end;
                Pc.nap((long) (Math.min(0.033, Math.max(0, until - now())) * 1000));
                if (now() < end) for (String k : keys) pc.keyDown(k);       // 자동 반복 (A 는 계속 눌린 채로)
            }
        } finally {
            for (int i = keys.size() - 1; i >= 0; i--) pc.keyUp(keys.get(i));
        }
    }

    /** 키들을 sec 초 동안 누르고 있다가 뗌 (멈추면 바로 뗌) · 진짜 키보드처럼 누르는 동안 '누름' 신호를 계속 다시 보냄 (약 30번/초) */
    void hold(String[] keys, double sec) {
        for (String k : keys) {
            if ((k.equals("w") || k.equals("a") || k.equals("s") || k.equals("d") || k.equals("space")) && sec > 0) {
                markMoved();
                break;
            }
        }
        try {
            for (String k : keys) pc.keyDown(k);
            double end = now() + Math.max(0, sec);
            while (now() < end) {
                check.run();
                Pc.nap((long) (Math.min(0.033, Math.max(0, end - now())) * 1000));
                if (now() < end) for (String k : keys) pc.keyDown(k);
            }
        } finally {
            for (int i = keys.length - 1; i >= 0; i--) pc.keyUp(keys[i]);
        }
    }

    /** O 키(최대 줌)를 누르고 있는 동안 동시에 우클릭 드래그로 화면을 내려다보게 함 · 드래그는 20px 씩 10ms 마다, 다 끌면 우클릭을 뗌 */
    void tiltAndZoom(int px, double sec) {
        pc.moveTo(scr.width() / 2, scr.height() / 2);
        sleep(0.1);
        boolean dragging = px > 0;
        try {
            pc.keyDown("o");
            if (dragging) pc.button("right", true);
            double start = now(), end = start + Math.max(0, sec), nextKey = start + 0.033;
            int done = 0;
            while (true) {
                check.run();
                double t = now();
                if (dragging) {
                    if (done < px) {
                        int step = Math.min(20, px - done);
                        pc.moveRel(0, step);
                        done += step;
                    } else {
                        pc.button("right", false);
                        dragging = false;
                    }
                }
                if (t >= end && !dragging) break;
                if (t >= nextKey && t < end) {
                    pc.keyDown("o");                   // 자동 반복
                    nextKey = t + 0.033;
                }
                Pc.nap(10);
            }
        } finally {
            if (dragging) pc.button("right", false);
            pc.keyUp("o");
        }
    }

    /** / → 채팅 버튼 → 도감 버튼 → 도감 Exit → / → Enter (각 0.5초) — 도감을 열고 닫으면 카메라가 캐릭터 뒤로 맞춰짐 */
    public void alignCamera() {
        String m = baseMissing();
        if (m != null) throw new RuntimeException(m);
        pc.keyTap("/");
        sleep(0.5);
        click(base.get("chat_pos"));
        sleep(0.5);
        click(base.get("collection_pos"));
        sleep(0.5);
        click(base.get("collection_close"));
        sleep(0.5);
        pc.keyTap("/");
        sleep(0.5);
        pc.keyTap("enter");
        sleep(0.3);
    }

    void reset() {
        releaseKeys();
        sleep(0.2);
        if (resetFresh) {
            out.log("방금 리셋함 — 리셋 건너뜀", 'd');
            return;
        }
        for (String k : new String[]{"esc", "r", "enter"}) {
            pc.keyTap(k);
            sleep(0.5);
        }
        resetFresh = true;
        sleep(Math.max(0, cfg.resetWait - 0.5));
    }

    /** Esc → R → Enter → 3.5초 → 카메라 정렬 → W 1초 → W+A 7초 → A 1초 (끝 0.25초 W 같이) → S+D 0.25초 → 0.5초 → 내려다보기 + 최대 줌 */
    public void goBase() {
        String m = baseMissing();
        if (m != null) throw new RuntimeException(m);
        out.state("기준 장소로 이동 · 리셋");
        reset();
        out.state("기준 장소로 이동 · 카메라 정렬 (채팅 · 도감)");
        alignCamera();
        out.state("기준 장소로 이동 · 걷기 (W → W+A → A → W → S+D)");
        hold(new String[]{"w"}, cfg.wTime);
        hold(new String[]{"w", "a"}, cfg.waTime);
        holdWith("a", cfg.aTime, "w", cfg.w2Time);
        hold(new String[]{"s", "d"}, cfg.sdTime);
        sleep(0.5);
        out.state("기준 장소로 이동 · 내려다보기 + 최대 줌 (우클릭 드래그 + O)");
        tiltAndZoom(cfg.tiltPx, cfg.oTime);
        sleep(0.3);
    }

    // ---- 2. 장소로
    /** 지점들을 차례로 (upto 앞까지) 누르고 각 지점의 시간만큼 기다림 */
    public void walk(String name, List<Point> pts, Integer upto) {
        int n = upto != null ? Math.min(upto, pts.size()) : pts.size();
        for (int i = 0; i < n; i++) {
            Point pt = pts.get(i);
            if (pt.pos == null) throw new RuntimeException(name + " · " + (i + 1) + "번 지점 위치가 없음");
            if (pt.time == null) throw new RuntimeException(name + " · " + (i + 1) + "번 지점 시간을 먼저 재야 함");
            out.state(name + " 가는 중 (" + (i + 1) + "/" + pts.size() + ")");
            click(pt.pos, cfg.button);
            sleep(pt.time + cfg.margin);
        }
    }

    public void goPlace(String name, List<Point> pts) {
        goBase();
        walk(name, pts, null);
        out.log(name + " 도착", 'g');
    }

    // ---- 3. 시간 재기
    /** 지점 j 를 눌러 놓고, 도착(arrived 가 true 가 될 때)까지 걸린 시간 · fromBase: 기준 장소부터 (아니면 지금 화면에서 바로) */
    public double measure(String name, List<Point> pts, int j, boolean fromBase, java.util.function.BooleanSupplier arrived) {
        if (j < 0 || j >= pts.size() || pts.get(j).pos == null) throw new RuntimeException("지점 위치를 먼저 지정");
        if (fromBase) {
            goBase();
            walk(name, pts, j);
        }
        String label = name + " · " + (j + 1) + "번 지점";
        click(pts.get(j).pos, cfg.button);
        double t0 = now();
        out.state(label + "까지 걷는 중 — 도착하면 [도착]");
        out.log(label + " 시간 재는 중 — 도착하면 Acrux 창의 [도착]", 'c');
        while (!arrived.getAsBoolean()) {
            check.run();
            if (now() - t0 > MEASURE_MAX) throw new RuntimeException((int) MEASURE_MAX + "초 안에 도착을 안 누름 — 시간 재기 취소");
            Pc.nap(10);
        }
        double sec = Math.round((now() - t0) * 100) / 100.0;
        pts.get(j).time = sec;
        out.log(label + " 걸린 시간 " + sec + "초 저장", 'g');
        return sec;
    }

    static double now() {
        return System.nanoTime() / 1e9;
    }
}
