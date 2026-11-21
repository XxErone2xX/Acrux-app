package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * 자동 낚시 (제자리 낚시) — PC 버전 fishing.Fisher 의 상태 기계를 모바일용으로 옮김
 *   · 대기(파란 Fish) → Fish 터치 · CAST_WAIT 초 안에 안 바뀌면 다시 · castRetry 번 넘으면 인벤토리 가득 → 정지
 *   · 입질 대기(빨간 Exit) → 기다림 · biteMax 초 넘으면 Exit
 *   · 릴링(릴링 바가 보임) → 릴링 → Fish 가 다시 보일 때까지 결과창 X 를 0.1초마다 (그 사이 제목 색으로 결과 기록)
 *   · 알 수 없음 3초 이상 → X 한 번
 * 화면 = 휴대폰 화면 전체 (로블록스 전체 화면) · 위치는 전부 화면 비율
 */
public final class Fisher {
    public interface Screen {
        int width();
        int height();
        /** 화면의 (x, y, w, h) 부분 (화면 밖은 잘림) · 화면이 아직 없으면 null */
        Img grab(int x, int y, int w, int h);

        /**
         * dst 크기만큼 (x, y) 부분을 dst 에 복사 (새 이미지를 안 만듦 · 화면 밖은 검은색) → 그 화면 번호
         * · known 과 같은 화면이면 복사하지 않고 known · 화면이 없으면 -1 (기본: 늘 새로 잡아서 새 화면으로 봄)
         */
        default long grabInto(Img dst, int x, int y, long known) {
            Img im = grab(x, y, dst.w, dst.h);
            if (im == null) return -1;
            dst.clear();
            im.cropInto(dst, Math.min(0, x), Math.min(0, y));
            return System.nanoTime() & Long.MAX_VALUE;
        }

        /** 줄인 화면 (step 칸마다 한 칸) · 화면이 없으면 null */
        default Img grabStep(int x, int y, int w, int h, int step) {
            Img im = grab(x, y, w, h);
            if (im == null || step <= 1) return im;
            Img o = new Img((im.w + step - 1) / step, (im.h + step - 1) / step);
            for (int yy = 0; yy < o.h; yy++) {
                for (int xx = 0; xx < o.w; xx++) {
                    int s = yy * step * im.w + xx * step, d = yy * o.w + xx;
                    o.r[d] = im.r[s];
                    o.g[d] = im.g[s];
                    o.b[d] = im.b[s];
                }
            }
            return o;
        }
    }

    public interface Input {
        void tap(int x, int y, int ms);

        /** 릴링 전에 누를 자리로 마우스를 미리 옮겨 둠 (PC: move_to 뒤 mouse_click_here) */
        default void hover(int x, int y) {}

        /** 지금까지 취소된 터치 수 (앞 터치가 끝나기 전에 새 터치를 보내면 안드로이드가 앞 터치를 취소함) — 릴링 기록용 */
        default long cancelled() {
            return 0;
        }
    }

    public interface Out {
        void log(String msg, char color);     // g 초록 · y 노랑 · r 빨강 · d 흐림
        void state(String msg);
    }

    public static final class Stopped extends RuntimeException {
        public Stopped() {
            super("stopped");
        }
    }

    /** 릴링: 새 화면이 왔는지 보는 간격 (새 화면만 분석 · 누를지는 그 사이에도 예측으로 정함) */
    public static final double REEL_POLL = 0.004, REEL_MAX = 15.0, REEL_GONE = 0.6, BOGUS_AFTER = 4.0;
    /**
     * 결과창 닫기: FINISH_GAP 초마다 누름 · LOOK_FROM 초부터 화면에서 X 를 찾음 (LOOK_GAP 초마다 한 번)
     * X 는 창이 뜨고 1.5 ~ 2초 뒤에 나타남 → X_SHOWN 초가 지난 뒤로 한 자리를 DWELL 초 동안 눌러도 안 닫히면 다음 자리
     */
    public static final double FINISH_GAP = 0.1, FINISH_MAX = 10.0, LOOK_FROM = 1.2, LOOK_GAP = 0.5, X_SHOWN = 2.2, DWELL = 0.8;
    /** 모바일은 UI 가 화면에서 차지하는 비율이 커서 창 폭 최대를 넓게 (PC 0.5) */
    public static final double MAX_FRAME_W = 0.92;

    final Screen scr;
    final Input in;
    final Out out;
    public final Settings cfg;
    volatile boolean stop;
    boolean diamondOk = false;
    public int success, junk, fail, full;
    /** 낚시 결과마다 ("success" · "junk" · "fail") — 앱이 통계를 따로 저장 (보정 · 테스트로 Fisher 가 새로 만들어져도 안 사라지게) */
    public java.util.function.Consumer<String> onResult;
    /** 낚시를 시작할 때 · 낚시 화면이 오래 안 보일 때: 기준 장소 → 낚시 장소 (없으면 지금 자리에서) */
    public Runnable onStart;
    /** 인벤토리 가득: 판매하고 돌아왔으면 true (낚시 이어감) · 못 하면 false (멈춤) */
    public java.util.function.BooleanSupplier onFull;
    /** 릴링 측정값 · 결과창 X 자리를 새로 배웠을 때 (앱이 저장) */
    public Runnable onLearn;
    /** 던지기 전 대기 화면 (Fish 버튼이 보이고 낚싯대를 안 던졌음) 마다: 낚시 밖에서 할 일 (서버 체크 등) 을 했으면 true (그 시간은 멈춰 선 걸로 안 셈) */
    public java.util.function.BooleanSupplier onIdle;

    /** 낚시 테스트 · 디버깅 모드에서 화면에 그려 볼 것 (앱이 넣음 · 없으면 안 씀) */
    public interface Probe {
        /** 릴링 화면 하나를 읽음: 바 (화면 px x, y, 폭, 높이) · 내 위치 · 구간 (바 안 px · 없으면 null) */
        void reel(int x, int y, int w, int h, Double marker, int[] zone);

        /**
         * 릴링 조작 (화면 하나마다 · 끝날 때 한 번 더): pred 지금 누르면 반영될 때의 내 위치 예측 · target 이보다 왼쪽이면 누름 (바 폭 비율 · 없으면 NaN)
         *  · s {지연 초, 구간 안 %, 터치 수, 화면 fps, 바 화면 수, 내 위치 읽은 화면 수, 구간 읽은 화면 수, 내 위치가 움직였나 (1 · 0)}
         */
        default void pilot(double pred, double target, double[] s) {}

        /** 릴링 끝 */
        void reelEnd();
    }

    public volatile Probe probe;
    /** 지금 단계 (화면 표시용): 0 모름 · 1 대기 · 입질 기다림 · 2 릴링 · 3 결과창 닫기 · 4 이동 · 판매 (낚시 창을 안 봄) · 5 자동 보정에서 낚시 창을 아직 못 찾음 */
    public volatile int phase;
    /** 디버깅 (앱이 켬): 낚시 창에서 읽은 것을 남김 (화면 표시용 · 낚시하는 방법은 그대로) */
    public volatile boolean debug;
    /** 마지막으로 읽은 것 — Fish 버튼 칸 ("fish" · "exit" · null 안 보임) · 미니게임 바가 보였나 · ◇ 흰 칸 수 {대기 자리, 미니게임 자리} (6 이상이면 ◇ 가 있음) */
    public volatile String seenBtn;
    public volatile boolean seenBar;
    public volatile int[] seenDia;
    /** 낚시 테스트 (한 판만) 중인지 · 끝났을 때 결과 한 줄 */
    boolean once;
    public volatile String testEnd;
    /** 지금 릴링 때 누르는 자리 (px · 기록 · 테스트용) */
    int[] reelTap;
    public static final double RELOC_AFTER = 30;
    /** Fish 를 누른 뒤 던지기 (Exit 로 바뀜) 를 기다리는 시간 — 느린 폰 · 렉에선 1.5초 넘게 걸림 (짧으면 다시 누른 게 Exit 가 돼서 취소됨) */
    public static final double CAST_WAIT = 3.0;

    public Fisher(Screen scr, Input in, Out out, Settings cfg) {
        this.scr = scr;
        this.in = in;
        this.out = out;
        this.cfg = cfg;
    }

    public void stop() {
        stop = true;
    }

    public void reset() {
        stop = false;
    }

    static double now() {
        return System.nanoTime() / 1e9;
    }

    void check() {
        if (stop || Thread.currentThread().isInterrupted()) throw new Stopped();
    }

    void sleep(double sec) {
        double end = now() + Math.max(0, sec);
        while (true) {
            check();
            double left = end - now();
            if (left <= 0) return;
            try {
                Thread.sleep(Math.max(1, (long) (Math.min(0.03, left) * 1000)));
            } catch (InterruptedException e) {
                throw new Stopped();
            }
        }
    }

    int W() {
        return scr.width();
    }

    int H() {
        return scr.height();
    }

    int[] px(double rx, double ry) {
        return new int[]{(int) (rx * W()), (int) (ry * H())};
    }

    void tapRatio(double[] pos) {
        int[] p = px(pos[0], pos[1]);
        in.tap(p[0], p[1], 40);
    }

    Img grab(int x, int y, int w, int h) {
        Img im = scr.grab(x, y, w, h);
        return im != null ? im : new Img(0, 0);
    }

    Img grabBox(double[] pos, double bw, double bh) {
        int[] p = px(pos[0], pos[1]);
        int w = Math.max(6, (int) (W() * bw)), h = Math.max(6, (int) (H() * bh));
        return grab(p[0] - w / 2, p[1] - h / 2, w, h);
    }

    Img grabBox(double[] pos) {
        return grabBox(pos, 0.045, 0.035);
    }

