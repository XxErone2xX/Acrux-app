package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * 화면 분석 (PC 버전 fishing.py 의 순수 함수를 그대로 옮김 · 같은 입력이면 같은 결과)
 * 상태는 화면 픽셀 색으로만 판단 · 위치는 전부 화면 비율
 */
public final class Vision {
    private Vision() {}

    // ---------------------------------------------------------------- 배치 (창 영역 안의 비율)
    public static final double[] DIAMOND_IDLE = {0.9268, 0.8264};
    public static final double[] DIAMOND_REEL = {0.9931, 0.8333};
    public static final double[] DIAMOND_IDLE_R = {0.8735, 0.8239};
    public static final double[] DIAMOND_REEL_R = {0.9317, 0.8310};
    public static final double[] PANEL_FISH_BTN = {0.2494, 0.8264};
    public static final double[] PANEL_BAR = {0.0389, 0.2847, 0.9703, 0.4722};
    public static final double[] REEL_BAR = {0.0944, 0.2746, 0.9116, 0.4648};
    public static final double[] RESULT_CLOSE = {0.9222, 0.0579};
    public static final double[] RESULT_TITLE = {0.4972, 0.0913};
    public static final double REEL_FROM_PANEL = 498.0 / 437.0;
    /** 창 이름 → 가로:세로 비율 범위 (대기 창 · 미니게임 창 · 결과창) */
    public static final String[] SHAPE_KEYS = {"panel_region", "reel_region", "result_region"};
    public static final double[][] SHAPES = {{2.7, 3.25}, {3.3, 3.9}, {0.65, 0.95}};

    /** 창 영역 [x1, y1, x2, y2] + 안쪽 비율 → 화면 비율 (위치 2개 또는 영역 4개) */
    public static double[] layout(double[] region, double[] rel) {
        double x1 = Math.min(region[0], region[2]), x2 = Math.max(region[0], region[2]);
        double y1 = Math.min(region[1], region[3]), y2 = Math.max(region[1], region[3]);
        double w = x2 - x1, h = y2 - y1;
        double[] out = new double[rel.length];
        for (int i = 0; i < rel.length; i++) out[i] = round4(i % 2 == 0 ? x1 + rel[i] * w : y1 + rel[i] * h);
        return out;
    }

    /** 파이썬 round(v, 4) 와 같게 (정확히 반일 때만 짝수 쪽) */
    public static double round4(double v) {
        if (Double.isNaN(v) || Double.isInfinite(v)) return v;
        return new java.math.BigDecimal(v).setScale(4, java.math.RoundingMode.HALF_EVEN).doubleValue();
    }

    // ---------------------------------------------------------------- 공통
    static double median(double[] v, int n) {
        if (n <= 0) return Double.NaN;
        double[] s = Arrays.copyOf(v, n);
        Arrays.sort(s);
        return n % 2 == 1 ? s[n / 2] : (s[n / 2 - 1] + s[n / 2]) / 2.0;
    }

    /** 이미지 세로 [y0, y1) 줄들의 열마다 중앙값 → band[x] = {r, g, b} */
    static double[][] colMedian(Img im, int y0, int y1) {
        y0 = Math.max(0, y0);
        y1 = Math.min(im.h, y1);
        int n = Math.max(0, y1 - y0);
        double[][] out = new double[im.w][3];
        double[] t = new double[Math.max(1, n)];
        for (int x = 0; x < im.w; x++) {
            for (int c = 0; c < 3; c++) {
                int[] ch = c == 0 ? im.r : c == 1 ? im.g : im.b;
                for (int y = 0; y < n; y++) t[y] = ch[(y0 + y) * im.w + x];
                out[x][c] = median(t, n);
            }
        }
        return out;
    }

    static double mx(double[] p) {
        return Math.max(p[0], Math.max(p[1], p[2]));
    }

    static double mn(double[] p) {
        return Math.min(p[0], Math.min(p[1], p[2]));
    }

