package com.acrux.mobile.core;

import java.util.Arrays;
import java.util.List;

/**
 * 릴링 조작 (모바일) — 위치는 전부 바 폭 비율 (0 = 왼쪽 끝 · 1 = 오른쪽 끝)
 *  · 미니게임: 안 누르면 내 위치가 일정한 속도로 왼쪽으로 떨어지고 · 한 번 누르면 일정한 만큼 오른쪽으로 튐
 *  · 폰은 누른 게 화면에 보이기까지 오래 걸림 (터치 전달 + 게임 프레임 + 화면 캡처 · 기기마다 0.05 ~ 0.5초)
 *    → 지금 보이는 위치가 아니라 '지금 누르면 반영될 때의 위치' 로 판단하고, 이미 눌렀지만 아직 화면에 안 보인 터치만큼 미리 더함
 *    (1.5.8 까지는 PC 방식 그대로라, 누른 게 아직 안 보이는 동안 또 눌러서 지연이 큰 기기에서 오른쪽으로 크게 넘어갔다가 떨어지기를 반복)
 *  · 지연: 최근 3초 동안 본 내 위치를 '일정하게 떨어짐 + 터치마다 같은 만큼 튐' 에 맞춰 보고 가장 잘 맞는 값 (0.01초 단위)
 *    (터치 하나하나를 튐과 짝지어 재면 처음 값이 틀렸을 때 틀린 값 쪽으로 짝을 지어서 못 고쳐짐)
 *  · 떨어지는 속도 · 한 번에 튀는 양도 화면을 보며 다시 잼 · 처음 값은 지난 판에서 잰 값 (앱이 저장)
 *  · 미니게임은 바가 뜬 뒤 'Ready!' 동안 (영상에서 1.25초) 내 위치가 멈춰 있고 누른 것도 안 먹힘 → 움직이기 시작할 때까지 안 누름
 *    (그 동안 누르면 안 먹은 터치로 쌓여서 처음 예측이 틀어지고, 누르는 자리가 안 되는 것으로 잘못 봤음)
 */
public final class ReelPilot {
    /** 처음 값 — 영상에서 잰 값: 떨어짐 바 폭의 0.29 / 초 · 한 번 누르면 0.087 · 지연 0.15초 */
    public static final double FALL0 = 0.29, JUMP0 = 0.087, DELAY0 = 0.15;
    /** 터치 사이 최소 간격 (누르는 시간 40ms + 여유 · 앞 터치가 끝나기 전에 새 터치를 보내면 안드로이드가 앞 터치를 취소함) */
    public static final double MIN_GAP = 0.07, DELAY_MIN = 0.03, DELAY_MAX = 0.6;
    static final double FIT_WIN = 3.0, FIT_STEP = 0.01;
    /** 'Ready!' 로 보는 최대 시간 (그 뒤로는 안 움직여도 시작한 것으로 봄) · 움직였다고 볼 거리 (바 폭 비율) */
    static final double READY_MAX = 2.5, READY_MOVE = 0.012;

    public double fall, jump, delay;                    // 바 폭 / 초 · 바 폭 · 초
    double jit = 0.03;                                  // 터치마다 지연이 흔들리는 정도 (초)
    // 이번 판 터치: 보낸 시각 · 상태 (0 아직 안 보임 · 1 보임 · -1 안 먹음)
    double[] tapT = new double[64];
    byte[] tapS = new byte[64];
    int nTap;
    // 이번 판 화면: 받은 시각 · 내 위치 (벽에 닿거나 이상한 값이 이어지면 seg 부터 새 구간)
    double[] obT = new double[256], obM = new double[256];
    int nOb, seg, odd;
    double lastTap = -1, tPrev = -1, mPrev = Double.NaN, lastObsT = -1, lastObsM = Double.NaN;
    double segT = -1, segM = Double.NaN;                // 이번 낙하 구간의 시작 (떨어지는 속도 재기)
    double[] zoneRef = null;                            // {폭, 가운데, 마지막으로 본 시각}
    double zv = 0;                                      // 구간이 움직이는 속도 (바 폭 / 초)
    double zc = Double.NaN, zw = Double.NaN, zt = -1;
    // 'Ready!' (아직 시작 안 함) → 내 위치가 움직이면 live · moved: 내 위치가 실제로 움직이는 걸 봄 (진짜 미니게임인지)
    public boolean live, moved;
    double firstT = -1, firstM = Double.NaN, liveM = Double.NaN;
    public double liveAt = -1;
    // 기록 (판이 끝나면 한 줄로 남김)
    public int sent, seenJumps, lost, frames, markerFrames, zoneFrames, fits, liveFrames;
    public double inZone, seenTime;

