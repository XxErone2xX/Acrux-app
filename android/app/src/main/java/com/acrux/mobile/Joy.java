package com.acrux.mobile;

import android.accessibilityservice.AccessibilityService;
import android.accessibilityservice.GestureDescription;
import android.graphics.Path;
import android.os.Handler;
import android.os.HandlerThread;

import com.acrux.mobile.core.Pilot;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Semaphore;
import java.util.concurrent.TimeUnit;

/**
 * 손가락 두 개 (조이스틱 · 점프 버튼) 를 따로따로 누르고 · 끌고 · 꾹 누르고 있기 (접근성 터치 · Shizuku 없이)
 *  · 조이스틱 손가락: 가운데를 누르고 원하는 방향으로 끌어서 계속 누르고 있음 (끄는 거리 = 반지름 × PUSH · 판 밖이어도 최대 속도)
 *  · 점프 손가락: 점프 버튼을 누르고 있음 (꾹 누르면 로블록스는 계속 뜀) → 떼라고 할 때 뗌
 *  · 계속 누르는 건 '이어지는 터치' (continueStroke) 를 짧게 (0.06 ~ 0.1초) 계속 이어 붙임 — 두 손가락을 같은 터치로 함께 이어 감
 *  · 새 손가락을 더해야 할 때 (조이스틱을 누르는 중에 점프 시작 등) 는 '이어지는 터치' 에 새 손가락을 섞으면 로블록스가 안 받음
 *    → 지금 누르고 있어야 하는 손가락 전부를 새 터치로 다시 시작 (조이스틱은 가운데부터 다시 끌기 · 끊김은 한 프레임 정도)
 *  · 언제 누르고 뗄지는 core/Pilot 이 정함 (바로 하기 · 기록한 대로 다시 하기) — 여기는 터치를 보내는 손
 * 주의: 접근성 터치는 한 번에 하나 — 이 밖에서 다른 탭을 보내면 지금 손가락들이 떨어짐 (다음 조각에서 다시 누름)
 */
final class Joy {
    private Joy() {}

    static final double PUSH = 1.35;

    static volatile float jx, jy;            // 점프 버튼 (px)
    static volatile double cx, cy, r;        // 조이스틱 (px)
    static volatile int sw, sh;              // 화면 크기 (px · 0 = 모름) — 손가락이 화면 밖(음수)으로 나가지 않게
    /** 사람 손가락이 기록용 패드에 닿은 시각 (nanoTime · 0 = 안 닿음) */
    static volatile long userDownAt;
    static Thread loop;
    static Handler cb;
    static final Pilot pilot = new Pilot(new H());

    /** 조이스틱 위치 (px) */
    static void at(double x, double y, double radius) {
        cx = x;
        cy = y;
        r = radius;
    }

    /** 조이스틱 방향 (길이 1 = 끝까지 · null = 뗌) */
    static void set(double[] d) {
        start();
        pilot.set(d);
    }

    /** 점프 버튼 누르기 (down) · 떼기 (!down) */
    static void jump(float x, float y, boolean down) {
        jx = x;
        jy = y;
        start();
        pilot.jump(down);
    }

    /** 누르던 손가락을 다 뗄 때까지 기다림 (최대 ms) */
    static void settle(long ms) {
        long end = System.currentTimeMillis() + ms;
        while (!pilot.idle() && System.currentTimeMillis() < end) nap(5);
    }

    static synchronized void start() {
        if (cb == null) {
            HandlerThread ht = new HandlerThread("acrux-joy-cb", android.os.Process.THREAD_PRIORITY_DISPLAY);
            ht.start();
            cb = new Handler(ht.getLooper());
        }
        if (loop != null && loop.isAlive()) return;
        loop = new Thread(pilot::run, "acrux-joy");
        loop.setDaemon(true);
        loop.setPriority(Thread.MAX_PRIORITY);
        loop.start();
    }

    /** 터치 하나 (손가락 여러 개) 보내고 끝날 때까지 기다림 → 끝까지 갔으면 true */
    static boolean send(List<GestureDescription.StrokeDescription> strokes, long ms) {
        TapService t = TapService.instance;
        if (t == null || strokes.isEmpty()) return false;
        GestureDescription.Builder gb = new GestureDescription.Builder();
        for (GestureDescription.StrokeDescription s : strokes) gb.addStroke(s);
        Semaphore done = new Semaphore(0);
        boolean[] ok = {false};
        boolean sent;
        try {
            sent = t.dispatchGesture(gb.build(), new AccessibilityService.GestureResultCallback() {
                @Override
                public void onCompleted(GestureDescription g) {
                    ok[0] = true;
                    done.release();
                }

                @Override
                public void onCancelled(GestureDescription g) {
                    done.release();
                }
            }, cb);
        } catch (RuntimeException e) {
            return false;
        }
        if (!sent) return false;
        try {
            if (!done.tryAcquire(ms + 1500, TimeUnit.MILLISECONDS)) return false;
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return false;
        }
        return ok[0];
    }

    /** 터치 길 — 좌표는 화면 안으로 (음수면 안드로이드가 터치를 거절함) */
    static Path line(double x0, double y0, double x1, double y1) {
        Path p = new Path();
        p.moveTo(fx(x0), fy(y0));
        p.lineTo(fx(x1), fy(y1));
        return p;
    }

    static float fx(double x) {
        return (float) Math.max(0, sw > 0 ? Math.min(sw - 1, x) : x);
    }

    static float fy(double y) {
        return (float) Math.max(0, sh > 0 ? Math.min(sh - 1, y) : y);
    }

