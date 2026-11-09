package com.acrux.mobile.core;

/**
 * 로블록스 '클래식 조이스틱' (이동 방식: 조이스틱) 을 화면에서 찾기 — 왼쪽 아래의 둥근 판
 *  · 화면 왼쪽 아래 (가로 0 ~ 50%, 세로 35 ~ 100%) 에서 '둥근 테두리' 를 찾음: 원 안쪽 띠와 바깥쪽 띠의 밝기가
 *    한 바퀴 내내 같은 쪽으로 다르면 (안이 어둡거나 밝거나) 원으로 봄 — 반투명이라 배경이 달라도 테두리는 남음
 *  · 가운데 손잡이(작은 원) 가 있으면 점수를 더 줌
 *  · 작게 줄인 화면에서 넓게 찾고 → 원래 크기에서 근처만 다시 맞춤
 * 결과: {가운데 x, 가운데 y, 반지름} (px) · 점수 · 못 찾으면 null
 */
public final class Stick {
    private Stick() {}

    public static final class Found {
        public double cx, cy, r, score;

        @Override
        public String toString() {
            return String.format(java.util.Locale.US, "(%.0f, %.0f) r=%.0f score=%.1f", cx, cy, r, score);
        }
    }

    static final int ANG = 32, SECT = 8;
    static final double[] COS = new double[ANG], SIN = new double[ANG];

    static {
        for (int i = 0; i < ANG; i++) {
            COS[i] = Math.cos(2 * Math.PI * i / ANG);
            SIN[i] = Math.sin(2 * Math.PI * i / ANG);
        }
    }