    /** fall · jump · delay: 지난 판에서 잰 값 (0 이하 · 이상한 값이면 처음 값) */
    public ReelPilot(double fall, double jump, double delay) {
        this.fall = fall > 0.03 && fall < 1.5 ? fall : FALL0;
        this.jump = jump > 0.02 && jump < 0.3 ? jump : JUMP0;
        this.delay = delay >= DELAY_MIN && delay <= DELAY_MAX ? delay : DELAY0;
    }

    double jumpMin() {
        return Math.max(0.012, jump * 0.35);
    }

    /** 새 화면 (tf: 화면을 받은 시각 · m: 내 위치 또는 null · zones: 구간 후보들 [시작, 끝]) */
    public void see(double tf, Double m, List<double[]> zones) {
        frames++;
        double[] z = trackZone(tf, zones, m);
        if (z != null) {
            zoneFrames++;
            double c = (z[0] + z[1]) / 2, ww = z[1] - z[0];
            if (!Double.isNaN(zc) && tf > zt && tf - zt < 0.3) {
                double v = (c - zc) / (tf - zt);
                zv = Math.max(-0.6, Math.min(0.6, zv * 0.7 + v * 0.3));
            }
            zw = Double.isNaN(zw) ? ww : zw * 0.7 + ww * 0.3;
            zc = c;
            zt = tf;
        }
        if (m == null) return;
        // 예상보다 한참 늦도록 안 보인 터치 = 안 먹은 터치 (내 위치가 보인 화면에서만 셈 — 안 보이는 동안 반영된 건 못 봤을 뿐)
        double late = delay + Math.max(0.15, jit * 4) + 0.05;
        for (int i = 0; i < nTap; i++) {
            if (tapS[i] == 0 && tf - tapT[i] > late) {
                tapS[i] = -1;
                lost++;
            }
        }
        // 한 화면만 크게 튀면 잘못 본 것 (글자 · 효과) → 버림 · 두 화면 이어지면 진짜로 보고 새 구간
        if (!Double.isNaN(mPrev) && Math.abs(m - mPrev) > 0.4 && tf - tPrev < 0.25) {
            if (++odd < 2) return;
            seg = nOb;
            segM = Double.NaN;
        }
        odd = 0;
        markerFrames++;
        if (!live) {
            // 'Ready!': 바와 내 위치는 보이지만 멈춰 있음 → 움직이면 (떨어짐 · 튐) 시작 · 오래 안 움직여도 시작으로 봄
            // (내 위치를 못 찾으면 0 으로 오기도 해서 '왼쪽 끝 = 시작' 으로는 안 봄)
            if (Double.isNaN(firstM)) {
                firstM = m;
                firstT = tf;
            }
            moved = Math.abs(m - firstM) >= READY_MOVE;
            if (moved || tf - firstT >= READY_MAX) {
                live = true;
                liveAt = tf;
                liveM = m;
                tPrev = -1;                                     // 멈춰 있던 화면과는 이어서 재지 않음
                mPrev = Double.NaN;
                segM = Double.NaN;
                seg = nOb;
            } else {
                lastObsT = tf;
                lastObsM = m;
                return;
            }
        }
        liveFrames++;
        if (!moved && Math.abs(m - liveM) >= READY_MOVE) moved = true;
        if (z != null && m >= z[0] && m <= z[1] && tPrev > 0) inZone += Math.min(0.1, tf - tPrev);
        if (tPrev > 0) seenTime += Math.min(0.1, tf - tPrev);
        boolean wall = m <= 0.01 || m >= 0.99;
        if (!Double.isNaN(mPrev) && tf > tPrev) {
            double dt = tf - tPrev, d = m - mPrev;
            if (d > jumpMin()) {
                // 튐 → 아직 안 보인 터치 중 '보낸 시각 + 지연' 이 이 사이에 가장 가까운 것들을 보인 것으로 (튄 양 / 한 번에 튀는 양 = 몇 번)
                int n = (int) Math.max(1, Math.round((d + fall * dt) / jump));
                double mid = tf - dt * 0.5;
                int k = 0;
                for (; k < n; k++) {
                    int best = -1;
                    double bd = 0;
                    for (int i = 0; i < nTap; i++) {
                        if (tapS[i] != 0 || tapT[i] > tf - DELAY_MIN * 0.5) continue;
                        double e = Math.abs(mid - tapT[i] - delay);
                        if (best < 0 || e < bd) {
                            best = i;
                            bd = e;
                        }
                    }
                    if (best < 0) break;
                    tapS[best] = 1;
                    if (n == 1) jit = Math.max(0.01, Math.min(0.1, jit * 0.8 + bd * 0.2));
                }
                seenJumps += k;
                if (n == 1 && k == 1 && !wall) {
                    double j = d + fall * dt;
                    if (j > 0.02 && j < 0.3) jump = jump * 0.7 + j * 0.3;
                }
                segT = tf;
                segM = m;
            } else if (d < -0.5 || wall || Double.isNaN(segM) || landing(segT, tf)) {
                segT = tf;
                segM = m;
            } else if (tf - segT >= 0.08) {
                // 떨어지는 속도: 이번 낙하 구간에서 0.08초 이상 떨어진 두 화면으로 (한 화면씩 보면 흔들림이 큼)
                double f = (segM - m) / (tf - segT);
                if (f > 0.03 && f < 1.5) fall = fall * 0.8 + f * 0.2;
                segT = tf;
                segM = m;
            }
        } else {
            segT = tf;
            segM = m;
        }
        // 벽에 닿은 화면은 맞춰 보기에서 빼고 (더 안 떨어지거나 더 안 올라감) 그 뒤로 새 구간
        if (wall) {
            seg = nOb;
        } else {
            push(tf, m);
            fit();
        }
        tPrev = tf;
        mPrev = m;
        lastObsT = tf;
        lastObsM = m;
    }

