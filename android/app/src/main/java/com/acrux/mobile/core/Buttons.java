package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.List;

/**
 * 판매 버튼을 화면 색으로 찾기 (글자 읽기 없이) — 모바일은 선택지가 Roll 버튼 위에 겹쳐 그려져서 글자 읽기가 헷갈림
 *  · 대화 선택지: [Open Shop] 초록 · [Sell Fish] 파랑 · [Leave] 빨강 테두리 상자가 한 줄로 → 가운데 파랑 = Sell Fish
 *  · 상점: [Sell] 초록 바로 오른쪽에 [Sell All] 파랑 (같은 높이 · 비슷한 크기) → 파랑 = Sell All
 * 테두리 색 칸을 이어 붙여 상자로 만들고 (모양 · 크기 · 줄 맞춤) 으로 거름 — 화면 크기 · 비율이 달라도 비율로만 봄
 */
public final class Buttons {
    private Buttons() {}

    public static final int GREEN = 0, BLUE = 1, RED = 2;

    /** 테두리 색 → GREEN · BLUE · RED · -1 */
    static int cls(int r, int g, int b) {
        // 색상(hue) 으로 봄 — 어둡게 · 옅게 · 압축돼도 색상은 거의 그대로 (밝기 · 채도는 낮게 잡음)
        int mx = Math.max(r, Math.max(g, b)), mn = Math.min(r, Math.min(g, b)), d = mx - mn;
        if (mx < 50 || d < 18 || d < mx * 0.2) return -1;
        double h;
        if (mx == r) h = 60.0 * (((g - b) / (double) d) % 6);
        else if (mx == g) h = 60.0 * ((b - r) / (double) d + 2);
        else h = 60.0 * ((r - g) / (double) d + 4);
        if (h < 0) h += 360;
        if (h >= 85 && h <= 160) return GREEN;
        if (h >= 190 && h <= 250) return BLUE;
        if (h >= 335 || h <= 18) return d >= 40 ? RED : -1;
        return -1;
    }

    /** 상자: x1 y1 x2 y2 (px) · 칸 수 · 색 */
    public static final class Box {
        public int x1, y1, x2, y2, n, c;

        public int w() {
            return x2 - x1 + 1;
        }

        public int h() {
            return y2 - y1 + 1;
        }

        public double cx() {
            return (x1 + x2) / 2.0;
        }

        public double cy() {
            return (y1 + y2) / 2.0;
        }

        @Override
        public String toString() {
            return "c" + c + "[" + x1 + "," + y1 + "," + x2 + "," + y2 + "]";
        }
    }

