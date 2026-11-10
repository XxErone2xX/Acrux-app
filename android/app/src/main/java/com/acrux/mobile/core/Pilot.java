package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.List;

/**
 * 손가락 두 개 (조이스틱 · 점프) 를 움직이는 머리 — 실제 터치는 Hand (안드로이드: Joy) 가 보냄
 *  · 바로 하기 (step): 원하는 방향 · 점프를 받아서 누르고 있음 (길 기록 · 테스트) — 손가락이 실제로 한 일을 시각과 함께 적음 (trace)
 *  · 다시 하기 (play): 적어 둔 일을 같은 시각에 그대로 다시 함 — 누른 시간이 기록과 같아야 간 거리가 같음
 *    터치 조각의 끝을 다음 일의 시각에 맞춰 보냄 (기다렸다 떼는 게 아니라 뗄 시각에 끝나는 조각을 보냄) · 시각은 처음부터 잰 값이라 밀려도 쌓이지 않음
 */
public final class Pilot {
    public interface Hand {
        /** 터치를 보낼 수 있나 (stick: 조이스틱 위치도 있어야 함) */
        boolean ready(boolean stick);

        /**
         * 새 터치: 조이스틱 손가락을 가운데에서 dir 쪽으로 frac (0 ~ 1) 만큼 ms 동안 끌기 (dir null = 없음) · jump = 점프 버튼도 누름
         * keep = 끝나도 누르고 있음 (false 면 끝에서 뗌) · 끝까지 갔으면 true (끊기면 false · 손가락 다 떨어짐)
         */
        boolean fresh(double[] dir, double frac, boolean jump, long ms, boolean keep);

        /** 누르고 있는 손가락을 ms 동안 이어 감 (dir != null 이면 조이스틱을 그쪽으로 옮김) · 끝에서 뗄 손가락 · 끊기면 false */
        boolean cont(double[] dir, long ms, boolean liftStick, boolean liftJump);

        /** 손가락이 다 떨어진 걸로 (사람이 화면을 눌러서 끊김) */
        void forget();

        long nanos();

        void sleepUntil(long nanos);

        /** 사람 손가락이 기록용 패드에 닿아 있으면 닿은 시각 (nanos) · 아니면 0 */
        long userDown();
    }

    public static final long START_MS = 90, MOVE_MS = 60, HOLD_MS = 100, JUMP_MIN_MS = 150, MIN_MS = 15;
    static final long MS = 1_000_000L;

    final Hand hand;
    final Object lock = new Object();
    double tx, ty;
    boolean want, wantJump;
    int jumpAsked, jumpDone;
    long jumpUntil;
    volatile boolean hs, hj;              // 지금 닿아 있는 손가락 (조이스틱 · 점프)
    double fx, fy;                        // 조이스틱 손가락이 가 있는 방향
    volatile boolean playing;
    boolean stepping;                     // 바로 하기가 터치를 보내는 중 (다시 하기는 끝날 때까지 기다림)
    /** 터치 하나를 보내고 끝났다는 답이 오기까지 더 걸리는 시간 (ms · 계속 잼) */
    volatile double ovh = 6;
    public volatile boolean holding;
    public volatile int jumpFallbacks;

    List<Route.Ev> trace;
    long traceT0;

    public Pilot(Hand hand) {
        this.hand = hand;
    }

    // ---------------------------------------------------------------- 바로 하기
    public void set(double[] d) {
        synchronized (lock) {
            if (d == null) {
                want = false;
            } else {
                tx = d[0];
                ty = d[1];
                want = true;
            }
            lock.notifyAll();
        }
    }

    public void jump(boolean down) {
        synchronized (lock) {
            if (down && !wantJump) jumpAsked++;
            wantJump = down;
            lock.notifyAll();
        }
    }

    public boolean idle() {
        return !hs && !hj;
    }

    public void traceStart() {
        synchronized (lock) {
            trace = new ArrayList<>();
            traceT0 = hand.nanos();
        }
    }

    public List<Route.Ev> traceEnd() {
        synchronized (lock) {
            List<Route.Ev> t = trace;
            trace = null;
            return t == null ? new ArrayList<>() : t;
        }
    }

