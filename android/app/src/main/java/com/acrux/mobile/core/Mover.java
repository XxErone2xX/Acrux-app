package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.function.BooleanSupplier;

/**
 * 이동 — 화면 터치만 씀 (루팅 · Shizuku 없음)
 *   1. 기준 장소 (스폰이 랜덤이라 어디서 시작하든 같은 곳으로 모음):
 *      재설정 (로블록스 메뉴 → 캐릭터 재설정 → 재설정 확인 · 탭) → 최대 줌 아웃 (두 손가락) → 도감 열고 닫기 (화면 맞추기)
 *      → 조이스틱 W+A 2.5초 (VIP 없으면 3.5초) → A 3초 → A+W 2초 → 0.5초 쉼 → D 1초
 *      → 내려다보기 (화면을 아래로 끌기)
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
        /** VIP 가 있으면 걸음이 빨라서 W + A 를 짧게 (기본 = VIP 있음) */
        public boolean vip = true;
        public double waVip = 2.5;

        /** 이번에 W + A 를 누를 시간 (VIP 있음 = waVip · 없음 = waTime) */
        public double wa() {
            return vip ? waVip : waTime;
        }
    }

    /** 공통 위치 (화면 비율 · ⚙ 에서 한 번 지정) */
    public static final String[][] BASE_KEYS = {
            {"menu_btn", "로블록스 메뉴 버튼 (왼쪽 위)"}, {"reset_btn", "캐릭터 재설정 버튼 (메뉴 안)"}, {"reset_confirm", "재설정 확인 버튼"},
            {"collection_pos", "도감 버튼"}, {"collection_close", "도감 Exit"}};
    /** 점프 버튼 (길 기록 · 재생에서 점프) — 없어도 됨 (그러면 점프는 기록 안 함) */
    public static final String[][] JUMP_KEYS = {{"jump_btn", "점프 버튼 (오른쪽 아래)"}};
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
        return miss.isEmpty() ? null : "먼저 지정 필요: " + String.join(", ", miss) + " (Acrux 창)";
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
        // 누른 시간을 정확히 (전에는 뗄 때 0.03 ~ 0.13초씩 들쭉날쭉해서 기준 장소가 매번 조금씩 달랐음) · + HOLD_EXTRA: 전에 평균으로 더 가던 만큼 (예전 길과 같은 자리)
        List<Route.Ev> evs = new ArrayList<>();
        evs.add(new Route.Ev('F', 0, v[0], v[1], true, false));
        evs.add(new Route.Ev('X', (sec + HOLD_EXTRA) * 1000, 0, 0, false, false));
        try {
            touch.play(evs, null, check);
        } finally {
            touch.stick(null);
        }
    }

    static final double HOLD_EXTRA = 0.08;

    /**
     * 최대 줌 아웃: 화면 가운데에서 두 손가락(위아래)을 오므리기 여러 번
     * 로블록스는 두 손가락 줌을 하는 동안 화면도 같이 돌려 버림 (한쪽 손가락 움직임을 화면 돌리기로 봄)
     * → 줌 아웃은 도감 열고 닫기(카메라 정렬) '전에' 함: 정렬이 돌아간 화면을 캐릭터 뒤로 되돌리고, 줌 거리는 그대로 남음
     */
    void zoomOut() {
        int W = scr.width(), H = scr.height();
        for (int i = 0; i < cfg.zoomPinches; i++) {
            touch.pinch((int) (W * 0.55), (int) (H * 0.5), (int) (H * 0.6), (int) (H * 0.06), 300);
            sleep(0.6);
        }
    }

    /** 내려다보기: 화면 오른쪽 가운데를 위 → 아래로 여러 번 끌기 (세로로만 끌어서 좌우 방향은 안 바뀜) */
    void lookDown() {
        int W = scr.width(), H = scr.height();
        for (int i = 0; i < cfg.tiltSwipes; i++) {
            touch.swipe((int) (W * 0.62), (int) (H * 0.22), (int) (W * 0.62), (int) (H * 0.80), 250);
            sleep(0.4);
        }
    }

    /** 도감 버튼 → 도감 Exit (각 0.5초) — 도감을 열고 닫으면 카메라가 캐릭터 뒤로 맞춰짐 */
    public void alignCamera() {
        String m = baseMissing();
        if (m != null) throw new RuntimeException(m);
        double[] open = base.get("collection_pos"), exit = base.get("collection_close");
        // 도감을 열면 가운데에 'Loading Assets...' 가 뜨는 동안은 [Exit] 를 눌러도 씹힘 → 화면 글자를 읽어서
        //  1. 도감이 열렸는지 ([Exit] · Collections 글자) · 2. Loading Assets 가 사라졌는지 확인한 뒤 [Exit]
        //  3. 닫혔는지 확인 · 안 닫혔으면 다시 [Exit] (글자를 못 읽는 기기면 예전처럼 시간으로)
        double[] exitBox = {Math.max(0, exit[0] - 0.14), Math.max(0, exit[1] - 0.06), Math.min(1, exit[0] + 0.08), Math.min(1, exit[1] + 0.06)};
        double[] center = {0.3, 0.25, 0.7, 0.7};
        tapUi(open);
        double t0 = now(), ready = -1;
        int ocrFails = 0;
        boolean seenOpen = false;
        while (now() - t0 < 15) {
            sleep(0.35);
            Boolean opened = hasText(exitBox, "exit", "collection");
            Boolean loading = hasText(center, "loading", "assets");
            if (opened == null || loading == null) {
                if (++ocrFails >= 3) break;                       // 글자 읽기가 안 되는 기기 → 시간으로
                continue;
            }
            if (opened) seenOpen = true;
            if (opened && !loading) {
                if (ready < 0) ready = now();
                else if (now() - ready >= 0.3) break;             // 0.3초 넘게 그대로면 다 열림
            } else {
                ready = -1;
                if (loading) out.state("기준 장소로 이동 · 도감 Loading Assets 기다리는 중");
            }
            // 6초 동안 [Exit] 글자를 한 번도 못 읽음 (글자가 작거나 읽기 실패) · 로딩도 안 보임 → 시간으로
            if (!seenOpen && !loading && now() - t0 > 6) {
                ocrFails = 3;
                break;
            }
        }
        if (ocrFails >= 3) {
            sleep(1.5);                                           // 예전 방식 (조금 더 기다림)
            tapUi(exit);
            sleep(0.8);
            tapUi(exit);
            sleep(0.5);
            return;
        }
        for (int i = 0; i < 5; i++) {
            tapUi(exit);
            sleep(0.6);
            Boolean opened = hasText(exitBox, "exit", "collection");
            if (opened == null || !opened) {
                if (i > 0) out.log("도감 닫힘", 'd');
                return;                                           // 닫힘
            }
            Boolean loading = hasText(center, "loading", "assets");
            if (Boolean.TRUE.equals(loading)) {                   // 아직 로딩 중 → 끝날 때까지 기다렸다가 다시
                out.state("기준 장소로 이동 · 도감 Loading Assets 기다리는 중");
                double w0 = now();
                while (now() - w0 < 8 && Boolean.TRUE.equals(hasText(center, "loading", "assets"))) sleep(0.3);
            }
            out.log("도감 [Exit] 가 씹힘 — 다시 누름 (" + (i + 1) + "/5)", 'y');
        }
        out.log("도감이 안 닫힘 — 그대로 진행", 'r');
    }

    /** 화면 영역에 이 글자들 중 하나가 보이나 (대소문자 무시) · 글자 읽기 실패면 null */
    Boolean hasText(double[] region, String... words) {
        if (ocr == null) return null;
        List<SellText.Box> bs;
        try {
            bs = ocr.read(region);
        } catch (RuntimeException e) {
            return null;
        }
        if (bs == null) return null;
        for (SellText.Box b : bs) {
            String t = b.text == null ? "" : b.text.toLowerCase(java.util.Locale.ROOT);
            for (String w : words) if (t.contains(w)) return true;
        }
        return false;
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

    /** 재설정 → 도감 열고 닫기 → W+A → A → A+W → 쉼 → D → 내려다보기 (줌 아웃은 정렬 전에) */
    public void goBase() {
        String m = baseMissing();
        if (m != null) throw new RuntimeException(m);
        reset();
        out.state("기준 장소로 이동 · 줌 아웃");
        zoomOut();
        out.state("기준 장소로 이동 · 카메라 정렬 (도감)");
        alignCamera();
        out.state("기준 장소로 이동 · 걷기 (W+A → A → A+W → 쉼 → D)");
        hold(new String[]{"w", "a"}, cfg.wa());
        hold(new String[]{"a"}, cfg.aTime);
        hold(new String[]{"a", "w"}, cfg.awTime);
        sleep(cfg.pauseTime);
        hold(new String[]{"d"}, cfg.dTime);
        sleep(0.5);
        out.state("기준 장소로 이동 · 내려다보기");
        lookDown();
        sleep(0.3);
    }

    static volatile boolean oldWarned;

    // ---- 2. 기록한 길 따라가기
    public void walk(String name, Route r) {
        if (r == null || (r.isEmpty() && !r.hasPath())) throw new RuntimeException(name + " 길을 먼저 기록해야 함 (Acrux 창 → " + name + " 길 기록)");
        markMoved();
        out.state(name + " 가는 중 (" + Math.round(r.seconds()) + "초)");
        int[] jp = r.jumps() > 0 ? jumpPos() : null;
        if (r.jumps() > 0 && jp == null) out.log("점프 버튼 위치가 없어서 길의 점프는 건너뜀 (Acrux 창 → 점프 버튼)", 'y');
        if (r.isEmpty() && r.hasPath()) {                     // 시간대로 걸을 조각이 없을 때만 화면 보면서 따라가기
            follow(name, r, jp);
            sleep(cfg.margin);
            return;
        }
        if (!r.evs.isEmpty()) {                               // 손가락 움직임을 기록한 길 → 같은 시각에 그대로
            try {
                touch.play(jp != null ? r.evs : Route.noJump(r.evs), jp, check);
            } finally {
                touch.stick(null);
                if (jp != null) touch.jumpUp();
            }
            sleep(cfg.margin);
            return;
        }
        if (!oldWarned) {
            oldWarned = true;
            out.log("예전 방식으로 기록한 길 — 다시 기록하면 간 거리가 기록과 똑같아짐 (길)", 'y');
        }
        // V0.10.13 전에 기록한 길은 '뗌' 이 없음 → 점프는 전부 톡 누르기로
        boolean legacy = true;
        for (Route.Seg g : r.segs) if (g.jumpUp) legacy = false;
        try {
            for (Route.Seg g : r.segs) {
                touch.stick(g.idle() ? null : new double[]{g.dx, g.dy});
                if (jp != null && g.jumpUp && !g.jump) touch.jumpUp();
                if (jp != null && g.jump) {
                    touch.jumpDown(jp[0], jp[1]);
                    if (g.jumpUp || legacy) {                         // 톡 누르고 바로 뗀 점프 (한 조각 안에서)
                        double hold = Math.min(0.12, g.sec);
                        sleep(hold);
                        touch.jumpUp();
                        sleep(Math.max(0, g.sec - hold));
                        continue;
                    }
                }
                sleep(g.sec);
            }
        } finally {
            touch.stick(null);
            if (jp != null) touch.jumpUp();
        }
        sleep(cfg.margin);
    }

    // ---- 2-1. 화면을 보면서 길 따라가기 (어긋나도 스스로 길로 돌아옴)
    /** 도착으로 보는 거리 (화면 px) · 앞을 보는 거리 · 길에서 이만큼 벗어나도 진행으로 봄 */
    static final double FOLLOW_DONE = 14, LOOKAHEAD = 45;

    /**
     * 기록한 위치 길(Route.pts) 을 따라감 — 프레임마다 화면으로 캐릭터 위치를 재고(Odo) → 길의 '앞쪽 점' 을 향해 조이스틱을 고쳐 밂
     *  · 렉 · 부딪힘 · 프레임 밀림으로 늦거나 빨라도 시간 대신 '길에서 어디까지 왔나' 로 따라감
     *  · 점프는 기록 때 그 점프를 누른 자리를 지날 때 누르고 · 뗀 자리에서 뗌
     *  · 화면으로 위치를 못 재는 순간 (단색 바닥 · 물) 은 그 자리에서 기록한 조이스틱 방향 그대로
     *  · 너무 오래 걸리면 (기록 시간 × 2 + 8초) 멈추고 이어서 진행
     */
    void follow(String name, Route r, int[] jp) {
        Odo odo = new Odo(scr);
        odo.step();
        List<double[]> P = r.pts;
        int last = P.size() - 1, k = 0, fired = -1;
        double t0 = now(), limit = (P.get(last)[0] - P.get(0)[0]) * 2 + 8;
        double theta = r.theta;
        // 조이스틱 방향 → 실제로 움직인 방향 차이를 걸으면서 계속 맞춤 (0.2초 전 명령 vs 지금 움직임)
        java.util.ArrayDeque<double[]> hist = new java.util.ArrayDeque<>();
        int lost = 0, frames = 0;
        boolean jumpHeld = false;
        out.log(name + " 길 따라가기 (화면 보면서 · " + P.size() + "점)", 'd');
        try {
            while (true) {
                check.run();
                boolean seen = odo.step();
                frames++;
                if (!seen) lost++;
                double x = odo.x, y = odo.y;
                // 길에서 어디까지 왔나: 지금 자리 ~ 40점 앞까지 중 가장 가까운 점 (뒤로는 안 감)
                int bk = k;
                double bd = Double.MAX_VALUE;
                for (int j = k; j <= Math.min(last, k + 40); j++) {
                    double d = Math.hypot(P.get(j)[1] - x, P.get(j)[2] - y);
                    if (d <= bd + 0.5) {                        // 거의 같으면 더 앞 점 (같은 자리에 선 점들)
                        bd = d;
                        bk = j;
                    }
                }
                k = bk;
                // 지나온 점들의 점프
                for (int j = fired + 1; j <= k; j++) {
                    int f = (int) P.get(j)[3];
                    if (jp == null || f == 0) continue;
                    if ((f & 1) != 0) {
                        touch.jumpDown(jp[0], jp[1]);
                        jumpHeld = true;
                    }
                    if ((f & 2) != 0) {
                        if ((f & 1) != 0) sleep(0.1);
                        touch.jumpUp();
                        jumpHeld = false;
                    }
                }
                fired = Math.max(fired, k);
                if (k >= last && bd <= FOLLOW_DONE) break;
                if (now() - t0 > limit) {
                    out.log(name + " 길 따라가기가 너무 오래 걸림 — 여기서 멈춤 (" + k + "/" + last + "점)", 'y');
                    break;
                }
                double[] v;
                if (seen) {
                    // 앞을 보는 점: 지금 점에서 길을 따라 LOOKAHEAD 만큼 앞 (마지막 근처면 마지막 점)
                    int j = k;
                    double acc = 0;
                    while (j < last && acc < LOOKAHEAD) {
                        acc += Math.hypot(P.get(j + 1)[1] - P.get(j)[1], P.get(j + 1)[2] - P.get(j)[2]);
                        j++;
                    }
                    double tx = P.get(j)[1] - x, ty = P.get(j)[2] - y, d = Math.hypot(tx, ty);
                    if (d < 1) {
                        v = null;
                    } else {
                        double ux = tx / d, uy = ty / d;
                        // 움직임 방향 → 조이스틱 방향 (기록에서 잰 각도만큼 되돌림)
                        double c = Math.cos(-theta), sn = Math.sin(-theta);
                        double jx = ux * c - uy * sn, jy = ux * sn + uy * c;
                        double rec = Math.hypot(P.get(k)[4], P.get(k)[5]);
                        double m = k >= last - 1 ? Math.max(0.5, Math.min(1, d / 30)) : Math.max(0.6, Math.min(1, rec < 0.15 ? 1 : rec));
                        v = new double[]{jx * m, jy * m};
                    }
                } else {
                    double[] p = P.get(Math.min(last, k + 1));
                    v = Math.hypot(p[4], p[5]) < 0.15 ? null : new double[]{p[4], p[5]};
                }
                touch.stick(v);
                double tn = now();
                hist.addLast(new double[]{tn, x, y, v == null ? 0 : v[0], v == null ? 0 : v[1], seen ? 1 : 0});
                while (hist.size() > 2 && tn - hist.peekFirst()[0] > 0.45) hist.pollFirst();
                double[] h0 = hist.peekFirst();
                if (seen && h0 != null && h0[5] > 0 && tn - h0[0] > 0.3) {
                    double mx = x - h0[1], my = y - h0[2], cm = Math.hypot(h0[3], h0[4]);
                    if (Math.hypot(mx, my) / (tn - h0[0]) > 40 && cm > 0.6) {
                        double d = Math.atan2(my, mx) - Math.atan2(h0[4], h0[3]);
                        d = Math.atan2(Math.sin(d - theta), Math.cos(d - theta));
                        if (Math.abs(d) < Math.toRadians(45)) theta += d * 0.1;
                    }
                }
                sleep(0.03);
            }
        } finally {
            touch.stick(null);
            if (jumpHeld || jp != null) touch.jumpUp();
        }
        out.log(name + " 길 따라가기 끝 (" + Math.round((now() - t0) * 10) / 10.0 + "초 · 화면 못 읽은 프레임 " + lost + "/" + frames + ")", 'd');
    }

    // ---- 3-1. Acrux WASD 패드로 길 기록 (화면 감지 없음 · 누른 키 그대로)
    /** 기록용 WASD 패드 · 점프 버튼 (안드로이드 쪽 화면) */
    public interface Pad {
        /** 지금 켜 둔 키의 방향 (단위 벡터 · 다 꺼졌으면 null) */
        double[] stick();

        /** 그 사이 점프 눌림 · 뗌 (+1 · -1) 하나씩 꺼냄 · 없으면 0 */
        int pollJump();

        void show();

        void hide();
    }

    /**
     * 기준 장소로 간 뒤 WASD 패드 · 점프 버튼을 띄움 → 키를 톡 누르면 켜지고 (다시 누르면 꺼짐) Acrux 가 로블록스 조이스틱을 그 방향으로 계속 밀면서 저장
     *  · 손가락이 화면에 닿는 순간 안드로이드가 대신 누르던 터치를 끊음 → 톡 누르기라 끊김이 짧음
     *  · 위치 길: 프레임마다 화면으로 캐릭터 위치를 잼 (다시 걸을 때 어긋남을 고치는 데 씀)
     */
    public Route recordPad(String name, Pad pad, BooleanSupplier arrived) {
        if (touch.stickAt() == null) throw new RuntimeException("먼저 조이스틱 찾기 (Acrux 창 → 조이스틱 찾기)");
        goBase();
        int[] jp = jumpPos();
        out.state(name + " 길 기록 중 — WASD 패드로 걸어가서 도착하면 [도착]");
        out.log(name + " 길 기록 시작 — W A S D 를 톡 누르면 켜짐 (홀드) · 다시 누르면 꺼짐 · ■ = 멈춤 · 도착하면 [도착]", 'c');
        double t0 = now();
        boolean down = false;
        List<Route.Ev> evs;
        // 손가락이 실제로 로블록스를 누르고 뗀 시각을 그대로 적음 (Pilot) — 다시 걸을 때 같은 시각에 똑같이 누름
        touch.traceStart();
        pad.show();
        try {
            while (!arrived.getAsBoolean()) {
                check.run();
                if (now() - t0 > RECORD_MAX) throw new RuntimeException((int) RECORD_MAX + "초 안에 [도착]을 안 누름 — 기록 취소");
                double[] v = pad.stick();
                boolean jd = false, ju = false;
                int ev;
                while ((ev = pad.pollJump()) != 0) {
                    if (ev > 0) {
                        jd = true;
                        down = true;
                    } else {
                        ju = true;
                        down = false;
                    }
                }
                if (jp != null) {
                    if (jd) touch.jumpDown(jp[0], jp[1]);
                    if (ju && !(jd && down)) touch.jumpUp();
                }
                touch.stick(v);
                Touch.nap(4);
            }
        } finally {
            pad.hide();
            touch.stick(null);
            touch.jumpUp();
            evs = touch.traceEnd();
        }
        Route r = Route.fromTrace(evs);
        out.log(name + " 길 기록 끝: " + Math.round(r.seconds() * 10) / 10.0 + "초 · 누름 " + r.presses() + "번 · 점프 " + r.jumps() + "번", r.moves() ? 'g' : 'y');
        if (!r.moves()) throw new RuntimeException("움직임이 없음 — WASD 패드로 걸어 주세요");
        return r;
    }

    /** 위치 길 앞뒤의 안 움직인 부분 빼기 (점프 표시는 남김) */
    static void trimPts(List<double[]> p) {
        while (p.size() > 2 && p.get(0)[3] == 0 && Math.hypot(p.get(1)[1] - p.get(0)[1], p.get(1)[2] - p.get(0)[2]) < 0.5
                && Math.hypot(p.get(0)[4], p.get(0)[5]) < 0.15) p.remove(0);
        // 끝에 멈춰 서 있던 점들 (점프 표시 없는 것) 은 하나만 남김
        while (p.size() > 2) {
            double[] a = p.get(p.size() - 2), b = p.get(p.size() - 1);
            if (a[3] != 0 || Math.hypot(b[1] - a[1], b[2] - a[2]) >= 0.5 || Math.hypot(a[4], a[5]) >= 0.15) break;
            b[0] = a[0];
            p.remove(p.size() - 2);
        }
    }

    /** 점프 버튼 한가운데 (px) — ⚙ 에서 찍은 자리 근처를 화면에서 다시 맞춤 · 위치가 없으면 null */
    public int[] jumpPos() {
        double[] jb = base.get("jump_btn");
        double[] st = touch.stickAt();
        if (jb == null) return null;
        int[] p = px(jb);
        if (st == null) return p;
        int half = (int) (st[2] * 2.2), qx = Math.max(0, p[0] - half), qy = Math.max(0, p[1] - half);
        Img im = scr.grab(qx, qy, half * 2, half * 2);
        if (im == null || im.w == 0) return p;
        double[] jl = Stick.locateJump(im, p[0] - qx, p[1] - qy, st[2]);
        if (jl[3] > -15) return p;                     // 버튼이 잘 안 보이면 찍은 자리 그대로
        return new int[]{qx + (int) Math.round(jl[0]), qy + (int) Math.round(jl[1])};
    }

    /** 점프 테스트: 제자리 점프 → 앞으로 걸으면서 점프 → 걸으면서 1.5초 꾹 누르기 */
    public void jumpTest() {
        int[] jp = jumpPos();
        if (jp == null) throw new RuntimeException("먼저 지정 필요: 점프 버튼 (Acrux 창)");
        out.log("점프 테스트: 점프 버튼 (" + jp[0] + ", " + jp[1] + ") — 초록 점이 버튼 위에 뜨는지 보세요", 'c');
        try {
            out.state("점프 테스트 — 제자리");
            touch.jumpDown(jp[0], jp[1]);
            sleep(0.12);
            touch.jumpUp();
            sleep(1.2);
            if (touch.stickAt() != null) {
                out.state("점프 테스트 — 걸으면서");
                touch.stick(new double[]{0, -1});
                sleep(0.4);
                touch.jumpDown(jp[0], jp[1]);
                sleep(0.12);
                touch.jumpUp();
                sleep(0.9);
                out.state("점프 테스트 — 걸으면서 꾹 누르기 (1.5초)");
                touch.jumpDown(jp[0], jp[1]);
                sleep(1.5);
                touch.jumpUp();
                sleep(0.4);
            }
        } finally {
            touch.stick(null);
            touch.jumpUp();
        }
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
        if (st == null) throw new RuntimeException("먼저 조이스틱 찾기 (Acrux 창 → 조이스틱 찾기)");
        goBase();
        // 조이스틱은 누를 때마다 판이 누른 자리로 옮겨짐 → 화면 왼쪽 아래를 넓게 보면서 판을 따라감 (Stick.Tracker)
        int rx0 = 0, ry0 = (int) (scr.height() * 0.3), rw = scr.width() / 2, rh = scr.height() - ry0;
        Stick.Tracker tk = new Stick.Tracker(st[0], st[1], st[2]);
        double[] held = null;
        double heldAt = -1;
        out.state(name + " 길 기록 중 — 조이스틱으로 걸어가서 도착하면 [도착]");
        out.log(name + " 길 기록 시작 — 지금부터 조이스틱 움직임을 저장 · 도착하면 Acrux 창의 [도착]", 'c');
        Route.Recorder rec = new Route.Recorder();
        // 위치 길: 바닥이 밀린 만큼으로 캐릭터 위치를 잼 (다시 걸을 때 이 길을 따라감 · 조이스틱 읽기가 틀려도 길은 정확)
        Odo odo = new Odo(scr);
        odo.step();
        List<double[]> pts = rec.route().pts;
        double lastPt = -1;
        int odoSeen = 0;
        double[] rawV = null;
        double[] jb = base.get("jump_btn");
        // 점프 버튼은 따로 빠르게 (약 60번/초) 봄 — 짧게 톡 누른 점프도 놓치지 않게 (조이스틱 읽기는 한 번에 시간이 더 걸림)
        java.util.concurrent.ConcurrentLinkedQueue<Integer> jumpEv = new java.util.concurrent.ConcurrentLinkedQueue<>();
        java.util.concurrent.atomic.AtomicBoolean jumpRun = new java.util.concurrent.atomic.AtomicBoolean(true);
        Thread jumpThread = null;
        if (jb != null) {
            int[] p = px(jb);
            int half = (int) (st[2] * 2.2), qx = Math.max(0, p[0] - half), qy = Math.max(0, p[1] - half);
            Img first = scr.grab(qx, qy, half * 2, half * 2);
            if (first != null && first.w > 0) {
                double[] jl = Stick.locateJump(first, p[0] - qx, p[1] - qy, st[2]);
                int jcx = qx + (int) Math.round(jl[0]), jcy = qy + (int) Math.round(jl[1]);
                double jr = jl[2], jbase = jl[3];
                out.log("점프 버튼: 가운데 (" + jcx + ", " + jcy + ") · 반지름 " + Math.round(jr) + "px · 평소 밝기 차이 " + Math.round(jbase), 'd');
                if (jbase > -15) out.log("점프 버튼이 잘 안 보임 — 점프 버튼 위치를 버튼 한가운데로 다시 지정해 주세요 (점프를 못 읽을 수 있음)", 'y');
                int jsz = (int) (jr * 2.9), jx = jcx - jsz / 2, jy = jcy - jsz / 2;
                Stick.JumpWatch jw = new Stick.JumpWatch(jbase);
                double jt0 = now();
                jumpThread = new Thread(() -> {
                    while (jumpRun.get()) {
                        Img ji = scr.grab(jx, jy, jsz, jsz);
                        if (ji != null && ji.w > 0) {
                            double tt = now() - jt0;
                            int ev = jw.step(Stick.jumpLevel(ji, jcx - Math.max(0, jx), jcy - Math.max(0, jy), jr), tt);
                            if (ev != 0) jumpEv.add(ev);
                        }
                        Touch.nap(15);
                    }
                }, "acrux-jump");
                jumpThread.setDaemon(true);
            }
        } else {
            out.log("점프 버튼 위치가 없어서 점프는 기록 안 함 (Acrux 창 → 점프 버튼)", 'y');
        }
        double t0 = now();
        if (jumpThread != null) jumpThread.start();
        double[][] last = new double[3][];
        int frames = 0, seen = 0, jumps = 0;
        try {
        while (!arrived.getAsBoolean()) {
            check.run();
            double t = now() - t0;
            if (t > RECORD_MAX) throw new RuntimeException((int) RECORD_MAX + "초 안에 [도착]을 안 누름 — 기록 취소");
            Img im = scr.grab(rx0, ry0, rw, rh);
            double[] v = null;
            if (im != null && im.w > 0) {
                double[] k = tk.step(im, rx0, ry0);
                frames++;
                if (k != null) {
                    v = new double[]{k[0], k[1]};
                    rawV = v;
                    held = v;
                    heldAt = t;
                    if (Math.hypot(k[0], k[1]) >= 0.3) seen++;
                } else if (held != null && t - heldAt <= 0.3) {
                    v = held;                       // 잠깐 판을 못 찾음 (흰 구름 · 효과에 가려짐) → 0.3초까지는 방금 방향 그대로
                }
            }
            // 3프레임 중 2프레임이 같은 쪽이면 그 값 (한 프레임 튀는 건 무시)
            last[0] = last[1];
            last[1] = last[2];
            last[2] = Route.quant(v);
            // 그 사이 눌린 점프 (따로 보는 스레드) — 시각은 기록 시작 기준이라 거의 같음
            boolean jump = false, up = false;
            Integer ev;
            while ((ev = jumpEv.poll()) != null) {
                if (ev > 0) {
                    jump = true;
                    jumps++;
                } else {
                    up = true;
                }
            }
            rec.add(t, steady(last), jump, up);
            if (odo.step()) odoSeen++;
            if (jump || up || t - lastPt >= 0.08) {
                double[] q = v != null && Math.hypot(v[0], v[1]) >= 0.3 ? v : null;
                pts.add(new double[]{t, odo.x, odo.y, (jump ? 1 : 0) | (up ? 2 : 0), q == null ? 0 : q[0], q == null ? 0 : q[1]});
                lastPt = t;
            }
            Touch.nap(5);
        }
        } finally {
            jumpRun.set(false);
        }
        double te = now() - t0;
        rec.add(te, null, false, true);          // 끝날 때 점프 버튼은 뗀 걸로
        pts.add(new double[]{te, odo.x, odo.y, 2, 0, 0});
        trimPts(pts);
        Route r = rec.done();
        out.log(name + " 길 기록 끝: " + Math.round(r.seconds() * 10) / 10.0 + "초 · 점프 " + r.jumps() + "번 · 위치 " + pts.size() + "점 (화면으로 위치를 잰 프레임 " + odoSeen + "/" + frames
                + " · 조이스틱 읽은 프레임 " + seen + ")", r.isEmpty() && !r.hasPath() ? 'y' : 'g');
        if (r.isEmpty() && !r.hasPath()) throw new RuntimeException("움직임을 못 읽음 — 조이스틱 찾기를 다시 하고 다시 기록해 주세요");
        if (!r.hasPath()) out.log("화면으로 위치를 잘 못 재서 (단색 바닥 · 너무 짧은 길) 다시 걸을 때는 시간대로만 감", 'y');
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
