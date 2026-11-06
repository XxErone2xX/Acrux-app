package com.acrux.mobile;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.Handler;
import android.os.Looper;
import android.os.Process;

import com.acrux.mobile.core.Fisher;
import com.acrux.mobile.core.Mover;
import com.acrux.mobile.core.Seller;
import com.acrux.mobile.core.Settings;

import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Date;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;

/** 앱 전체 상태: 설정 · 위치 · 경로 · 실행(자동 낚시 · 보정 · 판매 · 이동 테스트) · 기록 · 화면 갱신 알림 */
final class Hub {
    private Hub() {}

    static final Settings cfg = new Settings();
    /** 지정한 위치 (이름 → 화면 비율 x, y) — 이동 경로 · 판매 보정 */
    static final Map<String, double[]> pos = new ConcurrentHashMap<>();
    static String routeBase = Mover.BASE_DEFAULT, routeFish = Mover.FISH_DEFAULT, routeSell = Mover.SELL_DEFAULT;
    static boolean useMove = true, useSell = true;
    static int sellMax = 100;
    static double sellDelay = 0;

    static final List<String> logs = new ArrayList<>();
    static final List<Runnable> listeners = new CopyOnWriteArrayList<>();
    static final Handler main = new Handler(Looper.getMainLooper());
    static volatile String state = "대기";
    static volatile String mode = "";          // "" · run · cal · sellcal · test_fish · test_sell · sell
    static volatile boolean stopFlag;
    static volatile Fisher fisher;
    static Thread worker;
    static Context app;
    static boolean loaded;
    static int zeroSells;

    static final Seller.Routes ROUTES = new Seller.Routes() {
        @Override
        public String base() {
            return routeBase;
        }

        @Override
        public String fish() {
            return routeFish;
        }

        @Override
        public String sell() {
            return routeSell;
        }
    };

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
            while (logs.size() > 300) logs.remove(0);
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