    /** 바 영역 → 바 가운데 줄(35~65%)만 잡는 작은 캡처 */
    Img bandBox(double[] region) {
        int[] a = px(Math.min(region[0], region[2]), Math.min(region[1], region[3]));
        int[] b = px(Math.max(region[0], region[2]), Math.max(region[1], region[3]));
        int bh = Math.max(4, b[1] - a[1]);
        return grab(a[0], a[1] + (int) (bh * 0.35), Math.max(8, b[0] - a[0]), Math.max(1, (int) (bh * 0.3)));
    }

    boolean barSeen() {
        return Vision.barPresent(bandBox(cfg.barRegion));
    }

    /** 낚시 창 ◇ 가 미니게임 자리로 옮겨갔는지 · 창 영역이 없으면 null */
    Boolean diamondMoved() {
        double[] idleWin = cfg.panelRegion, reelWin = cfg.reelRegion;
        if (idleWin == null && reelWin == null) return null;
        double[][] idle = idleWin != null ? new double[][]{idleWin, Vision.DIAMOND_IDLE} : new double[][]{reelWin, Vision.DIAMOND_IDLE_R};
        double[][] reel = reelWin != null ? new double[][]{reelWin, Vision.DIAMOND_REEL_R} : new double[][]{idleWin, Vision.DIAMOND_REEL};
        return white(reel[0], reel[1]) >= 6 && white(idle[0], idle[1]) < 6;
    }

    /** ◇ 두 자리의 흰 칸 수 {대기 자리, 미니게임 자리} (diamondMoved 와 같은 칸) · 창 영역이 없으면 null */
    int[] diamondWhite() {
        double[] idleWin = cfg.panelRegion, reelWin = cfg.reelRegion;
        if (idleWin == null && reelWin == null) return null;
        double[][] idle = idleWin != null ? new double[][]{idleWin, Vision.DIAMOND_IDLE} : new double[][]{reelWin, Vision.DIAMOND_IDLE_R};
        double[][] reel = reelWin != null ? new double[][]{reelWin, Vision.DIAMOND_REEL_R} : new double[][]{idleWin, Vision.DIAMOND_REEL};
        return new int[]{white(idle[0], idle[1]), white(reel[0], reel[1])};
    }

    /** 디버깅: 지금 낚시 창에서 읽은 것을 남김 (Fish 버튼 칸 · 미니게임 바 · ◇) — 화면 표시용 */
    void look() {
        seenBtn = cfg.fishBtn == null ? null : Vision.buttonState(grabBox(cfg.fishBtn));
        seenBar = cfg.barRegion != null && barSeen();
        seenDia = diamondWhite();
    }

    int white(double[] win, double[] rel) {
        double x1 = Math.min(win[0], win[2]), x2 = Math.max(win[0], win[2]);
        double y1 = Math.min(win[1], win[3]), y2 = Math.max(win[1], win[3]);
        int size = Math.max(3, (int) (W() * (x2 - x1) * 0.012));
        int[] p = px(x1 + rel[0] * (x2 - x1), y1 + rel[1] * (y2 - y1));
        Img im = grab(p[0] - size, p[1] - size, size * 2 + 1, size * 2 + 1);
        // ◇ 는 흰색 — 화면이 어두우면 (기기 밝기 필터 · 어두운 바이옴 등) 180 정도로 보임 → 가장 밝은 값 기준으로 낮춤 (150 아래로는 안 내림)
        int peak = 0;
        for (int k = 0; k < im.w * im.h; k++) peak = Math.max(peak, im.min(k));
        int thr = Math.max(150, Math.min(200, peak - 25));
        int c = 0;
        for (int k = 0; k < im.w * im.h; k++) if (im.min(k) > thr) c++;
        return c;
    }

    /** 지금 화면 → "reel" / "maybe" / "idle" / "wait" / null */
    String readState() {
        if (barSeen()) {
            Boolean moved = diamondMoved();
            if (Boolean.TRUE.equals(moved)) {
                diamondOk = true;
                return "reel";
            }
            // 방금 바처럼 보였는데 미니게임이 아니었음 (결과창 물고기 사진 · 다른 창의 색) → 잠깐은 바만 보고 릴링으로 보지 않음
            // (안 그러면 그 화면에서 '릴링 → 결과창 닫기' 만 끝없이 반복하고 Fish 버튼 · 복구 쪽으로 못 감)
            if (now() < barQuietUntil) return null;
            if (Boolean.FALSE.equals(moved) && diamondOk) return null;
            sleep(0.04);
            if (barSeen()) return moved == null ? "reel" : "maybe";
        }
        String st = Vision.buttonState(grabBox(cfg.fishBtn));
        if ("fish".equals(st)) return "idle";
        if ("exit".equals(st)) return "wait";
        return null;
    }

    // ---------------------------------------------------------------- 메인
    public void run() {
        if (!cfg.ready()) {
            out.log("자동 낚시 안 함 — 먼저 [자동 보정]이 필요함", 'y');
            return;
        }
        out.log("자동 낚시 시작", 'g');
        boolean go = onStart != null, trip = false;
        int crashes = 0, okAt = rounds();
        try {
            while (true) {
                try {
                    if (go) {
                        trip = true;
                        out.state("낚시 장소로 가는 중");
                        away(onStart);
                        trip = false;
                    }
                    loop();
                    return;
                } catch (Stopped e) {
                    throw e;
                } catch (RuntimeException | OutOfMemoryError e) {
                    // 잠깐 생긴 오류 (화면 · 글자 읽기 · 이동 · 판매 중 예외 · 메모리 부족 등) 로 멈추지 않음 → 쉬었다가 이어감 (이어서 날수록 길게 · 최대 2분)
                    //  · 낚시 장소로 가다가 난 오류면 다시 감 · 낚시하다가 난 오류는 처음 한 번은 그 자리에서 이어 가고 또 나면 낚시 장소로 다시 감
                    //  · 그 사이 한 판이라도 끝났으면 처음부터 셈
                    if (rounds() > okAt) crashes = 0;
                    okAt = rounds();
                    crashes++;
                    if (e instanceof OutOfMemoryError) System.gc();
                    int wait = (int) Math.min(120, 5.0 * crashes);
                    go = onStart != null && (trip || crashes >= 2);
                    trip = false;
                    out.log("자동 낚시 오류: " + e + " — " + wait + "초 쉬고 " + (go ? "낚시 장소로 다시 가서 " : "") + "이어감 (" + crashes + "번째)", 'y');
                    out.state("잠깐 쉬는 중 — " + wait + "초 뒤 이어감");
                    sleep(wait);
                }
            }
        } catch (Stopped e) {
            out.log("자동 낚시 정지", 'd');
        } finally {
            fitSettle();
            out.state("대기");
        }
    }

    /** 끝난 판 수 (성공 · 쓰레기 · 실패 · 못 본 미니게임) — 진행됐는지 보는 데 씀 */
    int rounds() {
        return success + junk + fail;
    }

    /** 이어서 낚시가 안 풀려서 낚시 장소로 다시 간 횟수 (한 판이라도 끝나면 0) — 쉬는 시간을 늘리는 데 씀 */
    int stuck;

    /** 낚시 장소로 다시 가기 전에 쉬는 시간 (초): 처음엔 바로 · 15 · 30 · 60 · 120 · 240 · 최대 300 */
    static double backoff(int n) {
        return n <= 1 ? 0 : Math.min(300, 15 * Math.pow(2, n - 2));
    }

    /**
     * 낚시가 안 풀릴 때 (화면 복구로도 안 됨 · 낚시 화면이 안 보임 · 판매를 못 함 · 연속 실패) — 멈추지 않고 낚시 장소로 다시 감 (리셋부터)
     *  · 이어서 안 될수록 먼저 길게 쉼 (backoff) · 가다가 오류가 나도 멈추지 않음 (다음에 또 시도) · [정지] 할 때까지 계속
     */
    void relocate() {
        stuck++;
        double wait = backoff(stuck);
        if (wait > 0) {
            out.log("낚시 장소로 다시 감 — " + (int) wait + "초 쉰 뒤 (" + stuck + "번째 · 멈추지 않고 계속)", 'y');
            out.state("잠깐 쉬는 중 — " + (int) wait + "초 뒤 낚시 장소로 다시 감");
            sleep(wait);
        } else {
            out.log("낚시 장소로 다시 감", 'y');
        }
        if (onStart == null) return;                                   // 이동이 없음 (기능 테스트) → 쉬기만
        out.state("낚시 장소로 다시 가는 중");
        try {
            away(onStart);
        } catch (Stopped e) {
            throw e;
        } catch (RuntimeException e) {
            out.log("낚시 장소로 다시 가다가 오류: " + e.getMessage() + " — 이어서 다시 시도", 'y');
        }
    }

    /** 이동 · 판매 (낚시 창을 안 봄 — 화면 표시는 단계 4) */
    void away(Runnable r) {
        phase = 4;
        try {
            r.run();
        } finally {
            phase = 0;
        }
    }

    /**
     * 낚시 테스트: 지금 자리에서 한 판만 (Fish → 입질 → 릴링 → 결과창 닫기) — 앱은 그동안 보정된 칸을 화면에 그려 줌
     *  · 칸이 실제 창과 안 맞으면 어느 칸이 문제인지 기록에 남기고 끝냄 (Fish 버튼 칸 · 미니게임 바 칸 · 한 판을 못 끝냄)
     *  · 이동 · 판매 · 연속 실패 처리는 안 함
     */
    public void test() {
        testEnd = null;
        if (!cfg.ready()) {
            out.log("낚시 테스트 안 함 — 먼저 [자동 보정]이 필요함", 'y');
            return;
        }
        out.log("낚시 테스트 시작 — 보정된 칸을 화면에 그리고 한 판만 낚시", 'g');
        once = true;
        try {
            loop();
        } finally {
            once = false;
            fitSettle();
            out.state("대기");
        }
    }