    /** 밝기 (0~255) */
    static float[] lum(Img im, int step) {
        int w = im.w / step, h = im.h / step;
        float[] L = new float[w * h];
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                // step × step 칸 평균 (줄일 때 글자 · 무늬가 덜 튀게)
                int s = 0, n = 0;
                for (int dy = 0; dy < step; dy++) {
                    int row = (y * step + dy) * im.w;
                    for (int dx = 0; dx < step; dx++) {
                        int k = row + x * step + dx;
                        s += im.r[k] * 3 + im.g[k] * 6 + im.b[k];
                        n++;
                    }
                }
                L[y * w + x] = s / (10f * n);
            }
        }
        return L;
    }

    /** 반지름 r 인 원 테두리의 세기 — 원 안쪽 띠(0.82r)와 바깥쪽 띠(1.18r)의 밝기 차이가 한 바퀴 내내 같은 쪽이면 가장 약한 조각의 차이 · 아니면 0 · 화면 밖이면 -1 */
    static double ring(float[] L, int w, int h, double cx, double cy, double r) {
        return ring(L, w, h, cx, cy, r, 0.18);
    }

    static double ring(float[] L, int w, int h, double cx, double cy, double r, double band) {
        double ro = r * (1 + band), ri = r * (1 - band);
        if (cx - ro < 0 || cy - ro < 0 || cx + ro >= w - 1 || cy + ro >= h - 1) return -1;
        double[] sec = new double[SECT];
        for (int i = 0; i < ANG; i++) {
            double c = COS[i], s = SIN[i];
            double in = L[(int) (cy + s * ri) * w + (int) (cx + c * ri)];
            double out = L[(int) (cy + s * ro) * w + (int) (cx + c * ro)];
            sec[i * SECT / ANG] += in - out;
        }
        double mn = Double.MAX_VALUE, mx = -Double.MAX_VALUE;
        for (double v : sec) {
            v /= (double) ANG / SECT;
            mn = Math.min(mn, v);
            mx = Math.max(mx, v);
        }
        return mn > 0 ? mn : mx < 0 ? -mx : 0;
    }

    /** 안쪽이 바깥보다 밝은 원 테두리만 (손잡이는 하얀 반투명) — 8조각 중 세 번째로 약한 조각의 (안 - 밖) · 아니면 0 */
    static double brightRing(float[] L, int w, int h, double cx, double cy, double r, double band) {
        double ro = r * (1 + band), ri = r * (1 - band);
        if (cx - ro < 0 || cy - ro < 0 || cx + ro >= w - 1 || cy + ro >= h - 1) return -1;
        double[] sec = new double[SECT];
        for (int i = 0; i < ANG; i++) {
            double c = COS[i], s = SIN[i];
            sec[i * SECT / ANG] += L[(int) (cy + s * ri) * w + (int) (cx + c * ri)] - L[(int) (cy + s * ro) * w + (int) (cx + c * ro)];
        }
        // 손가락이 손잡이 일부를 가림 → 가장 약한 2조각은 빼고 그다음으로 약한 조각
        java.util.Arrays.sort(sec);
        return Math.max(0, sec[2] / ((double) ANG / SECT));
    }

    /** 조이스틱다운 정도: 바깥 판(r) 테두리 + 가운데 손잡이(약 r/2) 테두리가 둘 다 있어야 함 (네모난 무늬 · 손잡이만 · 다른 동그라미를 거름) */
    static double score(float[] L, int w, int h, double cx, double cy, double r) {
        double outer = ring(L, w, h, cx, cy, r);
        if (outer <= 0) return outer;
        double knob = 0;
        for (double k : KNOB) knob = Math.max(knob, ring(L, w, h, cx, cy, r * k));
        if (knob <= 0) return 0;
        return Math.min(outer, knob) * 2 + 0.25 * Math.max(outer, knob);
    }

    /** 손잡이 반지름 / 바깥 판 반지름 (로블록스 클래식: 0.5) */
    static final double[] KNOB = {0.45, 0.5, 0.55};

    /** 화면에서 조이스틱 찾기 · minScore: 이보다 약하면 null */
    public static Found find(Img im, double minScore) {
        if (im == null || im.w < 50 || im.h < 50) return null;
        int step = Math.max(1, Math.max(im.w, im.h) / 400);
        int w = im.w / step, h = im.h / step, mind = Math.min(w, h);
        float[] L = lum(im, step);
        int rMin = Math.max(4, (int) (mind * 0.035)), rMax = Math.max(rMin + 1, (int) (mind * 0.3));
        int x0 = 0, x1 = (int) (w * 0.5), y0 = (int) (h * 0.35), y1 = h;
        double best = 0;
        double bx = 0, by = 0, br = 0;
        for (int r = rMin; r <= rMax; r++) {
            for (int y = Math.max(y0, (int) (r * 1.2) + 1); y < y1 - r * 1.2 - 1; y++) {
                for (int x = Math.max(x0, (int) (r * 1.2) + 1); x < x1; x++) {
                    double s = score(L, w, h, x, y, r);
                    if (s > best) {
                        best = s;
                        bx = x;
                        by = y;
                        br = r;
                    }
                }
            }
        }
        if (best <= 0) return null;
        // 원래 크기에서 근처만 다시 맞춤
        Found f = new Found();
        f.cx = bx * step + step / 2.0;
        f.cy = by * step + step / 2.0;
        f.r = br * step;
        f.score = best;
        if (step > 1) {
            float[] F = lum(im, 1);
            double fb = score(F, im.w, im.h, f.cx, f.cy, f.r);
            double cx = f.cx, cy = f.cy, cr = f.r;
            for (int dr = -step; dr <= step; dr++) {
                for (int dy = -step; dy <= step; dy++) {
                    for (int dx = -step; dx <= step; dx++) {
                        double s = score(F, im.w, im.h, cx + dx, cy + dy, cr + dr);
                        if (s > fb) {
                            fb = s;
                            f.cx = cx + dx;
                            f.cy = cy + dy;
                            f.r = cr + dr;
                        }
                    }
                }
            }
            f.score = fb;
        }
        // 반지름 다듬기: 바깥 판 가장자리에 밝은 테두리가 있으면 그 안쪽 선에 걸리기 쉬움 → 좁은 띠로 재서 '센 가장자리 중 가장 바깥' 을 씀
        {
            float[] F = step > 1 ? lum(im, 1) : L;
            int fw = step > 1 ? im.w : w, fh = step > 1 ? im.h : h;
            double mx = 0;
            int lo = (int) (f.r * 0.8), hi = (int) (f.r * 1.45);
            double[] e = new double[hi - lo + 1];
            for (int rr = lo; rr <= hi; rr++) {
                e[rr - lo] = ring(F, fw, fh, f.cx, f.cy, rr, 0.07);
                mx = Math.max(mx, e[rr - lo]);
            }
            for (int rr = hi; rr >= lo && mx > 0; rr--) {
                if (e[rr - lo] >= mx * 0.6) {
                    f.r = rr;
                    break;
                }
            }
        }
        return f.score >= minScore ? f : null;
    }

    /**
     * 조이스틱 손잡이가 가운데에서 얼마나 밀렸나 — im: 조이스틱 둘레를 잘라 온 화면 · (cx, cy): 그 안의 판 가운데 · r: 판 반지름 (px)
     * 반지름 r/2 인 손잡이 테두리를 판 안(가운데에서 1.1r 까지) 에서 찾음 → {dx, dy} (반지름 단위 · 길이 1 = 끝까지) · 못 찾으면 null
     */
    public static double[] knob(Img im, double cx, double cy, double r) {
        if (im == null || im.w < 8 || im.h < 8 || r < 4) return null;
        float[] L = lum(im, 1);
        double kr = r * 0.5, lim = r * 1.1, st = Math.max(1, r / 20);
        double best = 0, bx = 0, by = 0;
        for (double oy = -lim; oy <= lim; oy += st) {
            for (double ox = -lim; ox <= lim; ox += st) {
                if (ox * ox + oy * oy > lim * lim) continue;
                double s = brightRing(L, im.w, im.h, cx + ox, cy + oy, kr, 0.2);
                if (s > best) {
                    best = s;
                    bx = ox;
                    by = oy;
                }
            }
        }
        if (best <= 0) return null;
        // 근처를 1px 단위로 다듬기
        double fx = bx, fy = by;
        for (double oy = by - st; oy <= by + st; oy += 1) {
            for (double ox = bx - st; ox <= bx + st; ox += 1) {
                double s = brightRing(L, im.w, im.h, cx + ox, cy + oy, kr, 0.2);
                if (s > best) {
                    best = s;
                    fx = ox;
                    fy = oy;
                }
            }
        }
        return new double[]{fx / r, fy / r, best};
    }

    /** 점프 버튼 안쪽 (반지름 0.6r 원) 의 평균 밝기 — 누르면 버튼이 밝아짐 · im: 버튼 둘레를 잘라 온 화면 · (cx, cy): 그 안의 버튼 가운데 */
    public static double glow(Img im, double cx, double cy, double r) {
        if (im == null || im.w == 0) return -1;
        double rr = r * 0.6, sum = 0;
        int n = 0;
        for (int y = (int) Math.max(0, cy - rr); y < Math.min(im.h, cy + rr); y++) {
            for (int x = (int) Math.max(0, cx - rr); x < Math.min(im.w, cx + rr); x++) {
                double dx = x - cx, dy = y - cy;
                if (dx * dx + dy * dy > rr * rr) continue;
                int k = y * im.w + x;
                sum += (im.r[k] * 3 + im.g[k] * 6 + im.b[k]) / 10.0;
                n++;
            }
        }
        return n == 0 ? -1 : sum / n;
    }

    /** 점프 버튼 눌림 감지: 안 눌렸을 때의 밝기를 천천히 따라가다가 갑자기 ON 만큼 밝아지면 눌림 · OFF 아래로 내려오면 뗌 */
    public static final class JumpWatch {
        static final double ON = 14, OFF = 7;
        double base = -1;
        boolean down;

        /** 이번 프레임 밝기 → 막 눌렸으면 true */
        public boolean step(double v) {
            if (v < 0) return false;
            if (base < 0) {
                base = v;
                return false;
            }
            double d = v - base;
            if (!down) {
                if (d >= ON) {
                    down = true;
                    return true;
                }
                base += (v - base) * 0.2;           // 걸어가면서 배경이 바뀌는 건 천천히 따라감
            } else if (d < OFF) {
                down = false;
                base += (v - base) * 0.2;
            }
            return false;
        }
    }

    /** WASD → 조이스틱 방향 (단위 벡터 · 화면 좌표: 오른쪽 +x, 아래 +y) · 아무것도 안 눌렸으면 null */
    public static double[] dir(boolean w, boolean a, boolean s, boolean d) {
        double x = (d ? 1 : 0) - (a ? 1 : 0), y = (s ? 1 : 0) - (w ? 1 : 0);
        double n = Math.hypot(x, y);
        return n == 0 ? null : new double[]{x / n, y / n};
    }
}