    // ---------------------------------------------------------------- 버튼 · 결과창
    /** Fish/Exit 버튼 주변 → "fish"(파랑) / "exit"(빨강) / null */
    public static String buttonState(Img im) {
        int n = Math.max(1, im.w * im.h), dark = 0, blue = 0, red = 0;
        for (int k = 0; k < im.w * im.h; k++) {
            int r = im.r[k], g = im.g[k], b = im.b[k];
            if (Math.max(r, Math.max(g, b)) < 70) dark++;
            if (b > 140 && b > r + 45 && b > g + 20) blue++;
            if (r > 150 && r > g + 60 && r > b + 40) red++;
        }
        if ((double) dark / n < 0.45) return null;
        if (Math.max(blue, red) < n * 0.01) return null;
        return blue >= red ? "fish" : "exit";
    }

    static final int[] FAIL_RGB = {255, 65, 65}, JUNK_RGB = {186, 186, 186};

    static boolean titleLike(boolean[] m, int w, int h, double least) {
        int cnt = 0, cols = 0, rows = 0;
        boolean[] col = new boolean[w];
        for (int y = 0; y < h; y++) {
            boolean any = false;
            for (int x = 0; x < w; x++) {
                if (m[y * w + x]) {
                    cnt++;
                    any = true;
                    col[x] = true;
                }
            }
            if (any) rows++;
        }
        for (boolean c : col) if (c) cols++;
        return cnt >= least && (w > 0 && (double) cols / w >= 0.3) && rows >= Math.max(4, h * 0.15);
    }

    /** 결과창 제목 주변 → "fail" / "junk" / "success" / null */
    public static String classifyTitle(Img im) {
        int n = Math.max(1, im.w * im.h), N = im.w * im.h;
        boolean[] fail = new boolean[N], junk = new boolean[N], other = new boolean[N];
        for (int k = 0; k < N; k++) {
            int r = im.r[k], g = im.g[k], b = im.b[k];
            fail[k] = Math.abs(r - FAIL_RGB[0]) <= 8 && Math.abs(g - FAIL_RGB[1]) <= 8 && Math.abs(b - FAIL_RGB[2]) <= 8;
            junk[k] = Math.abs(r - JUNK_RGB[0]) <= 8 && Math.abs(g - JUNK_RGB[1]) <= 8 && Math.abs(b - JUNK_RGB[2]) <= 8;
            int mx = Math.max(r, Math.max(g, b)), mn = Math.min(r, Math.min(g, b));
            other[k] = (mn >= 232 || (mx >= 220 && mx - mn >= 50)) && !fail[k];
        }
        double need = Math.max(12, n * 0.006);
        if (titleLike(fail, im.w, im.h, need)) return "fail";
        if (titleLike(other, im.w, im.h, need)) return "success";
        if (titleLike(junk, im.w, im.h, Math.max(20, n * 0.02))) return "junk";
        return null;
    }

    // ---------------------------------------------------------------- 릴링 바
    static boolean colored(double[] p) {
        double a = mx(p), b = mn(p);
        return (a - b) > 50 && a > 90;
    }

    static boolean isFill(double[] p) {
        double r = p[0], g = p[1], b = p[2];
        return r < 100 && g >= 80 && b >= 140 && (b - g) >= -15 && (b - g) <= 85;
    }

    static boolean leftOk(double[][] band) {
        int w = band.length, n = Math.max(3, (int) (w * 0.04)), m = Math.min(n, w);
        double[] head = new double[3], t = new double[Math.max(1, m)];
        for (int c = 0; c < 3; c++) {
            for (int x = 0; x < m; x++) t[x] = band[x][c];
            head[c] = median(t, m);
        }
        if (mx(head) < 45 || isFill(head)) return true;
        int dark = 0;
        for (double[] p : band) if (mx(p) < 45) dark++;
        return w > 0 && (double) dark / w >= 0.08;
    }

    /** 바 가운데 줄 몇 개만 잡은 이미지 → 릴링 바가 떠 있는지 */
    public static boolean barPresent(Img im) {
        double[][] band = colMedian(im, 0, im.h);
        int cnt = 0;
        for (double[] p : band) if (colored(p)) cnt++;
        return cnt >= Math.max(4, band.length * 0.03) && leftOk(band);
    }

