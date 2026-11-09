package com.acrux.mobile;

import android.accessibilityservice.AccessibilityService;
import android.accessibilityservice.GestureDescription;
import android.graphics.Path;
import android.os.Handler;
import android.os.HandlerThread;

import java.util.concurrent.Semaphore;
import java.util.concurrent.TimeUnit;

/**
 * 로블록스 클래식 조이스틱을 손가락처럼 누르고 끌기 (접근성 터치 · Shizuku 없이)
 *  · 처음엔 조이스틱 가운데를 누르고 (0.06초) → 그 다음 원하는 방향으로 끌어서 계속 누르고 있음
 *  · 계속 누르는 건 '이어지는 터치' (StrokeDescription.continueStroke) 를 짧게 (0.1초) 계속 이어 붙임
 *  · 방향을 다 떼면 손가락을 뗌
 *  · 끄는 거리 = 반지름 × PUSH (로블록스는 판 밖으로 끌어도 최대 속도로 걸음 → 반지름을 조금 작게 재도 최대 속도)
 * 주의: 접근성 터치는 한 번에 하나 — 조이스틱을 누르는 중에 다른 탭을 보내면 조이스틱 손가락이 떨어짐 (다시 누름)
 */
final class Joy {
    private Joy() {}

    static final double PUSH = 1.35;
    static final long PRESS_MS = 60, MOVE_MS = 35, HOLD_MS = 70;

    static final Object lock = new Object();
    static volatile double tx, ty;           // 원하는 방향 (길이 0 ~ 1 · 1 = 끝까지)
    static volatile boolean want;            // 누르고 있어야 함
    static volatile double cx, cy, r;        // 조이스틱 (px)
    static Thread loop;
    /** 조이스틱을 누르고 있는 중 (다른 탭을 따로 보내면 조이스틱 손가락이 떨어짐 → 점프는 같은 터치에 섞어 보냄) */
    static volatile boolean holding;
    /** 다음 조각에 같이 보낼 점프 탭 (px) · 없으면 null */
    static volatile float[] jumpAt;

    /** 조이스틱을 누르고 있으면 점프 탭을 그 손가락과 같이 보냄 → true · 안 누르고 있으면 false (그냥 탭하면 됨) */
    static boolean jump(float x, float y) {
        if (!holding) return false;
        jumpAt = new float[]{x, y};
        return true;
    }
    static Handler cb;

    /** 조이스틱 위치 (px) */
    static void at(double x, double y, double radius) {
        cx = x;
        cy = y;
        r = radius;
    }

    /** 방향 (길이 1 = 끝까지 · null = 뗌) */
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

    /** 터치 하나 보내고 끝날 때까지 기다림 → 끝까지 갔으면 true (취소 · 실패 · 접근성 꺼짐이면 false) */
    static boolean send(GestureDescription.StrokeDescription s, long ms) {
        return send(s, ms, null);
    }

    static boolean send(GestureDescription.StrokeDescription s, long ms, GestureDescription.StrokeDescription extra) {
        TapService t = TapService.instance;
        if (t == null) return false;
        GestureDescription.Builder gb = new GestureDescription.Builder().addStroke(s);
        if (extra != null) gb.addStroke(extra);
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
        GestureDescription.StrokeDescription cur = null;
        double px = 0, py = 0;
        boolean wiggle = false;
        while (true) {
            boolean w;
            double dx, dy;
            synchronized (lock) {
                if (!want && cur == null) {
                    try {
                        lock.wait(5000);
                    } catch (InterruptedException e) {
                        return;
                    }
                    continue;
                }
                w = want;
                dx = tx;
                dy = ty;
            }
            TapService t = TapService.instance;
            if (t == null || r <= 0) {
                cur = null;
                holding = false;
                synchronized (lock) {
                    want = false;
                }
                continue;
            }
            if (!w) {
                // 손가락 떼기
                send(cur.continueStroke(line(px, py, px + (wiggle ? -1 : 1), py), 0, 30, false), 30);
                cur = null;
                holding = false;
                continue;
            }
            if (cur == null) {
                // 가운데를 먼저 누름 (클래식 조이스틱은 판 안에서 누르기 시작해야 잡힘)
                t.letThrough((int) cx, (int) cy);
                px = cx;
                py = cy;
                GestureDescription.StrokeDescription s = new GestureDescription.StrokeDescription(line(px, py, px + 1, py), 0, PRESS_MS, true);
                px += 1;
                if (!send(s, PRESS_MS)) {
                    nap(50);
                    continue;
                }
                cur = s;
                holding = true;
            }
            double m = Math.hypot(dx, dy), k = m >= 0.95 ? PUSH / Math.max(m, 1e-9) : 1.0;     // 끝까지면 판 밖으로 조금 더 (최대 속도)
            double gx = cx + dx * r * k, gy = cy + dy * r * k;
            GestureDescription.StrokeDescription s;
            long ms;
            if (Math.hypot(gx - px, gy - py) >= 2) {
                ms = MOVE_MS;
                s = cur.continueStroke(line(px, py, gx, gy), 0, ms, true);
                px = gx;
                py = gy;
            } else {
                // 그 자리에서 계속 누르고 있음 (1px 흔들기 · 이어지는 터치는 움직임이 있어야 함)
                ms = HOLD_MS;
                double nx = px + (wiggle ? -1 : 1);
                wiggle = !wiggle;
                s = cur.continueStroke(line(px, py, nx, py), 0, ms, true);
                px = nx;
            }
            // 점프: 조이스틱 손가락은 그대로 두고 같은 터치에 점프 버튼 탭을 섞음
            float[] j = jumpAt;
            jumpAt = null;
            GestureDescription.StrokeDescription jp = null;
            if (j != null) {
                Path p = new Path();
                p.moveTo(j[0], j[1]);
                jp = new GestureDescription.StrokeDescription(p, 0, Math.min(ms, 80));
            }
            if (!send(s, ms, jp)) {
                cur = null;              // 끊김 (다른 터치 · 사람이 화면을 누름) → 다음에 가운데부터 다시
                holding = false;
                continue;
            }
            cur = s;
        }
    }

    static void nap(long ms) {
        try {
            Thread.sleep(ms);
        } catch (InterruptedException ignored) {
        }
    }
}
