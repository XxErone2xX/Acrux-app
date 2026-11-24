package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.List;

/**
 * 낚시 테스트: 화면에 그릴 칸 — 매크로가 실제로 보는 자리 (Fisher 와 같은 계산 · 화면 px)
 *  · 창 (대기 창 · 미니게임 창 · 결과창) · Fish 버튼 확인 칸 (+ 누르는 자리) · 미니게임 바 · ◇ 확인 칸 · 결과 제목 확인 칸 · X 자리
 *  · 선은 칸 바깥에 그림 (매크로가 그 칸 안을 볼 때 선이 안 섞이게) · 그린 자리는 Mask 로 화면 캡처에서도 지움
 */
public final class TestBoxes {
    public static final int C_PANEL = 0xFF29B6F6, C_FISH = 0xFF2EE66B, C_REEL = 0xFFFFD43B, C_BAR = 0xFFFF8A1F, C_RESULT = 0xFFE040FB,
            C_TITLE = 0xFFFF6FAE, C_CLOSE = 0xFFFF3B3B, C_DIA = 0xFF9BE7FF, C_MARK = 0xFFFFFFFF, C_ZONE = 0xFF2EE66B,
            C_TAP = 0xFFFFFFFF, C_PRED = 0xFF4DB8FF, C_AIM = 0xFFFFE14D, C_STICK = 0xFF3BA55D, C_OCR = 0xFFFFB84D;
    /** 글자 자리: 칸 위 왼쪽 · 아래 왼쪽 · 오른쪽 · 왼쪽 · 위 오른쪽 · 없음 */
    public static final int L_TOP = 0, L_BOTTOM = 1, L_RIGHT = 2, L_LEFT = 3, L_TOP_RIGHT = 4, L_NONE = -1;

    public static final class Box {
        public final String kind, label;
        public final int color, x0, y0, x1, y1, labelAt, labelGap;
        /** 가운데 + (누르는 자리) */
        public final boolean cross;

        Box(String kind, String label, int color, int x0, int y0, int x1, int y1, int labelAt, int labelGap, boolean cross) {
            this.kind = kind;
            this.label = label;
            this.color = color;
            this.x0 = x0;
            this.y0 = y0;
            this.x1 = Math.max(x0 + 1, x1);
            this.y1 = Math.max(y0 + 1, y1);
            this.labelAt = labelAt;
            this.labelGap = labelGap;
            this.cross = cross;
        }
    }

    private TestBoxes() {}

    static int[] px(double rx, double ry, int W, int H) {
        return new int[]{(int) (rx * W), (int) (ry * H)};
    }

    /**
     * 지금 단계에 매크로가 보는 칸만 (Fisher.phase) — 0 모름 (다 그림 · 칸이 안 맞을 때 어디가 틀렸는지 보이게) · 1 대기 · 입질 기다림 (대기 창 · Fish 버튼 · ◇)
     *  · 2 릴링 (미니게임 창 · 바 · ◇ 두 칸) · 3 결과창 (결과창 · 제목 · X) · 4 이동 · 판매 (안 그림 — 낚시 창을 안 봄)
     *  · 5 자동 보정에서 낚시 창을 아직 못 찾음 (안 그림 — 전에 보정한 칸이 실제 창 테두리 위에 그려지면 창을 못 찾을 수 있어서)
     */
    public static List<Box> of(Settings cfg, int W, int H, int phase) {
        List<Box> all = of(cfg, W, H), out = new ArrayList<>();
        for (Box b : all) {
            boolean show;
            switch (phase) {
                case 1:
                    show = b.kind.equals("panel") || b.kind.equals("fish") || b.kind.equals("dia_idle");
                    break;
                case 2:
                    show = b.kind.equals("reel") || b.kind.equals("bar") || b.kind.startsWith("dia_");
                    break;
                case 3:
                    show = b.kind.equals("result") || b.kind.equals("title") || b.kind.equals("close");
                    break;
                case 4:
                case 5:
                    show = false;
                    break;
                default:
                    show = true;
                    break;
            }
            if (show) out.add(b);
        }
        return out;
    }