    /** 낚시 테스트 끝 → 결과 한 줄 */
    void testDone(String msg, char color) {
        testEnd = msg;
        out.log("낚시 테스트 끝 — " + msg, color);
    }

    double refitAt = -1;
    /** 낚시 창을 옮길 때 크기가 이만큼 (비율) 넘게 다르면 다른 창으로 봄 — 낚시 창 크기는 게임 중에 안 바뀜 */
    public static final double REFIT_SIZE = 0.06, REFIT_BLOCK = 300;
    /** 보정한 자리 (낚시 창을 옮기기 전) — 옮긴 뒤 문제가 생기면 되돌림 · 옮긴 자리가 확인 안 된 채로 끝나면 되돌려서 저장 */
    double[][] fitBase;
    /** 옮긴 뒤 결과창까지 잘 끝난 판 수 (2판이면 옮긴 자리를 믿음) */
    int fitProven;
    /** 이 시각까지는 낚시 창을 다시 안 옮김 (되돌린 직후) */
    double refitBlockUntil = -1;
    /** 옮길 후보 — 0.3초 넘게 지나서 같은 자리로 또 보여야 옮김 */
    double[] refitCand;
    double refitCandAt;
    /** 마지막 릴링에서 바를 읽은 기록 (가짜 미니게임으로 끝났을 때 로그에) · 마지막 결과창을 닫았는지 */
    String lastRead = "";
    boolean lastClosed = true;
    /** 입질 뒤 미니게임을 못 보고 지나간 판 수 (실패에 들어감) */
    public int missed;
    /** 가짜 릴링 (바처럼 보였지만 미니게임이 아님) 뒤 바만 보고는 릴링으로 안 보는 시각까지 */
    double barQuietUntil = -1;
    /** 가짜 릴링이 이어진 횟수 · 이어서 실패한 횟수 */
    int bogusRun, failRun;
    /** 이어서 실패: FAIL_RESET 번이면 릴링 측정값을 처음부터 · FAIL_STOP 번이면 낚시 장소로 다시 감 (자동 낚시는 멈추지 않음) */
    public static final int FAIL_RESET = 6, FAIL_STOP = 30;

    /** 이어서 FAIL_STOP 번 실패 (릴링 실패 · 미니게임을 못 봄) — 릴링 측정값 · 누르는 자리를 처음부터 · 낚시 장소로 다시 감 (이동이 없으면 멈춤) */
    void failStreak() {
        if (onStart == null) {                                         // 이동이 없는 낚시 (기능 테스트) 만 여기서 끝남
            out.log("연속 실패 " + failRun + "번 — 자동 낚시 멈춤 (로그의 '릴링:' 줄 · 미니게임 보정 확인)", 'r');
            throw new Stopped();
        }
        out.log("연속 실패 " + failRun + "번 — 릴링 측정값을 처음부터 다시 재고 낚시 장소로 다시 가서 이어감 (계속 그러면 낚시 보정 → 칸을 띄우며 낚시 테스트로 확인)", 'y');
        failRun = 0;
        cfg.reelDelay = cfg.reelFall = cfg.reelJump = 0;
        cfg.reelSpot = 0;
        if (onLearn != null) onLearn.run();
        relocate();
    }

    /**
     * 낚시 창을 화면에서 다시 찾아서, 보정 때 창 자리 → 지금 창 자리로 모든 위치 (Fish 버튼 · 릴링 바 · 결과창 X · 제목 · 창 영역) 를 옮김
     * → 옮겼으면 true (Fish / Exit 버튼이 보이는 낚시 창을 찾았고, 자리가 달라졌을 때)
     */
    boolean refit() {
        double[] old = cfg.panelRegion;
        if (old == null || now() < refitBlockUntil) return false;
        double[] now = frameOf(full(), 0);
        if (now == null) {
            refitCand = null;
            return false;
        }
        double ow = old[2] - old[0], oh = old[3] - old[1], nw = now[2] - now[0], nh = now[3] - now[1];
        if (ow <= 0 || oh <= 0) return false;
        // 크기가 다르면 낚시 창이 아님 (결과창 · 미니게임 창 · 눈처럼 밝은 배경에서 잘못 본 모양) — 전에는 크기가 달라도 옮겨서 모든 위치가 틀어졌음
        if (Math.abs(nw / ow - 1) > REFIT_SIZE || Math.abs(nh / oh - 1) > REFIT_SIZE) {
            refitCand = null;
            return false;
        }
        double dx = now[0] - old[0], dy = now[1] - old[1];
        if (Math.abs(dx) + Math.abs(dy) < 0.006) {
            refitCand = null;
            return false;
        }
        // 한 번 본 것만으로는 안 옮김 — 0.3초 넘게 지나서 같은 자리로 또 보여야 옮김
        if (refitCand == null || Math.abs(refitCand[0] - now[0]) + Math.abs(refitCand[1] - now[1]) > 0.004) {
            refitCand = now;
            refitCandAt = now();
            return false;
        }
        if (now() - refitCandAt < 0.3) return false;
        refitCand = null;
        // 옮긴 Fish 버튼 자리에 정말 버튼이 있는지 확인한 뒤에만 바꿈 · 크기는 보정 때 그대로 (자리만 옮김)
        double[] fb = shift(cfg.fishBtn, dx, dy);
        if (fb == null || Vision.buttonState(grabBox(fb)) == null) return false;
        if (fitBase == null) fitBase = fitSnap();
        fitProven = 0;
        cfg.fishBtn = fb;
        cfg.barRegion = shift(cfg.barRegion, dx, dy);
        cfg.closePos = shift(cfg.closePos, dx, dy);
        cfg.titlePos = shift(cfg.titlePos, dx, dy);
        cfg.reelRegion = shift(cfg.reelRegion, dx, dy);
        cfg.resultRegion = shift(cfg.resultRegion, dx, dy);
        cfg.panelRegion = shift(old, dx, dy);
        if (cfg.closeLearn != null) {                         // 기억한 결과창 X 자리도 같이 옮김 ({x, y, 닫힌 횟수, …})
            double[] l = cfg.closeLearn.clone();
            for (int i = 0; i + 2 < l.length; i += 3) {
                l[i] = Vision.round4(l[i] + dx);
                l[i + 1] = Vision.round4(l[i + 1] + dy);
            }
            cfg.closeLearn = l;
        }
        out.log("낚시 창 자리가 바뀜 → 위치를 다시 맞춤 (Fish 버튼 " + SellText.r4(fb[0]) + ", " + SellText.r4(fb[1]) + ")", 'c');
        return true;
    }

    /** 위치 (x, y 짝) 를 dx, dy 만큼 옮긴 것 */
    static double[] shift(double[] p, double dx, double dy) {
        if (p == null) return null;
        double[] q = p.clone();
        for (int i = 0; i + 1 < q.length; i += 2) {
            q[i] = Vision.round4(q[i] + dx);
            q[i + 1] = Vision.round4(q[i + 1] + dy);
        }
        return q;
    }

    static double[] copy(double[] a) {
        return a == null ? null : a.clone();
    }

    double[][] fitSnap() {
        return new double[][]{copy(cfg.panelRegion), copy(cfg.reelRegion), copy(cfg.resultRegion), copy(cfg.fishBtn), copy(cfg.barRegion),
                copy(cfg.closePos), copy(cfg.titlePos), copy(cfg.closeLearn)};
    }

    void fitRestore(double[][] b) {
        cfg.panelRegion = b[0];
        cfg.reelRegion = b[1];
        cfg.resultRegion = b[2];
        cfg.fishBtn = b[3];
        cfg.barRegion = b[4];
        cfg.closePos = b[5];
        cfg.titlePos = b[6];
        cfg.closeLearn = b[7];
    }

    /** 옮긴 낚시 창 자리에서 문제가 생김 → 보정한 자리로 되돌림 (5분 동안은 다시 안 옮김) → 되돌렸으면 true */
    boolean revertFit(String why) {
        if (fitBase == null) return false;
        fitRestore(fitBase);
        fitBase = null;
        refitCand = null;
        refitBlockUntil = now() + REFIT_BLOCK;
        out.log("옮긴 낚시 창 자리에서 " + why + " — 보정한 자리로 되돌림", 'y');
        return true;
    }

    /** 끝날 때: 옮긴 자리가 (결과창까지 2판) 확인 안 됐으면 보정한 자리로 되돌려 둠 (앱이 끝날 때 설정을 저장함 — 잘못 옮긴 자리가 저장되던 문제) */
    void fitSettle() {
        if (fitBase == null) return;
        fitRestore(fitBase);
        fitBase = null;
        out.log("옮긴 낚시 창 자리가 확인되지 않아서 보정한 자리로 되돌려 둠", 'd');
    }