    void push(double t, double m) {
        if (nOb == obT.length) {
            // 오래된 화면은 버림 (맞춰 보기는 최근 FIT_WIN 초만 씀)
            int keep = 0;
            while (keep < nOb && obT[keep] < t - FIT_WIN - 0.5) keep++;
            if (keep > nOb / 4) {
                System.arraycopy(obT, keep, obT, 0, nOb - keep);
                System.arraycopy(obM, keep, obM, 0, nOb - keep);
                nOb -= keep;
                seg = Math.max(0, seg - keep);
            } else {
                obT = Arrays.copyOf(obT, obT.length * 2);
                obM = Arrays.copyOf(obM, obM.length * 2);
            }
        }
        obT[nOb] = t;
        obM[nOb] = m;
        nOb++;
    }

    /**
     * 지연 다시 재기 — 최근 FIT_WIN 초 (지금 구간 안) 의 내 위치를 '시작 위치 - 떨어지는 속도 × 시간 + 튀는 양 × 반영된 터치 수' 에 맞춰 봄
     *  · 지연 후보 (0.03 ~ 0.6초 · 0.01초 간격) 마다 각 화면까지 반영됐을 터치 수를 세서 최소제곱 → 가장 잘 맞는 지연
     *  · 지금 값 근처면 천천히 옮기고, 멀리 있는 값은 확실히 더 잘 맞을 때만 (규칙적으로 누를 때 생기는 가짜 답을 피함)
     */
    void fit() {
        int i0 = seg;
        double tEnd = obT[nOb - 1];
        while (i0 < nOb && obT[i0] < tEnd - FIT_WIN) i0++;
        int n = nOb - i0;
        if (n < 12) return;
        double t0 = obT[i0];
        double st = 0, stt = 0, sm = 0, stm = 0, smm = 0;
        for (int i = i0; i < nOb; i++) {
            double t = obT[i] - t0, m = obM[i];
            st += t;
            stt += t * t;
            sm += m;
            stm += t * m;
            smm += m * m;
        }
        int j0 = 0;
        while (j0 < nTap && tapT[j0] + DELAY_MAX <= obT[i0]) j0++;
        int steps = (int) Math.round((DELAY_MAX - DELAY_MIN) / FIT_STEP);
        int cur = (int) Math.round((delay - DELAY_MIN) / FIT_STEP);
        double bestE = Double.MAX_VALUE, curE = Double.MAX_VALUE;
        int bestK = -1;
        for (int k = 0; k <= steps; k++) {
            double dl = DELAY_MIN + k * FIT_STEP;
            int j = j0;
            while (j < nTap && tapT[j] + dl <= obT[i0]) j++;                  // 첫 화면 전에 반영된 터치는 시작 위치에 들어감
            int c = 0;
            double sn = 0, stn = 0, snn = 0, snm = 0;
            for (int i = i0; i < nOb; i++) {
                while (j < nTap && tapT[j] + dl <= obT[i]) {
                    c++;
                    j++;
                }
                double t = obT[i] - t0;
                sn += c;
                stn += t * c;
                snn += (double) c * c;
                snm += c * obM[i];
            }
            if (c < 2) continue;                                              // 이 지연이면 반영된 터치가 2번 미만 → 판단 못 함
            // m ≈ a - f·t + J·N → (a, f, J) 최소제곱 (3 × 3 대칭 행렬)
            double a11 = n, a12 = -st, a13 = sn, a22 = stt, a23 = -stn, a33 = snn;
            double b1 = sm, b2 = -stm, b3 = snm;
            double c11 = a22 * a33 - a23 * a23, c12 = a13 * a23 - a12 * a33, c13 = a12 * a23 - a13 * a22;
            double det = a11 * c11 + a12 * c12 + a13 * c13;
            if (Math.abs(det) < 1e-9) continue;
            double c22 = a11 * a33 - a13 * a13, c23 = a12 * a13 - a11 * a23, c33 = a11 * a22 - a12 * a12;
            double a = (c11 * b1 + c12 * b2 + c13 * b3) / det;
            double f = (c12 * b1 + c22 * b2 + c23 * b3) / det;
            double jj = (c13 * b1 + c23 * b2 + c33 * b3) / det;
            if (jj < 0.02 || jj > 0.3 || f < 0.03 || f > 1.5) continue;           // 말이 안 되는 맞춤
            double e = smm - (a * b1 + f * b2 + jj * b3);
            if (k == cur) curE = e;
            if (e < bestE) {
                bestE = e;
                bestK = k;
            }
        }
        if (bestK < 0) return;
        double best = DELAY_MIN + bestK * FIT_STEP;
        fits++;
        if (Math.abs(best - delay) <= 0.035) delay = delay * 0.6 + best * 0.4;
        else if (bestE < curE * 0.8) delay = best;
        delay = Math.max(DELAY_MIN, Math.min(DELAY_MAX, delay));
    }

