package com.acrux.mobile.core;

import java.util.ArrayList;

/**
 * 화면 위에 그린 칸 · 글자 (낚시 테스트) 가 화면 캡처에 같이 찍혀도 인식에 안 섞이게
 *  → 캡처에서 그린 자리 (화면 px 직사각형) 의 칸을 그 바로 바깥 칸 색으로 바꿔 넣음 (선은 얇아서 원래 화면과 거의 같음)
 *  · 그리는 쪽이 그릴 때마다 자리를 넘김 (set) · 옮기거나 지운 직후 몇 화면은 아직 찍혀 있을 수 있어서 TAIL_MS 동안 더 지움
 *  · 가로로 긴 칸은 위 · 아래 줄 색으로 · 세로로 긴 칸은 왼쪽 · 오른쪽 줄 색으로 (가까운 쪽 · 그 자리도 그린 자리면 그 너머)
 */
public final class Mask {
    /** 화면 (x, y) 의 색 0xRRGGBB · 화면 밖이면 -1 */
    public interface Src {
        int rgb(int x, int y);
    }

    public static final long TAIL_MS = 400;
    static int[][] cur = new int[0][];
    /** 지금은 안 그렸지만 아직 캡처에 남아 있을 수 있는 자리 {x0, y0, x1, y1, 끝 시각 ms} */
    static final ArrayList<long[]> old = new ArrayList<>();
    static volatile boolean any;

    private Mask() {}

    static long ms() {
        return System.nanoTime() / 1_000_000L;
    }

    /** 지금 그린 자리들 [x0, x1) × [y0, y1) (화면 px) — 전에 그렸는데 이번에 없는 자리는 TAIL_MS 동안 더 지움 */
    public static synchronized void set(int[][] rects) {
        long t = ms();
        for (int[] r : cur) if (!contains(rects, r)) old.add(new long[]{r[0], r[1], r[2], r[3], t + TAIL_MS});
        cur = rects.clone();
        prune(t);
        any = cur.length > 0 || !old.isEmpty();
    }

    /** 그린 창을 닫음 — 화면에서 사라지기까지 TAIL_MS 동안은 더 지움 */
    public static synchronized void clear() {
        long t = ms();
        for (int[] r : cur) old.add(new long[]{r[0], r[1], r[2], r[3], t + TAIL_MS});
        cur = new int[0][];
        prune(t);
        any = !old.isEmpty();
    }

    /** 바로 다 지움 (테스트용) */
    public static synchronized void reset() {
        cur = new int[0][];
        old.clear();
        any = false;
    }

    static boolean contains(int[][] rs, int[] r) {
        for (int[] q : rs) if (q[0] == r[0] && q[1] == r[1] && q[2] == r[2] && q[3] == r[3]) return true;
        return false;
    }

    static void prune(long t) {
        old.removeIf(o -> o[4] <= t);
    }

    /** 지금 지울 자리들 (그린 것 + 아직 남아 있을 수 있는 것) */
    public static synchronized int[][] active() {
        if (!any) return new int[0][];
        prune(ms());
        int n = cur.length + old.size();
        if (n == 0) {
            any = false;
            return new int[0][];
        }
        int[][] a = new int[n][];
        System.arraycopy(cur, 0, a, 0, cur.length);
        for (int i = 0; i < old.size(); i++) {
            long[] o = old.get(i);
            a[cur.length + i] = new int[]{(int) o[0], (int) o[1], (int) o[2], (int) o[3]};
        }
        return a;
    }

    public static boolean on() {
        return any;
    }

    /** 화면 (sx, sy) 부터 step 칸마다 w × h 칸을 dst 의 (dx, dy) 부터 넣은 뒤에 부름 → 그 안의 그린 자리 칸을 바깥 칸 색으로 */
    public static void fill(Img dst, int sx, int sy, int dx, int dy, int w, int h, int step, Src src) {
        if (!any) return;
        int[][] rs = active();
        if (rs.length > 0) fill(dst, sx, sy, dx, dy, w, h, step, src, rs);
    }

    public static void fill(Img dst, int sx, int sy, int dx, int dy, int w, int h, int step, Src src, int[][] rs) {
        if (w <= 0 || h <= 0) return;
        step = Math.max(1, step);
        int lx = sx + (w - 1) * step, ly = sy + (h - 1) * step;          // 마지막으로 찍은 칸 (포함)
        for (int[] r : rs) {
            if (r[2] <= sx || r[0] > lx || r[3] <= sy || r[1] > ly) continue;
            int i0 = Math.max(0, ceilDiv(r[0] - sx, step)), i1 = Math.min(w - 1, Math.floorDiv(r[2] - 1 - sx, step));
            int j0 = Math.max(0, ceilDiv(r[1] - sy, step)), j1 = Math.min(h - 1, Math.floorDiv(r[3] - 1 - sy, step));
            boolean horiz = r[2] - r[0] >= r[3] - r[1];
            for (int j = j0; j <= j1; j++) {
                int y = sy + j * step;
                for (int i = i0; i <= i1; i++) {
                    int c = outside(sx + i * step, y, r, horiz, rs, src);
                    if (c < 0) continue;
                    int k = (dy + j) * dst.w + dx + i;
                    if (k < 0 || k >= dst.r.length) continue;
                    dst.r[k] = (c >> 16) & 255;
                    dst.g[k] = (c >> 8) & 255;
                    dst.b[k] = c & 255;
                }
            }
        }
    }

    static int ceilDiv(int a, int b) {
        return -Math.floorDiv(-a, b);
    }

    /** 바깥 칸을 찾을 때 r 의 가장자리에서 더 나가 보는 최대 칸 수 (다른 그린 자리와 겹친 곳) */
    static final int REACH = 16;

    /** (x, y) 가 들어 있는 r 의 바깥에서 가장 가까운 칸의 색 · 그 칸도 그린 자리면 한 칸씩 더 나감 · 화면 밖이면 반대쪽 · 못 찾으면 -1 */
    static int outside(int x, int y, int[] r, boolean horiz, int[][] rs, Src src) {
        boolean nearNeg = horiz ? y - r[1] < r[3] - y : x - r[0] < r[2] - x;
        for (int side = 0; side < 2; side++) {
            boolean neg = side == 0 ? nearNeg : !nearNeg;        // neg: 위 · 왼쪽
            for (int d = 0; d < REACH; d++) {
                int qx = x, qy = y;
                if (horiz) qy = neg ? r[1] - 1 - d : r[3] + d;
                else qx = neg ? r[0] - 1 - d : r[2] + d;
                if (hit(qx, qy, rs) != null) continue;
                int c = src.rgb(qx, qy);
                if (c >= 0) return c;
                break;                                           // 화면 밖 → 반대쪽
            }
        }
        return -1;
    }

    static int[] hit(int x, int y, int[][] rs) {
        for (int[] q : rs) if (x >= q[0] && x < q[2] && y >= q[1] && y < q[3]) return q;
        return null;
    }
}