    void loop() {
        Double castAt = null, waitAt = null, unknownAt = null;
        int tries = 0, rescues = 0, maybe = 0;
        double progressAt = now(), knownAt = now(), testAt = now();
        boolean seen = false, waited = false, hinted = false;          // 낚시 화면을 봤는지 (테스트) · 던진 뒤 Exit 를 봤는지 · 안내했는지 (테스트)
        while (true) {
            check();
            String st = readState();
            if (debug) look();
            double t = now();
            if ("idle".equals(st) || "wait".equals(st)) phase = 1;
            else if (st == null) phase = 0;
            if (once) {
                if (st != null) seen = true;
                if (!seen && t - testAt > 15) {
                    testDone("Fish 버튼 칸에서 15초 동안 Fish / Exit 버튼이 안 보임 — 초록 칸이 Fish 버튼과 맞는지 확인 (낚시 자리에서 다시 · 안 맞으면 자동 보정 다시)", 'y');
                    return;
                }
                if (t - testAt > cfg.biteMax + 60) {
                    testDone("한 판을 끝내지 못함 — 칸이 실제 창과 맞는지 확인", 'y');
                    return;
                }
            }
            if (t - progressAt > cfg.biteMax + 60) {
                rescues++;
                if (rescues > 3) {
                    if (onStart == null) {                             // 이동이 없는 낚시 (기능 테스트) 만 여기서 끝남
                        out.log("낚시 화면을 계속 찾지 못함 — 자동 낚시 멈춤 (낚시 자리 · 자동 보정 확인)", 'r');
                        throw new Stopped();
                    }
                    // 화면 복구로도 안 됨 → 멈추지 않고 낚시 장소로 다시 감 (이어서 안 될수록 쉬는 시간을 늘림)
                    out.log("한동안 낚시가 진행되지 않음 — 화면 복구로도 안 됨", 'y');
                    relocate();
                    rescues = 0;
                } else {
                    out.log("한동안 낚시가 진행되지 않음 — 화면 복구 (" + rescues + "/3)", 'y');
                    rescue();
                }
                progressAt = knownAt = now();
                castAt = waitAt = unknownAt = null;
                tries = 0;
                waited = false;
                continue;
            }
            if ("maybe".equals(st)) {
                maybe++;
                st = maybe >= 3 ? "reel" : null;
                if (st == null) {
                    sleep(0.05);
                    continue;
                }
            } else {
                maybe = 0;
            }
            if (!"wait".equals(st)) waitAt = null;
            if (st != null && !"reel".equals(st)) {
                unknownAt = null;
                knownAt = t;
            }

            if ("reel".equals(st)) {
                Img base = cfg.titlePos != null ? grabBox(cfg.titlePos, 0.12, 0.05) : null;
                if (!reel()) {
                    // 미니게임이 아니었음 (내 위치가 한 번도 안 움직임) → 진행으로 안 침 · 결과창 닫기도 안 함 · 잠깐 바 무시
                    bogusRun++;
                    barQuietUntil = now() + 2.0;
                    if (bogusRun == 1 || bogusRun % 10 == 0)
                        out.log("미니게임 바처럼 보였지만 미니게임이 아님 — 다시 확인 (" + bogusRun + "번째) · " + lastRead, 'd');
                    revertFit("가짜 미니게임");
                    continue;
                }
                bogusRun = 0;
                waited = false;
                castAt = null;
                tries = 0;
                stuck = 0;                                             // 한 판을 했음 → 쉬는 시간도 처음부터
                rescues = 0;
                unknownAt = null;
                progressAt = knownAt = now();
                String kind = finish(base);
                if (!lastClosed) {
                    revertFit("결과창을 못 닫음");
                } else if (fitBase != null && ++fitProven >= 2) {
                    fitBase = null;                                    // 옮긴 자리에서 2판 잘 끝남 → 옮긴 자리를 믿음 (끝날 때 저장)
                }
                if (once) {
                    testDone(kind == null ? "결과를 못 봄" : kind.equals("success") ? "성공" : kind.equals("junk") ? "쓰레기" : "실패", kind == null ? 'y' : 'g');
                    return;
                }
                if ("fail".equals(kind)) {
                    failRun++;
                    if (failRun == FAIL_RESET) {
                        // 릴링이 계속 실패 → 잰 값이 틀어졌을 수 있음 → 처음 값으로 다시 잼 · 누르는 자리도 기본으로
                        cfg.reelDelay = cfg.reelFall = cfg.reelJump = 0;
                        cfg.reelSpot = 0;
                        if (onLearn != null) onLearn.run();
                        out.log("릴링이 " + FAIL_RESET + "번 이어서 실패 — 릴링 측정값을 지우고 다시 잼", 'y');
                    } else if (failRun >= FAIL_STOP) {
                        failStreak();
                        castAt = waitAt = unknownAt = null;
                        tries = 0;
                        progressAt = knownAt = now();
                    }
                } else if (kind != null) {
                    failRun = 0;
                }
                continue;
            }
            if ("wait".equals(st)) {
                castAt = null;
                tries = 0;
                if (waitAt == null) waitAt = t;
                // Exit 가 0.5초 넘게 보여야 '던졌음' 으로 봄 — 낚시 장소로 다시 간 직후 · 화면이 늦게 바뀔 때 지난 화면 (Exit) 이 잠깐 보이는 걸로
                //  뒤의 대기 화면을 '미니게임을 못 봄' 으로 세지 않게
                if (t - waitAt >= 0.5) waited = true;
                if (t - waitAt > cfg.biteMax) {
                    if (once) {
                        tapRatio(cfg.fishBtn);
                        // 낚시 창이 대기 (Fish) 로 돌아오는 걸 보고 끝냄 (최대 2초)
                        for (int i = 0; i < 20 && !"idle".equals(readState()); i++) sleep(0.1);
                        testDone("입질이 " + (int) cfg.biteMax + "초 동안 없음 (Exit 누름)", 'y');
                        return;
                    }
                    out.log("입질이 " + (int) cfg.biteMax + "초 동안 없음 — Exit 후 다시 던짐", 'y');
                    tapRatio(cfg.fishBtn);
                    waited = false;
                    // Exit 가 안 먹혔으면 5초 뒤에 다시 (전에는 1초마다 누르고 로그를 남겼음)
                    waitAt = now() - cfg.biteMax + 5;
                    sleep(1.0);
                    continue;
                }
                out.state("입질 기다리는 중");
            } else if ("idle".equals(st)) {
                if (castAt == null && !waited && onIdle != null && onIdle.getAsBoolean()) {
                    progressAt = knownAt = now();
                    continue;
                }
                if (once && waited) {
                    // 던진 뒤 (Exit 가 보였음) 릴링 없이 다시 Fish → 미니게임을 못 보고 지나감
                    testDone("입질 뒤 미니게임을 못 봄 — 주황 칸 (미니게임 바) 이 실제 바와 맞는지 확인", 'y');
                    return;
                }
                if (castAt == null || t - castAt > CAST_WAIT) {
                    // 화면 판독 뒤 Exit · 미니게임으로 바뀌었으면 재입력 · 가득 참 판정을 미룸
                    if (!"idle".equals(readState())) continue;
                    if (waited) {
                        // 던진 뒤 (Exit 가 보였음) 릴링 없이 다시 Fish → 입질 뒤 미니게임을 못 보고 지나감 (전에는 통계에서 빠져서 '실패 0' 으로 보였음)
                        waited = false;
                        missed++;
                        fail++;
                        if (onResult != null) onResult.accept("fail");
                        out.log("입질 뒤 미니게임을 못 보고 지나감 — 실패로 셈 · 성공 " + success + " / 쓰레기 " + junk + " / 실패 " + fail, 'y');
                        progressAt = now();                            // 한 판은 돌았음 (멈춰 선 게 아님) — 계속 못 보면 아래 연속 실패로 멈춤
                        rescues = 0;
                        revertFit("미니게임을 못 봄");
                        if (++failRun >= FAIL_STOP) {
                            failStreak();
                            castAt = waitAt = unknownAt = null;
                            tries = 0;
                            progressAt = knownAt = now();
                            continue;
                        }
                    }
                    if (castAt != null) {
                        tries++;
                        if (tries >= cfg.castRetry) {
                            if (revertFit("Fish 가 반응 없음")) {
                                castAt = null;
                                tries = 0;
                                continue;
                            }
                            full++;
                            if (once) {
                                testDone("Fish 를 " + tries + "번 눌러도 반응 없음 — 인벤토리가 꽉 찼거나 누르는 자리 (초록 칸의 +) 가 Fish 버튼에서 벗어남", 'y');
                                return;
                            }
                            if (onFull != null) {
                                out.log("Fish 를 " + tries + "번 눌러도 반응 없음 — 낚시 인벤토리 가득", 'y');
                                out.state("인벤토리 가득 — 판매하러 감");
                                phase = 4;
                                boolean back;
                                try {
                                    back = onFull.getAsBoolean();
                                } finally {
                                    phase = 0;
                                }
                                if (!back) {
                                    // 판매를 못 함 (판매 실패 · 팔 물고기 없음) → 멈추지 않고 낚시 장소로 다시 가서 Fish 부터 다시 (이어서 안 될수록 쉬는 시간을 늘림)
                                    out.log("판매를 못 함 — 낚시 장소로 다시 가서 Fish 부터 다시", 'y');
                                    relocate();
                                }
                                castAt = waitAt = unknownAt = null;
                                tries = 0;
                                progressAt = knownAt = now();
                                continue;
                            }
                            out.log("Fish 를 " + tries + "번 눌러도 반응 없음 — 낚시 인벤토리 가득 · 정지", 'y');
                            out.state("인벤토리 가득 — 판매 필요");
                            throw new Stopped();
                        }
                    }
                    out.state("Fish 터치" + (tries > 0 ? " (" + (tries + 1) + "번째)" : ""));
                    tapRatio(cfg.fishBtn);
                    castAt = now();
                }
            } else {
                if (unknownAt == null) unknownAt = t;
                out.state("낚시 화면 확인 중");
                if (once && waited && !hinted && t - unknownAt > 2.0) {
                    hinted = true;
                    out.log("Exit 버튼이 사라졌는데 미니게임으로 안 보임 — 주황 칸 (미니게임 바) · ◇ 칸이 실제 미니게임과 맞는지 확인", 'y');
                }
                // 30초 동안 낚시 화면이 한 번도 안 보임 → 낚시 장소에 못 간 것 → 다시 이동 (멈추지 않음 · 이어서 안 될수록 쉬는 시간을 늘림)
                if (onStart != null && t - knownAt > RELOC_AFTER) {
                    out.log("낚시 화면이 " + (int) RELOC_AFTER + "초 동안 안 보임", 'y');
                    relocate();
                    knownAt = progressAt = now();
                    unknownAt = null;
                    castAt = waitAt = null;
                    tries = 0;
                    waited = false;
                    continue;
                }
                // Fish 버튼 자리에 버튼이 안 보임 → 낚시 창이 다른 자리 · 크기로 떴을 수 있음 (화면 회전 · 해상도 · UI 크기 바뀜)
                // → 화면에서 낚시 창을 다시 찾아서 위치를 옮겨 맞춤 (그 뒤 바로 다시 읽음)
                if (t - unknownAt > 1.5 && t - refitAt > 2.0) {
                    refitAt = t;
                    if (refit()) {
                        unknownAt = null;
                        continue;
                    }
                }
                if (t - unknownAt > 3.0) {
                    tapClose();
                    unknownAt = now();
                    sleep(0.6);
                    continue;
                }
            }
            sleep(0.1);
        }
    }