    /** 구간 따라가기 (PC ReelControl.trackZone 과 같은 생각) — 글자 · 내 위치에 가려 잘려 보여도 같은 구간으로 이어 봄 */
    double[] trackZone(double now, List<double[]> cands, Double m) {
        double[] ref = zoneRef;
        if (ref == null) {
            double[] z = null;
            for (double[] c : cands) if (c[1] - c[0] >= 0.06 && (z == null || c[1] - c[0] > z[1] - z[0])) z = c;
            if (z == null) return null;
            zoneRef = new double[]{z[1] - z[0], (z[0] + z[1]) / 2, now};
            return z;
        }
        double rw = ref[0], rc = ref[1], rt = ref[2];
        double reach = rw * 0.6 + 0.5 * Math.max(0.0, now - rt);
        double[] best = null;
        double bestD = 0;
        for (double[] z : cands) {
            double d = Math.abs((z[0] + z[1]) / 2 - rc);
            if (z[1] - z[0] >= rw * 0.7 && d <= reach && (best == null || d < bestD)) {
                best = z;
                bestD = d;
            }
        }
        if (best != null) {
            double zw2 = best[1] - best[0];
            zoneRef = new double[]{rw * 0.8 + Math.min(zw2, rw * 1.3) * 0.2, (best[0] + best[1]) / 2, now};
            return best;
        }
        if (m != null) {
            double right = Double.NaN;
            for (double[] z : cands) {
                if (Math.abs(z[0] - m) <= 0.02 && z[1] > m && Math.abs(z[1] - (rc + rw / 2)) <= reach)
                    right = Double.isNaN(right) ? z[1] : Math.max(right, z[1]);
            }
            if (!Double.isNaN(right)) {
                zoneRef = new double[]{rw, right - rw / 2, now};
                return new double[]{right - rw, right};
            }
        }
        if (now - rt <= 1.0) return new double[]{rc - rw / 2, rc + rw / 2};
        zoneRef = null;
        return null;
    }