    /** 지금 설정의 칸들 (없는 값은 안 그림) */
    public static List<Box> of(Settings cfg, int W, int H) {
        List<Box> out = new ArrayList<>();
        if (W <= 0 || H <= 0) return out;
        if (cfg.panelRegion != null) out.add(win("panel", "대기 창", C_PANEL, cfg.panelRegion, W, H, L_TOP));
        if (cfg.reelRegion != null)
            out.add(win("reel", cfg.reelDerived ? "미니게임 창 (계산)" : "미니게임 창", C_REEL, cfg.reelRegion, W, H, L_TOP_RIGHT));
        if (cfg.barRegion != null) {
            double[] r = cfg.barRegion;
            int[] a = px(Math.min(r[0], r[2]), Math.min(r[1], r[3]), W, H), b = px(Math.max(r[0], r[2]), Math.max(r[1], r[3]), W, H);
            int bh = Math.max(4, b[1] - a[1]);
            // 글자는 릴링 때 잡는 칸 (바 아래 0.8 배까지) 밖에
            out.add(new Box("bar", "미니게임 바", C_BAR, a[0], a[1], b[0], b[1], L_BOTTOM, (int) (bh * 0.8) + 2, false));
        }
        double[][] dia = diamondSpots(cfg);
        for (int i = 0; i < 2; i++) {
            if (dia[i] == null) continue;
            double[] win = i == 0 ? dia[2] : dia[3];
            double x1 = Math.min(win[0], win[2]), x2 = Math.max(win[0], win[2]), y1 = Math.min(win[1], win[3]), y2 = Math.max(win[1], win[3]);
            int size = Math.max(3, (int) (W * (x2 - x1) * 0.012));
            int[] p = px(x1 + dia[i][0] * (x2 - x1), y1 + dia[i][1] * (y2 - y1), W, H);
            out.add(new Box(i == 0 ? "dia_idle" : "dia_reel", "◇", C_DIA, p[0] - size, p[1] - size, p[0] + size + 1, p[1] + size + 1, L_NONE, 0, false));
        }
        if (cfg.fishBtn != null) out.add(grabBox("fish", "Fish 버튼", C_FISH, cfg.fishBtn, 0.045, 0.035, W, H, L_TOP, true));
        if (cfg.resultRegion != null) out.add(win("result", "결과창", C_RESULT, cfg.resultRegion, W, H, L_TOP));
        if (cfg.titlePos != null) out.add(grabBox("title", "결과 제목", C_TITLE, cfg.titlePos, 0.12, 0.05, W, H, L_BOTTOM, false));
        if (cfg.closePos != null) out.add(square("close", "X", C_CLOSE, cfg.closePos, Math.max(7, (int) (H * 0.032)), W, H, L_LEFT));
        return out;
    }

    /** ◇ 확인 칸 {대기 자리, 미니게임 자리, 대기 창, 미니게임 창} (Fisher.diamondMoved 와 같음) */
    static double[][] diamondSpots(Settings cfg) {
        double[] idleWin = cfg.panelRegion, reelWin = cfg.reelRegion;
        if (idleWin == null && reelWin == null) return new double[4][];
        double[] iw = idleWin != null ? idleWin : reelWin, irel = idleWin != null ? Vision.DIAMOND_IDLE : Vision.DIAMOND_IDLE_R;
        double[] rw = reelWin != null ? reelWin : idleWin, rrel = reelWin != null ? Vision.DIAMOND_REEL_R : Vision.DIAMOND_REEL;
        return new double[][]{irel, rrel, iw, rw};
    }

    static Box win(String kind, String label, int color, double[] r, int W, int H, int labelAt) {
        int[] a = px(Math.min(r[0], r[2]), Math.min(r[1], r[3]), W, H), b = px(Math.max(r[0], r[2]), Math.max(r[1], r[3]), W, H);
        return new Box(kind, label, color, a[0], a[1], b[0], b[1], labelAt, 0, false);
    }

    /** Fisher.grabBox 와 같은 칸 */
    static Box grabBox(String kind, String label, int color, double[] pos, double bw, double bh, int W, int H, int labelAt, boolean cross) {
        int[] p = px(pos[0], pos[1], W, H);
        int w = Math.max(6, (int) (W * bw)), h = Math.max(6, (int) (H * bh));
        int x = p[0] - w / 2, y = p[1] - h / 2;
        return new Box(kind, label, color, x, y, x + w, y + h, labelAt, 0, cross);
    }

