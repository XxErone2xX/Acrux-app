package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.List;

/**
 * 릴링 조작 (PC 버전 fishing.py 의 ReelControl · reel_decision 을 그대로 옮김)
 * step(시각, 내 위치, 구간, 바 폭, 구간 후보) → 누를지 · 실제로 눌렀으면 clicked(시각)
 */
public final class ReelControl {
    public static final double RISE_GAIN = 2.0, RISE_MAX = 0.5, CLICK_SETTLE = 0.2;

    final Settings cfg;
    final List<double[]> hist = new ArrayList<>();    // (시각, 위치) 최근 5개
    public double vel = 0.0;
    Double odd = null;
    double[] zoneRef = null;                            // (폭, 가운데, 마지막으로 본 시각)
    double lastClick = 0.0;
    boolean rose = true;
    /** 모바일: 터치가 반영되기까지 걸린 시간(평균) — 누른 뒤 다시 누르지 않고 기다리는 시간을 그만큼 늘림 */
    public double riseDelay = 0.0;
    // 마지막 step 결과 (기록용)
    public Double usedMarker;
    public int[] usedZone;

    public ReelControl(Settings cfg) {
        this.cfg = cfg;
    }

    public void clicked(double t) {
        lastClick = t;
        rose = false;
    }

    public static boolean decision(Double marker, int[] zone, double vel, Settings cfg, int w) {
        if (marker == null || zone == null) return false;
        double target = zone[0] + (zone[1] - zone[0]) * cfg.targetPct / 100.0;
        double pred = marker + vel * cfg.leadMs / 1000.0;
        if (Math.max(marker, pred) > (zone[0] + zone[1]) / 2.0) return false;
        return pred <= target - cfg.deadband * w;
    }

    int[] trackZone(double now, List<int[]> cands, Double m, int w) {
        double[] ref = zoneRef;
        if (ref == null) {
            int[] z = null;
            for (int[] c : cands) {
                if (c[1] - c[0] >= Math.max(8, w * 0.06) && (z == null || c[1] - c[0] > z[1] - z[0])) z = c;
            }
            if (z == null) return null;
            zoneRef = new double[]{z[1] - z[0], (z[0] + z[1]) / 2.0, now};
            return z;
        }
        double rw = ref[0], rc = ref[1], rt = ref[2];
        double reach = rw * 0.6 + 300 * Math.max(0.0, now - rt);
        int[] best = null;
        double bestD = 0;
        for (int[] z : cands) {
            double d = Math.abs((z[0] + z[1]) / 2.0 - rc);
            if (z[1] - z[0] >= rw * 0.7 && d <= reach && (best == null || d < bestD)) {
                best = z;
                bestD = d;
            }
        }
        if (best != null) {
            int zw = best[1] - best[0];
            zoneRef = new double[]{rw * 0.8 + Math.min(zw, rw * 1.3) * 0.2, (best[0] + best[1]) / 2.0, now};
            return best;
        }
        if (m != null) {
            Integer right = null;
            for (int[] z : cands) {
                if (Math.abs(z[0] - m) <= Math.max(6, w * 0.02) && z[1] > m && Math.abs(z[1] - (rc + rw / 2)) <= reach) {
                    right = right == null ? z[1] : Math.max(right, z[1]);
                }
            }
            if (right != null) {
                zoneRef = new double[]{rw, right - rw / 2, now};
                return new int[]{(int) Math.rint(right - rw), right};
            }
        }
        if (now - rt <= 1.0) return new int[]{(int) Math.rint(rc - rw / 2), (int) Math.rint(rc + rw / 2)};
        zoneRef = null;
        return null;
    }

    public boolean step(double now, Double m, int[] zone, int w, List<int[]> zones) {
        if (m != null) {
            double jump = w * 0.25;
            int n = hist.size();
            if (n >= 2 && Math.abs(m - hist.get(n - 1)[1]) > jump && Math.abs(m - hist.get(n - 2)[1]) > jump) {
                if (odd != null && Math.abs(m - odd) <= jump * 0.5) {
                    hist.clear();
                    hist.add(new double[]{now, m});
                    odd = null;
                } else {
                    odd = m;
                    m = null;
                }
            } else {
                odd = null;
                hist.add(new double[]{now, m});
                while (hist.size() > 5) hist.remove(0);
            }
        }
        if (m == null && !hist.isEmpty() && now - hist.get(hist.size() - 1)[0] < 0.1) {
            double[] last = hist.get(hist.size() - 1);
            m = last[1] + vel * (now - last[0]);
        }
        if (hist.size() >= 3) {
            double[] a = hist.get(0), b = hist.get(hist.size() - 1);
            if (b[0] > a[0]) vel = vel * 0.5 + ((b[1] - a[1]) / (b[0] - a[0])) * 0.5;
        }
        List<int[]> cands = zones != null ? zones : new ArrayList<>();
        if (zones == null && zone != null) cands.add(zone);
        int[] z = trackZone(now, cands, m, w);
        boolean fast = false;
        if (m != null && z != null && hist.size() >= 3) {
            double target = z[0] + (z[1] - z[0]) * cfg.targetPct / 100.0;
            double allow = Math.min(w * RISE_MAX, Math.max(0.0, target - m) * RISE_GAIN);
            fast = vel > Math.max(w * 0.05, allow);
        }
        if (lastClick != 0 && !rose && hist.size() >= 3 && vel > w * 0.05 && hist.get(hist.size() - 1)[0] > lastClick) {
            rose = true;
            double d = hist.get(hist.size() - 1)[0] - lastClick;
            riseDelay = riseDelay == 0 ? d : riseDelay * 0.7 + d * 0.3;
        }
        double settle = cfg.adaptive ? Math.min(0.5, Math.max(CLICK_SETTLE, riseDelay * 1.6)) : CLICK_SETTLE;
        boolean waiting = lastClick != 0 && !rose && now - lastClick < settle;
        usedMarker = m;
        usedZone = z;
        return !waiting && !fast && (now - lastClick) * 1000 >= cfg.clickGapMs && decision(m, z, vel, cfg, w);
    }
}