    // ---------------------------------------------------------------- 자동 보정 (흰 꺾쇠 창 테두리)
    /** 각 칸에서 한 방향으로 이어진 true 칸 수 · horiz: 가로(x) 방향 · reverse: 오른쪽/아래에서부터 */
    static int[] runs(boolean[] m, int w, int h, boolean horiz, boolean reverse) {
        int[] out = new int[w * h];
        if (horiz) {
            for (int y = 0; y < h; y++) {
                int prev = 0;
                for (int j = 0; j < w; j++) {
                    int x = reverse ? w - 1 - j : j, k = y * w + x;
                    prev = m[k] ? prev + 1 : 0;
                    out[k] = prev;
                }
            }
        } else {
            for (int x = 0; x < w; x++) {
                int prev = 0;
                for (int j = 0; j < h; j++) {
                    int y = reverse ? h - 1 - j : j, k = y * w + x;
                    prev = m[k] ? prev + 1 : 0;
                    out[k] = prev;
                }
            }
        }
        return out;
    }

    static boolean at(boolean[] m, int w, int h, int x, int y) {
        return x >= 0 && y >= 0 && x < w && y < h && m[y * w + x];
    }

    static boolean anyIn(boolean[] m, int w, int h, int x0, int y0, int x1, int y1) {
        x0 = Math.max(0, x0);
        y0 = Math.max(0, y0);
        x1 = Math.min(w, x1);
        y1 = Math.min(h, y1);
        for (int y = y0; y < y1; y++) for (int x = x0; x < x1; x++) if (m[y * w + x]) return true;
        return false;
    }

    /** 화면 → 흰 꺾쇠로 둘러싸인 창들 [x1, y1, x2, y2] (px · 바깥 모서리) · maxW: 창 폭 최대 (화면 폭 비율) */
    public static List<int[]> findFrames(Img im, double maxW) {
        List<int[]> out = findFrames(im, maxW, 215);
        // 모바일: 창 꺾쇠(250 ~ 255) 바깥에 회색 테두리가 한 겹 더 있고 그 안에 216 ~ 231 짜리 밝은 점이 섞여 있음
        // → 215 기준이면 꺾쇠 모서리가 그 점들과 붙어서 모서리로 안 보임 → 더 밝은 기준(238)으로 한 번 더 찾아서 새로 나온 창만 뒤에 붙임
        for (int[] f : findFrames(im, maxW, 238)) {
            boolean dup = false;
            for (int[] g : out) {
                if (Math.abs(f[0] - g[0]) <= 6 && Math.abs(f[1] - g[1]) <= 6 && Math.abs(f[2] - g[2]) <= 6 && Math.abs(f[3] - g[3]) <= 6) {
                    dup = true;
                    break;
                }
            }
            if (!dup) out.add(f);
        }
        return out;
    }

    /** lo: 흰색 기준 (가장 어두운 채널이 이것보다 밝아야 흰색) */
    public static List<int[]> findFrames(Img im, double maxW, int lo) {
        int W = im.w, H = im.h, N = W * H;
        boolean[] white0 = new boolean[N], white = new boolean[N];
        for (int k = 0; k < N; k++) {
            int mn = im.min(k), mx = im.max(k);
            white0[k] = mn > lo && (mx - mn) < 35;
        }
        for (int y = 0; y < H; y++) {
            for (int x = 0; x < W; x++) {
                white[y * W + x] = white0[y * W + x] || at(white0, W, H, x, y - 1) || at(white0, W, H, x, y + 1)
                        || at(white0, W, H, x - 1, y) || at(white0, W, H, x + 1, y);
            }
        }
        int arm = Math.max(6, (int) (W * 0.004));
        int[] r = runs(white, W, H, true, true), l = runs(white, W, H, true, false);
        int[] d = runs(white, W, H, false, true), u = runs(white, W, H, false, false);
        List<int[]> tl = new ArrayList<>(), br = new ArrayList<>();
        boolean[] tr = new boolean[N], bl = new boolean[N];
        for (int y = 0; y < H; y++) {
            for (int x = 0; x < W; x++) {
                int k = y * W + x;
                if (!white[k]) continue;
                if (r[k] >= arm && d[k] >= arm && !at(white, W, H, x, y - 1) && !at(white, W, H, x - 1, y)) tl.add(new int[]{x, y});
                if (l[k] >= arm && u[k] >= arm && !at(white, W, H, x, y + 1) && !at(white, W, H, x + 1, y)) br.add(new int[]{x, y});
                tr[k] = l[k] >= arm && d[k] >= arm;
                bl[k] = r[k] >= arm && u[k] >= arm;
            }
        }
        List<int[]> frames = new ArrayList<>();
        for (int[] a : tl) {
            int x1 = a[0], y1 = a[1];
            long bestA = -1;
            int bx = 0, by = 0;
            for (int[] b : br) {
                int x2 = b[0], y2 = b[1], w = x2 - x1, h = y2 - y1;
                if (w < W * 0.1 || w > W * maxW || h < arm * 3) continue;
                long area = (long) w * h;
                if ((bestA < 0 || area < bestA) && anyIn(tr, W, H, x2 - 3, y1 - 3, x2 + 4, y1 + 4)
                        && anyIn(bl, W, H, x1 - 3, y2 - 3, x1 + 4, y2 + 4)) {
                    bestA = area;
                    bx = x2;
                    by = y2;
                }
            }
            if (bestA >= 0) frames.add(snap(white0, W, H, x1, y1, bx, by));
        }
        // 같은 창이 여러 번 잡히면 하나로 (가장 큰 것) — 정렬은 안정 정렬 (파이썬 sort 와 같음)
        frames.sort((p, q) -> Long.compare((long) (q[2] - q[0]) * (q[3] - q[1]), (long) (p[2] - p[0]) * (p[3] - p[1])));
        List<int[]> out = new ArrayList<>();
        for (int[] f : frames) {
            boolean ok = true;
            for (int[] g : out) {
                if (!(Math.abs(f[0] - g[0]) > 6 || Math.abs(f[1] - g[1]) > 6)) {
                    ok = false;
                    break;
                }
            }
            if (ok) out.add(f);
        }
        return out;
    }

