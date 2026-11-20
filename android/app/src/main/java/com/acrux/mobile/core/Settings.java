package com.acrux.mobile.core;

/** 자동 낚시 설정 (PC 버전 mfish 와 같은 값 · 위치는 전부 화면 비율) */
public final class Settings {
    // 자동 보정으로 채워지는 위치
    public double[] panelRegion, reelRegion, resultRegion;      // 창 영역 [x1, y1, x2, y2]
    public double[] fishBtn, barRegion, closePos, titlePos;      // 위치 [x, y] · 바 영역 [x1, y1, x2, y2]
    public boolean reelDerived = true;                            // 미니게임 창을 실제로 안 재고 대기 창으로 계산했는지
    // 세부 설정 (PC 버전 DEFAULTS)
    public double biteMax = 60.0;
    public double leadMs = 0;
    public double targetPct = 0;
    public double deadband = 0.0;
    public int clickMs = 30;          // 한 번 누르는 시간 (모바일은 터치라 PC 25ms 보다 조금 길게)
    public int clickGapMs = 45;
    public int castRetry = 3;
    /** 터치 지연에 맞춰 누른 뒤 기다리는 시간을 늘림 (모바일 기본 켜짐 · PC 와 똑같이 하려면 끔) */
    public boolean adaptive = true;
    /** 릴링 때 미니게임 창 위쪽을 누름 (모바일: 창 아래는 Roll 버튼) · false 면 PC 처럼 창 아래 */
    public boolean tapAbove = true;
    // ---- 1.6.0 부터 자동으로 재서 저장하는 값 (leadMs · targetPct · adaptive · clickGapMs 는 예전 릴링 조작용 — 지금은 안 씀)
    /** 릴링: 지난 판에서 잰 떨어지는 속도 · 한 번에 튀는 양 (바 폭 비율) · 터치가 화면에 보이기까지 걸리는 시간 (초) · 0 = 아직 없음 */
    public double reelFall, reelJump, reelDelay;
    /** 릴링 때 누르는 자리 (0 = 기본 · 1, 2 = 기본 자리에서 터치가 안 먹힐 때 쓰는 다른 자리) */
    public int reelSpot;
    /** 결과창 X: 보정한 자리가 아닌데 눌러서 닫혔던 자리들 {x, y, 닫힌 횟수, …} (화면 비율 · 최대 3곳 · 쓰레기 결과창처럼 크기가 다른 창) */
    public double[] closeLearn;

    /** 결과창 X 를 (x, y) 에서 눌러서 닫았음 → 기억 (가까운 자리가 있으면 합침 · 3곳 넘으면 가장 적게 닫힌 곳을 뺌) */
    public void learnClose(double x, double y) {
        double[] a = closeLearn == null ? new double[0] : closeLearn;
        for (int i = 0; i + 2 < a.length; i += 3) {
            if (Math.abs(a[i] - x) < 0.02 && Math.abs(a[i + 1] - y) < 0.03) {
                double n = a[i + 2];
                a[i] = Vision.round4((a[i] * n + x) / (n + 1));
                a[i + 1] = Vision.round4((a[i + 1] * n + y) / (n + 1));
                a[i + 2] = Math.min(999, n + 1);
                closeLearn = a;
                return;
            }
        }
        double[] b = java.util.Arrays.copyOf(a, a.length + 3);
        b[a.length] = Vision.round4(x);
        b[a.length + 1] = Vision.round4(y);
        b[a.length + 2] = 1;
        while (b.length > 9) {
            int low = 0;
            for (int i = 3; i + 2 < b.length; i += 3) if (b[i + 2] < b[low + 2]) low = i;
            double[] c = new double[b.length - 3];
            System.arraycopy(b, 0, c, 0, low);
            System.arraycopy(b, low + 3, c, low, b.length - low - 3);
            b = c;
        }
        closeLearn = b;
    }

    /** 기억한 결과창 X 자리들 (많이 닫힌 순서) → {x, y} 들 */
    public java.util.List<double[]> learnedClose() {
        java.util.List<double[]> out = new java.util.ArrayList<>();
        double[] a = closeLearn;
        if (a == null) return out;
        java.util.List<double[]> e = new java.util.ArrayList<>();
        for (int i = 0; i + 2 < a.length; i += 3) e.add(new double[]{a[i], a[i + 1], a[i + 2]});
        e.sort((p, q) -> Double.compare(q[2], p[2]));
        for (double[] v : e) out.add(new double[]{v[0], v[1]});
        return out;
    }

    public boolean ready() {
        return fishBtn != null && barRegion != null;          // 결과창 X 는 못 쟀어도 화면에서 찾아서 누름
    }

    /** 창 영역을 넣고 그 안의 위치들도 계산 (PC 버전 layout_from + WINDOW_KEYS) */
    public void apply(String key, double[] region) {
        switch (key) {
            case "panel_region":
                panelRegion = region;
                fishBtn = Vision.layout(region, Vision.PANEL_FISH_BTN);
                barRegion = Vision.layout(region, Vision.PANEL_BAR);
                break;
            case "reel_region":
                reelRegion = region;
                barRegion = Vision.layout(region, Vision.REEL_BAR);
                break;
            case "result_region":
                resultRegion = region;
                closePos = Vision.layout(region, Vision.RESULT_CLOSE);
                titlePos = Vision.layout(region, Vision.RESULT_TITLE);
                break;
            default:
                break;
        }
    }

    /** 대기 창으로 미니게임 창 계산 (가운데 · 위아래는 같고 가로만 넓음) */
    public void deriveReel() {
        double[] p = panelRegion;
        double c = (p[0] + p[2]) / 2, half = (p[2] - p[0]) / 2 * Vision.REEL_FROM_PANEL;
        apply("reel_region", new double[]{Vision.round4(c - half), p[1], Vision.round4(c + half), p[3]});
        reelDerived = true;
    }
}