    /** 결과창 X 를 화면에서 찾아서 누름 (구조 · 알 수 없는 화면에서 — 결과창을 닫는 중은 finish 가 따로 봄)
     *  1. 결과창 자체를 찾아서 그 창 오른쪽 위의 X (쓰레기 결과창처럼 크기 · 바탕이 다른 창도)
     *     X 모양이 안 보이면 보정한 결과창과 같은 자리일 때만 계산한 자리를 누름
     *  2. 결과창을 못 찾으면 어두운 바탕 위 X — 보정한 X 자리가 있으면 그 근처 (화면 높이의 10% 안) 만 봄
     *  · 한 번 본 것으로 누르지 않음: 0.12초 뒤에 다시 찍어서 같은 자리에 또 보일 때만 (지나가는 물고기 · 효과를 X 로 보지 않게) */
    boolean clickFoundX() {
        int[] a = findX(false);
        if (a == null) return false;
        sleep(0.12);
        int[] b = findX(false);
        if (b == null || b[2] != a[2] || !nearPx(a, b)) return false;
        out.log(xHow(a[2]), 'd');
        in.tap(b[0], b[1], 40);
        return true;
    }

    static String xHow(int how) {
        return how == 1 ? "결과창 X 를 결과창 모양으로 찾아서 누름"
                : how == 3 ? "보정한 X 자리에 결과창이 없음 — 결과창 테두리 옆 X 를 찾아서 누름"
                : how == 4 ? "결과창 X 모양이 안 보임 — 결과창 크기로 계산한 X 자리를 누름"
                : "결과창이 다른 자리에 뜨거나 깨짐 — X 를 찾아서 누름";
    }

    boolean nearPx(int[] a, int[] b) {
        int tol = Math.max(4, (int) (H() * 0.02));
        return Math.abs(a[0] - b[0]) <= tol && Math.abs(a[1] - b[1]) <= tol;
    }

    /** 화면 비율 두 자리가 같은 자리인지 (화면 높이의 2% 안) */
    boolean nearRatio(double[] a, double[] b) {
        int tol = Math.max(4, (int) (H() * 0.02));
        return Math.abs(a[0] - b[0]) * W() <= tol && Math.abs(a[1] - b[1]) * H() <= tol;
    }

    /**
     * 화면에서 결과창 X 찾기 → {x, y, how} | null
     *  how 1: 결과창 (흰 꺾쇠) 오른쪽 위의 X 모양 · 2: 보정한 X 근처의 X 모양 · 3: 화면 오른쪽 위 X 모양 + 그 오른쪽에 결과창 테두리
     *      4: X 모양은 안 보이고 결과창 모양만 보임 → 창 크기로 계산한 자리 (closing 이거나 보정한 결과창과 같은 자리일 때만)
     *  closing: 릴링 직후 결과창을 닫는 중 → 바탕이 밝은 X 도 봄 (3) · 보정한 창과 크기가 달라도 계산한 자리 (4)
     *    (1.5.4 ~ 1.5.8 은 4 를 켜려다 '보정한 창과 같은 자리일 때만' 조건을 그대로 둬서 한 번도 안 눌렀음 → 크기가 다른 쓰레기 결과창을 못 닫음)
     *  · 전체 화면은 반으로 줄여서 한 번만 잡음 (창 테두리 · 결과창 테두리 줄 찾기) · X 모양은 원래 크기로 필요한 곳만
     *    (예전엔 원래 크기 전체 화면을 한 번 찾을 때 2 ~ 4번 잡아서 폰에서 느렸음)
     */
    int[] findX(boolean closing) {
        int W = W(), H = H();
        Img small = scr.grabStep(0, 0, W, H, 2);
        if (small == null || small.w < 8 || small.h < 8) return null;
        double kx = (double) W / small.w, ky = (double) H / small.h;
        int[] derived = null;
        List<int[]> wins = Vision.resultWindows(small, MAX_FRAME_W);
        // 큰 창부터 (결과창 안의 물고기 사진 테두리도 같은 꺾쇠라서 작은 창으로 잡힘 → 계산한 자리는 가장 큰 창으로)
        wins.sort((p1, q1) -> Integer.compare((q1[2] - q1[0]) * (q1[3] - q1[1]), (p1[2] - p1[0]) * (p1[3] - p1[1])));
        for (int[] s : wins) {
            int x1 = (int) (s[0] * kx), y1 = (int) (s[1] * ky), x2 = (int) ((s[2] + 1) * kx) - 1, y2 = (int) ((s[3] + 1) * ky) - 1;
            int w = x2 - x1, h = y2 - y1;
            int sx = x1 + (int) (w * 0.6), sy = Math.max(0, y1 - (int) (h * 0.03));
            int ex = Math.min(W, x2 + (int) (w * 0.05)), ey = Math.min(H, y1 + (int) (h * 0.25));
            if (ex - sx > 4 && ey - sy > 4) {
                double[] x = Vision.findCloseX(grab(sx, sy, ex - sx, ey - sy), H, null, true);
                if (x != null) return new int[]{sx + (int) x[0], sy + (int) x[1], 1};
            }
            if (derived == null && (closing || sameResult(x1, y1, x2, y2)))
                derived = new int[]{x1 + (int) (w * Vision.RESULT_CLOSE[0]), y1 + (int) (h * Vision.RESULT_CLOSE[1]), 4};
            // X 도 없고 결과창으로 볼 수도 없음 → 다른 창 (상점 · 메뉴 등) 일 수 있어서 안 누름 · 다음 창 확인
        }
        int[] p = cfg.closePos != null ? px(cfg.closePos[0], cfg.closePos[1]) : null;
        int left, right, top, bottom;
        if (p != null) {
            int half = Math.max(12, (int) (H * 0.10));
            left = p[0] - half;
            right = p[0] + half;
            top = p[1] - half;
            bottom = p[1] + half;
        } else {
            left = (int) (W * 0.2);
            right = (int) (W * 0.8);
            top = (int) (H * 0.05);
            bottom = (int) (H * 0.75);
        }
        left = Math.max(0, left);
        right = Math.min(W, right);
        top = Math.max(0, top);
        bottom = Math.min(H, bottom);
        if (right - left >= 8 && bottom - top >= 8) {
            Img im = grab(left, top, right - left, bottom - top);
            double[] f = Vision.findCloseX(im, H, p != null ? new double[]{p[0] - left, p[1] - top} : null);
            if (f != null) return new int[]{left + (int) f[0], top + (int) f[1], 2};
        }
        // 3. 보정한 X 근처에 없으면 (결과창이 다른 크기 · 자리로 뜸 · 반투명 창이 눈 위에 떠서 창 모양을 못 찾음 · 창 왼쪽 위가 로블록스 버튼에 가림)
        //    화면 오른쪽 위에서 X 를 찾되, X 오른쪽에 결과창 테두리가 아래로 길게 이어질 때만 (순위표 아이콘 등 제외)
        //    · 결과창을 닫는 중이면 바탕이 밝은 X (쓰레기 결과창) 도 봄 · 맨 위 하나만 보지 않고 후보마다 테두리를 확인
        int l2 = (int) (W * 0.45), t2 = (int) (H * 0.02), r2 = (int) (W * 0.92), b2 = (int) (H * 0.45);
        Img tr = grab(l2, t2, r2 - l2, b2 - t2);
        if (tr.w == r2 - l2 && tr.h == b2 - t2) {
            for (double[] g : Vision.findCloseXAll(tr, H, closing)) {
                int gx = l2 + (int) g[0], gy = t2 + (int) g[1];
                if (Vision.popupEdgeRight(small, (int) (gx / kx), (int) (gy / ky), small.h)) return new int[]{gx, gy, 3};
            }
        }
        return derived;
    }

    /** 찾은 창이 보정한 결과창과 같은 자리 · 크기인지 (화면 비율 3% 안) */
    boolean sameResult(int x1, int y1, int x2, int y2) {
        double[] k = cfg.resultRegion;
        if (k == null) return false;
        double W = W(), H = H();
        return Math.abs(x1 / W - Math.min(k[0], k[2])) < 0.03 && Math.abs(y1 / H - Math.min(k[1], k[3])) < 0.03
                && Math.abs(x2 / W - Math.max(k[0], k[2])) < 0.03 && Math.abs(y2 / H - Math.max(k[1], k[3])) < 0.03;
    }

