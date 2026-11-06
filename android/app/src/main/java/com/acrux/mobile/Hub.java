package com.acrux.mobile;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.Handler;
import android.os.Looper;
import android.os.Process;

import com.acrux.mobile.core.Fisher;
import com.acrux.mobile.core.Mover;
import com.acrux.mobile.core.Pc;
import com.acrux.mobile.core.Seller;
import com.acrux.mobile.core.Settings;
import com.acrux.mobile.core.TouchPc;

import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Date;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/** 앱 전체 상태 (PC 버전과 같은 설정): 자동 낚시 · 공통 위치 · 낚시 장소 / 판매 장소 지점 · 이동 시간 · 판매 위치 · 실행 · 기록 */
final class Hub {
    private Hub() {}

    static final Settings cfg = new Settings();
    /** 위치 (화면 비율): 공통 위치(chat_pos · collection_pos · collection_close) + 판매 자동 보정(dialog_pos …) */
    static final Map<String, double[]> pos = new ConcurrentHashMap<>();
    static final Mover.Cfg move = new Mover.Cfg();
    static final List<Mover.Point> fishPts = new CopyOnWriteArrayList<>(), sellPts = new CopyOnWriteArrayList<>();
    static boolean useMove = true, useSell = true, relDrag = false, imeKeys = false;
    static int touchTapMs = 120;
    static double touchTilt = 0.5;
    static int sellMax = 100;
    static double sellDelay = 0;

    static final List<String> logs = new ArrayList<>();
    static final List<Runnable> listeners = new CopyOnWriteArrayList<>();
    static final Handler main = new Handler(Looper.getMainLooper());
    static volatile String state = "대기";
    static volatile String mode = "";
    static volatile boolean stopFlag, measuring, arrived, touchMode;
    static volatile Fisher fisher;
    static Thread worker;
    static Context app;
    static boolean loaded;
    static int zeroSells;

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

    static final Runnable CHECK = () -> {
        if (stopFlag || Thread.currentThread().isInterrupted()) throw new Fisher.Stopped();
    };

    // ---- 실행
    static boolean running() {
        Thread w = worker;
        return w != null && w.isAlive();
    }

    /** 자동 낚시가 누르는 입력: Shizuku 가 연결돼 있으면 PC 처럼 마우스 클릭 · 아니면 화면 터치 */
    static Fisher.Input fishInput(Pc pc) {
        if (pc == null) {
            return (x, y, ms) -> {
                TapService t = TapService.instance;
                if (t != null) t.tap(x, y, Math.max(ms, 40));
            };
        }
        return new Fisher.Input() {
            int hx = -1, hy = -1;

            @Override
            public void tap(int x, int y, int ms) {
                if (x == hx && y == hy) {
                    pc.clickHere("left", ms);           // 릴링: 마우스를 안 움직이고 지금 자리에서 (PC mouse_click_here)
                } else {
                    pc.click(x, y, "left");
                    hx = x;
                    hy = y;
                }
            }

            @Override
            public void hover(int x, int y) {
                pc.moveTo(x, y);
                hx = x;
                hy = y;
            }
        };
    }