    /** (t0, t1] 사이에 반영될 터치가 있는지 (떨어지는 속도를 잴 때 빼려고) */
    boolean landing(double t0, double t1) {
        for (int i = 0; i < nTap; i++) {
            double at = tapT[i] + delay;
            if (tapS[i] >= 0 && at > t0 - 0.05 && at <= t1 + 0.05) return true;
        }
        return false;
    }

    /** 아직 화면에 안 보인 터치 수 (화면을 받은 시각 tf 기준 · 지연이 흔들리는 만큼 여유) */
    int pending(double tf) {
        double from = tf - delay - Math.max(0.03, jit * 2);
        int n = 0;
        for (int i = 0; i < nTap; i++) if (tapS[i] == 0 && tapT[i] > from) n++;
        return n;
    }

    /** 마지막으로 판단할 때 본 것 (디버깅 화면 표시용 · 바 폭 비율 · 판단 못 했으면 NaN): 지금 누르면 반영될 때의 내 위치 예측 · 이보다 왼쪽이면 누름 */
    public double lastPred = Double.NaN, lastTarget = Double.NaN;

    /** 지금 누를지 */
    public boolean decide(double now) {
        if (!live || Double.isNaN(lastObsM) || Double.isNaN(zc) || now - lastObsT > 0.35 || now - zt > 0.6) {
            lastPred = lastTarget = Double.NaN;
            return false;
        }
        double te = now + delay;                                   // 지금 누르면 반영될 때
        double m = lastObsM - fall * (te - lastObsT) + jump * pending(lastObsT);
        m = Math.max(0, m);
        double c = zc + zv * Math.min(0.3, te - zt), width = zw;
        // 누른 뒤 구간 가운데를 기준으로 위아래로 같은 만큼 오가게: 반영될 때 위치가 (가운데 - 튀는 양/2) 아래면 누름
        double target = c - jump / 2;
        // 구간이 튀는 양보다 넓으면 아래쪽 여유를 조금 더 (떨어지는 쪽이 늘 먼저 벗어나니까)
        if (width > jump) target += Math.min((width - jump) * 0.15, 0.02);
        lastPred = m;
        lastTarget = target;
        if (lastTap >= 0 && now - lastTap < MIN_GAP) return false;
        return m < target;
    }

    public void tapped(double now) {
        lastTap = now;
        sent++;
        if (nTap == tapT.length) {
            // 오래된 터치는 버림 (DELAY_MAX + 맞춰 보는 시간보다 오래된 것)
            int keep = 0;
            while (keep < nTap && tapT[keep] < now - FIT_WIN - DELAY_MAX - 0.5) keep++;
            if (keep > nTap / 4) {
                System.arraycopy(tapT, keep, tapT, 0, nTap - keep);
                System.arraycopy(tapS, keep, tapS, 0, nTap - keep);
                nTap -= keep;
            } else {
                tapT = Arrays.copyOf(tapT, tapT.length * 2);
                tapS = Arrays.copyOf(tapS, tapS.length * 2);
            }
        }
        tapT[nTap] = now;
        tapS[nTap] = 0;
        nTap++;
    }
}
