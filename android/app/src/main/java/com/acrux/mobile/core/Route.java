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
        /** 이 조각을 시작할 때 점프 버튼을 누름 · 뗌 (꾹 누르고 있으면 누름 → 몇 조각 뒤 뗌) */
        public boolean jump, jumpUp;

        boolean marked() {
            return jump || jumpUp;
        }

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

    /**
     * 손가락이 실제로 한 일 (V1.4.1 부터의 기록) — 시각 (ms · 처음 누른 때 = 0) 그대로 다시 함
     *  · F: 새로 누름 (a = 조이스틱을 가운데에서 dx, dy 로 끌기 · b = 점프 버튼) — 그 전에 누르던 손가락은 다 떨어짐
     *  · M: 조이스틱 손가락을 dx, dy 로 옮김 · U: 뗌 (a = 조이스틱 · b = 점프) · X: 전부 떨어짐
     */
    public static final class Ev {
        public final char k;
        public double t, dx, dy;
        public boolean a, b;

        public Ev(char k, double t, double dx, double dy, boolean a, boolean b) {
            this.k = k;
            this.t = t;
            this.dx = dx;
            this.dy = dy;
            this.a = a;
            this.b = b;
        }
    }

    public final List<Ev> evs = new ArrayList<>();

    /** 기록한 손가락 움직임 → 길 (처음 누른 때를 0 으로 · 시각은 뒤로 가지 않게) */
    public static Route fromTrace(List<Ev> in) {
        Route r = new Route();
        int i0 = 0;
        while (i0 < in.size() && in.get(i0).k != 'F') i0++;
        if (i0 >= in.size()) return r;
        double t0 = in.get(i0).t, last = 0;
        boolean s = false, j = false;
        for (int i = i0; i < in.size(); i++) {
            Ev e = in.get(i);
            double t = Math.max(last, Math.round((e.t - t0) * 10) / 10.0);
            last = t;
            r.evs.add(new Ev(e.k, t, r3(e.dx), r3(e.dy), e.a, e.b));
            if (e.k == 'F') {
                s = e.a;
                j = e.b;
            } else if (e.k == 'X') {
                s = j = false;
            } else if (e.k == 'U') {
                s &= !e.a;
                j &= !e.b;
            }
        }
        if (s || j) r.evs.add(new Ev('X', last, 0, 0, false, false));       // 끝에 누르고 있던 손가락은 뗌
        while (!r.evs.isEmpty() && r.evs.get(r.evs.size() - 1).k == 'F') r.evs.remove(r.evs.size() - 1);
        return r;
    }

    /** 점프 버튼 위치가 없을 때: 점프를 뺀 손가락 움직임 */
    public static List<Ev> noJump(List<Ev> in) {
        List<Ev> o = new ArrayList<>();
        for (Ev e : in) {
            if (e.k == 'F') o.add(new Ev('F', e.t, e.dx, e.dy, e.a, false));       // 조이스틱 없는 F 도 남김 (그 전 손가락이 떨어지는 건 같음)
            else if (e.k == 'U' && !e.a) continue;
            else o.add(new Ev(e.k, e.t, e.dx, e.dy, e.a, false));
        }
        return o;
    }

    /** 조이스틱을 누른 횟수 */
    public int presses() {
        int n = 0;
        for (Ev e : evs) if (e.k == 'F' && e.a) n++;
        return n;
    }

    /** 조각 수 (화면 표시용) */
    public int parts() {
        return evs.isEmpty() ? segs.size() : presses();
    }

    /** 걸은 적이 있나 */
    public boolean moves() {
        if (evs.isEmpty()) return !segs.isEmpty();
        return presses() > 0;
    }

    /**
     * 길 위치 (화면을 보고 잰 캐릭터 위치 · Odo) — 다시 걸을 때 이 길을 따라가며 어긋난 걸 고침
     * 한 점: {시각, x, y, 점프 (1 누름 · 2 뗌 · 3 둘 다), 그때 조이스틱 dx, dy}
     */
    public final List<double[]> pts = new ArrayList<>();
    /** 조이스틱 방향 → 실제로 움직인 방향 사이 각도 (라디안 · 기록에서 잼) */
    public double theta;

    /** 위치 길이 있고 쓸 만한가 (충분히 움직였음) */
    public boolean hasPath() {
        if (pts.size() < 5) return false;
        double len = 0;
        for (int i = 1; i < pts.size(); i++) len += Math.hypot(pts.get(i)[1] - pts.get(i - 1)[1], pts.get(i)[2] - pts.get(i - 1)[2]);
        return len >= 40;
    }

    /** 조이스틱 방향과 실제 움직임 방향 사이 각도 맞추기 (원형 평균) */
    public void fitTheta() {
        double sx = 0, sy = 0;
        int n = 0;
        for (int i = 1; i < pts.size(); i++) {
            double[] a = pts.get(i - 1), b = pts.get(i);
            double vx = b[1] - a[1], vy = b[2] - a[2], dt = b[0] - a[0];
            if (dt <= 0 || Math.hypot(vx, vy) / dt < 30 || Math.hypot(a[4], a[5]) < 0.6) continue;
            double d = Math.atan2(vy, vx) - Math.atan2(a[5], a[4]);
            sx += Math.cos(d);
            sy += Math.sin(d);
            n++;
        }
        theta = n >= 5 && Math.hypot(sx, sy) / n > 0.6 ? Math.atan2(sy, sx) : 0;
    }

    public boolean isEmpty() {
        return segs.isEmpty() && evs.isEmpty();
    }

    public int jumps() {
        int n = 0;
        boolean held = false;
        for (Ev e : evs) {
            if (e.k == 'F') {
                if (e.b && !held) n++;
                held = e.b;
            } else if (e.k == 'X' || (e.k == 'U' && e.b)) {
                held = false;
            }
        }
        if (!evs.isEmpty()) return n;
        for (Seg g : segs) if (g.jump) n++;
        return n;
    }

    public double seconds() {
        if (!evs.isEmpty()) return evs.get(evs.size() - 1).t / 1000.0;
        double s = 0;
        for (Seg g : segs) s += g.sec;
        return s;
    }

    /** 방향 맞추기: 5도 · 길이 0.25 단위 (0.3 미만은 멈춤 — 안 누를 때도 손잡이가 조금 흔들려 보임 · 0.85 이상은 끝까지) */
    static double[] quant(double[] v) {
        if (v == null) return new double[]{0, 0};
        double m = Math.hypot(v[0], v[1]);
        if (m < 0.3) return new double[]{0, 0};
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
        /** 정확한 입력 (Acrux 조이스틱) — 방향을 1도 · 길이 0.05 단위로 · 흔들림 막기 없음 */
        final boolean exact;
        double lastT = -1;

        public Recorder() {
            this(false);
        }

        public Recorder(boolean exact) {
            this.exact = exact;
        }

        /** 정확한 입력 맞추기 */
        static double[] fine(double[] v) {
            if (v == null) return new double[]{0, 0};
            double m = Math.min(1, Math.hypot(v[0], v[1]));
            if (m < 0.15) return new double[]{0, 0};
            m = m >= 0.95 ? 1 : Math.round(m * 20) / 20.0;
            double a = Math.toRadians(Math.round(Math.toDegrees(Math.atan2(v[1], v[0]))));
            return new double[]{r3(Math.cos(a) * m), r3(Math.sin(a) * m)};
        }

        public Route route() {
            return r;
        }
        double[] lastV;
        boolean lastJump, lastUp;

        public void add(double t, double[] v) {
            add(t, v, false, false);
        }

        public void add(double t, double[] v, boolean jump) {
            add(t, v, jump, false);
        }

        /** jump: 이 프레임에 점프 버튼이 막 눌림 · up: 막 뗌 */
        public void add(double t, double[] v, boolean jump, boolean up) {
            double[] q = exact ? fine(v) : quant(v);
            // 흔들림 막기: 방금 방향과 10도 · 길이 0.25 안쪽이면 방금 방향 그대로 (손가락은 그대로인데 읽은 값만 조금 튀는 것)
            if (!exact && lastV != null && !(q[0] == 0 && q[1] == 0) && !(lastV[0] == 0 && lastV[1] == 0)) {
                double da = Math.abs(Math.atan2(q[1], q[0]) - Math.atan2(lastV[1], lastV[0]));
                if (da > Math.PI) da = 2 * Math.PI - da;
                if (Math.toDegrees(da) <= 10 && Math.abs(Math.hypot(q[0], q[1]) - Math.hypot(lastV[0], lastV[1])) <= 0.25) q = lastV;
            }
            if (lastT >= 0) {
                double dt = t - lastT;
                if (dt > 0) {
                    Seg last = r.segs.isEmpty() ? null : r.segs.get(r.segs.size() - 1);
                    if (last != null && !lastJump && !lastUp && last.dx == lastV[0] && last.dy == lastV[1]) last.sec += dt;
                    else {
                        Seg g = new Seg(lastV[0], lastV[1], dt);
                        g.jump = lastJump;
                        g.jumpUp = lastUp;
                        r.segs.add(g);
                    }
                }
            }
            lastT = t;
            lastV = q;
            lastJump = jump;
            lastUp = up;
        }

        /** 끝: 앞뒤의 멈춘 시간은 뺌 · 아주 짧은 흔들림(0.12초 미만)은 앞 조각에 붙임 */
        public Route done() {
            List<Seg> s = r.segs;
            while (!s.isEmpty() && s.get(0).idle() && !s.get(0).marked()) s.remove(0);
            while (!s.isEmpty() && s.get(s.size() - 1).idle() && !s.get(s.size() - 1).marked()) s.remove(s.size() - 1);
            for (int i = 1; i < s.size(); ) {
                Seg g = s.get(i);
                if (g.marked()) {
                    i++;                                   // 점프 조각은 안 합침
                } else if (g.sec < 0.12 && i + 1 < s.size() && close(s.get(i - 1), s.get(i + 1))) {
                    s.get(i - 1).sec += g.sec;          // 앞뒤가 같은 방향인데 가운데만 잠깐 튄 것 (빠르게 돌린 건 그대로 둠)
                    s.remove(i);
                } else if (s.get(i - 1).dx == g.dx && s.get(i - 1).dy == g.dy) {
                    s.get(i - 1).sec += g.sec;
                    s.remove(i);
                } else {
                    i++;
                }
            }
            for (Seg g : s) g.sec = Math.round(g.sec * 1000) / 1000.0;
            if (!r.pts.isEmpty()) r.fitTheta();
            return r;
        }
    }

    /** 두 조각이 거의 같은 방향 (15도 · 길이 0.25 안) */
    static boolean close(Seg a, Seg b) {
        if (a.idle() || b.idle()) return a.idle() && b.idle();
        double da = Math.abs(Math.atan2(a.dy, a.dx) - Math.atan2(b.dy, b.dx));
        if (da > Math.PI) da = 2 * Math.PI - da;
        return Math.toDegrees(da) <= 15 && Math.abs(Math.hypot(a.dx, a.dy) - Math.hypot(b.dx, b.dy)) <= 0.25;
    }

    /** "dx,dy,sec;dx,dy,sec,j;…" (j = 점프) */
    public String save() {
        StringBuilder b = new StringBuilder();
        if (!evs.isEmpty()) {                              // "T:F,ms,dx,dy,조이스틱,점프;M,ms,dx,dy;U,ms,조이스틱,점프;X,ms"
            b.append("T:");
            for (int i = 0; i < evs.size(); i++) {
                Ev e = evs.get(i);
                if (i > 0) b.append(';');
                b.append(e.k).append(',').append(String.format(Locale.ROOT, "%.1f", e.t));
                if (e.k == 'F' || e.k == 'M') b.append(',').append(e.dx).append(',').append(e.dy);
                if (e.k == 'F' || e.k == 'U') b.append(',').append(e.a ? 1 : 0).append(',').append(e.b ? 1 : 0);
            }
            return b.toString();
        }
        for (Seg g : segs) {
            if (b.length() > 0) b.append(';');
            b.append(String.format(Locale.ROOT, "%s,%s,%s", g.dx, g.dy, g.sec));
            if (g.jump || g.jumpUp) b.append(',').append(g.jump ? "j" : "").append(g.jumpUp ? "u" : "");
        }
        // 위치 길: "#각도#t:x:y:f:dx:dy,..."
        if (!pts.isEmpty()) {
            b.append('#').append(String.format(Locale.ROOT, "%.4f", theta)).append('#');
            for (int i = 0; i < pts.size(); i++) {
                double[] p = pts.get(i);
                if (i > 0) b.append(',');
                b.append(String.format(Locale.ROOT, "%.2f:%.1f:%.1f:%d:%.2f:%.2f", p[0], p[1], p[2], (int) p[3], p[4], p[5]));
            }
        }
        return b.toString();
    }

    public static Route load(String s) {
        Route r = new Route();
        if (s == null || s.isEmpty()) return r;
        if (s.startsWith("T:")) {
            try {
                for (String p : s.substring(2).split(";")) {
                    String[] v = p.split(",");
                    char k = v[0].charAt(0);
                    double t = Double.parseDouble(v[1]);
                    if (k == 'F') r.evs.add(new Ev(k, t, Double.parseDouble(v[2]), Double.parseDouble(v[3]), v[4].equals("1"), v[5].equals("1")));
                    else if (k == 'M') r.evs.add(new Ev(k, t, Double.parseDouble(v[2]), Double.parseDouble(v[3]), false, false));
                    else if (k == 'U') r.evs.add(new Ev(k, t, 0, 0, v[2].equals("1"), v[3].equals("1")));
                    else if (k == 'X') r.evs.add(new Ev(k, t, 0, 0, false, false));
                }
            } catch (RuntimeException e) {
                r.evs.clear();                             // 깨진 기록 → 없는 걸로 (다시 기록)
            }
            return r;
        }
        String[] parts = s.split("#");
        s = parts[0];
        if (parts.length >= 3) {
            try {
                r.theta = Double.parseDouble(parts[1]);
                for (String p : parts[2].split(",")) {
                    String[] v = p.split(":");
                    if (v.length < 6) continue;
                    double[] q = new double[6];
                    for (int i = 0; i < 6; i++) q[i] = Double.parseDouble(v[i]);
                    r.pts.add(q);
                }
            } catch (NumberFormatException e) {
                r.pts.clear();
            }
        }
        for (String p : s.split(";")) {
            String[] v = p.split(",");
            if (v.length < 3) continue;
            try {
                Seg g = new Seg(Double.parseDouble(v[0]), Double.parseDouble(v[1]), Double.parseDouble(v[2]));
                g.jump = v.length > 3 && v[3].contains("j");
                g.jumpUp = v.length > 3 && v[3].contains("u");
                r.segs.add(g);
            } catch (NumberFormatException ignored) {
            }
        }
        return r;
    }
}