    /** 결과창 X 누르기 (화면에서 찾은 X 먼저 · 없으면 보정한 자리) */
    void tapClose() {
        if (!clickFoundX() && cfg.closePos != null) tapRatio(cfg.closePos);
    }

    void rescue() {
        out.state("화면 복구 중");
        for (int i = 0; i < 3; i++) {
            tapClose();
            sleep(0.5);
        }
    }

    /**
     * 결과창 닫기 — Fish / Exit 버튼이 다시 보일 때까지
     *  · 누를 자리: 화면에서 찾은 X 모양 (두 번 연속 같은 자리에 보일 때만) → 보정한 X → 전에 눌러서 닫혔던 자리 (쓰레기 결과창처럼 크기가 다른 창)
     *    → X 모양은 없고 결과창 모양만 보이면 창 크기로 계산한 자리
     *  · X 가 나타날 시간 (X_SHOWN) 이 지났는데 한 자리를 DWELL 초 동안 눌러도 안 닫히면 다음 자리 · 다 해 봤으면 처음부터
     *  · 보정한 자리가 아닌 곳을 눌러서 닫혔으면 그 자리를 기억 (다음 결과창부터는 X 를 못 찾아도 바로 누름)
     */
    /** → "success" · "junk" · "fail" (셌을 때) · null (결과창을 못 닫았고 결과도 못 봄 → 안 셈) */
    String finish(Img base) {
        phase = 3;
        out.state("결과창 닫는 중");
        double[] title = base != null ? cfg.titlePos : null;
        double start = now(), end = start + FINISH_MAX, lookAt = start + LOOK_FROM;
        boolean seenFail = false, seenSuccess = false, seenJunk = false, closed = false;
        double[] found = null, guess = null, pend = null, target = null, lastTap = null;
        double targetAt = start, lastTapAt = -1;
        List<double[]> failed = new ArrayList<>();
        boolean first = true;
        while (now() < end) {
            check();
            String bs = Vision.buttonState(grabBox(cfg.fishBtn));
            if ("fish".equals(bs) || "exit".equals(bs)) {
                if (first) {
                    // 결과창이 한 번도 안 가렸음 → 결과가 없었던 것 (미니게임으로 잘못 봤거나 이미 닫힘) → 안 셈
                    lastClosed = true;
                    out.log("결과창이 안 보임 — 통계에 안 넣음", 'd');
                    return null;
                }
                closed = true;
                break;
            }
            first = false;
            if (title != null && !seenFail) {
                Img im = grabBox(title, 0.12, 0.05);
                if (im.w == base.w && im.h == base.h && meanAbsDiff(im, base) > 18) {
                    String k = Vision.classifyTitle(im);
                    if ("fail".equals(k)) seenFail = true;
                    else if ("success".equals(k)) seenSuccess = true;
                    else if ("junk".equals(k)) seenJunk = true;
                }
            }
            double t = now();
            if (t >= lookAt) {
                int[] hit = findX(true);
                double[] h = hit == null ? null : new double[]{(double) hit[0] / W(), (double) hit[1] / H(), hit[2]};
                if (h != null && pend != null && pend[2] == h[2] && nearRatio(h, pend)) {
                    if (h[2] == 4) {
                        guess = h;
                    } else if (found == null || !nearRatio(found, h)) {
                        found = h;
                        out.log(xHow((int) h[2]), 'd');
                    }
                    pend = null;
                    lookAt = now() + LOOK_GAP;
                } else {
                    pend = h;
                    lookAt = now() + (h != null ? 0.15 : LOOK_GAP);   // 처음 보인 자리는 바로 한 번 더 봐서 확인
                }
                t = now();
            }
            double[] want = pickClose(found, guess, failed);
            if (want != null && (target == null || !nearRatio(want, target))) {
                target = want;
                targetAt = t;
                if (want == guess) out.log(xHow(4), 'd');
            }
            if (target != null && t - start >= X_SHOWN && t - Math.max(targetAt, start + X_SHOWN) >= DWELL) {
                failed.add(target);
                out.log("누른 자리에서 결과창이 안 닫힘 — 다른 자리를 눌러 봄", 'd');
                target = null;
                continue;
            }
            if (target != null) {
                tapRatio(target);
                lastTap = target;
                lastTapAt = now();
            }
            sleep(FINISH_GAP);
        }
        lastClosed = closed;
        if (!closed) {
            out.log("결과창을 닫은 뒤 " + (int) FINISH_MAX + "초 동안 Fish 버튼이 보이지 않음 — 다시 확인", 'y');
        } else if (lastTap != null && (cfg.closePos == null || !nearRatio(lastTap, cfg.closePos)) && now() - lastTapAt < 1.5 && lastTapAt - targetAt >= 0.25) {
            // 닫히기 직전에 (0.25초 넘게 이어서) 누른 자리 = 닫은 자리 (보정한 자리가 아닐 때만 기억 · 막 다른 자리로 바꾼 직후면 어느 쪽인지 몰라서 안 함)
            cfg.learnClose(lastTap[0], lastTap[1]);
            out.log("결과창이 닫힌 자리를 기억함 (" + SellText.r4(lastTap[0]) + ", " + SellText.r4(lastTap[1]) + ")", 'd');
            if (onLearn != null) onLearn.run();
        }
        if (!closed && !seenFail && !seenSuccess && !seenJunk) {
            out.log("결과 확인 안 됨 — 통계에 안 넣음", 'd');
            return null;
        }
        String kind = seenFail ? "fail" : seenSuccess ? "success" : seenJunk ? "junk" : "success";
        if (kind.equals("fail")) fail++;
        else if (kind.equals("junk")) junk++;
        else success++;
        if (onResult != null) onResult.accept(kind.equals("fail") || kind.equals("junk") ? kind : "success");
        String name = kind.equals("success") ? "성공" : kind.equals("junk") ? "쓰레기" : "실패";
        out.log("낚시 결과: " + name + " · 성공 " + success + " / 쓰레기 " + junk + " / 실패 " + fail, kind.equals("success") ? 'g' : 'd');
        return kind;
    }

    /** 다음에 누를 결과창 X 자리 (화면 비율) — 찾은 X 모양 → 보정한 X → 기억한 자리 → 창 크기로 계산한 자리 · 다 안 됐으면 처음부터 */
    double[] pickClose(double[] found, double[] guess, List<double[]> failed) {
        List<double[]> c = new ArrayList<>();
        if (found != null) c.add(found);
        if (cfg.closePos != null) c.add(cfg.closePos);
        c.addAll(cfg.learnedClose());
        if (guess != null) c.add(guess);
        for (double[] p : c) {
            boolean bad = false;
            for (double[] f : failed) bad |= nearRatio(p, f);
            if (!bad) return p;
        }
        if (c.isEmpty()) return null;
        failed.clear();
        return c.get(0);
    }

    static double meanAbsDiff(Img a, Img b) {
        long s = 0;
        int n = a.w * a.h;
        for (int k = 0; k < n; k++) s += Math.abs(a.r[k] - b.r[k]) + Math.abs(a.g[k] - b.g[k]) + Math.abs(a.b[k] - b.b[k]);
        return n == 0 ? 0 : (double) s / (n * 3);
    }

    // ---------------------------------------------------------------- 릴링
    /** 릴링 → 진짜 미니게임이었는지 (내 위치가 움직이는 걸 봤는지) */
    boolean reel() {
        phase = 2;
        out.state("릴링 중");
        double[] region = cfg.barRegion;
        int[] a = px(Math.min(region[0], region[2]), Math.min(region[1], region[3]));
        int[] b = px(Math.max(region[0], region[2]), Math.max(region[1], region[3]));
        int bh = Math.max(4, b[1] - a[1]);
        int extra = (int) (bh * 1.8), side = Math.max(4, (int) ((b[0] - a[0]) * 0.05));
        int bx = a[0] - side, by = Math.max(0, a[1] - extra), bw = Math.max(8, b[0] - a[0]) + side * 2;
        int topIn = a[1] - by, bhh = topIn + bh + (int) (bh * 0.8);
        int[] xs0 = {side, side + Math.max(8, b[0] - a[0])};
        int[][] spots = tapSpots(bx, by, bw, bhh, a[1], bh);
        int spot = Math.floorMod(cfg.reelSpot, spots.length);
        in.hover(spots[spot][0], spots[spot][1]);
        return reelLoop(bx, by, bw, bhh, topIn, bh, xs0, spots, spot);
    }

    /**
     * 릴링 때 누를 자리들 (px) — 아무 데나 눌러도 릴링됨
     *  0: 기본 — PC 는 바 아래쪽 (미니게임 창 밑 빈 화면) · 모바일은 UI 가 커서 그 자리가 Roll 버튼 위 → 미니게임 창 위쪽 빈 화면 (캐릭터 쪽)
     *  1, 2: 기본 자리에서 터치가 안 먹힐 때 (다른 앱의 떠 있는 창 · 버튼이 그 자리를 가림) — 화면 왼쪽 가운데 · 오른쪽 위의 빈 화면
     *        (왼쪽 버튼 줄 · 조이스틱 · Roll · 순위표 · 오른쪽 알림은 피함)
     */
    int[][] tapSpots(int bx, int by, int bw, int bhh, int barTop, int bh) {
        int W = W(), H = H();
        int tapX = bx + bw / 2, tapY = Math.min(H - 2, by + bhh + bh * 3);
        if (cfg.tapAbove) {
            double[] rr = cfg.reelRegion;
            int top = rr != null ? (int) (Math.min(rr[1], rr[3]) * H) : barTop - (int) (bh * 1.5);
            tapY = Math.max((int) (H * 0.15), top - Math.max(bh, (int) (H * 0.05)) * 3 / 2);
        }
        return new int[][]{{tapX, tapY}, {(int) (W * 0.22), (int) (H * 0.42)}, {(int) (W * 0.70), (int) (H * 0.30)}};
    }