    static Box square(String kind, String label, int color, double[] pos, int half, int W, int H, int labelAt) {
        int[] p = px(pos[0], pos[1], W, H);
        return new Box(kind, label, color, p[0] - half, p[1] - half, p[0] + half + 1, p[1] + half + 1, labelAt, 0, false);
    }

    /** 선 두께 (px) */
    public static int stroke(float density) {
        return Math.max(2, Math.round(density * 1.25f));
    }

    /**
     * 칸과 선 사이 틈 (px) — 칸 가장자리 바로 바깥에는 창 테두리 · 바 테두리가 있어서 거기에 그리면 (지워도) 테두리 위치가 1 ~ 3 px 틀어짐
     *  (실제 스크린샷으로 확인: 틈 없이 그리면 릴링 바 줄 찾기 · 결과창 꺾쇠 찾기가 달라짐)
     */
    public static int gap(float density) {
        return Math.max(4, Math.round(density * 2.2f));
    }

    /** 칸 하나 → 그릴 직사각형들 (칸에서 g 만큼 떨어진 바깥 테두리 4개 + 가운데 +) [x0, y0, x1, y1) */
    public static List<int[]> strips(Box b, int t, int g) {
        List<int[]> s = new ArrayList<>();
        int x0 = b.x0 - g, y0 = b.y0 - g, x1 = b.x1 + g, y1 = b.y1 + g;
        s.add(new int[]{x0 - t, y0 - t, x1 + t, y0});
        s.add(new int[]{x0 - t, y1, x1 + t, y1 + t});
        s.add(new int[]{x0 - t, y0, x0, y1});
        s.add(new int[]{x1, y0, x1 + t, y1});
        if (b.cross) {
            int cx = (b.x0 + b.x1) / 2, cy = (b.y0 + b.y1) / 2, arm = Math.max(4, Math.min(b.x1 - b.x0, b.y1 - b.y0) / 4);
            s.add(new int[]{cx - arm, cy - t / 2, cx + arm + 1, cy - t / 2 + t});
            s.add(new int[]{cx - t / 2, cy - arm, cx - t / 2 + t, cy + arm + 1});
        }
        return s;
    }

    /** 글자 칸 (배경 포함) — tw · th: 글자 크기 (px) · pad: 글자 둘레 여백 · t · g: 선 두께 · 틈 · 화면 안으로 넣음 */
    public static int[] labelRect(Box b, int tw, int th, int pad, int t, int g, int W, int H) {
        int w = tw + pad * 2, h = th + pad * 2, x, y;
        t += g;                                                  // 선 바깥에
        switch (b.labelAt) {
            case L_BOTTOM:
                x = b.x0 - t;
                y = b.y1 + t + 2 + b.labelGap;
                break;
            case L_RIGHT:
                x = b.x1 + t + 3;
                y = (b.y0 + b.y1) / 2 - h / 2;
                break;
            case L_LEFT:
                x = b.x0 - t - 3 - w;
                y = (b.y0 + b.y1) / 2 - h / 2;
                break;
            case L_TOP_RIGHT:
                x = b.x1 + t - w;
                y = b.y0 - t - 2 - h;
                break;
            default:
                x = b.x0 - t;
                y = b.y0 - t - 2 - h;
                break;
        }
        // 화면 밖이면 반대쪽 (위 ↔ 아래 · 왼쪽 ↔ 오른쪽) → 그래도 밖이면 화면 끝에 붙임
        if (y < 0) y = b.y1 + t + 2;
        if (y + h > H) y = Math.max(0, b.y0 - t - 2 - h);
        if (x < 0) x = b.labelAt == L_LEFT ? b.x1 + t + 3 : 0;
        if (x + w > W) x = Math.max(0, W - w);
        y = Math.max(0, Math.min(H - h, y));
        return new int[]{x, y, x + w, y + h};
    }

    /** 릴링 중 읽은 내 위치 · 구간 → 바 아래 (릴링 때 잡는 칸 밖) 에 그릴 직사각형 {내 위치 세모 자리, 구간 줄} · bar: 바 (화면 px x, y, 폭, 높이) */
    public static int[][] reelMarks(int[] bar, Double marker, int[] zone, int t) {
        int[][] r = reelMarks(bar, marker, zone, Double.NaN, Double.NaN, t);
        return new int[][]{r[0], r[1]};
    }

