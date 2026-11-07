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
