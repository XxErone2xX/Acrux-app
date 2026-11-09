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

    @Override
    public boolean jump() {
        double[] j = Hub.pos.get("jump_btn");
        if (j == null) return false;
        float x = (float) (j[0] * sw), y = (float) (j[1] * sh);
        if (!Joy.jump(x, y)) svc().tap((int) x, (int) y, 80);     // 조이스틱을 안 누르고 있으면 그냥 탭
        return true;
    }

    @Override
    public double[] stickAt() {
        double[] s = Hub.stick;
        return s == null ? null : new double[]{s[0] * sw, s[1] * sh, s[2] * sh};
    }
}
