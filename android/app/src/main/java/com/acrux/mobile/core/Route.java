package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * 기준 장소에서 목적지까지 플레이어가 직접 걸어간 길 — 조이스틱 방향을 시간 순서대로
 *  · 한 조각: 방향 (dx, dy · 길이 0 ~ 1 · 0 이면 멈춤) 을 sec 초 동안 · 시작할 때 점프할지
 *  · 기록할 때: 화면에서 조이스틱 손잡이 위치를 매 프레임 읽어서 (Stick.knob) → 같은 방향끼리 이어 붙임
 *  · 재생할 때: 조각마다 조이스틱을 그 방향으로 그 시간만큼 밂
 */
public final class Route {
    public static final class Seg {
        public double dx, dy, sec;
        /** 이 조각을 시작할 때 점프 */
        public boolean jump;

        public Seg(double dx, double dy, double sec) {
            this.dx = dx;
            this.dy = dy;
            this.sec = sec;
        }

        public boolean idle() {
            return dx == 0 && dy == 0;
        }
    }

    public final List<Seg> segs = new ArrayList<>();

    public boolean isEmpty() {
        return segs.isEmpty();
    }

    public int jumps() {
        int n = 0;
        for (Seg g : segs) if (g.jump) n++;
        return n;
    }

    public double seconds() {
        double s = 0;
        for (Seg g : segs) s += g.sec;
        return s;
    }

    /** 방향 맞추기: 5도 · 길이 0.25 단위 (0.2 미만은 멈춤 · 0.85 이상은 끝까지) */
    static double[] quant(double[] v) {
        if (v == null) return new double[]{0, 0};
        double m = Math.hypot(v[0], v[1]);
        if (m < 0.2) return new double[]{0, 0};
        m = m >= 0.85 ? 1 : Math.round(m * 4) / 4.0;
        double a = Math.toRadians(Math.round(Math.toDegrees(Math.atan2(v[1], v[0])) / 5.0) * 5.0);
        return new double[]{r3(Math.cos(a) * m), r3(Math.sin(a) * m)};
    }

    static double r3(double v) {
        double r = Math.round(v * 1000) / 1000.0;
        return r == 0 ? 0 : r;            // -0.0 → 0
    }

    /** 기록기: add(시각 초, 방향) 를 프레임마다 → done() */
    public static final class Recorder {
        final Route r = new Route();
        double lastT = -1;
        double[] lastV;
        boolean lastJump;

        public void add(double t, double[] v) {
            add(t, v, false);
        }

        /** jump: 이 프레임에 점프 버튼이 막 눌림 */
        public void add(double t, double[] v, boolean jump) {
            double[] q = quant(v);
            if (lastT >= 0) {
                double dt = t - lastT;
                if (dt > 0) {
                    Seg last = r.segs.isEmpty() ? null : r.segs.get(r.segs.size() - 1);
                    if (last != null && !lastJump && last.dx == lastV[0] && last.dy == lastV[1]) last.sec += dt;
                    else {
                        Seg g = new Seg(lastV[0], lastV[1], dt);
                        g.jump = lastJump;
                        r.segs.add(g);
                    }
                }
            }
            lastT = t;
            lastV = q;
            lastJump = jump;
        }

        /** 끝: 앞뒤의 멈춘 시간은 뺌 · 아주 짧은 흔들림(0.08초 미만)은 앞 조각에 붙임 */
        public Route done() {
            List<Seg> s = r.segs;
            while (!s.isEmpty() && s.get(0).idle() && !s.get(0).jump) s.remove(0);
            while (!s.isEmpty() && s.get(s.size() - 1).idle() && !s.get(s.size() - 1).jump) s.remove(s.size() - 1);
            for (int i = 1; i < s.size(); ) {
                Seg g = s.get(i);
                if (g.jump) {
                    i++;                                   // 점프 조각은 안 합침
                } else if (g.sec < 0.08 && i + 1 < s.size()) {
                    s.get(i - 1).sec += g.sec;
                    s.remove(i);
                } else if (s.get(i - 1).dx == g.dx && s.get(i - 1).dy == g.dy) {
                    s.get(i - 1).sec += g.sec;
                    s.remove(i);
                } else {
                    i++;
                }
            }
            for (Seg g : s) g.sec = Math.round(g.sec * 1000) / 1000.0;
            return r;
        }
    }

    /** "dx,dy,sec;dx,dy,sec,j;…" (j = 점프) */
    public String save() {
        StringBuilder b = new StringBuilder();
        for (Seg g : segs) {
            if (b.length() > 0) b.append(';');
            b.append(String.format(Locale.ROOT, "%s,%s,%s", g.dx, g.dy, g.sec));
            if (g.jump) b.append(",j");
        }
        return b.toString();
    }

    public static Route load(String s) {
        Route r = new Route();
        if (s == null || s.isEmpty()) return r;
        for (String p : s.split(";")) {
            String[] v = p.split(",");
            if (v.length < 3) continue;
            try {
                Seg g = new Seg(Double.parseDouble(v[0]), Double.parseDouble(v[1]), Double.parseDouble(v[2]));
                g.jump = v.length > 3 && v[3].equals("j");
                r.segs.add(g);
            } catch (NumberFormatException ignored) {
            }
        }
        return r;
    }
}