    void log(char k, long at, double dx, double dy, boolean a, boolean b) {
        synchronized (lock) {
            if (trace != null) trace.add(new Route.Ev(k, (at - traceT0) / 1e6, dx, dy, a, b));
        }
    }

    long lastOk;

    /**
     * 끊긴 시각 (기록 기준 = 터치를 보낸 시각 · 실제 손가락은 보낸 뒤 반 지연만큼 늦게 움직임 → 같은 기준으로 맞춤)
     *  · 사람이 패드를 누른 때 (그때 안드로이드가 끊음 · 마지막으로 잘 끝난 터치 뒤일 때) − 반 지연
     *  · 아니면 끊겼다는 답이 온 때 − 지연
     */
    long cutAt(long t, long te) {
        long ud = hand.userDown();
        if (ud != 0 && ud >= Math.max(lastOk, t) - ovhNanos() && ud <= te) return ud - ovhNanos() / 2;     // (이 터치를 보내기 직전 · 보내는 중에 누른 것만)
        return Math.max(t, te - ovhNanos());
    }

    void measure(long elapsed, long ms) {
        double o = elapsed / 1e6 - ms;
        if (o >= 0 && o < 40) ovh = ovh * 0.9 + o * 0.1;
    }

    long ovhNanos() {
        return (long) (ovh * MS);
    }

    public void run() {
        while (true) {
            boolean did;
            try {
                did = step();
            } catch (RuntimeException e) {
                did = false;
                hs = hj = holding = false;
                hand.forget();
                synchronized (lock) {
                    want = wantJump = false;
                    jumpDone = jumpAsked;
                }
            }
            if (did) continue;
            synchronized (lock) {
                if (playing || (!want && !wantJump && jumpAsked == jumpDone && !hs && !hj)) {
                    try {
                        lock.wait(5000);
                    } catch (InterruptedException e) {
                        return;
                    }
                }
            }
        }
    }

    /** 터치 조각 하나 (할 일이 없으면 false) */
    public boolean step() {
        synchronized (lock) {
            if (playing) return false;
            stepping = true;
        }
        try {
            return step0();
        } finally {
            synchronized (lock) {
                stepping = false;
                lock.notifyAll();
            }
        }
    }