    /**
     * 릴링 — 새 화면이 올 때만 분석 (같은 화면을 다시 분석하지 않음 · 캡처 이미지를 매번 새로 만들지 않음)
     *  · 누를지는 REEL_POLL 초마다 ReelPilot 이 예측으로 정함 (화면과 화면 사이에도)
     *  · 내 위치는 보이는데 누른 게 화면에 한 번도 반영이 안 보이면 (6번 넘게 · 1초 넘게) 누르는 자리를 바꿈 · 잘 된 자리는 기억
     */
    boolean reelLoop(int bx, int by, int bw, int bhh, int top0, int bh0, int[] xs0, int[][] spots, int spot) {
        Double goneSince = null, diaOff = null, lostAt = null;
        ReelPilot pilot = new ReelPilot(cfg.reelFall, cfg.reelJump, cfg.reelDelay);
        double start = now(), end = start + REEL_MAX, diaAt = 0, snapAt = 0, spotAt = start;
        Boolean dia = null;
        boolean diaSeen = false, fixed = false, present = false, ended = false;
        int[] snap = null, fillRef = null;
        int topIn = top0, bh = bh0, spotSent = 0, spotSeen = 0, switches = 0;
        int[] xs = xs0;
        Img im = new Img(bw, bhh), bar = null;
        long frame = Long.MIN_VALUE, cancel0 = in.cancelled();
        reelTap = spots[spot];
        try {
            while (now() < end) {
                check();
                long fr = scr.grabInto(im, bx, by, frame);
                if (fr < 0) {
                    sleep(0.01);
                    continue;
                }
                double t = now();
                if (fr != frame) {
                    frame = fr;
                    if (!fixed) {
                        int[] f = Vision.findBarRows(im, top0, bh0);
                        if (f != null && f[3] - f[2] >= (xs0[1] - xs0[0]) * 0.7 && bh0 * 0.6 <= f[1] && f[1] <= bh0 * 1.5) {
                            boolean same = snap != null;
                            if (same) for (int i = 0; i < 4; i++) same &= Math.abs(f[i] - snap[i]) <= 1;
                            if (!same) {
                                snap = f;
                                snapAt = t;
                            } else if (t - snapAt >= 0.15) {
                                fixed = true;
                                topIn = f[0];
                                bh = f[1];
                                xs = new int[]{f[2] + 1, f[3]};
                            }
                        } else {
                            snap = null;
                        }
                    }
                    int cw = Math.max(1, Math.min(im.w, xs[1]) - xs[0]);
                    if (bar == null || bar.w != cw) bar = new Img(cw, im.h);
                    im.cropInto(bar, xs[0], 0);
                    if (fixed && fillRef == null) fillRef = Vision.learnFill(bar, topIn, bh);
                    Vision.Bar a = Vision.analyzeBar(bar, topIn, bh, fillRef);
                    present = a.present;
                    Probe pr = probe;
                    if (pr != null) {
                        try {
                            pr.reel(bx + xs[0], by + topIn, bar.w, bh, a.present ? a.marker : null, a.present ? a.zone : null);
                        } catch (RuntimeException ignored) {
                        }
                    }
                    if (fixed && !a.present) {
                        if (lostAt == null) lostAt = t;
                        if (t - lostAt > 0.3) {
                            fixed = false;
                            snap = null;
                            lostAt = null;
                            topIn = top0;
                            bh = bh0;
                            xs = xs0;
                        }
                    } else {
                        lostAt = null;
                    }
                    if (a.present) pilot.see(t, a.marker == null ? null : a.marker / Math.max(1, a.w), fractions(a.zones, a.w));
                    if (pr != null) {
                        try {
                            pr.pilot(pilot.lastPred, pilot.lastTarget, reelStats(pilot, start));
                        } catch (RuntimeException ignored) {
                        }
                    }
                }
                if (t - diaAt >= 0.1) {
                    diaAt = t;
                    dia = diamondMoved();
                    if (Boolean.TRUE.equals(dia)) {
                        diaSeen = true;
                        diaOff = null;
                        diamondOk = true;
                    } else if (Boolean.FALSE.equals(dia) && (diaSeen || diamondOk)) {
                        if (diaOff == null) diaOff = t;
                    }
                }
                if (diaOff != null && t - diaOff > 0.5) {
                    ended = true;
                    break;
                }
                // 4초가 지나도 내 위치가 한 번도 안 움직임 → 미니게임이 아님 (바처럼 보이는 다른 색) → 바로 끝냄
                // (그대로 두면 깜빡이는 색 때문에 '바가 사라짐' 도 안 돼서 15초를 다 기다리고 화면 복구까지 했음)
                if (!pilot.moved && t - start > BOGUS_AFTER) {
                    ended = true;
                    break;
                }
                if (!present) {
                    if (Boolean.TRUE.equals(dia)) {
                        goneSince = null;
                        sleep(0.01);
                        continue;
                    }
                    if (goneSince == null) goneSince = t;
                    if (t - goneSince > REEL_GONE) {
                        ended = true;
                        break;
                    }
                    sleep(0.01);
                    continue;
                }
                goneSince = null;
                if (pilot.decide(now())) {
                    int[] s = spots[spot];
                    in.tap(s[0], s[1], cfg.clickMs);
                    pilot.tapped(now());
                    spotSent++;
                }
                if (switches < spots.length - 1 && pilot.live && pilot.liveFrames >= 20 && spotSent >= 6 && pilot.seenJumps == spotSeen
                        && t - Math.max(spotAt, pilot.liveAt) >= 1.0) {
                    spot = (spot + 1) % spots.length;
                    switches++;
                    spotSent = 0;
                    spotSeen = pilot.seenJumps;
                    spotAt = t;
                    reelTap = spots[spot];
                    out.log("누른 게 미니게임에 반영되지 않음 — 릴링 때 누르는 자리를 바꿈 (" + (spot + 1) + "/" + spots.length + ")", 'y');
                }
                sleep(REEL_POLL);
            }
        } finally {
            reelDone(pilot, start, spot, spotSeen, cancel0);
            Probe pr = probe;
            if (pr != null) {
                try {
                    pr.pilot(Double.NaN, Double.NaN, reelStats(pilot, start));
                    pr.reelEnd();
                } catch (RuntimeException ignored) {
                }
            }
        }
        boolean real = pilot.moved && pilot.liveFrames >= 10;
        if (!ended) {
            out.log("미니게임이 " + (int) REEL_MAX + "초 넘게 끝나지 않음 — 멈춘 것으로 보고 닫기", 'y');
            rescue();
        }
        return real;
    }

    static List<double[]> fractions(List<int[]> zones, int w) {
        List<double[]> out = new ArrayList<>(zones.size());
        double k = 1.0 / Math.max(1, w);
        for (int[] z : zones) out.add(new double[]{z[0] * k, z[1] * k});
        return out;
    }

    /** 릴링이 끝나면: 기록 한 줄 (제보 받을 때 원인 보기) · 잘 잰 값은 저장 (다음 판의 처음 값) */
    /** 릴링 기록 (화면 표시용 · Probe.pilot 의 s) */
    static double[] reelStats(ReelPilot p, double start) {
        double dur = Math.max(0.001, now() - start);
        return new double[]{p.delay, p.seenTime > 0 ? p.inZone * 100 / p.seenTime : 0, p.sent, p.frames / dur, p.frames, p.markerFrames, p.zoneFrames,
                p.moved ? 1 : 0};
    }

    void reelDone(ReelPilot p, double start, int spot, int spotSeen, long cancel0) {
        lastRead = "바 화면 " + p.frames + "장 · 내 위치 읽음 " + p.markerFrames + "장 · 구간 읽음 " + p.zoneFrames + "장";
        if (p.markerFrames < 10 || !p.moved) return;           // 미니게임을 거의 못 봄 · 미니게임이 아니었음 → 기록 · 학습 안 함
        double dur = Math.max(0.001, now() - start);
        long cancelled = Math.max(0, in.cancelled() - cancel0);
        long zone = Math.round(p.seenTime > 0 ? p.inZone * 100 / p.seenTime : 0);
        String d = String.format(java.util.Locale.ROOT, "%.2f", p.delay);
        out.log("릴링: 터치 " + p.sent + "번 · 화면에 보임 " + p.seenJumps + " · 안 먹음 " + p.lost + " · 지연 " + d + "초 · 구간 안 " + zone
                + "% · 화면 " + Math.round(p.frames / dur) + "fps" + (cancelled > 0 ? " · 취소된 터치 " + cancelled : ""), 'd');
        if (p.fits >= 5 && p.seenJumps >= 5) {
            cfg.reelDelay = Math.round(p.delay * 1000) / 1000.0;
            cfg.reelFall = Vision.round4(p.fall);
            cfg.reelJump = Vision.round4(p.jump);
            if (p.seenJumps > spotSeen) cfg.reelSpot = spot;          // 이 자리에서 누른 게 반영됨 → 다음 판도 여기
            if (onLearn != null) onLearn.run();
        }
    }

    // ---------------------------------------------------------------- 자동 보정 (PC 버전 _autocal_run)
    /** 늘어난 화면용 넓은 비율 (안의 내용으로 확인할 때만) */
    static final double[][] LOOSE = {{1.6, 5.5}, {2.0, 6.5}, {0.35, 1.6}};
    /** 어두운 화면용 흰색 기준 (꺾쇠가 215 보다 어두울 때) */
    static final int[] DIM = {190, 165, 140};

