package com.acrux.mobile;

import com.acrux.mobile.core.Touch;

/** 이동 · 판매의 터치 (접근성) — 탭 · 끌기 · 두 손가락 · 조이스틱 (Joy) */
final class TouchIn implements Touch {
    final int sw, sh;            // 화면 크기 (조이스틱 위치를 px 로)

    TouchIn(int sw, int sh) {
        this.sw = sw;
        this.sh = sh;
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
    public void jumpUp() {
        Joy.jump(jx, jy, false);
    }

    @Override
    public void jumpDown(int x, int y) {
        jx = x;
        jy = y;
        Joy.jump(x, y, true);
        // 어디를 눌렀는지 잠깐 보여 줌 (점프가 안 될 때 위치가 틀렸는지 바로 보이게)
        Hub.main.post(() -> {
            TapService t = TapService.instance;
            if (t != null) t.showDot(x, y, 450);
        });
    }

    @Override
    public double[] stickAt() {
        double[] s = Hub.stick;
        return s == null ? null : new double[]{s[0] * sw, s[1] * sh, s[2] * sh};
    }
}