    /** m: run 자동 낚시 · cal 낚시 자동 보정 · sellcal 판매 자동 보정 · sell 판매 한 번 · base / fish / sellgo 이동 테스트
     *  · addpt_fish / addpt_sell 지점 추가 · measure_fish:N / measure_sell:N 시간 다시 재기 → 못 하면 이유 */
    static synchronized String start(String m) {
        if (running()) return "이미 실행 중";
        if (TapService.instance == null) return "접근성 권한(Acrux Mobile)을 먼저 켜야 함";
        CaptureService cap = CaptureService.active;
        if (cap == null) return "앱에서 [화면 캡처 시작]을 먼저 눌러야 함";
        if (m.equals("run") && !cfg.ready()) return "먼저 [보정]을 해야 함 (낚시 자리에서 Fish 버튼이 보일 때)";
        boolean shz = ShizukuPc.ready();
        Pc pc = shz ? new ShizukuPc(relDrag) : touchPc(cap);
        touchMode = !shz;
        if (!shz && !m.equals("run") && !m.equals("cal")) log("Shizuku 없음 — 터치 모드 (조이스틱 · 탭 · 핀치" + (imeKeys ? " · 키보드 앱 방식 키" : "") + ")", 'd');
        if (!shz && (m.equals("sell") || m.equals("sellgo")) && !pos.containsKey("interact_pos"))
            return "터치 모드: 상호작용 버튼 위치를 먼저 지정 (⚙ → 터치 조작)";
        Fisher f = new Fisher(cap, fishInput(shz ? pc : null), OUT, cfg);
        Mover mv = new Mover(cap, pc, new MlOcr(), pos, move, OUT, CHECK);
        Seller sl = seller(cap, mv, pc);
        if (m.equals("run")) {
            if (!useMove) {
                log("낚시 장소로 이동 꺼짐 — 지금 자리에서 낚시", 'd');
            } else {
                String why = sl.fishMissing();
                if (why == null) f.onStart = sl::goFish;
                else log("낚시 장소로 이동 안 함 — " + why + " · 지금 자리에서 낚시", 'd');
            }
            zeroSells = 0;
            f.onFull = () -> sellOnce(sl);
        }
        stopFlag = false;
        fisher = f;
        mode = m;
        worker = new Thread(() -> {
            Process.setThreadPriority(Process.THREAD_PRIORITY_DISPLAY);
            try {
                work(m, f, mv, sl);
            } catch (Fisher.Stopped e) {
                log("멈춤", 'd');
            } catch (RuntimeException e) {
                log("오류: " + e.getMessage(), 'r');
            } finally {
                if (mv != null) mv.releaseKeys();
                measuring = false;
                mode = "";
                state = "대기";
                save();
                changed();
            }
        }, "acrux-worker");
        worker.start();
        changed();
        return "";
    }

    /** Shizuku 가 없을 때: PC 키 · 마우스 → 로블록스 모바일 터치 */
    static TouchPc touchPc(CaptureService cap) {
        TouchPc t = new TouchPc(TOUCH, new MlOcr(), pos, cap);
        t.tapMs = touchTapMs;
        t.tiltScale = touchTilt;
        t.out = OUT;
        if (imeKeys) t.keys = (k, down) -> {
            TapService s = TapService.instance;
            return s != null && s.imeKey(k, down);
        };
        return t;
    }

    static final TouchPc.Touch TOUCH = new TouchPc.Touch() {
        @Override
        public void tap(int x, int y, int ms) {
            TapService t = TapService.instance;
            if (t != null) t.tapWait(x, y, ms);
        }

        @Override
        public void swipe(int x1, int y1, int x2, int y2, int ms) {
            TapService t = TapService.instance;
            if (t != null) t.swipe(x1, y1, x2, y2, ms);
        }

        @Override
        public void pinch(int cx, int cy, int far, int near, int ms) {
            TapService t = TapService.instance;
            if (t != null) t.pinch(cx, cy, far, near, ms);
        }

        @Override
        public void stick(int cx, int cy, int[] to) {
            TapService t = TapService.instance;
            if (t != null) t.stick(cx, cy, to);
        }
    };

    static Seller seller(CaptureService cap, Mover mv, Pc pc) {
        Seller s = new Seller(cap, mv, pc, new MlOcr(), pos, sellPts, fishPts, OUT);
        s.sellMax = sellMax;
        s.sellDelay = sellDelay;
        return s;
    }