    static int[] snap(boolean[] w0, int W, int H, int x1, int y1, int x2, int y2) {
        int ax = Math.max(0, x1 - 1), ay = Math.max(0, y1 - 1);
        int minX = Integer.MAX_VALUE, minY = Integer.MAX_VALUE;
        for (int y = ay; y < Math.min(H, y1 + 4); y++) {
            for (int x = ax; x < Math.min(W, x1 + 4); x++) {
                if (w0[y * W + x]) {
                    minX = Math.min(minX, x - ax);
                    minY = Math.min(minY, y - ay);
                }
            }
        }
        if (minX != Integer.MAX_VALUE) {
            x1 = ax + minX;
            y1 = ay + minY;
        }
        int bx = Math.max(0, x2 - 3), by = Math.max(0, y2 - 3);
        int maxX = -1, maxY = -1;
        for (int y = by; y < Math.min(H, y2 + 2); y++) {
            for (int x = bx; x < Math.min(W, x2 + 2); x++) {
                if (w0[y * W + x]) {
                    maxX = Math.max(maxX, x - bx);
                    maxY = Math.max(maxY, y - by);
                }
            }
        }
        if (maxX >= 0) {
            x2 = bx + maxX;
            y2 = by + maxY;
        }
        return new int[]{x1, y1, x2, y2};
    }

    /** 화면 → {"panel_region" | "reel_region" | "result_region": [비율 x1, y1, x2, y2]} (+ "_reel_derived") */
    public static Map<String, double[]> autocal(Img im, double maxW) {
        Map<String, double[]> found = new LinkedHashMap<>();
        int W = im.w, H = im.h;
        for (int[] f : findFrames(im, maxW)) {
            double ratio = (double) (f[2] - f[0]) / Math.max(1, f[3] - f[1]);
            for (int i = 0; i < SHAPE_KEYS.length; i++) {
                if (SHAPES[i][0] <= ratio && ratio <= SHAPES[i][1] && !found.containsKey(SHAPE_KEYS[i])) {
                    found.put(SHAPE_KEYS[i], new double[]{round4((double) f[0] / W), round4((double) f[1] / H),
                            round4((double) f[2] / W), round4((double) f[3] / H)});
                }
            }
        }
        if (found.containsKey("panel_region") && !found.containsKey("reel_region")) {
            double[] p = found.get("panel_region");
            double c = (p[0] + p[2]) / 2, half = (p[2] - p[0]) / 2 * REEL_FROM_PANEL;
            found.put("reel_region", new double[]{round4(c - half), p[1], round4(c + half), p[3]});
            found.put("_reel_derived", new double[]{1});
        }
        return found;
    }