    boolean step0() {
        boolean ws, wj;
        double dx, dy;
        int asked;
        synchronized (lock) {
            if (playing) return false;
            wj = wantJump || jumpAsked != jumpDone || (hj && hand.nanos() < jumpUntil);
            ws = want;
            dx = tx;
            dy = ty;
            asked = jumpAsked;
            if (!ws && !wj && !hs && !hj) return false;
        }
        if (!hand.ready(ws)) {
            if (hs || hj) log('X', hand.nanos(), 0, 0, false, false);
            hs = hj = holding = false;
            hand.forget();
            synchronized (lock) {
                want = wantJump = false;
                jumpDone = jumpAsked;
            }
            return true;
        }
        // 사람 손가락이 패드에 닿아 있는 동안은 새로 누르지 않음 (누르자마자 끊김) — 누르던 손가락은 그대로 이어 감
        //  · 기종에 따라 사람 터치에 Acrux 터치가 끊기기도 하고 안 끊기기도 함 → '끊겼겠지' 하고 잊으면 안 끊긴 기종에서는
        //    로블록스 쪽 손가락이 눌린 채 남아서 키를 꺼도 계속 걸어감 (V1.4.1 버그) → 이어 가기가 실패했을 때만 끊긴 걸로 봄
        boolean userOn = hand.userDown() != 0;
        if (userOn && !hs && !hj) {
            hand.sleepUntil(hand.nanos() + 4 * MS);
            return true;
        }
        if (!userOn && ((ws && !hs) || (wj && !hj))) {
            // 새 손가락이 필요 → 누르고 있어야 하는 손가락 전부를 새 터치로 다시 시작
            long t = hand.nanos();
            boolean ok = hand.fresh(ws ? new double[]{dx, dy} : null, 1, wj, START_MS, true);
            long te = hand.nanos();
            // 적는 건 결과를 본 뒤 (누르기 전에 이미 끊겼으면 안 누른 것 · 다시 할 때 쓸데없는 짧은 누름이 생기지 않게)
            long cut = ok ? 0 : cutAt(t, te);
            if (ok || cut > t) log('F', t, ws ? dx : 0, ws ? dy : 0, ws, wj);
            if (ok) {
                measure(te - t, START_MS);
                lastOk = te;
                hs = ws;
                hj = wj;
                fx = dx;
                fy = dy;
                if (wj) {
                    synchronized (lock) {
                        if (jumpDone != asked) {
                            jumpDone = asked;
                            jumpUntil = te + (JUMP_MIN_MS - (START_MS - 30)) * MS;
                        }
                    }
                }
            } else {
                if (cut > t) log('X', cut, 0, 0, false, false);
                hs = hj = false;
                if (wj) jumpFallbacks++;
                hand.sleepUntil(te + 15 * MS);
            }
            holding = hs;
            return true;
        }
        // 이어 가기: 손가락마다 그대로 · 움직임 · 떼기
        boolean moving = hs && ws && Math.hypot(dx - fx, dy - fy) > 0.02;
        long ms = moving ? MOVE_MS : HOLD_MS;
        if (!ws && !wj) ms = 30;
        boolean ls = hs && !ws, lj = hj && !wj;
        long t = hand.nanos();
        boolean ok = hand.cont(moving ? new double[]{dx, dy} : null, ms, ls, lj);
        long te = hand.nanos();
        long cut = ok ? 0 : cutAt(t, te);
        if (moving && (ok || cut > t)) log('M', t, dx, dy, false, false);      // 옮기기 전에 끊겼으면 옮긴 적 없음
        if (ok) {
            measure(te - t, ms);
            lastOk = te;
            if (ls || lj) log('U', Math.max(t, te - ovhNanos()), 0, 0, ls, lj);
            if (moving) {
                fx = dx;
                fy = dy;
            }
            if (ls) hs = false;
            if (lj) hj = false;
        } else {
            log('X', cut, 0, 0, false, false);
            hs = hj = false;
            hand.sleepUntil(te + 10 * MS);
        }
        holding = hs;
        return true;
    }

    // ---------------------------------------------------------------- 다시 하기
    boolean ps, pj, wS, wJ;               // 닿아 있는 손가락 · 닿아 있어야 하는 손가락
    double[] dir;

    /** 적어 둔 일을 같은 시각에 그대로 (check: 멈춤 확인 — 멈추면 예외) */
    public void play(List<Route.Ev> evs, Runnable check) {
        synchronized (lock) {
            playing = true;
            want = wantJump = false;
            jumpDone = jumpAsked;
            long until = System.currentTimeMillis() + 3000;
            while (stepping && System.currentTimeMillis() < until) {               // 바로 하기가 보내던 터치가 끝날 때까지
                try {
                    lock.wait(50);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    break;
                }
            }
        }
        try {
            if (hs || hj) {                                                        // 바로 하기가 누르던 손가락은 뗌
                hand.cont(null, 20, hs, hj);
                hand.forget();
                hs = hj = holding = false;
            }
            ps = pj = wS = wJ = false;
            dir = null;
            long t0 = hand.nanos() + 20 * MS;
            int n = evs.size();
            for (int i = 0; i < n; i++) {
                Route.Ev e = evs.get(i), nx = i + 1 < n ? evs.get(i + 1) : null;
                long at = t0 + (long) (e.t * MS);
                if (e.k == 'F') {
                    if (!e.a && !e.b) {                                 // 점프를 뺀 길의 빈 누름 → 그 전 손가락만 뗌
                        alive(at, true, true, check);
                        wS = wJ = false;
                        continue;
                    }
                    reach(at, check);
                    double[] v = e.a ? new double[]{e.dx, e.dy} : null;
                    double d = nx == null ? 1e9 : (t0 + (long) (nx.t * MS) - hand.nanos()) / 1e6;
                    if (!hand.ready(e.a)) throw new RuntimeException("접근성 권한(Acrux Mobile)이 꺼짐");
                    if (nx != null && endsAll(nx, e.a, e.b) && d < START_MS + 25) {
                        // 누르자마자 떨어진 기록 → 그 시간만큼만 누르고 뗌
                        long ms = Math.max(MIN_MS, Math.round(d));
                        hand.fresh(v, Math.min(1, ms / (double) START_MS), e.b, ms, false);
                        ps = pj = wS = wJ = false;
                        i++;
                    } else {
                        boolean ok = hand.fresh(v, 1, e.b, START_MS, true);
                        wS = e.a;
                        wJ = e.b;
                        dir = v;
                        ps = ok && e.a;
                        pj = ok && e.b;
                    }
                } else if (e.k == 'M') {
                    reach(at, check);
                    dir = new double[]{e.dx, e.dy};
                    if (!ps) continue;                                  // 끊겨 있으면 다음에 다시 누를 때 새 방향으로
                    double d = nx == null ? 1e9 : (t0 + (long) (nx.t * MS) - hand.nanos()) / 1e6;
                    if (nx != null && endsAll(nx, true, pj) && d < MOVE_MS + 25) {
                        hand.cont(dir, Math.max(MIN_MS, Math.round(d)), true, pj);
                        ps = pj = wS = wJ = false;
                        i++;
                    } else if (!hand.cont(dir, MOVE_MS, false, false)) {
                        ps = pj = false;
                    }
                } else if (e.k == 'U') {
                    if (e.a) wS = false;
                    if (e.b) wJ = false;
                    alive(at, e.a, e.b, check);
                } else {
                    wS = wJ = false;
                    alive(at, true, true, check);
                }
            }
        } finally {
            try {
                if (ps || pj) hand.cont(null, 20, ps, pj);
            } catch (RuntimeException ignored) {
            }
            hand.forget();
            ps = pj = false;
            synchronized (lock) {
                playing = false;
                lock.notifyAll();
            }
        }
    }

