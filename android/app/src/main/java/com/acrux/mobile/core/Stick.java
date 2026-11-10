package com.acrux.mobile.core;

import java.util.Arrays;

/**
 * 로블록스 클래식 조이스틱 · 점프 버튼을 화면에서 읽기 (실제 화면 녹화로 맞춤)
 *  · 조이스틱 바깥 판: 얇은 회색 테두리 선 (안은 비어 있음) — 반지름 r
 *  · 조이스틱 손잡이: 반투명 회색 원 (반지름 약 r/2) — 밀면 그 방향으로 최대 r 만큼 움직임
 *  · 점프 버튼: 평소엔 어두운 반투명 원 (안이 바깥보다 어두움) · 누르면 밝은 반투명 원 (안이 바깥보다 밝음)
 * 회색은 '색이 옅음' (채도: 가장 밝은 색 - 가장 어두운 색) 으로 봄 — 풀 · 하늘 · 모래 같은 게임 배경은 색이 짙어서 회색이 잘 드러남
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

    static final int ANG = 48, SECT = 8;
    static final double[] COS = new double[ANG], SIN = new double[ANG];

    static {
        for (int i = 0; i < ANG; i++) {
            COS[i] = Math.cos(2 * Math.PI * i / ANG);
            SIN[i] = Math.sin(2 * Math.PI * i / ANG);
        }
    }

    /**
     * '조이스틱 색이 아닌 정도' 지도 = 채도 + 0.5 × (255 - 밝기)  (작을수록 조이스틱의 밝은 회색에 가까움)
     *  · 채도만 보면 어두운 배경 (나무 그늘 · 밤 · 검은 글자판) 도 채도가 낮아서 회색 선과 구별이 안 됐음 → 어두움도 더함
     *  · 풀 ≈ 165 · 하늘 ≈ 115 · 어두운 배경 ≈ 135 · 조이스틱 테두리 선 ≈ 30 · 손잡이 안 ≈ 90 ~ 115 (흰 구름 위에서만 구별이 어려움)
     * step 칸씩 줄일 때는 가장 작은 값 (얇은 선이 안 사라지게)
     */
    static float[] chroma(Img im, int step) {
        return chroma(im, step, 0.5);
    }

    /** dark: 어두움을 얼마나 더할지 (0 = 채도만) */
    static float[] chroma(Img im, int step, double dark) {
        int w = im.w / step, h = im.h / step;
        float[] C = new float[w * h];
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                int mn = 999;
                for (int dy = 0; dy < step; dy++) {
                    int row = (y * step + dy) * im.w;
                    for (int dx = 0; dx < step; dx++) {
                        int k = row + x * step + dx;
                        int r = im.r[k], g = im.g[k], b = im.b[k];
                        int c = Math.max(r, Math.max(g, b)) - Math.min(r, Math.min(g, b));
                        if (dark > 0) c += (int) ((255 - (r * 3 + g * 6 + b) / 10) * dark);
                        if (c < mn) mn = c;
                    }
                }
                C[y * w + x] = mn;
            }
        }
        return C;
    }

    /** 밝기 */
    static float[] lum(Img im) {
        float[] L = new float[im.w * im.h];
        for (int k = 0; k < L.length; k++) L[k] = (im.r[k] * 3 + im.g[k] * 6 + im.b[k]) / 10f;
        return L;
    }

    static float at(float[] M, int w, double x, double y) {
        return M[(int) y * w + (int) x];
    }

    /** 8조각 점수 중 skip 개를 뺀 가장 약한 값 (글자 · 손가락이 일부를 가려도) */
    static double weakest(double[] sec, int skip) {
        double[] s = sec.clone();
        Arrays.sort(s);
        return s[skip];
    }

    /** 반지름 r 에 얇은 회색 선이 한 바퀴 있나: (선 안팎의 채도 중 낮은 쪽) - (선 위의 채도) · 화면 밖이면 -1 */
    static double thinRing(float[] C, int w, int h, double cx, double cy, double r) {
        double ro = r * 1.15 + 1.5, ri = r * 0.85 - 1.5;
        if (ri < 2 || cx - ro < 0 || cy - ro < 0 || cx + ro >= w - 1 || cy + ro >= h - 1) return -1;
        double[] sec = new double[SECT];
        for (int i = 0; i < ANG; i++) {
            double c = COS[i], s = SIN[i];
            double line = Math.min(at(C, w, cx + c * r, cy + s * r), Math.min(at(C, w, cx + c * (r - 1), cy + s * (r - 1)), at(C, w, cx + c * (r + 1), cy + s * (r + 1))));
            double side = Math.min(at(C, w, cx + c * ri, cy + s * ri), at(C, w, cx + c * ro, cy + s * ro));
            sec[i * SECT / ANG] += side - line;
        }
        for (int i = 0; i < SECT; i++) sec[i] /= (double) ANG / SECT;
        return Math.max(0, weakest(sec, 1));
    }

    /** (cx, cy) 를 가운데로 반지름 kr 인 '색이 옅은 원' (손잡이) 이 있나: (바깥 1.3kr 의 채도) - (안 0.55kr 의 채도) */
    static double grayDisk(float[] C, int w, int h, double cx, double cy, double kr) {
        double ro = kr * 1.3, ri = kr * 0.55;
        if (cx - ro < 0 || cy - ro < 0 || cx + ro >= w - 1 || cy + ro >= h - 1) return -1;
        double[] sec = new double[SECT];
        for (int i = 0; i < ANG; i++) {
            double c = COS[i], s = SIN[i];
            sec[i * SECT / ANG] += at(C, w, cx + c * ro, cy + s * ro) - at(C, w, cx + c * ri, cy + s * ri);
        }
        for (int i = 0; i < SECT; i++) sec[i] /= (double) ANG / SECT;
        return Math.max(0, weakest(sec, 2));
    }

    /** 화면에서 조이스틱 찾기 (왼쪽 아래 · 얇은 회색 테두리 원) · minScore 보다 약하면 null */
    public static Found find(Img im, double minScore) {
        if (im == null || im.w < 50 || im.h < 50) return null;
        int step = Math.max(1, Math.max(im.w, im.h) / 480);
        int w = im.w / step, h = im.h / step, mind = Math.min(w, h);
        float[] C = chroma(im, step);
        int rMin = Math.max(4, (int) (mind * 0.025)), rMax = Math.max(rMin + 1, (int) (mind * 0.25));
        double best = 0, bx = 0, by = 0, br = 0;
        for (int r = rMin; r <= rMax; r++) {
            for (int y = (int) (h * 0.35); y < h - r * 1.15 - 2; y++) {
                for (int x = (int) (r * 1.15) + 2; x < w * 0.5; x++) {
                    double s = thinRing(C, w, h, x, y, r);
                    if (s <= best * 0.7) continue;
                    // 손잡이가 가운데 있으면 (안 누를 때) 조금 더 줌
                    double k = grayDisk(C, w, h, x, y, r * 0.5);
                    s += 0.1 * Math.max(0, k);
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
        Found f = new Found();
        f.cx = bx * step + step / 2.0;
        f.cy = by * step + step / 2.0;
        f.r = br * step;
        f.score = best;
        if (step > 1) {
            float[] F = chroma(im, 1);
            double fb = thinRing(F, im.w, im.h, f.cx, f.cy, f.r), cx = f.cx, cy = f.cy, cr = f.r;
            for (int dr = -step; dr <= step; dr++) {
                for (int dy = -step; dy <= step; dy++) {
                    for (int dx = -step; dx <= step; dx++) {
                        double s = thinRing(F, im.w, im.h, cx + dx, cy + dy, cr + dr);
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
        return f.score >= minScore ? f : null;
    }

    /**
     * 조이스틱 손잡이가 가운데에서 얼마나 밀렸나 — im: 조이스틱 둘레를 잘라 온 화면 · (cx, cy): 그 안의 판 가운데 · r: 판 반지름 (px)
     * → {dx, dy, 점수} (반지름 단위 · 길이 1 = 끝까지) · 못 찾으면 null
     */
    public static double[] knob(Img im, double cx, double cy, double r) {
        return knob(im, cx, cy, r, null);
    }

    /** prev: 바로 전 프레임의 손잡이 {dx, dy} (없으면 null) — 점수가 비슷한 후보끼리는 방금 자리와 가까운 쪽 (손잡이는 한 프레임에 순간이동하지 않음) */
    public static double[] knob(Img im, double cx, double cy, double r, double[] prev) {
        if (im == null || im.w < 8 || im.h < 8 || r < 4) return null;
        // 1. 손잡이 그림(반투명 회색) 을 그대로 맞춰 봄 — 배경 · 겹친 UI 와 상관없이 가장 정확
        double[] m = knobModelOn(im, cx, cy, r);
        if (m != null && m[2] >= 20) return m;
        // 2. 잘 안 맞으면 (손가락 · 효과로 많이 가려짐) 예전 방법 세 가지를 맞춰 봄
        // 세 가지로 따로 찾고 → 후보마다 두 가지 점수 (회색 원 · 테두리 원) 를 합쳐서 가장 그럴듯한 자리
        //  · 채도만: 하늘 · 흰 구름 위에서 정확  · 채도 + 어두움: 그늘 · 어두운 배경  · 테두리: 글자판 · 복잡한 배경
        float[] C0 = chroma(im, 1, 0), C1 = chroma(im, 1, 0.5), L = lum(im);
        double[][] cand = {knobOn(C0, im.w, im.h, cx, cy, r), knobOn(C1, im.w, im.h, cx, cy, r), knobEdgeOn(L, im.w, im.h, cx, cy, r)};
        double[] best = null;
        double bs = -1;
        for (double[] k : cand) {
            if (k == null) continue;
            double x = cx + k[0] * r, y = cy + k[1] * r, kr = r * 0.5;
            double e = 0;
            for (double f : new double[]{0.46, 0.5, 0.54}) e = Math.max(e, edgeCircle(L, im.w, im.h, x, y, r * f));
            double g = Math.max(grayDisk(C0, im.w, im.h, x, y, kr), grayDisk(C1, im.w, im.h, x, y, kr) * 0.8);
            double sc = Math.max(0, e) / 50 + Math.max(0, g) / 45;
            if (prev != null) sc -= 0.35 * Math.min(2, Math.hypot(k[0] - prev[0], k[1] - prev[1]));
            if (sc > bs) {
                bs = sc;
                best = new double[]{k[0], k[1], sc};
            }
        }
        return best;
    }

    static double[] knobOn(float[] C, int imw, int imh, double cx, double cy, double r) {
        double kr = r * 0.5, lim = r * 1.1, st = Math.max(1, r / 20);
        double best = 0, bx = 0, by = 0;
        for (double oy = -lim; oy <= lim; oy += st) {
            for (double ox = -lim; ox <= lim; ox += st) {
                if (ox * ox + oy * oy > lim * lim) continue;
                double s = grayDisk(C, imw, imh, cx + ox, cy + oy, kr);
                if (s > best) {
                    best = s;
                    bx = ox;
                    by = oy;
                }
            }
        }
        if (best < 8) return null;
        double fx = bx, fy = by;
        for (double oy = by - st; oy <= by + st; oy += 1) {
            for (double ox = bx - st; ox <= bx + st; ox += 1) {
                double s = grayDisk(C, imw, imh, cx + ox, cy + oy, kr);
                if (s > best) {
                    best = s;
                    fx = ox;
                    fy = oy;
                }
            }
        }
        // 소수점 단위로 다듬기: 찾은 원 안 픽셀들의 '회색인 정도' 무게 중심 (격자 · 배경 무늬 때문에 1~2px 씩 튀는 걸 줄임)
        double[] cg = centroid(C, imw, imh, cx + fx, cy + fy, kr);
        if (cg != null && Math.hypot(cg[0] - (cx + fx), cg[1] - (cy + fy)) < kr * 0.3) {
            fx = cg[0] - cx;
            fy = cg[1] - cy;
        }
        double m = Math.hypot(fx, fy) / r;
        if (m > 1) {                                // 손잡이는 판 반지름까지만 움직임
            fx /= m;
            fy /= m;
        }
        return new double[]{fx / r, fy / r, best};
    }

    /**
     * 기록용: 조이스틱 판 + 손잡이 둘 다 찾기 — 로블록스 클래식 조이스틱은 누른 자리로 판이 따라 옮겨짐 (손을 떼면 원래 자리로)
     * → 저장된 가운데 (cx, cy) 근처 (반지름의 0.7배 안) 에서 지금 판 가운데를 먼저 찾고, 손잡이는 그 가운데 기준으로 잼
     * → {dx, dy, 점수} (반지름 단위) · 못 찾으면 null
     */
    public static double[] track(Img im, double cx, double cy, double r) {
        if (im == null || im.w < 8 || im.h < 8 || r < 4) return null;
        float[] C = chroma(im, 1);
        double lim = r * 0.7, best = 0, bx = cx, by = cy;
        int st = Math.max(1, (int) (r / 25));
        for (double y = cy - lim; y <= cy + lim; y += st) {
            for (double x = cx - lim; x <= cx + lim; x += st) {
                double s = thinRing(C, im.w, im.h, x, y, r);
                if (s > best) {
                    best = s;
                    bx = x;
                    by = y;
                }
            }
        }
        if (best < 10) return null;                 // 판이 안 보임 (메뉴 · 창에 가려짐)
        double fx = bx, fy = by;
        for (double y = by - st; y <= by + st; y += 1) {
            for (double x = bx - st; x <= bx + st; x += 1) {
                double s = thinRing(C, im.w, im.h, x, y, r);
                if (s > best) {
                    best = s;
                    fx = x;
                    fy = y;
                }
            }
        }
        return knob(im, fx, fy, r);
    }

    /**
     * 기록용 조이스틱 따라가기 — 로블록스 클래식 조이스틱은 누를 때마다 판이 누른 자리로 옮겨짐 (떼면 원래 자리)
     *  1. 방금 판 자리 근처 → 2. 원래 자리 근처 → 3. 그래도 없으면 넓게 (화면 왼쪽 아래 전체 · 줄여서) 찾음
     *  판을 찾으면 손잡이는 그 판 가운데 기준으로 잼
     * 화면 좌표: im 은 (ox, oy) 부터 잘라 온 넓은 영역
     */
    public static final class Tracker {
        public final double homeX, homeY, r;
        double curX = Double.NaN, curY;
        double[] prevKnob;
        /** 마지막으로 찾은 판 가운데 (화면 좌표) · 디버그용 */
        public double ringX, ringY, ringScore;

        public Tracker(double homeX, double homeY, double r) {
            this.homeX = homeX;
            this.homeY = homeY;
            this.r = r;
        }

        /** → {dx, dy, 점수} (반지름 단위) · 판을 못 찾으면 null */
        public double[] step(Img im, int ox, int oy) {
            if (im == null || im.w < 8 || im.h < 8) return null;
            double[] got = null;
            if (!Double.isNaN(curX)) got = nearSub(im, curX - ox, curY - oy, 0.25);
            if (got == null) got = nearSub(im, homeX - ox, homeY - oy, 0.65);   // 누른 자리로 옮겨진 판은 대개 원래 자리 근처
            if (got == null) got = wide(im);
            if (got == null) {
                curX = Double.NaN;
                return null;
            }
            curX = got[0] + ox;
            curY = got[1] + oy;
            ringX = curX;
            ringY = curY;
            ringScore = got[2];
            // 손잡이는 판 둘레만 잘라서 봄 (넓은 영역 전체를 세 번 훑으면 느림)
            int half = (int) (r * 2.3), sx = Math.max(0, (int) got[0] - half), sy = Math.max(0, (int) got[1] - half);
            Img sub = sub(im, sx, sy, Math.min(im.w - sx, half * 2), Math.min(im.h - sy, half * 2));
            double[] k = knob(sub, got[0] - sx, got[1] - sy, r, prevKnob);
            prevKnob = k == null ? null : new double[]{k[0], k[1]};
            return k;
        }

        static final double OK = 25, FIT_OK = 45;

        /** 테두리 선 그림 맞추기로 (cx, cy) 근처에서 판 찾기 */
        double[] nearFit(Img im, double cx, double cy, double lim) {
            double best = 0, bx = 0, by = 0;
            for (double y = cy - lim; y <= cy + lim; y += 1) {
                for (double x = cx - lim; x <= cx + lim; x += 1) {
                    double s = ringFit(im, x, y, r);
                    if (s > best) {
                        best = s;
                        bx = x;
                        by = y;
                    }
                }
            }
            return best >= FIT_OK ? new double[]{bx, by, best} : null;
        }

        /** (cx, cy) 근처만 잘라서 판 찾기 (빠름) → 영역 좌표 */
        double[] nearSub(Img im, double cx, double cy, double limR) {
            int half = (int) (r * (1.2 + limR)) + 6, sx = Math.max(0, (int) cx - half), sy = Math.max(0, (int) cy - half);
            int w = Math.min(im.w - sx, half * 2), h = Math.min(im.h - sy, half * 2);
            if (w < 8 || h < 8) return null;
            Img s = sub(im, sx, sy, w, h);
            // 그림 맞추기 먼저 (겹친 UI 에 강함) → 안 되면 색 지도로 (작게 줄어 선이 흐릴 때)
            double[] g = nearFit(s, cx - sx, cy - sy, r * limR);
            if (g == null) g = near(chroma(s, 1), s.w, s.h, cx - sx, cy - sy, r * limR);
            if (g == null) return null;
            g[0] += sx;
            g[1] += sy;
            return g;
        }

        double[] near(float[] C, int w, int h, double cx, double cy, double lim) {
            double best = 0, bx = 0, by = 0;
            for (double y = cy - lim; y <= cy + lim; y += 1) {
                for (double x = cx - lim; x <= cx + lim; x += 1) {
                    double s = thinRing(C, w, h, x, y, r);
                    if (s > best) {
                        best = s;
                        bx = x;
                        by = y;
                    }
                }
            }
            return best >= OK ? new double[]{bx, by, best} : null;
        }

        /** 넓게: 3칸씩 줄인 화면에서 반지름 r/3 원을 찾고 → 원래 크기에서 근처만 다시 */
        double[] wide(Img im) {
            int st = 3, w = im.w / st, h = im.h / st;
            float[] D = chroma(im, st);
            double rr = r / st, best = 0, bx = 0, by = 0;
            for (int y = 0; y < h; y++) {
                for (int x = 0; x < w; x++) {
                    double s = thinRing(D, w, h, x, y, rr);
                    if (s > best) {
                        best = s;
                        bx = x;
                        by = y;
                    }
                }
            }
            if (best < OK) return null;
            double[] g = nearFit(im, bx * st + st / 2.0, by * st + st / 2.0, st + 1);
            return g != null ? g : near(chroma(im, 1), im.w, im.h, bx * st + st / 2.0, by * st + st / 2.0, st + 1);
        }
    }

    static Img sub(Img im, int x0, int y0, int w, int h) {
        Img o = new Img(Math.max(0, w), Math.max(0, h));
        for (int y = 0; y < o.h; y++) {
            int a = (y0 + y) * im.w + x0, b = y * o.w;
            System.arraycopy(im.r, a, o.r, b, o.w);
            System.arraycopy(im.g, a, o.g, b, o.w);
            System.arraycopy(im.b, a, o.b, b, o.w);
        }
        return o;
    }

    /** (x, y) 반지름 kr 원 안의 회색 무게 중심 — 무게 = (원 바깥 1.3kr 의 평균 채도) - (그 픽셀 채도) */
    static double[] centroid(float[] C, int w, int h, double x, double y, double kr) {
        double ref = 0;
        int n = 0;
        for (int i = 0; i < ANG; i++) {
            double px = x + COS[i] * kr * 1.3, py = y + SIN[i] * kr * 1.3;
            if (px < 0 || py < 0 || px >= w || py >= h) continue;
            ref += at(C, w, px, py);
            n++;
        }
        if (n < ANG / 2) return null;
        ref /= n;
        if (ref < 40) return null;                  // 배경도 색이 옅으면 (흰 구름 · 회색 바닥) 무게 중심이 끌려감 → 안 씀
        double sw = 0, sx = 0, sy = 0, rr = kr * 1.05;
        for (int py = (int) Math.max(0, y - rr); py <= Math.min(h - 1, y + rr); py++) {
            for (int px = (int) Math.max(0, x - rr); px <= Math.min(w - 1, x + rr); px++) {
                double dx = px - x, dy = py - y;
                if (dx * dx + dy * dy > rr * rr) continue;
                double wt = ref - C[py * w + px];
                if (wt <= 0) continue;
                sw += wt;
                sx += wt * px;
                sy += wt * py;
            }
        }
        return sw <= 0 ? null : new double[]{sx / sw, sy / sw};
    }

    /**
     * 손잡이 테두리 찾기 (배경색과 상관없이): 반지름 kr 원 둘레에서 안쪽(kr-2) · 바깥쪽(kr+2) 밝기 차이가 한 바퀴 내내 큰가
     * 손잡이는 테두리 선이 또렷한 원 → 글자 · 무늬 같은 배경 모서리는 한 바퀴 내내 원 모양으로 이어지지 않음
     * 8조각 중 가장 약한 2조각은 뺌 (손가락 · 판 테두리와 겹침)
     */
    static double edgeCircle(float[] L, int w, int h, double cx, double cy, double kr) {
        double ro = kr + 2.5, ri = kr - 2.5;
        if (ri < 2 || cx - ro < 0 || cy - ro < 0 || cx + ro >= w - 1 || cy + ro >= h - 1) return -1;
        double[] sec = new double[SECT];
        for (int i = 0; i < ANG; i++) {
            double c = COS[i], s = SIN[i];
            sec[i * SECT / ANG] += Math.abs(at(L, w, cx + c * ro, cy + s * ro) - at(L, w, cx + c * ri, cy + s * ri));
        }
        for (int i = 0; i < SECT; i++) sec[i] /= (double) ANG / SECT;
        return weakest(sec, 2);
    }

    /** 테두리로 손잡이 찾기 → {dx, dy, 점수} (반지름 단위) · 못 찾으면 null */
    public static double[] knobEdge(Img im, double cx, double cy, double r) {
        return knobEdgeOn(lum(im), im.w, im.h, cx, cy, r);
    }

    static double[] knobEdgeOn(float[] L, int imw, int imh, double cx, double cy, double r) {
        double best = 0, bx = 0, by = 0, lim = r * 1.1, st = Math.max(1, r / 20);
        double[] krs = {r * 0.46, r * 0.5, r * 0.54};
        double bk = r * 0.5;
        for (double oy = -lim; oy <= lim; oy += st) {
            for (double ox = -lim; ox <= lim; ox += st) {
                if (ox * ox + oy * oy > lim * lim) continue;
                for (double kr : krs) {
                    double s = edgeCircle(L, imw, imh, cx + ox, cy + oy, kr);
                    if (s > best) {
                        best = s;
                        bx = ox;
                        by = oy;
                        bk = kr;
                    }
                }
            }
        }
        if (best <= 0) return null;
        double fx = bx, fy = by;
        for (double oy = by - st; oy <= by + st; oy += 1) {
            for (double ox = bx - st; ox <= bx + st; ox += 1) {
                double s = edgeCircle(L, imw, imh, cx + ox, cy + oy, bk);
                if (s > best) {
                    best = s;
                    fx = ox;
                    fy = oy;
                }
            }
        }
        double m = Math.hypot(fx, fy) / r;
        if (m > 1) {
            fx /= m;
            fy /= m;
        }
        return new double[]{fx / r, fy / r, best};
    }

    /**
     * 판 테두리 선 맞추기 (로블록스 그림: 반지름 r 에 불투명한 밝은 회색 선 ≈ 155)
     * 선 위 화면이 '배경 (선 안팎 평균)' 보다 '회색 155' 에 얼마나 더 가까운가 · 8조각 중 약한 2조각은 뺌 (겹친 UI · 글자)
     */
    static final double RING_GRAY = 155;

    static double ringFit(Img im, double cx, double cy, double r) {
        double ro = r * 1.15 + 1.5, ri = r * 0.85 - 1.5;
        if (ri < 2 || cx - ro < 0 || cy - ro < 0 || cx + ro >= im.w - 1 || cy + ro >= im.h - 1) return -1;
        double[] sec = new double[SECT];
        for (int i = 0; i < ANG; i++) {
            double c = COS[i], s = SIN[i];
            int ka = (int) (cy + s * ri) * im.w + (int) (cx + c * ri), kb = (int) (cy + s * ro) * im.w + (int) (cx + c * ro);
            double br = (im.r[ka] + im.r[kb]) / 2.0, bg = (im.g[ka] + im.g[kb]) / 2.0, bb = (im.b[ka] + im.b[kb]) / 2.0;
            double gain = -1e9;
            for (double d = -1; d <= 1; d += 1) {          // 선 굵기 · 반지름 오차 1px
                int k = (int) (cy + s * (r + d)) * im.w + (int) (cx + c * (r + d));
                double eNone = Math.abs(im.r[k] - br) + Math.abs(im.g[k] - bg) + Math.abs(im.b[k] - bb);
                double eRing = Math.abs(im.r[k] - RING_GRAY) + Math.abs(im.g[k] - RING_GRAY) + Math.abs(im.b[k] - RING_GRAY);
                gain = Math.max(gain, eNone - eRing);
            }
            sec[i * SECT / ANG] += gain;
        }
        for (int i = 0; i < SECT; i++) sec[i] /= (double) ANG / SECT;
        return weakest(sec, 2);
    }

    // ---- 손잡이 모양 맞추기 (로블록스 TouchControlsSheet 의 손잡이 그림 그대로)
    // 손잡이 그림 반지름 = 판 테두리 선 반지름의 0.53배 · 반지름 비율별 (회색값, 불투명도)
    static final double KR = 0.53;
    static final double[] KF = {0.15, 0.4, 0.62, 0.88};
    static final double[] KT = {117, 104, 89, 90};
    static final double[] KA = {0.38, 0.46, 0.57, 0.77};
    static final double KOUT = 1.22;

    /**
     * (cx, cy) 에 손잡이가 있다고 봤을 때 화면이 얼마나 잘 설명되나 — 손잡이는 반투명 회색이라 배경 b 위에 그리면 a·t + (1-a)·b
     * 바깥(1.22배) 화면을 배경 b 로 보고 → 안쪽 4군데를 '손잡이 있음' 으로 예측한 오차 vs '없음 (배경 그대로)' 오차
     * 배경이 밝든 어둡든 (풀 · 하늘 · 검은 UI 상자) 같은 식으로 맞음 · 8조각 중 약한 2조각은 뺌
     */
    static double knobFit(Img im, double cx, double cy, double kr) {
        double ro = kr * KOUT;
        if (cx - ro < 0 || cy - ro < 0 || cx + ro >= im.w - 1 || cy + ro >= im.h - 1) return -1;
        double[] sec = new double[SECT];
        for (int i = 0; i < ANG; i++) {
            double c = COS[i], s = SIN[i];
            int ko = (int) (cy + s * ro) * im.w + (int) (cx + c * ro);
            int br = im.r[ko], bg = im.g[ko], bb = im.b[ko];
            double gain = 0;
            for (int j = 0; j < KF.length; j++) {
                int k = (int) (cy + s * kr * KF[j]) * im.w + (int) (cx + c * kr * KF[j]);
                double a = KA[j], t = KT[j] * a;
                double pr = t + (1 - a) * br, pg = t + (1 - a) * bg, pb = t + (1 - a) * bb;
                double eKnob = Math.abs(im.r[k] - pr) + Math.abs(im.g[k] - pg) + Math.abs(im.b[k] - pb);
                double eNone = Math.abs(im.r[k] - br) + Math.abs(im.g[k] - bg) + Math.abs(im.b[k] - bb);
                gain += eNone - eKnob;
            }
            sec[i * SECT / ANG] += gain / KF.length;
        }
        for (int i = 0; i < SECT; i++) sec[i] /= (double) ANG / SECT;
        return weakest(sec, 2);
    }

    static double[] knobModelOn(Img im, double cx, double cy, double r) {
        double kr = r * KR, lim = r * 1.1, st = Math.max(1, r / 20);
        double best = 0, bx = 0, by = 0;
        for (double oy = -lim; oy <= lim; oy += st) {
            for (double ox = -lim; ox <= lim; ox += st) {
                if (ox * ox + oy * oy > lim * lim) continue;
                double sc = knobFit(im, cx + ox, cy + oy, kr);
                if (sc > best) {
                    best = sc;
                    bx = ox;
                    by = oy;
                }
            }
        }
        if (best <= 0) return null;
        double fx = bx, fy = by;
        for (double oy = by - st; oy <= by + st; oy += 1) {
            for (double ox = bx - st; ox <= bx + st; ox += 1) {
                double sc = knobFit(im, cx + ox, cy + oy, kr);
                if (sc > best) {
                    best = sc;
                    fx = ox;
                    fy = oy;
                }
            }
        }
        double m = Math.hypot(fx, fy) / r;
        if (m > 1) {
            fx /= m;
            fy /= m;
        }
        return new double[]{fx / r, fy / r, best};
    }

    public static double[] knobModel(Img im, double cx, double cy, double r) {
        return knobModelOn(im, cx, cy, r);
    }

    /** 점프 버튼 밝기 차이: (버튼 안 0.45r ~ 0.8r 의 평균 밝기) - (버튼 밖 1.15r ~ 1.35r 의 평균 밝기) · 평소엔 음수 (어두운 원) · 누르면 양수 (밝은 원) */
    public static double jumpLevel(Img im, double cx, double cy, double r) {
        if (im == null || im.w == 0) return Double.NaN;
        float[] L = lum(im);
        double in = 0, out = 0;
        int ni = 0, no = 0;
        for (int i = 0; i < ANG; i++) {
            for (double f : new double[]{0.45, 0.6, 0.8}) {
                double x = cx + COS[i] * r * f, y = cy + SIN[i] * r * f;
                if (x < 0 || y < 0 || x >= im.w || y >= im.h) continue;
                in += at(L, im.w, x, y);
                ni++;
            }
            for (double f : new double[]{1.15, 1.25, 1.35}) {
                double x = cx + COS[i] * r * f, y = cy + SIN[i] * r * f;
                if (x < 0 || y < 0 || x >= im.w || y >= im.h) continue;
                out += at(L, im.w, x, y);
                no++;
            }
        }
        return ni == 0 || no == 0 ? Double.NaN : in / ni - out / no;
    }

    /**
     * 점프 버튼 자리 · 크기 맞추기 (안 누를 때): 지정한 자리 근처 (반지름의 0.5배 안) · 크기 0.8 ~ 1.3배 중
     * '안은 어둡고 바깥은 밝은' 차이가 가장 큰 곳 — 손으로 찍은 자리가 조금 어긋나도 버튼 한가운데로
     * → {가운데 x, 가운데 y, 반지름, 밝기 차이} (im 좌표)
     */
    public static double[] locateJump(Img im, double cx, double cy, double r) {
        double best = Double.MAX_VALUE, bx = cx, by = cy, br = r;
        float[] L = lum(im);
        double st = Math.max(1, r / 16);
        for (double f = 0.8; f <= 1.31; f += 0.05) {
            double rr = r * f;
            for (double y = cy - r * 0.5; y <= cy + r * 0.5; y += st) {
                for (double x = cx - r * 0.5; x <= cx + r * 0.5; x += st) {
                    double v = level(L, im.w, im.h, x, y, rr);
                    if (!Double.isNaN(v) && v < best) {
                        best = v;
                        bx = x;
                        by = y;
                        br = rr;
                    }
                }
            }
        }
        return new double[]{bx, by, br, best};
    }

    static double level(float[] L, int w, int h, double cx, double cy, double r) {
        double in = 0, out = 0;
        int ni = 0, no = 0;
        for (int i = 0; i < ANG; i += 2) {
            for (double f : new double[]{0.45, 0.6, 0.8}) {
                double x = cx + COS[i] * r * f, y = cy + SIN[i] * r * f;
                if (x < 0 || y < 0 || x >= w || y >= h) continue;
                in += at(L, w, x, y);
                ni++;
            }
            for (double f : new double[]{1.15, 1.25, 1.35}) {
                double x = cx + COS[i] * r * f, y = cy + SIN[i] * r * f;
                if (x < 0 || y < 0 || x >= w || y >= h) continue;
                out += at(L, w, x, y);
                no++;
            }
        }
        return ni < 8 || no < 8 ? Double.NaN : in / ni - out / no;
    }

    /**
     * 점프 버튼 눌림 · 뗌 — 버튼이 반투명이라 걸어가면서 뒤 배경이 바뀌면 '평소 밝기' 도 같이 바뀜
     * (예전엔 시작할 때 잰 값으로만 판단해서, 배경이 밝은 곳으로 가면 손을 떼도 '누르고 있음' 으로 남았음 → 그 뒤 점프 · 길이 망가짐)
     *  · 안 누를 때: 평소 밝기(base) 를 천천히 따라감
     *  · 눌림: base 보다 ON 이상 갑자기 밝아짐 (ON = 시작 때 평소 밝기 크기의 절반 · 최소 20)
     *  · 뗌: 누르는 동안의 가장 밝은 값(peak) 에서 (peak - 누르기 직전 base) 의 절반 넘게 다시 어두워짐
     *  · 안전장치: 4초 넘게 '누르고 있음' 이면 뗀 걸로 보고 평소 밝기를 다시 잼
     */
    public static final class JumpWatch {
        static final double MAX_HOLD = 4.0;
        double base = Double.NaN, on = 30, pressBase, peak, downAt;
        boolean down;

        public JumpWatch() {}

        /** base0: 시작할 때 잰 안 누를 때의 밝기 차이 (음수) */
        public JumpWatch(double base0) {
            if (!Double.isNaN(base0)) {
                base = base0;
                on = Math.max(20, Math.abs(base0) * 0.5);
            }
        }

        public boolean isDown() {
            return down;
        }

        /** 이번 프레임 밝기 차이 · t: 시각 (초) → +1 막 눌림 · -1 막 뗌 · 0 그대로 */
        public int step(double v, double t) {
            if (Double.isNaN(v)) return 0;
            if (Double.isNaN(base)) {
                base = v;
                return 0;
            }
            if (!down) {
                if (v - base >= on) {
                    down = true;
                    pressBase = base;
                    peak = v;
                    downAt = t;
                    return 1;
                }
                base += (v - base) * 0.15;
                return 0;
            }
            peak = Math.max(peak, v);
            if (v < peak - (peak - pressBase) * 0.5 || t - downAt > MAX_HOLD) {
                down = false;
                base = v;
                return -1;
            }
            return 0;
        }
    }

    /** WASD → 조이스틱 방향 (단위 벡터 · 화면 좌표: 오른쪽 +x, 아래 +y) · 아무것도 안 눌렸으면 null */
    public static double[] dir(boolean w, boolean a, boolean s, boolean d) {
        double x = (d ? 1 : 0) - (a ? 1 : 0), y = (s ? 1 : 0) - (w ? 1 : 0);
        double n = Math.hypot(x, y);
        return n == 0 ? null : new double[]{x / n, y / n};
    }
}