    static void work(String m, Fisher f, Mover mv, Seller sl) {
        if (m.equals("run")) {
            f.run();
        } else if (m.equals("cal")) {
            String r = f.calibrate();
            log("자동 보정: " + r, cfg.ready() ? 'g' : 'y');
        } else if (m.equals("sellcal")) {
            List<String> notes = sl.autocal();
            log("판매 자동 보정 완료" + (notes.isEmpty() ? "" : " · " + String.join(" · ", notes)), 'g');
        } else if (m.equals("sell")) {
            log("판매 테스트: " + sl.run() + "종류 판매", 'g');
        } else if (m.equals("base")) {
            mv.goBase();
            log("기준 장소 도착", 'g');
        } else if (m.equals("fish")) {
            String why = Mover.pathMissing("낚시 장소", fishPts, null);
            if (why != null || fishPts.isEmpty()) throw new RuntimeException(why != null ? why : "낚시 장소 지점이 없음");
            mv.goPlace("낚시 장소", fishPts);
        } else if (m.equals("sellgo")) {
            String why = Mover.pathMissing("물고기 판매 장소", sellPts, null);
            if (why != null || sellPts.isEmpty()) throw new RuntimeException(why != null ? why : "판매 장소 지점이 없음");
            mv.goPlace("물고기 판매 장소", sellPts);
        } else if (m.startsWith("addpt_")) {
            addPoint(mv, m.endsWith("sell"));
        } else if (m.startsWith("measure_")) {
            boolean sell = m.startsWith("measure_sell");
            int j = Integer.parseInt(m.substring(m.indexOf(':') + 1));
            List<Mover.Point> pts = sell ? sellPts : fishPts;
            String why = Mover.pathMissing(sell ? "물고기 판매 장소" : "낚시 장소", pts, j);
            if (why != null) throw new RuntimeException(why);
            measure(mv, sell, j, true);
        }
    }

    /** 지점 추가 (PC 와 같음): 기준 장소 → 앞 지점들까지 간 화면에서 다음 지점을 누름 → 우클릭으로 걸어가며 시간 잼 */
    static void addPoint(Mover mv, boolean sell) {
        List<Mover.Point> pts = sell ? sellPts : fishPts;
        String name = sell ? "물고기 판매 장소" : "낚시 장소";
        String why = Mover.pathMissing(name, pts, null);
        if (why != null) throw new RuntimeException(why + " — 먼저 그 지점 시간을 다시 재 주세요");
        mv.goBase();
        mv.walk(name, pts, null);
        state = name + " · " + (pts.size() + 1) + "번 지점 — 갈 곳을 누르세요";
        changed();
        double[] at = requestPick(name + " " + (pts.size() + 1) + "번 지점 (Click to Move 로 갈 곳)");
        if (at == null) throw new Fisher.Stopped();
        pts.add(new Mover.Point(at, null));
        save();
        measure(mv, sell, pts.size() - 1, false);
    }

    static void measure(Mover mv, boolean sell, int j, boolean fromBase) {
        arrived = false;
        measuring = true;
        changed();
        try {
            mv.measure(sell ? "물고기 판매 장소" : "낚시 장소", sell ? sellPts : fishPts, j, fromBase, () -> arrived);
        } finally {
            measuring = false;
            save();
            changed();
        }
    }

    /** 로블록스 화면 위에서 위치 고르기 (일하는 스레드에서 기다림) → 화면 비율 [x, y] · 취소하면 null */
    static double[] requestPick(String label) {
        final double[][] got = new double[1][];
        CountDownLatch done = new CountDownLatch(1);
        main.post(() -> {
            TapService t = TapService.instance;
            if (t == null) {
                done.countDown();
                return;
            }
            t.pick(label, at -> {
                got[0] = at;
                done.countDown();
            });
        });
        while (true) {
            CHECK.run();
            try {
                if (done.await(100, TimeUnit.MILLISECONDS)) return got[0];
            } catch (InterruptedException e) {
                throw new Fisher.Stopped();
            }
        }
    }