    /** 다음 일이 누르고 있는 손가락을 전부 떼나 */
    static boolean endsAll(Route.Ev nx, boolean s, boolean j) {
        return nx.k == 'X' || (nx.k == 'U' && (!s || nx.a) && (!j || nx.b));
    }

    /** at 까지 손가락을 그대로 누르고 있다가 정확히 at 에 돌아옴 (다음 터치를 at 에 보내려고 조금 일찍 끝냄) */
    void reach(long at, Runnable check) {
        if (ps || pj) alive(at - ovhNanos() - 2 * MS, false, false, check);
        while (true) {
            check.run();
            long now = hand.nanos();
            if (at - now <= 25 * MS) break;
            hand.sleepUntil(now + 20 * MS);
        }
        hand.sleepUntil(at);
    }

    /** 손가락을 to 까지 누르고 있음 · ls / lj = to 에 맞춰 뗄 손가락 */
    void alive(long to, boolean ls, boolean lj, Runnable check) {
        while (true) {
            check.run();
            long rem = (to - hand.nanos()) / MS;
            if (!ps && !pj) {
                // 끊겨서 (사람이 화면을 누름 등) 손가락이 없음 → 남은 시간이 넉넉하면 다시 누름
                if ((wS || wJ) && rem > START_MS + 30 && hand.ready(wS)) {
                    boolean ok = hand.fresh(wS ? dir : null, 1, wJ, START_MS, true);
                    ps = ok && wS;
                    pj = ok && wJ;
                    if (!ok) hand.sleepUntil(hand.nanos() + 15 * MS);
                    continue;
                }
                return;
            }
            boolean a = ls && ps, b = lj && pj;
            if (rem < MIN_MS) {
                if (a || b) {
                    hand.cont(null, MIN_MS, a, b);
                    if (a) ps = false;
                    if (b) pj = false;
                }
                return;
            }
            long ms;
            boolean last;
            if (rem > HOLD_MS * 3 / 2) {
                ms = HOLD_MS;
                last = false;
            } else if (rem > HOLD_MS) {
                ms = rem / 2;
                last = false;
            } else {
                ms = rem;
                last = true;
            }
            if (!hand.cont(null, ms, last && a, last && b)) {
                ps = pj = false;
                continue;
            }
            if (last) {
                if (a) ps = false;
                if (b) pj = false;
                return;
            }
        }
    }
}