    /**
     * reelMarks + 릴링 조작 (디버깅): {내 위치 세모 자리, 구간 줄, 예측 세모 자리, 누름 기준 줄}
     *  · pred: 지금 누르면 반영될 때의 내 위치 예측 · aim: 이보다 왼쪽이면 누름 (바 폭 비율 · NaN 이면 없음) — 세모 줄에 같이 · 기준 줄은 구간 줄부터 세모 줄 끝까지
     */
    public static int[][] reelMarks(int[] bar, Double marker, int[] zone, double pred, double aim, int t) {
        int bx = bar[0], by = bar[1], bw = bar[2], bh = Math.max(4, bar[3]);
        int y0 = by + bh + (int) (bh * 0.8) + 3, zh = Math.max(3, t * 2), s = Math.max(5, bh / 3), ty = y0 + zh + 1;
        int[] z = null, m = null, p = null, a = null;
        if (zone != null && zone[1] >= zone[0]) z = new int[]{bx + zone[0], y0, bx + zone[1] + 1, y0 + zh};
        if (marker != null) {
            int mx = bx + (int) Math.round(Math.max(0, Math.min(bw, marker)));
            m = new int[]{mx - s, ty, mx + s + 1, ty + s + 1};
        }
        if (!Double.isNaN(pred)) {
            int px = bx + (int) Math.round(Math.max(0, Math.min(1, pred)) * bw);
            p = new int[]{px - s, ty, px + s + 1, ty + s + 1};
        }
        if (!Double.isNaN(aim)) {
            int w = Math.max(2, t), ax = bx + (int) Math.round(Math.max(0, Math.min(1, aim)) * bw) - w / 2;
            a = new int[]{ax, y0, ax + w, ty + s + 1};
        }
        return new int[][]{m, z, p, a};
    }

    /** 글자 폭 (px) */
    public interface Measure {
        int width(String s);
    }

    /**
     * 옆 칸 (디버깅 글자) 을 놓을 가로 범위 {x0, x1} — 낚시 칸들 (모든 단계 · 글자 포함) 이 차지하는 가로 범위의 바깥에서 넓은 쪽 (같으면 오른쪽)
     *  · 낚시 창 · 미니게임 창 · 결과창은 화면 가운데라 양옆은 매크로가 안 봄 (위쪽 가운데는 결과창 제목 · X 와 가까워서 한 줄만)
     *  · margin: 칸과 띄울 거리 · 칸이 하나도 없으면 오른쪽 절반
     */
    public static int[] sideRange(Settings cfg, int W, int H, int t, int g, int pad, int th, int margin, Measure ms) {
        int minX = W, maxX = 0;
        for (Box b : of(cfg, W, H)) {
            minX = Math.min(minX, b.x0 - t - g);
            maxX = Math.max(maxX, b.x1 + t + g);
            if (b.labelAt == L_NONE || b.label.isEmpty()) continue;
            int[] lr = labelRect(b, ms.width(b.label), th, pad, t, g, W, H);
            minX = Math.min(minX, lr[0]);
            maxX = Math.max(maxX, lr[2]);
        }
        if (minX >= maxX) return new int[]{W / 2, W - pad};
        int lx0 = pad, lx1 = minX - margin, rx0 = maxX + margin, rx1 = W - pad;
        return rx1 - rx0 >= lx1 - lx0 ? new int[]{rx0, rx1} : new int[]{lx0, lx1};
    }

    /** 세모 자리 (reelMarks) → 위쪽 (바 쪽) 을 가리키는 계단 모양 직사각형들 (안티에일리어싱 없이 · 지울 자리가 정확하게) */
    public static List<int[]> triangle(int[] m, int t) {
        List<int[]> out = new ArrayList<>();
        int steps = Math.max(2, (m[3] - m[1]) / Math.max(1, t));
        for (int i = 0; i < steps; i++) {
            int y0 = m[1] + (m[3] - m[1]) * i / steps, y1 = m[1] + (m[3] - m[1]) * (i + 1) / steps;
            int cx = (m[0] + m[2]) / 2, half = Math.max(1, (m[2] - m[0]) / 2 * (i + 1) / steps);
            out.add(new int[]{cx - half, y0, cx + half + 1, y1});
        }
        return out;
    }

