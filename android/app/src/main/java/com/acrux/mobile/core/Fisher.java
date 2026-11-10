package com.acrux.mobile.core;

import java.util.List;
import java.util.Map;

/**
 * 자동 낚시 (제자리 낚시) — PC 버전 fishing.Fisher 의 상태 기계를 모바일용으로 옮김
 *   · 대기(파란 Fish) → Fish 터치 · 1.5초 안에 안 바뀌면 다시 · castRetry 번 넘으면 인벤토리 가득 → 정지
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
    }

    public interface Input {
        void tap(int x, int y, int ms);

        /** 릴링 전에 누를 자리로 마우스를 미리 옮겨 둠 (PC: move_to 뒤 mouse_click_here) */
        default void hover(int x, int y) {}
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

    public static final double REEL_FRAME = 0.008, REEL_MAX = 15.0, REEL_GONE = 0.6;
    public static final double FINISH_GAP = 0.1, FINISH_MAX = 10.0, FIND_X_AFTER = 2.0;
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
    public static final double RELOC_AFTER = 30;

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

    int white(double[] win, double[] rel) {
        double x1 = Math.min(win[0], win[2]), x2 = Math.max(win[0], win[2]);
        double y1 = Math.min(win[1], win[3]), y2 = Math.max(win[1], win[3]);
        int size = Math.max(3, (int) (W() * (x2 - x1) * 0.012));
        int[] p = px(x1 + rel[0] * (x2 - x1), y1 + rel[1] * (y2 - y1));
        Img im = grab(p[0] - size, p[1] - size, size * 2 + 1, size * 2 + 1);
        int c = 0;
        for (int k = 0; k < im.w * im.h; k++) if (im.min(k) > 200) c++;
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
        try {
            if (onStart != null) {
                out.state("낚시 장소로 가는 중");
                onStart.run();
            }
            loop();
        } catch (Stopped e) {
            out.log("자동 낚시 정지", 'd');
        } catch (RuntimeException e) {
            out.log("자동 낚시 오류: " + e, 'r');
        } finally {
            out.state("대기");
        }
    }

    double refitAt = -1;

    /**
     * 낚시 창을 화면에서 다시 찾아서, 보정 때 창 자리 → 지금 창 자리로 모든 위치 (Fish 버튼 · 릴링 바 · 결과창 X · 제목 · 창 영역) 를 옮김
     * → 옮겼으면 true (Fish / Exit 버튼이 보이는 낚시 창을 찾았고, 자리가 달라졌을 때)
     */
    boolean refit() {
        double[] old = cfg.panelRegion;
        if (old == null) return false;
        double[] now = frameOf(full(), 0);
        if (now == null) return false;
        double ow = old[2] - old[0], oh = old[3] - old[1], nw = now[2] - now[0], nh = now[3] - now[1];
        if (ow <= 0 || oh <= 0) return false;
        double moved = Math.abs(now[0] - old[0]) + Math.abs(now[1] - old[1]) + Math.abs(nw - ow) + Math.abs(nh - oh);
        if (moved < 0.006) return false;
        double sx = nw / ow, sy = nh / oh;
        java.util.function.UnaryOperator<double[]> map = p -> {
            if (p == null) return null;
            double[] q = p.clone();
            for (int i = 0; i + 1 < q.length; i += 2) {
                q[i] = now[0] + (p[i] - old[0]) * sx;
                q[i + 1] = now[1] + (p[i + 1] - old[1]) * sy;
            }
            return q;
        };
        // 옮긴 Fish 버튼 자리에 정말 버튼이 있는지 확인한 뒤에만 바꿈
        double[] fb = map.apply(cfg.fishBtn);
        if (fb == null || Vision.buttonState(grabBox(fb)) == null) return false;
        cfg.fishBtn = fb;
        cfg.barRegion = map.apply(cfg.barRegion);
        cfg.closePos = map.apply(cfg.closePos);
        cfg.titlePos = map.apply(cfg.titlePos);
        cfg.reelRegion = map.apply(cfg.reelRegion);
        cfg.resultRegion = map.apply(cfg.resultRegion);
        cfg.panelRegion = now.clone();
        out.log("낚시 창 자리가 바뀜 → 위치를 다시 맞춤 (Fish 버튼 " + SellText.r4(fb[0]) + ", " + SellText.r4(fb[1]) + ")", 'c');
        return true;
    }

    void loop() {
        Double castAt = null, waitAt = null, unknownAt = null;
        int tries = 0, rescues = 0, maybe = 0, relocs = 0;
        double progressAt = now(), knownAt = now();
        while (true) {
            check();
            String st = readState();
            double t = now();
            if (t - progressAt > cfg.biteMax + 60) {
                rescues++;
                if (rescues > 3) {
                    out.log("낚시 화면을 계속 찾지 못함 — 자동 낚시 멈춤 (낚시 자리 · 자동 보정 확인)", 'r');
                    throw new Stopped();
                }
                out.log("한동안 낚시가 진행되지 않음 — 화면 복구 (" + rescues + "/3)", 'y');
                rescue();
                progressAt = now();
                castAt = waitAt = unknownAt = null;
                tries = 0;
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
            if (st != null) {
                unknownAt = null;
                knownAt = t;
            }

            if ("reel".equals(st)) {
                castAt = null;
                tries = 0;
                relocs = 0;
                progressAt = t;
                rescues = 0;
                Img base = cfg.titlePos != null ? grabBox(cfg.titlePos, 0.12, 0.05) : null;
                reel();
                finish(base);
                continue;
            }
            if ("wait".equals(st)) {
                castAt = null;
                tries = 0;
                if (waitAt == null) waitAt = t;
                if (t - waitAt > cfg.biteMax) {
                    out.log("입질이 " + (int) cfg.biteMax + "초 동안 없음 — Exit 후 다시 던짐", 'y');
                    tapRatio(cfg.fishBtn);
                    sleep(1.0);
                    continue;
                }
                out.state("입질 기다리는 중");
            } else if ("idle".equals(st)) {
                if (castAt == null || t - castAt > 1.5) {
                    if (castAt != null) {
                        tries++;
                        if (tries >= cfg.castRetry) {
                            full++;
                            if (onFull != null) {
                                out.log("Fish 를 " + tries + "번 눌러도 반응 없음 — 낚시 인벤토리 가득", 'y');
                                out.state("인벤토리 가득 — 판매하러 감");
                                if (onFull.getAsBoolean()) {
                                    castAt = waitAt = unknownAt = null;
                                    tries = 0;
                                    progressAt = knownAt = now();
                                    continue;
                                }
                            } else {
                                out.log("Fish 를 " + tries + "번 눌러도 반응 없음 — 낚시 인벤토리 가득 · 정지", 'y');
                            }
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
                // 30초 동안 낚시 화면이 한 번도 안 보임 → 낚시 장소에 못 간 것 → 다시 이동 (2번까지 · 그래도 안 되면 멈춤)
                if (onStart != null && t - knownAt > RELOC_AFTER) {
                    if (relocs >= 2) {
                        out.log("낚시 장소로 다시 가도 낚시 화면이 안 보임 — 자동 낚시 멈춤 (경로 · 위치 확인)", 'r');
                        throw new Stopped();
                    }
                    relocs++;
                    out.log((int) RELOC_AFTER + "초 동안 낚시 화면이 안 보임 — 낚시 장소로 다시 이동 (" + relocs + "/2)", 'y');
                    out.state("낚시 장소로 다시 가는 중");
                    onStart.run();
                    knownAt = progressAt = now();
                    unknownAt = null;
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

    /** 결과창 X 를 화면에서 찾아서 누름 (PC _click_found_x 와 같음)
     *  1. 결과창 자체를 찾아서 그 창의 X (쓰레기 결과창처럼 크기 · 바탕이 다른 창도) → 2. 가운데 넓은 영역에서 어두운 바탕 위 X */
    boolean clickFoundX() {
        int W = W(), H = H();
        double[] r = Vision.findResultClose(full(), H, MAX_FRAME_W);
        if (r != null) {
            out.log("결과창 X 를 결과창 모양으로 찾아서 누름", 'd');
            in.tap((int) r[0], (int) r[1], 40);
            return true;
        }
        int[] p = cfg.closePos != null ? px(cfg.closePos[0], cfg.closePos[1]) : null;
        int half = Math.max(10, (int) (H * 0.06));
        int left = (int) (W * 0.2), right = (int) (W * 0.8);
        if (p != null) {
            left = Math.min(p[0] - half, left);
            right = Math.max(p[0] + half, right);
        }
        int top = (int) (H * 0.05), h = (int) (H * 0.7);
        left = Math.max(0, left);
        right = Math.min(W, right);
        Img im = grab(left, top, Math.max(1, right - left), h);
        double[] f = Vision.findCloseX(im, H, p != null ? new double[]{p[0] - left, p[1] - top} : null);
        if (f == null) return false;
        out.log("결과창이 다른 자리에 뜨거나 깨짐 — X 를 찾아서 누름", 'd');
        in.tap(left + (int) f[0], top + (int) f[1], 40);
        return true;
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

    void finish(Img base) {
        out.state("결과창 닫는 중");
        double[] title = base != null ? cfg.titlePos : null;
        double start = now(), end = start + FINISH_MAX, findAt = start + FIND_X_AFTER;
        boolean seenFail = false, seenSuccess = false, seenJunk = false, closed = false;
        while (now() < end) {
            check();
            String bs = Vision.buttonState(grabBox(cfg.fishBtn));
            if ("fish".equals(bs) || "exit".equals(bs)) {
                closed = true;
                break;
            }
            if (title != null && !seenFail) {
                Img im = grabBox(title, 0.12, 0.05);
                if (im.w == base.w && im.h == base.h && meanAbsDiff(im, base) > 18) {
                    String k = Vision.classifyTitle(im);
                    if ("fail".equals(k)) seenFail = true;
                    else if ("success".equals(k)) seenSuccess = true;
                    else if ("junk".equals(k)) seenJunk = true;
                }
            }
            if (now() >= findAt) {
                findAt = now() + 0.5;
                if (clickFoundX()) {
                    sleep(0.3);
                    continue;
                }
            }
            if (cfg.closePos != null) tapRatio(cfg.closePos);
            else clickFoundX();
            sleep(FINISH_GAP);
        }
        if (!closed) out.log("결과창을 닫은 뒤 " + (int) FINISH_MAX + "초 동안 Fish 버튼이 보이지 않음 — 다시 확인", 'y');
        String kind = seenFail ? "fail" : seenSuccess ? "success" : seenJunk ? "junk" : "success";
        if (kind.equals("fail")) fail++;
        else if (kind.equals("junk")) junk++;
        else success++;
        if (onResult != null) onResult.accept(kind.equals("fail") || kind.equals("junk") ? kind : "success");
        String name = kind.equals("success") ? "성공" : kind.equals("junk") ? "쓰레기" : "실패";
        out.log("낚시 결과: " + name + " · 성공 " + success + " / 쓰레기 " + junk + " / 실패 " + fail, kind.equals("success") ? 'g' : 'd');
    }

    static double meanAbsDiff(Img a, Img b) {
        long s = 0;
        int n = a.w * a.h;
        for (int k = 0; k < n; k++) s += Math.abs(a.r[k] - b.r[k]) + Math.abs(a.g[k] - b.g[k]) + Math.abs(a.b[k] - b.b[k]);
        return n == 0 ? 0 : (double) s / (n * 3);
    }

    // ---------------------------------------------------------------- 릴링
    void reel() {
        out.state("릴링 중");
        double[] region = cfg.barRegion;
        int[] a = px(Math.min(region[0], region[2]), Math.min(region[1], region[3]));
        int[] b = px(Math.max(region[0], region[2]), Math.max(region[1], region[3]));
        int bh = Math.max(4, b[1] - a[1]);
        int extra = (int) (bh * 1.8), side = Math.max(4, (int) ((b[0] - a[0]) * 0.05));
        int bx = a[0] - side, by = Math.max(0, a[1] - extra), bw = Math.max(8, b[0] - a[0]) + side * 2;
        int topIn = a[1] - by, bhh = topIn + bh + (int) (bh * 0.8);
        int[] xs0 = {side, side + Math.max(8, b[0] - a[0])};
        // 누르는 자리: 아무 데나 눌러도 릴링됨 — PC 는 바 아래쪽 (미니게임 창 밑 빈 화면)
        // 모바일은 UI 가 커서 그 자리가 Roll 버튼 위 → 미니게임 창 위쪽 빈 화면 (캐릭터 쪽)을 누름
        int tapX = bx + bw / 2, tapY = Math.min(H() - 2, by + bhh + bh * 3);
        if (cfg.tapAbove) {
            double[] rr = cfg.reelRegion;
            int top = rr != null ? (int) (Math.min(rr[1], rr[3]) * H()) : a[1] - (int) (bh * 1.5);
            tapY = Math.max((int) (H() * 0.15), top - Math.max(bh, (int) (H() * 0.05)) * 3 / 2);
        }
        in.hover(tapX, tapY);
        reelLoop(bx, by, bw, bhh, topIn, bh, xs0, tapX, tapY);
    }

    void reelLoop(int bx, int by, int bw, int bhh, int top0, int bh0, int[] xs0, int tapX, int tapY) {
        Double goneSince = null, diaOff = null, lostAt = null;
        ReelControl ctl = new ReelControl(cfg);
        double start = now(), end = start + REEL_MAX, diaAt = 0, snapAt = 0;
        Boolean dia = null;
        boolean diaSeen = false, fixed = false;
        int[] snap = null, fillRef = null;
        int topIn = top0, bh = bh0;
        int[] xs = xs0;
        while (now() < end) {
            double t0 = now();
            check();
            Img im = grab(bx, by, bw, bhh);
            if (im.w == 0) {
                sleep(0.01);
                continue;
            }
            if (!fixed) {
                int[] f = Vision.findBarRows(im, top0, bh0);
                if (f != null && f[3] - f[2] >= (xs0[1] - xs0[0]) * 0.7 && bh0 * 0.6 <= f[1] && f[1] <= bh0 * 1.5) {
                    boolean same = snap != null;
                    if (same) for (int i = 0; i < 4; i++) same &= Math.abs(f[i] - snap[i]) <= 1;
                    if (!same) {
                        snap = f;
                        snapAt = t0;
                    } else if (t0 - snapAt >= 0.15) {
                        fixed = true;
                        topIn = f[0];
                        bh = f[1];
                        xs = new int[]{f[2] + 1, f[3]};
                    }
                } else {
                    snap = null;
                }
            }
            Img bar = im.crop(xs[0], 0, xs[1], im.h);
            if (fixed && fillRef == null) fillRef = Vision.learnFill(bar, topIn, bh);
            Vision.Bar a = Vision.analyzeBar(bar, topIn, bh, fillRef);
            if (fixed && !a.present) {
                if (lostAt == null) lostAt = t0;
                if (t0 - lostAt > 0.3) {
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
            double t = now();
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
            if (diaOff != null && t - diaOff > 0.5) return;
            if (!a.present) {
                if (Boolean.TRUE.equals(dia)) {
                    goneSince = null;
                    sleep(0.01);
                    continue;
                }
                if (goneSince == null) goneSince = t;
                if (t - goneSince > REEL_GONE) return;
                sleep(0.01);
                continue;
            }
            goneSince = null;
            boolean click = ctl.step(t, a.marker, a.zone, a.w, a.zones);
            if (click) {
                in.tap(tapX, tapY, cfg.clickMs);
                ctl.clicked(now());
            }
            double rest = REEL_FRAME - (now() - t0);
            if (rest > 0.001) sleep(rest);
        }
        out.log("미니게임이 " + (int) REEL_MAX + "초 넘게 끝나지 않음 — 멈춘 것으로 보고 닫기", 'y');
        rescue();
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
        try {
            out.state("자동 보정: 낚시 창 찾는 중");
            double[] panel = frameOf(full(), 0);
            if (panel == null) {
                Map<String, double[]> any = Vision.autocal(full(), MAX_FRAME_W);
                return "낚시 창을 찾지 못함 — 낚시 자리에서 Fish 버튼이 보일 때 눌러주세요" + (any.isEmpty() ? "" : " (다른 창만 보임)");
            }
            cfg.apply("panel_region", panel);
            cfg.deriveReel();
            done.append("대기 창");
            out.state("자동 보정: Fish 터치");
            boolean cast = false;
            for (int i = 0; i < 3 && !cast; i++) {
                tapRatio(cfg.fishBtn);
                double end = now() + 1.5;
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
            out.state("대기");
        }
    }

    /** 테스트용: 지금 화면에서 찾은 창들 */
    public List<int[]> frames(Img im) {
        return Vision.findFrames(im, MAX_FRAME_W);
    }
}
