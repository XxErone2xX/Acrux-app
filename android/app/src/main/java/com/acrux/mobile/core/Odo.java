package com.acrux.mobile.core;

/**
 * 내려다본 화면에서 캐릭터가 얼마나 움직였는지 재기 (영상 위상 상관 · phase correlation)
 *  · 위에서 내려다보면 캐릭터는 늘 화면 가운데 · 바닥만 반대로 밀림 → 프레임마다 바닥이 밀린 만큼 = 캐릭터가 간 만큼
 *  · 캐릭터 · UI 가 없는 화면 위쪽 두 칸 (왼쪽 · 오른쪽) 을 64×64 로 줄여서 앞 프레임과 비교 → 밀린 거리 (소수점까지)
 *  · 두 칸이 같은 쪽으로 밀렸다고 하면 믿음 · 하나만 확실하면 그것만 · 둘 다 흐리면 (단색 바닥 · 물) 모름
 * 위치 단위: 화면 px (같은 줌이면 기록 · 재생이 같은 단위)
 */
public final class Odo {
    public static final int N = 64;
    /** 비교하는 두 칸의 가운데 (화면 비율) · 칸 한 변 = 화면 높이 × SIDE */
    static final double[][] CENTERS = {{0.28, 0.33}, {0.72, 0.33}, {0.50, 0.25}};
    static final double SIDE = 0.30;
    /** 위상 상관 봉우리가 이보다 낮으면 믿지 않음 */
    static final double MIN_PEAK = 0.08;

    final Fisher.Screen scr;
    final double[][] prev = new double[CENTERS.length][];
    final double[] hann = new double[N];
    /** 캐릭터 위치 (화면 px · 시작 = 0, 0) */
    public double x, y;
    /** 이번 프레임에 잰 게 믿을 만했나 */
    public boolean ok;
    public double lastPeak;

    public Odo(Fisher.Screen scr) {
        this.scr = scr;
        for (int i = 0; i < N; i++) hann[i] = 0.5 - 0.5 * Math.cos(2 * Math.PI * i / (N - 1));
    }

    public void reset() {
        x = y = 0;
        for (int i = 0; i < prev.length; i++) prev[i] = null;
    }

    /** 한 프레임 → 위치 갱신 · 믿을 만하게 쟀으면 true */
    public boolean step() {
        int W = scr.width(), H = scr.height();
        int side = (int) (H * SIDE);
        double scale = (double) side / N;
        double[][] got = new double[CENTERS.length][];
        for (int i = 0; i < CENTERS.length; i++) {
            int cx = (int) (CENTERS[i][0] * W), cy = (int) (CENTERS[i][1] * H);
            Img im = scr.grab(cx - side / 2, cy - side / 2, side, side);
            if (im == null || im.w < side / 2 || im.h < side / 2) {
                prev[i] = null;
                continue;
            }
            double[] cur = small(im);
            if (prev[i] != null) got[i] = shift(prev[i], cur);
            prev[i] = cur;
        }
        // 칸마다 잰 값 중 믿을 만한 것들 → 서로 가장 많이 맞는 값 (붙어 있는 UI 가 있는 칸은 혼자 0 이라고 함 → 빠짐)
        double[] best = null;
        int bestVotes = 0;
        double bestPeak = 0;
        for (double[] g : got) {
            if (g == null || g[2] < MIN_PEAK) continue;
            int votes = 0;
            double sx = 0, sy = 0, pk = 0;
            for (double[] h : got) {
                if (h == null || h[2] < MIN_PEAK || Math.hypot(h[0] - g[0], h[1] - g[1]) > 2.5) continue;
                votes++;
                sx += h[0];
                sy += h[1];
                pk = Math.max(pk, h[2]);
            }
            if (votes > bestVotes || (votes == bestVotes && pk > bestPeak)) {
                bestVotes = votes;
                bestPeak = pk;
                best = new double[]{sx / votes, sy / votes};
            }
        }
        lastPeak = bestPeak;
        if (best == null) {
            ok = false;
            return false;
        }
        // 두 칸 이상이 '안 움직임' 이고 한 칸만 움직였다고 하면 → 안 움직임 (그 반대도 같은 식으로 다수결)
        double dx = best[0], dy = best[1];
        // 바닥이 (dx, dy) 밀림 → 캐릭터는 반대로
        x -= dx * scale;
        y -= dy * scale;
        ok = true;
        return true;
    }

    /** 칸 → N×N 밝기 (평균으로 줄임 · 평균 빼고 창 함수) */
    double[] small(Img im) {
        double[] o = new double[N * N];
        double sx = (double) im.w / N, sy = (double) im.h / N, mean = 0;
        for (int y = 0; y < N; y++) {
            int y0 = (int) (y * sy), y1 = Math.max(y0 + 1, (int) ((y + 1) * sy));
            for (int x = 0; x < N; x++) {
                int x0 = (int) (x * sx), x1 = Math.max(x0 + 1, (int) ((x + 1) * sx));
                double s = 0;
                int n = 0;
                for (int yy = y0; yy < y1 && yy < im.h; yy++) {
                    for (int xx = x0; xx < x1 && xx < im.w; xx++) {
                        int k = yy * im.w + xx;
                        s += im.r[k] * 0.3 + im.g[k] * 0.6 + im.b[k] * 0.1;
                        n++;
                    }
                }
                o[y * N + x] = n == 0 ? 0 : s / n;
                mean += o[y * N + x];
            }
        }
        mean /= N * N;
        for (int y = 0; y < N; y++) for (int x = 0; x < N; x++) o[y * N + x] = (o[y * N + x] - mean) * hann[x] * hann[y];
        return o;
    }