    // ---------------------------------------------------------------- 릴링 바 분석
    static boolean[] edge(Img im, int row, double lo, double hi) {
        boolean[] out = new boolean[im.w];
        for (int x = 0; x < im.w; x++) {
            int k = row * im.w + x, a = im.max(k), b = im.min(k);
            double mean = (im.r[k] + im.g[k] + im.b[k]) / 3.0;
            out[x] = (a - b) < 42 && mean > lo && mean < hi;
        }
        return out;
    }

    static double frac(boolean[] m) {
        if (m.length == 0) return Double.NaN;
        int c = 0;
        for (boolean v : m) if (v) c++;
        return (double) c / m.length;
    }

    /** 릴링 바 테두리 → {바 안쪽 맨 윗줄, 안쪽 높이, 왼쪽 끝 x, 오른쪽 끝 x} 또는 null */
    public static int[] findBarRows(Img im, double guessTop, double barH) {
        int h = im.h;
        int lo = Math.max(0, (int) (guessTop - barH)), hi = Math.min(h - 2, (int) (guessTop + barH));
        int best = -1;
        for (int r = lo; r < hi; r++) {
            if (frac(edge(im, r, 90, 200)) >= 0.75) {
                if (best < 0 || Math.abs(r + 1 - guessTop) < Math.abs(best + 1 - guessTop)) best = r;
            }
        }
        if (best < 0) return null;
        // 테두리 맨 윗줄이 흐릿하게 끊겨 있으면 (확대 · 압축) 바로 아래의 더 꽉 찬 줄로
        while (best + 1 < hi && frac(edge(im, best + 1, 90, 200)) > frac(edge(im, best, 90, 200)) + 0.05) best++;
        boolean[] g = edge(im, best, 90, 200);
        List<int[]> rs = new ArrayList<>();
        int[] cur = null;
        int gap = 0;
        for (int x = 0; x < g.length; x++) {
            if (g[x]) {
                cur = cur == null ? new int[]{x, x} : new int[]{cur[0], x};
                gap = 0;
            } else if (cur != null) {
                gap++;
                if (gap > 2) {
                    rs.add(cur);
                    cur = null;
                    gap = 0;
                }
            }
        }
        if (cur != null) rs.add(cur);
        // 테두리 줄이 ◇ · 글자에 가려 끊기면 한 줄로 이음 (끊긴 틈이 바 폭의 4% 이하)
        int join = Math.max(3, (int) (im.w * 0.04));
        for (int i = rs.size() - 1; i > 0; i--) {
            int[] a = rs.get(i - 1), b = rs.get(i);
            if (b[0] - a[1] <= join) {
                rs.set(i - 1, new int[]{a[0], b[1]});
                rs.remove(i);
            }
        }
        int x0 = 0, x1 = im.w - 1;
        if (!rs.isEmpty()) {
            int[] m = rs.get(0);
            for (int[] q : rs) if (q[1] - q[0] > m[1] - m[0]) m = q;
            x0 = m[0];
            x1 = m[1];
        }
        int top = best + 1;
        int inner = (int) barH, lim = Math.min(h, top + (int) (barH * 1.6));
        // 모바일은 바 테두리가 두꺼움 (3 ~ 4줄) → 윗 테두리가 끝나는 줄부터 아래 테두리를 찾음
        int r0 = top;
        while (r0 < lim && frac(edge(im, r0, 40, 200)) >= 0.6) r0++;
        for (int r = Math.max(top + 3, r0 + 1); r < lim; r++) {
            if (frac(edge(im, r, 40, 200)) >= 0.6) {
                inner = r - top;
                break;
            }
        }
        return new int[]{top, Math.max(4, inner), x0, x1};
    }

    static double[][] band(Img im, double barTop, double barH) {
        int y0 = (int) (barTop + barH * 0.35);
        int y1 = Math.max(y0 + 1, (int) (barTop + barH * 0.65));
        return colMedian(im, y0, y1);
    }