    double[] frameOf(Img im, int keyIndex) {
        double lo = Vision.SHAPES[keyIndex][0], hi = Vision.SHAPES[keyIndex][1];
        List<int[]> frames = Vision.findFrames(im, MAX_FRAME_W);
        // 1. PC 와 같음: 모양 비율 (낚시 창은 Fish / Exit 버튼까지 — 순위표도 비율이 비슷함)
        for (int[] f : frames) {
            double ratio = ratio(f);
            if (lo <= ratio && ratio <= hi && placed(im, f) && (keyIndex == 2 || (keyIndex == 0 ? hasButton(im, f) : hasBar(im, f)))) return ratioBox(im, f);
        }
        // 2. 늘어난 화면: 비율을 넓게 보고 안의 내용으로 확인 (버튼 · 릴링 바 · X)
        double[] got = loose(im, keyIndex, frames);
        if (got != null) return got;
        // 3. 어두운 화면: 흰색 기준을 낮춰서 다시 (내용 확인 필수)
        for (int lo2 : DIM) {
            List<int[]> more = Vision.findFrames(im, MAX_FRAME_W, lo2);
            got = loose(im, keyIndex, more);
            if (got != null) return got;
        }
        return null;
    }

    double[] loose(Img im, int keyIndex, List<int[]> frames) {
        double lo = LOOSE[keyIndex][0], hi = LOOSE[keyIndex][1];
        for (int[] f : frames) {
            double r = ratio(f);
            if (r < lo || r > hi || !placed(im, f)) continue;
            boolean ok = keyIndex == 0 ? hasButton(im, f) : keyIndex == 1 ? hasBar(im, f) : hasX(im, f);
            if (ok) return ratioBox(im, f);
        }
        return null;
    }

    /** 낚시 창 · 미니게임 창 · 결과창은 화면 가로 가운데에 뜨고 화면 높이의 10% 이상 (순위표 · Roll 버튼 테두리는 아님) */
    static boolean placed(Img im, int[] f) {
        double cx = (f[0] + f[2]) / 2.0;
        return Math.abs(cx - im.w / 2.0) <= im.w * 0.12 && (f[3] - f[1]) >= im.h * 0.1;
    }

    static double ratio(int[] f) {
        return (double) (f[2] - f[0]) / Math.max(1, f[3] - f[1]);
    }

    static double[] ratioBox(Img im, int[] f) {
        return new double[]{Vision.round4((double) f[0] / im.w), Vision.round4((double) f[1] / im.h),
                Vision.round4((double) f[2] / im.w), Vision.round4((double) f[3] / im.h)};
    }

    /** 미니게임 창: 안에 릴링 바 테두리가 보임 */
    static boolean hasBar(Img im, int[] f) {
        double[] fr = {(double) f[0] / im.w, (double) f[1] / im.h, (double) f[2] / im.w, (double) f[3] / im.h};
        double[] r = Vision.layout(fr, Vision.REEL_BAR);
        int ax = (int) (r[0] * im.w), ay = (int) (r[1] * im.h), bx2 = (int) (r[2] * im.w), by2 = (int) (r[3] * im.h);
        int bh = Math.max(4, by2 - ay), extra = (int) (bh * 1.8), side = Math.max(4, (int) ((bx2 - ax) * 0.05));
        int x0 = Math.max(0, ax - side), y0 = Math.max(0, ay - extra), x1 = Math.min(im.w, bx2 + side), y1 = Math.min(im.h, ay + bh + (int) (bh * 0.8));
        if (x1 - x0 < 8 || y1 - y0 < 8) return false;
        int[] rows = Vision.findBarRows(im.crop(x0, y0, x1, y1), ay - y0, bh);
        // 낚시 창 (Fishing 밑줄이 바 테두리처럼 보임) 은 아님 — 미니게임 창엔 Fish / Exit 버튼이 없음
        return rows != null && rows[3] - rows[2] >= (bx2 - ax) * 0.6 && !hasButton(im, f);
    }

    /** 결과창: 오른쪽 위에 닫기 X */
    static boolean hasX(Img im, int[] f) {
        int w = f[2] - f[0], h = f[3] - f[1];
        int x0 = Math.max(0, f[0] + (int) (w * 0.6)), y0 = Math.max(0, f[1] - (int) (h * 0.03));
        int x1 = Math.min(im.w, f[2] + (int) (w * 0.05)), y1 = Math.min(im.h, f[1] + (int) (h * 0.25));
        if (x1 - x0 < 8 || y1 - y0 < 8) return false;
        return Vision.findCloseX(im.crop(x0, y0, x1, y1), im.h, null) != null;
    }

    static boolean hasButton(Img im, int[] f) {
        double[] fr = {(double) f[0] / im.w, (double) f[1] / im.h, (double) f[2] / im.w, (double) f[3] / im.h};
        double[] b = Vision.layout(fr, Vision.PANEL_FISH_BTN);
        int w = Math.max(6, (int) (im.w * 0.045)), h = Math.max(6, (int) (im.h * 0.035));
        int x = (int) (b[0] * im.w) - w / 2, y = (int) (b[1] * im.h) - h / 2;
        if (x < 0 || y < 0 || x + w > im.w || y + h > im.h) return false;
        String st = Vision.buttonState(im.crop(x, y, x + w, y + h));
        return "fish".equals(st) || "exit".equals(st);
    }

    Img full() {
        return grab(0, 0, W(), H());
    }

    double[] settled(int keyIndex, double sec) {
        double[] prev = null;
        double end = now() + sec;
        while (now() < end) {
            check();
            double[] cur = frameOf(full(), keyIndex);
            if (cur != null && prev != null) {
                boolean same = true;
                for (int i = 0; i < 4; i++) same &= Math.abs(cur[i] - prev[i]) <= 0.002;
                if (same) return cur;
            }
            prev = cur;
            sleep(0.1);
        }
        return null;
    }

    boolean biting() {
        return barSeen() || Boolean.TRUE.equals(diamondMoved());
    }

    /** 낚시 자리에서 Fish 버튼이 보일 때: 대기 창 → Fish → 입질 → 미니게임 창 → 릴링 → 결과창 순서로 잼 · 결과 설명 */
    public String calibrate() {
        StringBuilder done = new StringBuilder();
        phase = 5;                                                // 화면 표시: 찾기 전엔 전에 보정한 칸을 안 그림 · 찾은 칸만 단계에 맞게
        try {
            out.state("자동 보정: 낚시 창 찾는 중");
            double[] panel = frameOf(full(), 0);
            if (panel == null) {
                Map<String, double[]> any = Vision.autocal(full(), MAX_FRAME_W);
                return "낚시 창을 찾지 못함 — 낚시 자리에서 Fish 버튼이 보일 때 눌러주세요" + (any.isEmpty() ? "" : " (다른 창만 보임)");
            }
            cfg.apply("panel_region", panel);
            cfg.deriveReel();
            phase = 1;
            done.append("대기 창");
            out.state("자동 보정: Fish 터치");
            boolean cast = false;
            for (int i = 0; i < 3 && !cast; i++) {
                check();
                // 첫 입력도 이미 던진 상태면 Exit 를 누르지 않음
                String bs = Vision.buttonState(grabBox(cfg.fishBtn));
                if (!"fish".equals(bs) && !"exit".equals(bs)) {
                    sleep(1.0);
                    bs = Vision.buttonState(grabBox(cfg.fishBtn));
                }
                if ("exit".equals(bs) || biting()) {
                    cast = true;
                    break;
                }
                if (!"fish".equals(bs)) continue;
                tapRatio(cfg.fishBtn);
                double end = now() + CAST_WAIT;
                while (now() < end) {
                    check();
                    if ("exit".equals(Vision.buttonState(grabBox(cfg.fishBtn))) || biting()) {
                        cast = true;
                        break;
                    }
                    sleep(0.1);
                }
            }
            if (!cast) return done + " 맞춤 · Fish 를 눌러도 반응이 없음 (인벤토리 가득?)";
            out.state("자동 보정: 입질 기다리는 중");
            double end = now() + cfg.biteMax + 10;
            boolean bit = false;
            while (now() < end) {
                check();
                if (biting()) {
                    bit = true;
                    break;
                }
                sleep(0.1);
            }
            if (!bit) {
                tapRatio(cfg.fishBtn);
                return done + " 맞춤 · 입질이 오지 않아 미니게임 창은 재지 못함";
            }
            double[] reel = settled(1, 3.0);
            if (reel != null) {
                cfg.apply("reel_region", reel);
                cfg.reelDerived = false;
                done.append(" · 미니게임 창");
            } else {
                done.append(" · 미니게임 창 (대기 창으로 계산)");
            }
            out.state("자동 보정: 릴링 중");
            reel();
            double[] result = settled(2, 6.0);
            if (result != null) {
                cfg.apply("result_region", result);
                phase = 3;
                done.append(" · 결과창");
            }
            double stopAt = now() + FINISH_MAX;
            while (now() < stopAt) {
                String bs = Vision.buttonState(grabBox(cfg.fishBtn));
                if ("fish".equals(bs) || "exit".equals(bs)) break;
                if (cfg.closePos != null) tapRatio(cfg.closePos);
                else clickFoundX();
                sleep(0.2);
            }
            return done + " 맞춤" + (cfg.closePos == null ? " · 결과창은 못 잼 (X 는 화면에서 찾아서 누름)" : "");
        } catch (Stopped e) {
            return "자동 보정 취소됨";
        } finally {
            phase = 0;
            out.state("대기");
        }
    }

    /** 테스트용: 지금 화면에서 찾은 창들 */
    public List<int[]> frames(Img im) {
        return Vision.findFrames(im, MAX_FRAME_W);
    }
}