    /** 영역 안의 색 상자들 (작은 조각은 이어 붙임) */
    public static List<Box> boxes(Img im, int x0, int y0, int x1, int y1) {
        int W = im.w;
        x0 = Math.max(0, x0);
        y0 = Math.max(0, y0);
        x1 = Math.min(im.w, x1);
        y1 = Math.min(im.h, y1);
        int w = x1 - x0, h = y1 - y0;
        List<Box> out = new ArrayList<>();
        if (w <= 2 || h <= 2) return out;
        byte[] m = new byte[w * h];
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                int k = (y0 + y) * W + x0 + x;
                m[y * w + x] = (byte) (cls(im.r[k], im.g[k], im.b[k]) + 1);
            }
        }
        int[] stack = new int[w * h];
        for (int i = 0; i < m.length; i++) {
            if (m[i] <= 0) continue;
            int c = m[i];
            Box b = new Box();
            b.c = c - 1;
            b.x1 = b.y1 = Integer.MAX_VALUE;
            int sp = 0;
            stack[sp++] = i;
            m[i] = -1;
            while (sp > 0) {
                int k = stack[--sp], x = k % w, y = k / w;
                b.n++;
                b.x1 = Math.min(b.x1, x);
                b.x2 = Math.max(b.x2, x);
                b.y1 = Math.min(b.y1, y);
                b.y2 = Math.max(b.y2, y);
                for (int dy = -1; dy <= 1; dy++) {
                    for (int dx = -1; dx <= 1; dx++) {
                        int nx = x + dx, ny = y + dy;
                        if (nx < 0 || ny < 0 || nx >= w || ny >= h) continue;
                        int q = ny * w + nx;
                        if (m[q] == c) {
                            m[q] = -1;
                            stack[sp++] = q;
                        }
                    }
                }
            }
            if (b.n < 6) continue;
            b.x1 += x0;
            b.x2 += x0;
            b.y1 += y0;
            b.y2 += y0;
            out.add(b);
        }
        // 같은 색 조각끼리 가까우면 이어 붙임 (테두리가 글자 · 그림자에 끊긴 경우)
        int gap = Math.max(2, im.w / 300);
        boolean merged = true;
        while (merged) {
            merged = false;
            for (int i = 0; i < out.size() && !merged; i++) {
                for (int j = i + 1; j < out.size(); j++) {
                    Box a = out.get(i), b = out.get(j);
                    if (a.c != b.c) continue;
                    if (b.x1 > a.x2 + gap || a.x1 > b.x2 + gap || b.y1 > a.y2 + gap || a.y1 > b.y2 + gap) continue;
                    a.x1 = Math.min(a.x1, b.x1);
                    a.y1 = Math.min(a.y1, b.y1);
                    a.x2 = Math.max(a.x2, b.x2);
                    a.y2 = Math.max(a.y2, b.y2);
                    a.n += b.n;
                    out.remove(j);
                    merged = true;
                    break;
                }
            }
        }
        return out;
    }

    /** 버튼처럼 생긴 상자 (가로로 긴 테두리 · 화면에 비해 알맞은 크기) */
    static List<Box> buttons(List<Box> all, Img im) {
        List<Box> out = new ArrayList<>();
        for (Box b : all) {
            double w = b.w(), h = b.h(), r = w / h;
            if (h < im.h * 0.012 || h < 6 || h > im.h * 0.16 || w < im.w * 0.025 || w > im.w * 0.3) continue;
            if (r < 1.6 || r > 9) continue;
            double fill = b.n / (w * h);
            if (fill > 0.75) continue;                  // 꽉 찬 색 덩어리 (그림 · 배경) 는 버튼 테두리가 아님
            out.add(b);
        }
        return out;
    }

    static boolean sameRow(Box a, Box b, Img im) {
        double hr = (double) a.h() / b.h();
        return Math.abs(a.cy() - b.cy()) <= Math.max(a.h(), b.h()) * 0.35 + im.h * 0.006 && hr > 0.6 && hr < 1.65;
    }

    /**
     * 초록 상자 g 의 줄에서, x 가 [from, to) 인 칸 중 파란 테두리가 있는 범위 → 상자 (없으면 null)
     * 파랑은 로블록스 바닥 · 하늘과 이어져 한 덩어리가 되기 쉬워서 상자 찾기 대신 '그 줄의 파란 칸' 으로 봄
     */
    static Box blueSpan(Img im, Box g, int from, int to) {
        return span(im, g, from, to, BLUE);
    }

    static Box span(Img im, Box g, int from, int to, int want) {
        from = Math.max(0, from);
        to = Math.min(im.w, to);
        int y1 = Math.max(0, g.y1), y2 = Math.min(im.h - 1, g.y2), n = to - from;
        if (n <= 4) return null;
        int[] col = new int[n];
        for (int x = from; x < to; x++) {
            int c = 0;
            for (int y = y1; y <= y2; y++) {
                int k = y * im.w + x;
                if (cls(im.r[k], im.g[k], im.b[k]) == want) c++;
            }
            col[x - from] = c;
        }
        // 위 · 아래 테두리 (열마다 파란 칸 2개 이상) 가 이어진 가장 긴 범위 (작은 틈은 넘어감)
        int bestA = -1, bestB = -1, a = -1, last = -1, skip = Math.max(3, g.w() / 12);
        for (int i = 0; i < n; i++) {
            if (col[i] >= 2) {
                if (a < 0 || i - last > skip) a = i;
                last = i;
                if (last - a > bestB - bestA) {
                    bestA = a;
                    bestB = last;
                }
            }
        }
        if (bestA < 0) return null;
        Box b = new Box();
        b.c = want;
        b.x1 = from + bestA;
        b.x2 = from + bestB;
        b.y1 = g.y1;
        b.y2 = g.y2;
        b.n = 0;
        for (int i = bestA; i <= bestB; i++) b.n += col[i];
        return b;
    }

    /** 대화 선택지 줄 → {Open Shop, Sell Fish, Leave} 또는 null — 초록 상자 → 오른쪽 파란 칸 → 그 오른쪽 빨간 칸 (아래쪽 60% 에서 찾음) */
    public static Box[] dialog(Img im) {
        return dialog(im, buttons(boxes(im, 0, (int) (im.h * 0.4), im.w, im.h), im));
    }

    static Box[] dialog(Img im, List<Box> bs) {
        Box[] best = null;
        double bestScore = 1e18;
        for (Box g : bs) {
            if (g.c != GREEN) continue;
            Box b = span(im, g, g.x2 + 1, g.x2 + (int) (g.w() * 1.9), BLUE);
            if (b == null || b.w() < g.w() * 0.5 || b.w() > g.w() * 1.6) continue;
            double gl = b.x1 - g.x2;
            if (gl > g.w() * 0.6) continue;
            // 빨강 [Leave] (테두리 일부가 Quick roll 글자에 가려도 칸으로 보면 잡힘)
            Box r = span(im, g, b.x2 + 1, b.x2 + (int) (g.w() * 1.9), RED);
            if (r == null || r.w() < g.w() * 0.3) continue;
            double gr = r.x1 - b.x2;
            if (gr > g.w() * 0.6) continue;
            double score = Math.abs(gl - gr);
            if (score < bestScore) {
                bestScore = score;
                best = new Box[]{g, b, r};
            }
        }
        return best;
    }

    /** 상점 [Sell] · [Sell All] → {Sell, Sell All} 또는 null */
    public static Box[] shop(Img im) {
        List<Box> bs = buttons(boxes(im, 0, (int) (im.h * 0.3), im.w, im.h), im);
        Box[] best = null;
        double bestScore = 1e18;
        for (Box g : bs) {
            if (g.c != GREEN) continue;
            // 상점의 [Sell] 은 가운데 창 왼쪽 칸 아래 (오른쪽 위 순위표 · Acrux 창의 초록 · 파랑이 아님)
            if (g.cx() > im.w * 0.6 || g.cx() < im.w * 0.08 || g.cy() < im.h * 0.45) continue;
            Box b = blueSpan(im, g, g.x2 + 1, g.x2 + (int) (g.w() * 1.8));
            if (b == null) continue;
            // 그 오른쪽에 빨강 [Leave] 가 있으면 대화 선택지 ([Open Shop] · [Sell Fish]) — 상점이 아님
            Box r = span(im, g, b.x2 + 1, b.x2 + (int) (g.w() * 1.9), RED);
            if (r != null && r.w() >= g.w() * 0.3 && r.x1 - b.x2 <= g.w() * 0.6) continue;
            double gap = b.x1 - g.x2, wr = (double) g.w() / b.w();
            if (gap > g.w() * 0.4 || wr < 0.6 || wr > 1.65) continue;
            double score = Math.abs(gap) + Math.abs(g.w() - b.w());
            if (score < bestScore) {
                bestScore = score;
                best = new Box[]{g, b};
            }
        }
        return best;
    }

    /** 상자 가운데 (화면 비율) */
    public static double[] center(Box b, Img im) {
        return new double[]{SellText.r4(b.cx() / im.w), SellText.r4(b.cy() / im.h)};
    }
}