    /** 미니게임 시작 때 막대 색 → {r, g, b} 또는 null */
    public static int[] learnFill(Img im, double barTop, double barH) {
        double[][] bd = band(im, barTop, barH);
        int w = bd.length;
        int s0 = Math.max(1, (int) (w * 0.01)), s1 = Math.min(w, Math.max(3, (int) (w * 0.15)));
        if (s1 <= s0) return null;
        int n = s1 - s0, okN = 0;
        boolean[] ok = new boolean[n];
        for (int i = 0; i < n; i++) {
            double[] p = bd[s0 + i];
            ok[i] = colored(p) && isFill(p);
            if (ok[i]) okN++;
        }
        if ((double) okN / n < 0.9) return null;
        double[] ref = new double[3], t = new double[okN];
        for (int c = 0; c < 3; c++) {
            int j = 0;
            for (int i = 0; i < n; i++) if (ok[i]) t[j++] = bd[s0 + i][c];
            ref[c] = median(t, okN);
        }
        double sum = 0;
        for (int i = 0; i < n; i++) {
            if (!ok[i]) continue;
            double[] p = bd[s0 + i];
            sum += Math.max(Math.abs(p[0] - ref[0]), Math.max(Math.abs(p[1] - ref[1]), Math.abs(p[2] - ref[2])));
        }
        if (sum / okN > 12) return null;
        return new int[]{(int) ref[0], (int) ref[1], (int) ref[2]};
    }

    /** 바 분석 결과 */
    public static final class Bar {
        public boolean present;
        public Double marker;               // 내 위치 (px · 없으면 null)
        public int[] zone;                  // 물고기 구간 (시작, 끝) 또는 null
        public List<int[]> zones = new ArrayList<>();
        public int w;
    }

    /** im: 릴링 바 + 그 위 ◇ 까지 잡은 이미지 · barTop/barH: 그 안의 바 세로 위치 · fillRef: 이번 막대 색 */
    public static Bar analyzeBar(Img im, double barTop, double barH, int[] fillRef) {
        int w = im.w;
        double[][] bd = band(im, barTop, barH);
        boolean[] col = new boolean[w];
        int cc = 0;
        for (int x = 0; x < w; x++) if (col[x] = colored(bd[x])) cc++;
        Bar out = new Bar();
        out.w = w;
        out.present = cc >= Math.max(4, w * 0.03) && leftOk(bd);
        if (!out.present) return out;
        boolean[] texty = new boolean[w], fill = new boolean[w], zone = new boolean[w], skip = new boolean[w];
        for (int x = 0; x < w; x++) {
            double a = mx(bd[x]), b = mn(bd[x]);
            texty[x] = (a - b) <= 50 && a > 90;
        }
        if (fillRef != null) {
            boolean[] white = new boolean[w];
            for (int x = 0; x < w; x++) {
                double[] p = bd[x];
                double dist = Math.max(Math.abs(p[0] - fillRef[0]), Math.max(Math.abs(p[1] - fillRef[1]), Math.abs(p[2] - fillRef[2])));
                boolean lit = mx(p) > 75;
                fill[x] = lit && dist <= 22;
                white[x] = mn(p) > 185;
            }
            for (int x = 0; x < w; x++) {
                boolean s = white[x];
                for (int k = 1; k <= 2 && !s; k++) s = (x - k >= 0 && white[x - k]) || (x + k < w && white[x + k]);
                skip[x] = s;
            }
            for (int x = 0; x < w; x++) zone[x] = mx(bd[x]) > 75 && !fill[x] && !skip[x];
        } else {
            for (int x = 0; x < w; x++) {
                fill[x] = col[x] && isFill(bd[x]);
                zone[x] = col[x] && !fill[x];
                skip[x] = texty[x];
            }
        }
        int fillEnd = -1;
        for (int x = 0; x < w; x++) if (fill[x]) fillEnd = x;
        List<int[]> segs = new ArrayList<>();
        int[] cur = null;
        int gap = 0;
        for (int x = 0; x < w; x++) {
            if (zone[x]) {
                if (cur == null) cur = new int[]{x, x};
                cur[1] = x;
                gap = 0;
            } else if (cur != null) {
                if (skip[x]) continue;
                gap++;
                if (gap > 4) {
                    segs.add(cur);
                    cur = null;
                    gap = 0;
                }
            }
        }
        if (cur != null) segs.add(cur);
        for (int[] s : segs) if (s[1] - s[0] >= 3) out.zones.add(s);
        int[] best = null;
        for (int[] s : out.zones) if (best == null || s[1] - s[0] > best[1] - best[0]) best = s;
        if (best != null && best[1] - best[0] >= Math.max(3, w * 0.02)) out.zone = best;
        // 내 위치: 바 위쪽 ◇ (밝은 흰색) 가로 위치
        if (barTop >= 3) {
            int ty0 = Math.max(0, (int) (barTop - barH * 1.7)), ty1 = Math.min(im.h, (int) barTop - 1);
            int rows = Math.max(1, ty1 - ty0);
            double[] cnt = new double[w];
            for (int y = ty0; y < ty1; y++) for (int x = 0; x < w; x++) if (im.min(y * w + x) > 185) cnt[x]++;
            double[] orig = cnt.clone();
            for (int x = 0; x < w; x++) {
                double side = Math.max(x - 2 >= 0 ? orig[x - 2] : 0, x + 2 < w ? orig[x + 2] : 0);
                if (orig[x] >= rows * 0.8 && side < orig[x] * 0.3) cnt[x] = 0;
            }
            double total = 0, cmax = 0;
            for (double v : cnt) {
                total += v;
                cmax = Math.max(cmax, v);
            }
            if (total >= 6) {
                int k = Math.max(5, (int) (w * 0.05)), off = (k - 1) / 2;
                int c = 0;
                double bestD = -1;
                for (int i = 0; i < w; i++) {
                    double s = 0;
                    for (int j = i + off - (k - 1); j <= i + off; j++) if (j >= 0 && j < w) s += cnt[j];
                    if (s > bestD) {
                        bestD = s;
                        c = i;
                    }
                }
                int lo = Math.max(0, c - k), hi = Math.min(w, c + k + 1);
                double ss = 0, sx = 0;
                for (int x = lo; x < hi; x++) {
                    ss += cnt[x];
                    sx += x * cnt[x];
                }
                if (ss >= 6) {
                    out.marker = sx / ss;
                    double e0 = Math.max(cnt[0], w > 1 ? cnt[1] : 0), e1 = Math.max(cnt[w - 1], w > 1 ? cnt[w - 2] : 0);
                    if (e0 >= cmax * 0.5) out.marker = 0.0;
                    else if (e1 >= cmax * 0.5) out.marker = (double) (w - 1);
                }
            }
        }
        int end = Math.max(0, fillEnd);
        int[] z = out.zone;
        Double fillPos = (z == null || !(z[0] - 3 <= end && end <= z[1] + 2)) ? (double) end : null;
        if (fillPos != null && fillEnd >= 0) {
            int a0 = fillEnd + 1, a1 = Math.min(w, fillEnd + 1 + Math.max(8, (int) (w * 0.025)));
            boolean bad = false;
            for (int x = a0; x < a1 && !bad; x++) bad = col[x] || zone[x];
            for (int x = Math.max(0, fillEnd - 3); x < Math.max(a1, Math.max(0, fillEnd - 3)) && !bad; x++) bad = x < w && texty[x];
            if (bad) fillPos = null;
        }
        if (out.marker == null) {
            out.marker = fillEnd < 0 ? Double.valueOf(0.0) : fillPos;
        } else if (fillPos != null && Math.abs(out.marker - fillPos) > Math.max(12, w * 0.04)) {
            out.marker = fillPos;
        }
        return out;
    }

