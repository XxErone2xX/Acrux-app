package com.acrux.mobile;

import com.acrux.mobile.core.Touch;

/** 이동 · 판매의 터치 (접근성) — 탭 · 끌기 · 두 손가락 · 조이스틱 (Joy) */
final class TouchIn implements Touch {
    final int sw, sh;            // 화면 크기 (조이스틱 위치를 px 로)

    TouchIn(int sw, int sh) {
        this.sw = sw;
        this.sh = sh;
        Joy.sw = sw;
        Joy.sh = sh;
        double[] s = stickAt();
        if (s != null) Joy.at(s[0], s[1], s[2]);
    }

    static TapService svc() {
        TapService t = TapService.instance;
        if (t == null) throw new RuntimeException("접근성 권한(Acrux Mobile)이 꺼짐");
        return t;
    }

    @Override
    public void tap(int x, int y, int ms) {
        svc().tap(x, y, Math.max(ms, 40));
    }

    @Override
    public void swipe(int x0, int y0, int x1, int y1, int ms) {
        svc().swipe(x0, y0, x1, y1, ms);
    }

    @Override
    public void pinch(int cx, int cy, int d0, int d1, int ms) {
        svc().pinch(cx, cy, d0, d1, ms);
    }

    @Override
    public void stick(double[] v) {
        Joy.set(v);
    }

    int jx, jy;

    @Override
    public boolean stickHeld() {
        return Joy.pilot.holding;
    }

    @Override
    public void play(java.util.List<com.acrux.mobile.core.Route.Ev> evs, int[] jump, Runnable check) {
        svc();
        double[] s = stickAt();
        if (s != null) Joy.at(s[0], s[1], s[2]);
        if (jump != null) {
            jx = jump[0];
            jy = jump[1];
            Joy.jx = jx;
            Joy.jy = jy;
        }
        Joy.start();
        int tid = android.os.Process.myTid(), pr = android.os.Process.getThreadPriority(tid);
        try {
            android.os.Process.setThreadPriority(android.os.Process.THREAD_PRIORITY_URGENT_DISPLAY);      // 시각을 맞춰야 해서 그동안만 우선
        } catch (RuntimeException ignored) {
        }
        // Acrux 창이 조이스틱 · 점프 버튼 위에 있으면: 걷기 전에 한 번만 창을 통과하게 해 두고 끝까지 유지
        //  (전에는 누를 때마다 창 설정을 바꾸느라 그 누름이 0.04초 넘게 늦게 시작 → 누른 시간이 기록과 달라짐)
        TapService t = TapService.instance;
        if (t != null) {
            t.passKeep = true;
            t.letThrough((int) Joy.cx, (int) Joy.cy);
            if (jump != null) t.letThrough(jump[0], jump[1]);
        }
        try {
            Joy.pilot.play(evs, check);
        } finally {
            if (t != null) {
                t.passKeep = false;
                t.passUntil = android.os.SystemClock.uptimeMillis() + 300;
            }
            try {
                android.os.Process.setThreadPriority(pr);
            } catch (RuntimeException ignored) {
            }
            int cuts = Joy.pilot.cuts;
            if (cuts > 0) Hub.log("이동 중 조이스틱 터치가 " + cuts + "번 끊김 (화면을 건드림) — 다시 눌러서 이어 감 · 그만큼 늦게 도착 (+"
                    + Math.round(Joy.pilot.lostMs / 100.0) / 10.0 + "초)", 'y');
        }
    }

    @Override
    public void traceStart() {
        Joy.pilot.traceStart();
    }

    @Override
    public java.util.List<com.acrux.mobile.core.Route.Ev> traceEnd() {
        Joy.settle(500);
        return Joy.pilot.traceEnd();
    }

    @Override
    public void jumpUp() {
        Joy.jump(jx, jy, false);
    }

    @Override
    public void jumpDown(int x, int y) {
        jx = x;
        jy = y;
        Joy.jump(x, y, true);
        // (전에는 누른 자리에 점을 띄웠는데 · 화면 전체 창을 새로 띄우는 동안 로블록스가 다른 터치를 놓칠 수 있어서 뺌)
    }

    @Override
    public double[] stickAt() {
        double[] s = Hub.fitStick(Hub.stick, sw, sh);
        return s == null ? null : new double[]{s[0] * sw, s[1] * sh, s[2] * sh};
    }
}
