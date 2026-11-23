package com.acrux.mobile;

import android.app.NotificationManager;
import android.content.Context;
import android.content.SharedPreferences;
import android.os.Build;

/**
 * 매크로 중 알림 끄기 — 매크로 (자동 낚시 · 테스트 등) 가 도는 동안 방해 금지 모드를 켜서 알림 배너가 안 뜨게
 *  (배너가 화면 위쪽에 떠 있을 때 매크로가 그 자리를 누르면 다른 앱이 열렸음)
 *  · 켜는 것: 방해 금지 = 알람만 (전화 · 메시지 · 앱 알림 모두 조용히) + 배너 · 전체 화면 알림 숨기기
 *  · 끝나면 켜기 전 방해 금지 상태 · 설정으로 되돌림 · 앱이 강제로 꺼졌으면 다음에 켤 때 되돌림
 *  · 처음 한 번 '방해 금지 접근 권한' 이 필요 (설정 화면에서 Acrux 허용)
 */
final class Quiet {
    static final String PREF = "quiet";                 // 켜기 · 끄기 (설정)
    static final String ACTIVE = "quiet_active";        // 지금 Acrux 가 방해 금지를 켜 둔 상태 (되돌릴 것이 있음)

    private Quiet() {
    }

    static SharedPreferences prefs(Context c) {
        return c.getSharedPreferences("acrux", Context.MODE_PRIVATE);
    }

    static boolean wanted(Context c) {
        return prefs(c).getBoolean(PREF, false);
    }

    static void setWanted(Context c, boolean on) {
        prefs(c).edit().putBoolean(PREF, on).apply();
    }

    static boolean granted(Context c) {
        NotificationManager nm = c.getSystemService(NotificationManager.class);
        return nm != null && nm.isNotificationPolicyAccessGranted();
    }

    /** 매크로 시작: 켜져 있고 권한이 있으면 방해 금지를 켬 (전 상태 저장) */
    static synchronized void on(Context c) {
        if (c == null || !wanted(c)) return;
        NotificationManager nm = c.getSystemService(NotificationManager.class);
        SharedPreferences p = prefs(c);
        if (nm == null || p.getBoolean(ACTIVE, false)) return;
        if (!nm.isNotificationPolicyAccessGranted()) {
            Hub.log("매크로 중 알림 끄기: 방해 금지 접근 권한이 없어서 못 켬 (설정 → 매크로 중 알림 끄기)", 'y');
            return;
        }
        try {
            NotificationManager.Policy pol = nm.getNotificationPolicy();
            SharedPreferences.Editor e = p.edit()
                    .putInt("quiet_filter", nm.getCurrentInterruptionFilter())
                    .putInt("quiet_cat", pol.priorityCategories)
                    .putInt("quiet_calls", pol.priorityCallSenders)
                    .putInt("quiet_msgs", pol.priorityMessageSenders)
                    .putInt("quiet_fx", pol.suppressedVisualEffects);
            if (Build.VERSION.SDK_INT >= 30) e.putInt("quiet_conv", pol.priorityConversationSenders);
            e.putBoolean(ACTIVE, true).commit();               // 바꾸기 전에 저장 (도중에 앱이 꺼져도 되돌릴 수 있게)
            if (Build.VERSION.SDK_INT >= 28) {
                int fx = pol.suppressedVisualEffects | NotificationManager.Policy.SUPPRESSED_EFFECT_PEEK
                        | NotificationManager.Policy.SUPPRESSED_EFFECT_FULL_SCREEN_INTENT;
                nm.setNotificationPolicy(policy(pol.priorityCategories, pol.priorityCallSenders, pol.priorityMessageSenders, fx,
                        Build.VERSION.SDK_INT >= 30 ? pol.priorityConversationSenders : 0));
            }
            nm.setInterruptionFilter(NotificationManager.INTERRUPTION_FILTER_ALARMS);
            Hub.log("매크로 중 알림 끄기: 방해 금지 켬 (끝나면 되돌림)", 'd');
        } catch (RuntimeException e) {
            Hub.log("매크로 중 알림 끄기 실패: " + e.getMessage(), 'y');
            off(c);
        }
    }

    /** 매크로 끝 · 앱 시작 (강제로 꺼졌던 경우): Acrux 가 켜 둔 방해 금지를 전 상태로 되돌림 */
    static synchronized void off(Context c) {
        if (c == null) return;
        SharedPreferences p = prefs(c);
        if (!p.getBoolean(ACTIVE, false)) return;
        NotificationManager nm = c.getSystemService(NotificationManager.class);
        try {
            if (nm != null && nm.isNotificationPolicyAccessGranted()) {
                if (Build.VERSION.SDK_INT >= 28)
                    nm.setNotificationPolicy(policy(p.getInt("quiet_cat", 0), p.getInt("quiet_calls", 0), p.getInt("quiet_msgs", 0),
                            p.getInt("quiet_fx", 0), p.getInt("quiet_conv", 0)));
                int f = p.getInt("quiet_filter", NotificationManager.INTERRUPTION_FILTER_ALL);
                nm.setInterruptionFilter(f == NotificationManager.INTERRUPTION_FILTER_UNKNOWN ? NotificationManager.INTERRUPTION_FILTER_ALL : f);
                Hub.log("매크로 중 알림 끄기: 방해 금지 되돌림", 'd');
            }
        } catch (RuntimeException e) {
            Hub.log("방해 금지를 되돌리지 못함 — 휴대폰 설정에서 방해 금지를 확인해 주세요: " + e.getMessage(), 'y');
        } finally {
            p.edit().putBoolean(ACTIVE, false).apply();
        }
    }

    @android.annotation.TargetApi(28)
    static NotificationManager.Policy policy(int cat, int calls, int msgs, int fx, int conv) {
        if (Build.VERSION.SDK_INT >= 30) return new NotificationManager.Policy(cat, calls, msgs, fx, conv);
        return new NotificationManager.Policy(cat, calls, msgs, fx);
    }
}