    // ---------------------------------------------------------------- 결과창 X 찾기
    /** 화면 안에서 흰 × 버튼 → {가운데 x, y} 또는 null · H = 창 높이 · want: 원래 X 자리 (있으면 가장 가까운 것) */
    public static double[] findCloseX(Img im, int H, double[] want) {
        int W = im.w, Hh = im.h, N = W * Hh;
        boolean[] white = new boolean[N], dark = new boolean[N];
        for (int k = 0; k < N; k++) {
            white[k] = im.min(k) > 150;
            dark[k] = im.max(k) < 95;
        }
        // 8방향 이어진 덩어리 (위 → 아래 · 왼쪽 → 오른쪽 순서로 번호)
        int[] lab = new int[N];
        List<int[]> stats = new ArrayList<>();          // x1, y1, x2, y2, n
        stats.add(new int[]{0, 0, 0, 0, 0});
        int[] q = new int[N];
        for (int s = 0; s < N; s++) {
            if (!white[s] || lab[s] != 0) continue;
            int id = stats.size(), qh = 0, qt = 0;
            int[] st = {s % W, s / W, s % W, s / W, 0};
            lab[s] = id;
            q[qt++] = s;
            while (qh < qt) {
                int p = q[qh++], px = p % W, py = p / W;
                st[0] = Math.min(st[0], px);
                st[1] = Math.min(st[1], py);
                st[2] = Math.max(st[2], px);
                st[3] = Math.max(st[3], py);
                st[4]++;
                for (int dy = -1; dy <= 1; dy++) {
                    for (int dx = -1; dx <= 1; dx++) {
                        int nx = px + dx, ny = py + dy;
                        if ((dx | dy) == 0 || nx < 0 || ny < 0 || nx >= W || ny >= Hh) continue;
                        int nk = ny * W + nx;
                        if (white[nk] && lab[nk] == 0) {
                            lab[nk] = id;
                            q[qt++] = nk;
                        }
                    }
                }
            }
            stats.add(st);
        }
        boolean[] big = new boolean[stats.size()];
        for (int i = 1; i < stats.size(); i++) {
            int[] st = stats.get(i);
            int bw = st[2] - st[0] + 1, bh = st[3] - st[1] + 1;
            big[i] = Math.max(bw, bh) > 0.05 * H || st[4] > Math.pow(0.04 * H, 2);
        }
        double[] best = null;
        double bestD = 0;
        for (int i = 1; i < stats.size(); i++) {
            if (big[i]) continue;
            int[] st = stats.get(i);
            int x1 = st[0], y1 = st[1], x2 = st[2], y2 = st[3], n = st[4], bw = x2 - x1 + 1, bh = y2 - y1 + 1;
            if (!(0.004 * H <= bw && bw <= 0.035 * H && 0.004 * H <= bh && bh <= 0.035 * H && 0.7 <= (double) bw / bh && (double) bw / bh <= 1.4)) continue;
            double fill = (double) n / (bw * bh);
            if (!(0.15 <= fill && fill <= 0.65)) continue;
            int m = Math.max(bw, bh);
            int ring = 0;
            for (int y = Math.max(0, y1 - m); y < Math.min(Hh, y2 + m + 1); y++) {
                for (int x = Math.max(0, x1 - m); x < Math.min(W, x2 + m + 1); x++) {
                    int k = y * W + x;
                    if (white[k] && !big[lab[k]]) ring++;
                }
            }
            if (ring - n > 0.15 * n) continue;
            if (diag(lab, W, Hh, i, x1, y1, bw, bh, false) < 0.85 || diag(lab, W, Hh, i, x1, y1, bw, bh, true) < 0.85) continue;
            double left = meanOf(dark, W, Hh, Math.max(0, x1 - 5 * m), y1, Math.max(0, x1 - m), y2 + 1);
            double below = meanOf(dark, W, Hh, x1, y2 + m, x2 + 1, y2 + 5 * m);
            if (!((!Double.isNaN(left) && left > 0.6) || (!Double.isNaN(below) && below > 0.6))) continue;
            double cx = (x1 + x2) / 2.0, cy = (y1 + y2) / 2.0;
            double dd = want != null ? (cx - want[0]) * (cx - want[0]) + (cy - want[1]) * (cy - want[1]) : cy;
            if (best == null || dd < bestD) {
                best = new double[]{cx, cy};
                bestD = dd;
            }
        }
        return best;
    }

