package com.acrux.mobile;

import android.accessibilityservice.AccessibilityService;
import android.accessibilityservice.GestureDescription;
import android.graphics.Path;
import android.os.Handler;
import android.os.HandlerThread;

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
 * 주의: 접근성 터치는 한 번에 하나 — 이 밖에서 다른 탭을 보내면 지금 손가락들이 떨어짐 (다음 조각에서 다시 누름)
 */
final class Joy {
    private Joy() {}

    static final double PUSH = 1.35;
    static final long MOVE_MS = 60, HOLD_MS = 100, START_MS = 90;

    static final Object lock = new Object();
    static volatile double tx, ty;           // 조이스틱 방향 (길이 0 ~ 1 · 1 = 끝까지)
    static volatile boolean want;            // 조이스틱을 누르고 있어야 함
    static volatile boolean wantJump;        // 점프 버튼을 누르고 있어야 함
    static volatile float jx, jy;            // 점프 버튼 (px)
    static volatile double cx, cy, r;        // 조이스틱 (px)
    static Thread loop;
    /** 조이스틱 손가락이 닿아 있음 */
    static volatile boolean holding;
    /** 새 터치가 안 먹혀서 점프만 따로 누른 횟수 (점프 테스트 기록용) */
    static volatile int jumpFallbacks;
    static Handler cb;

    /** 조이스틱 위치 (px) */
    static void at(double x, double y, double radius) {
        cx = x;
        cy = y;
        r = radius;
    }

    /** 조이스틱 방향 (길이 1 = 끝까지 · null = 뗌) */
    static void set(double[] d) {
        synchronized (lock) {
            if (d == null) {
                want = false;
            } else {
                tx = d[0];
                ty = d[1];
                want = true;
            }
            start();
            lock.notifyAll();
        }
    }

    /** 점프 버튼 누르기 (down) · 떼기 (!down) */
    static void jump(float x, float y, boolean down) {
        synchronized (lock) {
            jx = x;
            jy = y;
            wantJump = down;
            start();
            lock.notifyAll();
        }
    }

    static void start() {
        if (loop != null && loop.isAlive()) return;
        if (cb == null) {
            HandlerThread ht = new HandlerThread("acrux-joy-cb");
            ht.start();
            cb = new Handler(ht.getLooper());
        }
        loop = new Thread(Joy::run, "acrux-joy");
        loop.setDaemon(true);
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
            return false;
        }
        return ok[0];
    }

    static Path line(double x0, double y0, double x1, double y1) {
        Path p = new Path();
        p.moveTo((float) x0, (float) y0);
        p.lineTo((float) x1, (float) y1);
        return p;
    }

    static void run() {
        GestureDescription.StrokeDescription stick = null, jumpF = null;
        double px = 0, py = 0, qx = 0, qy = 0;   // 조이스틱 손가락 · 점프 손가락 지금 자리
        boolean wig = false;
        while (true) {
            boolean ws, wj;
            double dx, dy;
            float bx, by;
            synchronized (lock) {
                if (!want && !wantJump && stick == null && jumpF == null) {
                    try {
                        lock.wait(5000);
                    } catch (InterruptedException e) {
                        return;
                    }
                    continue;
                }
                ws = want;
                wj = wantJump;
                dx = tx;
                dy = ty;
                bx = jx;
                by = jy;
            }
            TapService t = TapService.instance;
            if (t == null || (ws && r <= 0)) {
                stick = null;
                jumpF = null;
                holding = false;
                synchronized (lock) {
                    want = false;
                    wantJump = false;
                }
                continue;
            }
            double m = Math.hypot(dx, dy), k = m >= 0.95 ? PUSH / Math.max(m, 1e-9) : 1.0;     // 끝까지면 판 밖으로 조금 더 (최대 속도)
            double gx = cx + dx * r * k, gy = cy + dy * r * k;
            List<GestureDescription.StrokeDescription> g = new ArrayList<>();

            if ((ws && stick == null) || (wj && jumpF == null)) {
                // 새 손가락이 필요 → 누르고 있어야 하는 손가락 전부를 새 터치로 다시 시작
                GestureDescription.StrokeDescription ns = null, nj = null;
                if (ws) {
                    t.letThrough((int) cx, (int) cy);
                    ns = new GestureDescription.StrokeDescription(line(cx, cy, gx, gy), 0, START_MS, true);   // 가운데를 누르고 그 방향으로 끌기
                    g.add(ns);
                }
                if (wj) {
                    t.letThrough((int) bx, (int) by);
                    long st = ws ? 30 : 0;                                       // 조이스틱이 먼저 닿은 뒤
                    nj = new GestureDescription.StrokeDescription(line(bx, by, bx + 1, by), st, START_MS - st, true);
                    g.add(nj);
                }
                if (send(g, START_MS)) {
                    stick = ns;
                    jumpF = nj;
                    px = gx;
                    py = gy;
                    qx = bx + 1;
                    qy = by;
                    holding = stick != null;
                } else {
                    stick = null;
                    jumpF = null;
                    holding = false;
                    if (wj) {                                                    // 그래도 안 되면 점프만 따로 탭
                        t.tap((int) bx, (int) by, 90);
                        jumpFallbacks++;
                    }
                    nap(30);
                }
                continue;
            }

            // 이어 가기: 손가락마다 그대로 · 움직임 · 떼기
            boolean moving = stick != null && ws && Math.hypot(gx - px, gy - py) >= 2;
            long ms = moving ? MOVE_MS : HOLD_MS;
            if (stick == null && jumpF == null) continue;
            if (!ws && !wj) ms = 30;                                             // 다 뗌
            GestureDescription.StrokeDescription ns = null, nj = null;
            if (stick != null) {
                if (!ws) {
                    ns = stick.continueStroke(line(px, py, px + (wig ? -1 : 1), py), 0, ms, false);
                } else if (moving) {
                    ns = stick.continueStroke(line(px, py, gx, gy), 0, ms, true);
                    px = gx;
                    py = gy;
                } else {
                    double nx = px + (wig ? -1 : 1);                             // 그 자리 (1px 흔들기 · 이어지는 터치는 움직임이 있어야 함)
                    ns = stick.continueStroke(line(px, py, nx, py), 0, ms, true);
                    px = nx;
                }
                g.add(ns);
            }
            if (jumpF != null) {
                double nx = qx + (wig ? -1 : 1);
                nj = jumpF.continueStroke(line(qx, qy, nx, qy), 0, ms, wj);
                qx = nx;
                g.add(nj);
            }
            wig = !wig;
            boolean ok = send(g, ms);
            stick = ok && ns != null && ws ? ns : null;
            jumpF = ok && nj != null && wj ? nj : null;
            holding = stick != null;
            if (!ok) nap(10);              // 끊김 (다른 터치 · 사람이 화면을 누름) → 다음 조각에서 다시 누름
        }
    }

    static void nap(long ms) {
        try {
            Thread.sleep(ms);
        } catch (InterruptedException ignored) {
        }
    }
}
