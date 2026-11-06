package com.acrux.mobile;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.Handler;
import android.os.Looper;
import android.os.Process;

import com.acrux.mobile.core.Fisher;
import com.acrux.mobile.core.Settings;

import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Date;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.CopyOnWriteArrayList;

/** 앱 전체 상태: 설정 · 자동 낚시 실행 · 기록 · 화면 갱신 알림 */
final class Hub {
    private Hub() {}

    static final Settings cfg = new Settings();
    static final List<String> logs = new ArrayList<>();
    static final List<Runnable> listeners = new CopyOnWriteArrayList<>();
    static final Handler main = new Handler(Looper.getMainLooper());
    static volatile String state = "대기";
    static volatile String mode = "";          // "" · "run" · "cal"
    static volatile Fisher fisher;
    static Thread worker;
    static Context app;
    static boolean loaded;

    static void init(Context c) {
        if (app == null) app = c.getApplicationContext();
        if (!loaded) {
            loaded = true;
            load();
        }
    }

    // ---- 화면 갱신
    static void changed() {
        main.post(() -> {
            for (Runnable r : listeners) r.run();
        });
    }

    static void log(String msg, char color) {
        String t = new SimpleDateFormat("HH:mm:ss", Locale.ROOT).format(new Date());
        synchronized (logs) {
            logs.add(t + "  " + msg);
            while (logs.size() > 200) logs.remove(0);
        }
        changed();
    }

    static List<String> recentLogs(int n) {
        synchronized (logs) {
            return new ArrayList<>(logs.subList(Math.max(0, logs.size() - n), logs.size()));
        }
    }

    // ---- 실행
    static boolean running() {
        Thread w = worker;
        return w != null && w.isAlive();
    }

    /** cal: 자동 보정 · 아니면 자동 낚시 → 못 하면 이유 */
    static synchronized String start(boolean cal) {
        if (running()) return "이미 실행 중";
        if (TapService.instance == null) return "접근성 권한(Acrux Mobile)을 먼저 켜야 함";
        CaptureService cap = CaptureService.active;
        if (cap == null) return "앱에서 [화면 캡처 시작]을 먼저 눌러야 함";
        if (!cal && !cfg.ready()) return "먼저 [보정]을 해야 함 (낚시 자리에서 Fish 버튼이 보일 때)";
        Fisher.Out out = new Fisher.Out() {
            @Override
            public void log(String msg, char color) {
                Hub.log(msg, color);
            }

            @Override
            public void state(String msg) {
                state = msg;
                changed();
            }
        };
        Fisher.Input in = (x, y, ms) -> {
            TapService t = TapService.instance;
            if (t != null) t.tap(x, y, ms);
        };
        Fisher f = new Fisher(cap, in, out, cfg);
        fisher = f;
        mode = cal ? "cal" : "run";
        worker = new Thread(() -> {
            Process.setThreadPriority(Process.THREAD_PRIORITY_DISPLAY);
            try {
                if (cal) {
                    String r = f.calibrate();
                    log("자동 보정: " + r, cfg.ready() ? 'g' : 'y');
                    save();
                } else {
                    f.run();
                }
            } catch (RuntimeException e) {
                log("오류: " + e, 'r');
            } finally {
                mode = "";
                state = "대기";
                changed();
            }
        }, "acrux-fisher");
        worker.start();
        changed();
        return "";
    }

    static void stop() {
        Fisher f = fisher;
        if (f != null) f.stop();
        Thread w = worker;
        if (w != null) w.interrupt();
    }

    // ---- 설정 저장 (이 폰에만)
    static SharedPreferences prefs() {
        return app.getSharedPreferences("acrux", Context.MODE_PRIVATE);
    }

    static String arr(double[] a) {
        if (a == null) return "";
        StringBuilder s = new StringBuilder();
        for (int i = 0; i < a.length; i++) {
            if (i > 0) s.append(',');
            s.append(a[i]);
        }
        return s.toString();
    }

    static double[] arr(String s) {
        if (s == null || s.isEmpty()) return null;
        try {
            String[] p = s.split(",");
            double[] a = new double[p.length];
            for (int i = 0; i < p.length; i++) a[i] = Double.parseDouble(p[i]);
            return a;
        } catch (NumberFormatException e) {
            return null;
        }
    }

    static void load() {
        SharedPreferences p = prefs();
        cfg.panelRegion = arr(p.getString("panel", ""));
        cfg.reelRegion = arr(p.getString("reel", ""));
        cfg.resultRegion = arr(p.getString("result", ""));
        cfg.fishBtn = arr(p.getString("fish", ""));
        cfg.barRegion = arr(p.getString("bar", ""));
        cfg.closePos = arr(p.getString("close", ""));
        cfg.titlePos = arr(p.getString("title", ""));
        cfg.reelDerived = p.getBoolean("derived", true);
        cfg.biteMax = p.getFloat("biteMax", (float) cfg.biteMax);
        cfg.clickMs = p.getInt("clickMs", cfg.clickMs);
        cfg.leadMs = p.getFloat("leadMs", (float) cfg.leadMs);
        cfg.targetPct = p.getFloat("targetPct", (float) cfg.targetPct);
        cfg.adaptive = p.getBoolean("adaptive", cfg.adaptive);
    }

    static void save() {
        prefs().edit()
                .putString("panel", arr(cfg.panelRegion)).putString("reel", arr(cfg.reelRegion)).putString("result", arr(cfg.resultRegion))
                .putString("fish", arr(cfg.fishBtn)).putString("bar", arr(cfg.barRegion))
                .putString("close", arr(cfg.closePos)).putString("title", arr(cfg.titlePos))
                .putBoolean("derived", cfg.reelDerived)
                .putFloat("biteMax", (float) cfg.biteMax).putInt("clickMs", cfg.clickMs)
                .putFloat("leadMs", (float) cfg.leadMs).putFloat("targetPct", (float) cfg.targetPct)
                .putBoolean("adaptive", cfg.adaptive)
                .apply();
    }

    static void resetCalibration() {
        cfg.panelRegion = cfg.reelRegion = cfg.resultRegion = null;
        cfg.fishBtn = cfg.barRegion = cfg.closePos = cfg.titlePos = null;
        cfg.reelDerived = true;
        save();
        changed();
    }
}