    /** 조이스틱 (디버깅 · 이동 중): 판 둘레 네모 테두리 (cx, cy, r: 화면 px) → 직사각형 4개 */
    public static List<int[]> stickRing(double cx, double cy, double r, int t) {
        Box b = new Box("stick", "", C_STICK, (int) Math.round(cx - r), (int) Math.round(cy - r), (int) Math.round(cx + r) + 1, (int) Math.round(cy + r) + 1,
                L_NONE, 0, false);
        return strips(b, t, 0);
    }

    /** 조이스틱 손가락 자리 (꽉 찬 네모 · 한 변 2k+1) */
    public static int[] finger(double x, double y, int k) {
        int fx = (int) Math.round(x), fy = (int) Math.round(y);
        return new int[]{fx - k, fy - k, fx + k + 1, fy + k + 1};
    }

    /**
     * 누른 자리 표시: 둘레 네 귀퉁이 꺾쇠 (가운데 · 같은 줄은 비움 — 누른 버튼 · 그 글자를 가리지 않게 · 글자 읽기 · 버튼 색 확인에 안 섞이게)
     *  → 직사각형 8개 · 반지름 r = 화면 높이의 2.8% · 꺾쇠 길이는 r 의 1/3 (위 · 아래 꺾쇠는 가운데에서 r × 2/3 넘게 떨어짐)
     */
    public static List<int[]> tapMark(int x, int y, int H, int t) {
        int r = Math.max(8, (int) (H * 0.028));
        return corners(x - r, y - r, x + r + 1, y + r + 1, Math.max(3, r / 3), t);
    }

    /**
     * 글자 읽기 (OCR) 로 읽은 줄 표시: 글자 칸 양옆에 g 만큼 떨어진 세로 막대 '[ 글자 ]' (글자 위에는 안 그림)
     *  · 위 · 아래로는 안 그림 — 줄 사이가 좁은 글 (대화 · 상점 목록) 에서 이웃 줄 글자를 덮지 않게
     *  · x0 … y1: 읽은 글자 칸 (화면 px) · 막대 끝에 칸 안쪽으로 짧은 턱 (줄 높이 안에서만)
     */
    public static List<int[]> ocrMark(int x0, int y0, int x1, int y1, int g, int t) {
        List<int[]> s = new ArrayList<>();
        int h = Math.max(1, y1 - y0), tick = Math.max(t, Math.min(8, h / 4));
        s.add(new int[]{x0 - g - t, y0, x0 - g, y1});
        s.add(new int[]{x1 + g, y0, x1 + g + t, y1});
        s.add(new int[]{x0 - g, y0, x0 - g + tick, y0 + t});
        s.add(new int[]{x0 - g, y1 - t, x0 - g + tick, y1});
        s.add(new int[]{x1 + g - tick, y0, x1 + g, y0 + t});
        s.add(new int[]{x1 + g - tick, y1 - t, x1 + g, y1});
        return s;
    }

    /** 직사각형 [x0, x1) × [y0, y1) 바깥 귀퉁이 네 곳의 꺾쇠 (두께 t · 길이 arm) → 직사각형 8개 */
    public static List<int[]> corners(int x0, int y0, int x1, int y1, int arm, int t) {
        List<int[]> s = new ArrayList<>();
        int L = arm + t;
        // 왼쪽 위 · 오른쪽 위 · 왼쪽 아래 · 오른쪽 아래 (가로 꺾쇠 · 세로 꺾쇠)
        s.add(new int[]{x0 - t, y0 - t, x0 - t + L, y0});
        s.add(new int[]{x0 - t, y0, x0, y0 - t + L});
        s.add(new int[]{x1 + t - L, y0 - t, x1 + t, y0});
        s.add(new int[]{x1, y0, x1 + t, y0 - t + L});
        s.add(new int[]{x0 - t, y1, x0 - t + L, y1 + t});
        s.add(new int[]{x0 - t, y1 + t - L, x0, y1});
        s.add(new int[]{x1 + t - L, y1, x1 + t, y1 + t});
        s.add(new int[]{x1, y1 + t - L, x1 + t, y1});
        return s;
    }
}