    /** a → b 로 내용이 밀린 거리 {dx, dy, 봉우리 높이} (N 칸 단위) */
    public static double[] shift(double[] a, double[] b) {
        double[] ar = a.clone(), ai = new double[N * N], br = b.clone(), bi = new double[N * N];
        fft2(ar, ai, false);
        fft2(br, bi, false);
        // 교차 파워 스펙트럼: B · conj(A) / |B · conj(A)|
        for (int k = 0; k < N * N; k++) {
            double re = br[k] * ar[k] + bi[k] * ai[k], im = bi[k] * ar[k] - br[k] * ai[k];
            double m = Math.hypot(re, im);
            if (m < 1e-9) {
                ar[k] = ai[k] = 0;
            } else {
                ar[k] = re / m;
                ai[k] = im / m;
            }
        }
        fft2(ar, ai, true);
        int best = 0;
        for (int k = 1; k < N * N; k++) if (ar[k] > ar[best]) best = k;
        // 화면에 붙어 있는 UI (글자 · 버튼) 는 '안 움직임' (0, 0) 봉우리를 만듦 → 0 근처를 뺀 가장 높은 봉우리가
        // 충분히 높으면 (0 봉우리의 35% 이상) 그쪽이 진짜 바닥 움직임
        if (near0(best)) {
            int alt = -1;
            for (int k = 0; k < N * N; k++) if (!near0(k) && (alt < 0 || ar[k] > ar[alt])) alt = k;
            if (alt >= 0 && ar[alt] >= Math.max(MIN_PEAK, ar[best] * 0.35)) best = alt;
        }
        int py = best / N, px = best % N;
        double peak = ar[best];
        // 소수점: 봉우리 양옆으로 포물선
        double fx = px + sub(ar[py * N + (px + N - 1) % N], peak, ar[py * N + (px + 1) % N]);
        double fy = py + sub(ar[((py + N - 1) % N) * N + px], peak, ar[((py + 1) % N) * N + px]);
        if (fx > N / 2.0) fx -= N;
        if (fy > N / 2.0) fy -= N;
        return new double[]{fx, fy, peak};
    }

    static boolean near0(int k) {
        int y = k / N, x = k % N;
        int dx = Math.min(x, N - x), dy = Math.min(y, N - y);
        return dx <= 1 && dy <= 1;
    }

    static double sub(double l, double c, double r) {
        double d = l - 2 * c + r;
        return Math.abs(d) < 1e-12 ? 0 : Math.max(-0.5, Math.min(0.5, 0.5 * (l - r) / d));
    }

    // ---- FFT (2의 거듭제곱 · 제자리)
    static void fft2(double[] re, double[] im, boolean inv) {
        double[] r = new double[N], i = new double[N];
        for (int y = 0; y < N; y++) {
            System.arraycopy(re, y * N, r, 0, N);
            System.arraycopy(im, y * N, i, 0, N);
            fft(r, i, inv);
            System.arraycopy(r, 0, re, y * N, N);
            System.arraycopy(i, 0, im, y * N, N);
        }
        for (int x = 0; x < N; x++) {
            for (int y = 0; y < N; y++) {
                r[y] = re[y * N + x];
                i[y] = im[y * N + x];
            }
            fft(r, i, inv);
            for (int y = 0; y < N; y++) {
                re[y * N + x] = r[y];
                im[y * N + x] = i[y];
            }
        }
    }

    static void fft(double[] re, double[] im, boolean inv) {
        int n = re.length;
        for (int i = 1, j = 0; i < n; i++) {
            int bit = n >> 1;
            for (; (j & bit) != 0; bit >>= 1) j ^= bit;
            j ^= bit;
            if (i < j) {
                double t = re[i];
                re[i] = re[j];
                re[j] = t;
                t = im[i];
                im[i] = im[j];
                im[j] = t;
            }
        }
        for (int len = 2; len <= n; len <<= 1) {
            double ang = 2 * Math.PI / len * (inv ? 1 : -1), wr = Math.cos(ang), wi = Math.sin(ang);
            for (int i = 0; i < n; i += len) {
                double cr = 1, ci = 0;
                for (int k = 0; k < len / 2; k++) {
                    int a = i + k, b = a + len / 2;
                    double xr = re[b] * cr - im[b] * ci, xi = re[b] * ci + im[b] * cr;
                    re[b] = re[a] - xr;
                    im[b] = im[a] - xi;
                    re[a] += xr;
                    im[a] += xi;
                    double t = cr * wr - ci * wi;
                    ci = cr * wi + ci * wr;
                    cr = t;
                }
            }
        }
        if (inv) {
            for (int i = 0; i < n; i++) {
                re[i] /= n;
                im[i] /= n;
            }
        }
    }
}