    static double diag(int[] lab, int W, int H, int id, int x1, int y1, int bw, int bh, boolean flip) {
        int k = 9, hit = 0;
        for (int j = 1; j < k; j++) {
            double t = (double) j / k;
            int yy = (int) (t * (bh - 1)), xx = (int) ((flip ? 1 - t : t) * (bw - 1));
            boolean any = false;
            for (int y = Math.max(0, yy - 1); y < Math.min(bh, yy + 2) && !any; y++) {
                for (int x = Math.max(0, xx - 1); x < Math.min(bw, xx + 2) && !any; x++) {
                    int gx = x1 + x, gy = y1 + y;
                    if (gx < W && gy < H && lab[gy * W + gx] == id) any = true;
                }
            }
            if (any) hit++;
        }
        return (double) hit / (k - 1);
    }

    static double meanOf(boolean[] m, int W, int H, int x0, int y0, int x1, int y1) {
        x0 = Math.max(0, x0);
        y0 = Math.max(0, y0);
        x1 = Math.min(W, x1);
        y1 = Math.min(H, y1);
        int n = 0, c = 0;
        for (int y = y0; y < y1; y++) {
            for (int x = x0; x < x1; x++) {
                n++;
                if (m[y * W + x]) c++;
            }
        }
        return n == 0 ? Double.NaN : (double) c / n;
    }
}