    /** 인벤토리 가득 → 판매 · 돌아오면 true (PC 와 같음: 두 번 연속 팔 물고기가 없으면 낚시 자리 문제로 보고 멈춤) */
    static boolean sellOnce(Seller sl) {
        if (!useSell) {
            log("인벤토리 가득 — 판매가 꺼져 있음", 'y');
            return false;
        }
        if (touchMode && !pos.containsKey("interact_pos")) {
            log("인벤토리 가득 — 터치 모드 판매는 상호작용 버튼 위치가 필요함 (⚙ → 터치 조작)", 'y');
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

    static String pts(List<Mover.Point> l) {
        StringBuilder s = new StringBuilder();
        for (Mover.Point p : l) {
            if (s.length() > 0) s.append(';');
            s.append(arr(p.pos)).append('|').append(p.time == null ? "" : String.valueOf(p.time));
        }
        return s.toString();
    }

    static void pts(String s, List<Mover.Point> into) {
        into.clear();
        if (s == null || s.isEmpty()) return;
        for (String part : s.split(";")) {
            String[] q = part.split("\\|", -1);
            double[] at = arr(q[0]);
            if (at == null || at.length != 2) continue;
            Double t = null;
            try {
                if (q.length > 1 && !q[1].isEmpty()) t = Double.parseDouble(q[1]);
            } catch (NumberFormatException ignored) {
            }
            into.add(new Mover.Point(at, t));
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
        useMove = p.getBoolean("useMove", true);
        useSell = p.getBoolean("useSell", true);
        relDrag = p.getBoolean("relDrag", false);
        imeKeys = p.getBoolean("imeKeys", false);
        touchTapMs = p.getInt("touchTapMs", 120);
        touchTilt = p.getFloat("touchTilt", 0.5f);
        sellMax = p.getInt("sellMax", 100);
        sellDelay = p.getFloat("sellDelay", 0f);
        move.resetWait = p.getFloat("m_reset_wait", (float) move.resetWait);
        move.wTime = p.getFloat("m_w_time", (float) move.wTime);
        move.waTime = p.getFloat("m_wa_time", (float) move.waTime);
        move.aTime = p.getFloat("m_a_time", (float) move.aTime);
        move.w2Time = p.getFloat("m_w2_time", (float) move.w2Time);
        move.sdTime = p.getFloat("m_sd_time", (float) move.sdTime);
        move.oTime = p.getFloat("m_o_time", (float) move.oTime);
        move.margin = p.getFloat("m_margin", (float) move.margin);
        move.tiltPx = p.getInt("m_tilt_px", move.tiltPx);
        move.button = p.getString("m_button", move.button);
        pos.clear();
        for (String kv : p.getString("pos", "").split(";")) {
            int e = kv.indexOf('=');
            if (e <= 0) continue;
            double[] v = arr(kv.substring(e + 1));
            if (v != null && v.length == 2) pos.put(kv.substring(0, e), v);
        }
        pts(p.getString("fishPts", ""), fishPts);
        pts(p.getString("sellPts", ""), sellPts);
    }

    static void save() {
        StringBuilder ps = new StringBuilder();
        for (Map.Entry<String, double[]> e : pos.entrySet()) {
            if (ps.length() > 0) ps.append(';');
            ps.append(e.getKey()).append('=').append(arr(e.getValue()));
        }
        prefs().edit()
                .putString("panel", arr(cfg.panelRegion)).putString("reel", arr(cfg.reelRegion)).putString("result", arr(cfg.resultRegion))
                .putString("fish", arr(cfg.fishBtn)).putString("bar", arr(cfg.barRegion))
                .putString("close", arr(cfg.closePos)).putString("title", arr(cfg.titlePos))
                .putBoolean("derived", cfg.reelDerived)
                .putFloat("biteMax", (float) cfg.biteMax).putInt("clickMs", cfg.clickMs)
                .putFloat("leadMs", (float) cfg.leadMs).putFloat("targetPct", (float) cfg.targetPct)
                .putBoolean("adaptive", cfg.adaptive)
                .putBoolean("useMove", useMove).putBoolean("useSell", useSell).putBoolean("relDrag", relDrag).putBoolean("imeKeys", imeKeys)
                .putInt("touchTapMs", touchTapMs).putFloat("touchTilt", (float) touchTilt)
                .putInt("sellMax", sellMax).putFloat("sellDelay", (float) sellDelay)
                .putFloat("m_reset_wait", (float) move.resetWait).putFloat("m_w_time", (float) move.wTime)
                .putFloat("m_wa_time", (float) move.waTime).putFloat("m_a_time", (float) move.aTime)
                .putFloat("m_w2_time", (float) move.w2Time).putFloat("m_sd_time", (float) move.sdTime)
                .putFloat("m_o_time", (float) move.oTime).putFloat("m_margin", (float) move.margin)
                .putInt("m_tilt_px", move.tiltPx).putString("m_button", move.button)
                .putString("pos", ps.toString())
                .putString("fishPts", pts(fishPts)).putString("sellPts", pts(sellPts))
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