    /** 터치를 보내는 손 (Pilot 이 부름 · 한 번에 한 스레드만) */
    static final class H implements Pilot.Hand {
        GestureDescription.StrokeDescription stick, jumpF;
        double px, py, qx, qy;               // 조이스틱 손가락 · 점프 손가락 지금 자리
        boolean wig;

        @Override
        public boolean ready(boolean needStick) {
            return TapService.instance != null && (!needStick || r > 0);
        }

        /** 방향 → 손가락이 갈 자리 (끝까지면 판 밖으로 조금 더 = 최대 속도) */
        double[] goal(double[] d, double frac) {
            double m = Math.hypot(d[0], d[1]), k = m >= 0.95 ? PUSH / Math.max(m, 1e-9) : 1.0;
            double tx = d[0] * r * k * frac, ty = d[1] * r * k * frac;
            // 조이스틱이 화면 끝에 가까우면 판 밖으로 더 미는 자리가 화면 밖(음수)이 됨 → 같은 방향 그대로 화면 안까지만
            //  (안드로이드는 화면 밖 좌표 터치를 'Path bounds must not be negative' 로 거절 → 매크로가 멈췄음)
            double s = 1, maxX = sw > 0 ? sw - 2 : Double.MAX_VALUE, maxY = sh > 0 ? sh - 2 : Double.MAX_VALUE;
            if (cx + tx < 1 && tx < 0) s = Math.min(s, (cx - 1) / -tx);
            if (cx + tx > maxX && tx > 0) s = Math.min(s, (maxX - cx) / tx);
            if (cy + ty < 1 && ty < 0) s = Math.min(s, (cy - 1) / -ty);
            if (cy + ty > maxY && ty > 0) s = Math.min(s, (maxY - cy) / ty);
            s = Math.max(0, s);
            return new double[]{cx + tx * s, cy + ty * s};
        }

        @Override
        public boolean fresh(double[] dir, double frac, boolean jump, long ms, boolean keep) {
            TapService t = TapService.instance;
            stick = null;
            jumpF = null;
            if (t == null) return false;
            ms = Math.max(1, ms);
            List<GestureDescription.StrokeDescription> g = new ArrayList<>();
            GestureDescription.StrokeDescription ns = null, nj = null;
            double gx = cx, gy = cy;
            float bx = jx, by = jy;
            if (dir != null) {
                double[] to = goal(dir, frac);
                gx = to[0];
                gy = to[1];
                t.letThrough((int) cx, (int) cy);
                ns = new GestureDescription.StrokeDescription(line(cx, cy, gx, gy), 0, ms, keep);      // 가운데를 누르고 그 방향으로 끌기
                g.add(ns);
            }
            if (jump) {
                t.letThrough((int) bx, (int) by);
                long st = dir != null ? Math.min(30, ms / 3) : 0;                                       // 조이스틱이 먼저 닿은 뒤
                nj = new GestureDescription.StrokeDescription(line(bx, by, bx + 1, by), st, Math.max(1, ms - st), keep);
                g.add(nj);
            }
            if (!send(g, ms)) return false;
            if (keep) {
                stick = ns;
                jumpF = nj;
                px = gx;
                py = gy;
                qx = bx + 1;
                qy = by;
            }
            return true;
        }

        @Override
        public boolean cont(double[] dir, long ms, boolean liftStick, boolean liftJump) {
            if (stick == null && jumpF == null) return false;
            ms = Math.max(1, ms);
            List<GestureDescription.StrokeDescription> g = new ArrayList<>();
            GestureDescription.StrokeDescription ns = null, nj = null;
            double nx = px, ny = py, mx = qx;
            if (stick != null) {
                if (dir != null && !liftStick) {
                    double[] to = goal(dir, 1);
                    nx = to[0];
                    ny = to[1];
                } else {
                    nx = px + (wig && px >= 1 ? -1 : 1);                         // 그 자리 (1px 흔들기 · 이어지는 터치는 움직임이 있어야 함 · 화면 왼쪽 끝이면 오른쪽으로)
                }
                ns = stick.continueStroke(line(px, py, nx, ny), 0, ms, !liftStick);
                g.add(ns);
            }
            if (jumpF != null) {
                mx = qx + (wig && qx >= 1 ? -1 : 1);
                nj = jumpF.continueStroke(line(qx, qy, mx, qy), 0, ms, !liftJump);
                g.add(nj);
            }
            wig = !wig;
            if (!send(g, ms)) {
                stick = null;
                jumpF = null;
                return false;
            }
            stick = liftStick ? null : ns;
            jumpF = liftJump ? null : nj;
            px = nx;
            py = ny;
            qx = mx;
            return true;
        }

        @Override
        public void forget() {
            stick = null;
            jumpF = null;
        }

        @Override
        public long nanos() {
            return System.nanoTime();
        }

        @Override
        public void sleepUntil(long at) {
            long left;
            while ((left = at - System.nanoTime()) > 0) {
                if (left > 3_000_000L) {
                    try {
                        Thread.sleep((left - 2_000_000L) / 1_000_000L);
                    } catch (InterruptedException e) {
                        Thread.currentThread().interrupt();
                        return;
                    }
                } else {
                    Thread.yield();                                              // 마지막 2 ~ 3ms 는 정확히
                }
            }
        }

        @Override
        public long userDown() {
            long u = userDownAt;
            return u != 0 && System.nanoTime() - u < 600_000_000L ? u : 0;
        }
    }

    static void nap(long ms) {
        try {
            Thread.sleep(ms);
        } catch (InterruptedException ignored) {
        }
    }
}