    static final Fisher.Out OUT = new Fisher.Out() {
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

    static Mover.Gestures gestures() {
        return TapService.GESTURES;
    }

    static Mover mover() {
        Mover m = new Mover(CaptureService.active, gestures(), new MlOcr(), pos, OUT, () -> {
            if (stopFlag || Thread.currentThread().isInterrupted()) throw new Fisher.Stopped();
        });
        return m;
    }

    static Seller seller(Mover mv) {
        Seller s = new Seller(CaptureService.active, mv, new MlOcr(), pos, ROUTES, OUT);
        s.sellMax = sellMax;
        s.sellDelay = sellDelay;
        return s;
    }

    /** m: run 자동 낚시 · cal 낚시 보정 · sellcal 판매 보정 · test_fish / test_sell 이동 테스트 · sell 판매 한 번 → 못 하면 이유 */
    static synchronized String start(String m) {
        if (running()) return "이미 실행 중";
        if (TapService.instance == null) return "접근성 권한(Acrux Mobile)을 먼저 켜야 함";
        CaptureService cap = CaptureService.active;
        if (cap == null) return "앱에서 [화면 캡처 시작]을 먼저 눌러야 함";
        if (m.equals("run") && !cfg.ready()) return "먼저 [보정]을 해야 함 (낚시 자리에서 Fish 버튼이 보일 때)";
        Fisher f = new Fisher(cap, (x, y, ms) -> {
            TapService t = TapService.instance;
            if (t != null) t.tap(x, y, ms);
        }, OUT, cfg);
        Mover mv = mover();
        Seller sl = seller(mv);
        if (m.equals("run")) {
            String why = useMove ? sl.fishMissing() : "꺼져 있음";
            if (why == null) f.onStart = sl::goFish;
            else if (useMove) log("낚시 장소로 이동 안 함 — " + why + " · 지금 자리에서 낚시", 'd');
            zeroSells = 0;
            f.onFull = () -> sellOnce(sl);
        }
        stopFlag = false;
        fisher = f;
        mode = m;
        worker = new Thread(() -> {
            Process.setThreadPriority(Process.THREAD_PRIORITY_DISPLAY);
            try {
                switch (m) {
                    case "cal": {
                        String r = f.calibrate();
                        log("자동 보정: " + r, cfg.ready() ? 'g' : 'y');
                        save();
                        break;
                    }
                    case "sellcal": {
                        List<String> notes = sl.autocal();
                        log("판매 보정 완료" + (notes.isEmpty() ? "" : " · " + String.join(" · ", notes)), 'g');
                        save();
                        break;
                    }
                    case "test_fish":
                        mv.run(routeBase, "기준 장소로");
                        mv.run(routeFish, "낚시 장소로");
                        log("이동 테스트: 낚시 장소 도착", 'g');
                        break;
                    case "test_sell":
                        mv.run(routeBase, "기준 장소로");
                        mv.run(routeSell, "판매 장소로");
                        log("이동 테스트: 판매 장소 도착", 'g');
                        break;
                    case "sell":
                        log("판매 테스트: " + sl.run() + "종류 판매", 'g');
                        break;
                    default:
                        f.run();
                        break;
                }
            } catch (Fisher.Stopped e) {
                log("멈춤", 'd');
            } catch (RuntimeException e) {
                log("오류: " + e.getMessage(), 'r');
                save();
            } finally {
                mode = "";
                state = "대기";
                changed();
            }
        }, "acrux-worker");
        worker.start();
        changed();
        return "";
    }

    /** 인벤토리 가득 → 판매 · 돌아오면 true (PC 와 같음: 두 번 연속 팔 물고기가 없으면 낚시 자리 문제로 보고 멈춤) */
    static boolean sellOnce(Seller sl) {
        if (!useSell) {
            log("인벤토리 가득 — 판매가 꺼져 있음", 'y');
            return false;
        }
        String m = sl.missing();
        if (m != null) {
            log("인벤토리 가득 — 판매 못 함: " + m, 'y');
            return false;
        }
        try {
            int sold = sl.run();
            if (sold > 0) {
                zeroSells = 0;
                return true;
            }
            zeroSells++;
            if (zeroSells >= 2) {
                log("판매하러 갔는데 두 번 연속 팔 물고기가 없음 — 낚시 자리 문제로 보고 멈춤", 'y');
                return false;
            }
            log("판매하러 갔는데 팔 물고기가 없음 — 낚시 자리를 다시 잡고 이어감", 'y');
            return true;
        } catch (Fisher.Stopped e) {
            throw e;
        } catch (RuntimeException e) {
            log("판매 실패: " + e.getMessage(), 'r');
            return false;
        }
    }

    static void stop() {
        stopFlag = true;
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
        routeBase = p.getString("routeBase", Mover.BASE_DEFAULT);
        routeFish = p.getString("routeFish", Mover.FISH_DEFAULT);
        routeSell = p.getString("routeSell", Mover.SELL_DEFAULT);
        useMove = p.getBoolean("useMove", true);
        useSell = p.getBoolean("useSell", true);
        sellMax = p.getInt("sellMax", 100);
        sellDelay = p.getFloat("sellDelay", 0f);
        pos.clear();
        for (String kv : p.getString("pos", "").split(";")) {
            int e = kv.indexOf('=');
            if (e <= 0) continue;
            double[] v = arr(kv.substring(e + 1));
            if (v != null && v.length == 2) pos.put(kv.substring(0, e), v);
        }
    }

    static void save() {
        StringBuilder ps = new StringBuilder();
        for (Map.Entry<String, double[]> e : pos.entrySet()) {
            if (ps.length() > 0) ps.append(';');
            ps.append(e.getKey().replace(";", "").replace("=", "")).append('=').append(arr(e.getValue()));
        }
        prefs().edit()
                .putString("panel", arr(cfg.panelRegion)).putString("reel", arr(cfg.reelRegion)).putString("result", arr(cfg.resultRegion))
                .putString("fish", arr(cfg.fishBtn)).putString("bar", arr(cfg.barRegion))
                .putString("close", arr(cfg.closePos)).putString("title", arr(cfg.titlePos))
                .putBoolean("derived", cfg.reelDerived)
                .putFloat("biteMax", (float) cfg.biteMax).putInt("clickMs", cfg.clickMs)
                .putFloat("leadMs", (float) cfg.leadMs).putFloat("targetPct", (float) cfg.targetPct)
                .putBoolean("adaptive", cfg.adaptive)
                .putString("routeBase", routeBase).putString("routeFish", routeFish).putString("routeSell", routeSell)
                .putBoolean("useMove", useMove).putBoolean("useSell", useSell)
                .putInt("sellMax", sellMax).putFloat("sellDelay", (float) sellDelay)
                .putString("pos", ps.toString())
                .apply();
    }

    static void resetCalibration() {
        cfg.panelRegion = cfg.reelRegion = cfg.resultRegion = null;
        cfg.fishBtn = cfg.barRegion = cfg.closePos = cfg.titlePos = null;
        cfg.reelDerived = true;
        save();
        changed();
    }

    /** 지금 경로들에서 지정해야 하는 위치 이름 (기본 위치 이름 + 경로에 나온 이름) */
    static List<String> positionNames() {
        List<String> out = new ArrayList<>(Mover.POS_HELP.keySet());
        for (String r : new String[]{routeBase, routeFish, routeSell}) {
            try {
                for (String n : Mover.needed(Mover.parse(r))) if (!out.contains(n)) out.add(n);
            } catch (Mover.ParseError ignored) {
            }
        }
        for (String n : new String[]{"리셋버튼", "리셋확인"}) if (!out.contains(n)) out.add(n);
        return out;
    }

    /** 경로 끝에 한 줄 더하기 (지점 추가) */
    static void appendRoute(boolean sell, String line) {
        String r = sell ? routeSell : routeFish;
        r = (r.endsWith("\n") || r.isEmpty() ? r : r + "\n") + line;
        if (sell) routeSell = r;
        else routeFish = r;
        save();
        changed();
    }
}
