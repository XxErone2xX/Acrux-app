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
    static boolean autoUpdate = true;
    static boolean useMove = true, useSell = true, relDrag = false;
    /** 로블록스 클래식 조이스틱 {가운데 x / 화면 너비, 가운데 y / 화면 높이, 반지름 / 화면 높이} (못 찾았으면 null) · W A S D 를 조이스틱 터치로 */
    static volatile double[] stick;
    static boolean useStick = true;
    static int sellMax = 100;
    static double sellDelay = 0;

    static final List<String> logs = new ArrayList<>();
    static final List<Runnable> listeners = new CopyOnWriteArrayList<>();
    static final Handler main = new Handler(Looper.getMainLooper());
    static volatile String state = "대기";
    /** 마지막 실행이 멈춘 이유 (떠 있는 창에도 보이게) */
    static volatile String lastError;
    static volatile String mode = "";
    static volatile boolean stopFlag, measuring, arrived, appVisible;
    static volatile Fisher fisher;
    static Thread worker;
    static Context app;
    static boolean loaded;
    static int zeroSells;

    static void init(Context c) {
        if (app == null) app = c.getApplicationContext();
        if (!loaded) {
            loaded = true;
            loadLang();
            load();
            Online.start();
        }
    }

    // ---- 언어 (PC 와 같은 사전 + 모바일 사전 · 한국어 글자가 열쇠)
    static void loadLang() {
        for (String f : new String[]{"i18n/pc.tsv", "i18n/mobile.tsv"}) {
            try (java.io.InputStream in = app.getAssets().open(f)) {
                com.acrux.mobile.core.I18n.load(in);
            } catch (java.io.IOException ignored) {
            }
        }
        String l = prefs().getString("lang", null);
        if (l == null) l = com.acrux.mobile.core.I18n.guess(Locale.getDefault().getLanguage());
        com.acrux.mobile.core.I18n.setLang(l);
    }

    // ---- 낚시 통계: 이번 실행 (앱을 켠 뒤) · 전체 (저장) — PC 의 '이번 실행 · 올타임'
    static final String[] STAT_KEYS = {"success", "junk", "fail", "sells", "sold"};
    static final long[] statRun = new long[STAT_KEYS.length];
    static long runStart = System.currentTimeMillis();

    static long statAll(int i) {
        return prefs().getLong("stat_" + STAT_KEYS[i], 0);
    }

    static synchronized void stat(String key, long add) {
        for (int i = 0; i < STAT_KEYS.length; i++) {
            if (!STAT_KEYS[i].equals(key)) continue;
            statRun[i] += add;
            prefs().edit().putLong("stat_" + key, statAll(i) + add).apply();
        }
        changed();
    }

    static synchronized void resetStats(boolean all) {
        java.util.Arrays.fill(statRun, 0);
        runStart = System.currentTimeMillis();
        if (all) {
            SharedPreferences.Editor e = prefs().edit();
            for (String k : STAT_KEYS) e.remove("stat_" + k);
            e.putLong("stat_since", System.currentTimeMillis()).apply();
        }
        changed();
    }

    /** 떠 있는 Acrux 창 켜기 · 끄기 (매크로를 안 돌릴 때) — 끄면 돌던 매크로도 멈춤 (창이 없으면 게임에서 멈출 수 없어서) */
    static boolean uiOn() {
        return prefs().getBoolean("panelOn", true);
    }

    static void setUi(boolean on) {
        prefs().edit().putBoolean("panelOn", on).apply();
        if (!on && running()) stop();
        TapService t = TapService.instance;
        if (t != null) t.updateVisible();
        log(on ? "UI 켬 — 떠 있는 Acrux 창 보임" : "UI 끔 — 떠 있는 Acrux 창 숨김", 'd');
        changed();
    }

    /** 언어 바꾸기 → 앱 화면 · 떠 있는 창을 다시 그림 */
    static void setLang(String code) {
        com.acrux.mobile.core.I18n.setLang(code);
        prefs().edit().putString("lang", code).apply();
        TapService t = TapService.instance;
        if (t != null) t.rebuildPanel();
        for (Runnable r : langListeners) r.run();
        changed();
    }

    static final List<Runnable> langListeners = new CopyOnWriteArrayList<>();

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

    /** 화면에 보일 기록 (시각은 두고 내용만 지금 언어로) */
    static List<String> recentLogsShown(int n) {
        List<String> out = new ArrayList<>();
        for (String l : recentLogs(n)) {
            int k = l.indexOf("  ");
            out.add(k > 0 ? l.substring(0, k + 2) + com.acrux.mobile.core.I18n.t(l.substring(k + 2)) : l);
        }
        return out;
    }

    static void clearLogs() {
        synchronized (logs) {
            logs.clear();
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
        lastError = null;
        // 새로 실행할 때는 리셋을 꼭 함 ('방금 리셋함' 은 한 실행 안에서만 — 그 사이 손으로 움직였을 수 있음)
        Mover.markMoved();
        if (TapService.instance == null) return "접근성 권한(Acrux Mobile)을 먼저 켜야 함";
        CaptureService cap = CaptureService.active;
        if (cap == null) return "앱에서 [화면 캡처 시작]을 먼저 눌러야 함";
        if (m.equals("run") && !cfg.ready()) return "먼저 [보정]을 해야 함 (낚시 자리에서 Fish 버튼이 보일 때)";
        Pc shz = ShizukuPc.ready() ? new ShizukuPc(relDrag) : null;
        double[] st = stick;
        // W A S D 는 조이스틱 터치로 (조이스틱을 찾아 뒀으면) · 나머지 키 · 마우스는 Shizuku
        Pc pc = useStick && st != null ? new StickPc(shz, st, cap.width(), cap.height()) : shz;
        if (m.equals("sticktest") && st == null) return "먼저 조이스틱 찾기 (로블록스 설정 → 이동 방식: 조이스틱 · ⚙ → 조이스틱 찾기)";
        boolean needKeys = !m.equals("run") && !m.equals("cal") && !m.startsWith("stick");
        if (needKeys && pc == null) return "이동 · 판매는 조이스틱 찾기 또는 Shizuku 연결이 필요함 — ⚙ → 조이스틱 찾기";
        Fisher f = new Fisher(cap, fishInput(shz), OUT, cfg);
        Mover mv = pc != null ? new Mover(cap, pc, new MlOcr(), pos, move, OUT, CHECK) : null;
        if (mv != null) mv.touch = fishInput(null);         // 재설정 · 도감 버튼은 손가락 탭으로
        Seller sl = mv != null ? seller(cap, mv, pc) : null;
        // 판매 한 번: 필요한 위치 · 지점이 없으면 시작 전에 바로 알려 줌 (예전엔 시작하자마자 멈추고 기록 칸에만 남았음)
        if (m.equals("sell") && sl != null) {
            String miss = sl.missing();
            if (miss != null) return miss;
        }
        if (m.equals("run")) {
            if (!useMove) {
                log("낚시 장소로 이동 꺼짐 — 지금 자리에서 낚시", 'd');
            } else if (sl == null) {
                log("Shizuku 가 연결 안 됨 — 이동 · 판매 없이 지금 자리에서 낚시", 'y');
            } else {
                String why = sl.fishMissing();
                if (why == null) f.onStart = sl::goFish;
                else log("낚시 장소로 이동 안 함 — " + why + " · 지금 자리에서 낚시", 'd');
            }
            zeroSells = 0;
            f.onFull = () -> sellOnce(sl);
        }
        TapService ts = TapService.instance;
        if (ts != null && ts.coversGame() && (m.equals("cal") || m.equals("run")))
            log("Acrux 창이 낚시 창 쪽을 가리고 있음 — 위쪽 구석으로 옮겨 주세요 (화면 캡처에 같이 찍혀서 인식이 틀릴 수 있음)", 'y');
        stopFlag = false;
        f.onResult = kind -> stat(kind, 1);
        fisher = f;
        mode = m;
        worker = new Thread(() -> {
            Process.setThreadPriority(Process.THREAD_PRIORITY_DISPLAY);
            try {
                // 시작하고 1초 기다림 — [시작]을 누른 손가락이 떨어지고 게임 화면으로 돌아온 뒤에 키 · 클릭을 보냄
                state = "1초 뒤 시작";
                changed();
                long t0 = System.nanoTime();
                while (System.nanoTime() - t0 < 1_000_000_000L) {
                    CHECK.run();
                    Pc.nap(20);
                }
                if (m.equals("keytest")) keyTest(pc);
                else if (m.equals("stickfind")) stickFind(cap);
                else if (m.equals("sticktest")) stickTest(new StickPc(null, st, cap.width(), cap.height()));
                else work(m, f, mv, sl);
            } catch (Fisher.Stopped e) {
                log("멈춤", 'd');
            } catch (RuntimeException e) {
                lastError = e.getMessage();
                log("오류: " + e.getMessage(), 'r');
            } catch (OutOfMemoryError e) {
                lastError = "메모리 부족 — 멈춤 (다른 앱을 닫고 다시 시작)";
                log(lastError, 'r');
                System.gc();
            } catch (Throwable e) {
                lastError = String.valueOf(e);
                log("오류: " + e, 'r');
                Crash.save(e, "worker");
            } finally {
                if (mv != null) mv.releaseKeys();
                if (pc instanceof StickPc) ((StickPc) pc).releaseAll();
                if (m.startsWith("stick")) Joy.set(null);
                measuring = false;
                mode = "";
                state = lastError != null ? "멈춤 — " + lastError : "대기";     // 멈춘 이유를 떠 있는 창에도
                String why = lastError;
                if (why != null) main.post(() -> android.widget.Toast.makeText(app, com.acrux.mobile.core.I18n.t("멈춤 — " + why), android.widget.Toast.LENGTH_LONG).show());
                save();
                changed();
            }
        }, "acrux-worker");
        worker.start();
        changed();
        return "";
    }

    static Seller seller(CaptureService cap, Mover mv, Pc pc) {
        Seller s = new Seller(cap, mv, pc, new MlOcr(), pos, sellPts, fishPts, OUT);
        s.sellMax = sellMax;
        s.sellDelay = sellDelay;
        s.touch = fishInput(null);                 // 마우스 클릭이 씹히면 손가락 탭으로
        return s;
    }

    /** 키 테스트: W 를 1초 동안 누르고 있음 (진짜 키보드처럼 계속 '누름' 신호) */
    static void keyTest(Pc pc) {
        state = "키 테스트 — W 1초";
        changed();
        long end = System.nanoTime() + 1_000_000_000L;
        try {
            while (System.nanoTime() < end) {
                CHECK.run();
                pc.keyDown("w");
                Pc.nap(33);
            }
        } finally {
            pc.keyUp("w");
        }
        String km = ShizukuPc.keyMode();
        log("키 테스트: W 1초 보냄 — 캐릭터가 앞으로 갔으면 Shizuku 키 입력이 됨", 'g');
        log(km.startsWith("uhid") ? "키보드: 진짜 키보드 장치(Acrux Keyboard)로 보냄" : "키보드: 키보드 장치를 못 만듦 → 입력 주입으로 보냄 · " + km, km.startsWith("uhid") ? 'd' : 'y');
    }

    /** 조이스틱 찾기: 지금 화면에서 왼쪽 아래 둥근 조이스틱을 찾아 저장 · 찾은 자리에 원을 잠깐 그려 줌 */
    static void stickFind(CaptureService cap) {
        state = "조이스틱 찾는 중";
        changed();
        int w = cap.width(), h = cap.height();
        com.acrux.mobile.core.Img im = cap.grab(0, 0, w, h);
        if (im == null) throw new RuntimeException("화면을 아직 못 받음 — 잠깐 뒤 다시");
        com.acrux.mobile.core.Stick.Found fd = com.acrux.mobile.core.Stick.find(im, 20);
        if (fd == null)
            throw new RuntimeException("조이스틱을 못 찾음 — 로블록스 설정에서 이동 방식을 '조이스틱'(클래식)으로 바꾸고, 조이스틱이 보일 때 다시 (안 되면 ⚙ → 조이스틱 직접 지정)");
        double sx = (double) w / im.w, sy = (double) h / im.h;
        stick = new double[]{fd.cx * sx / w, fd.cy * sy / h, fd.r * sy / h};
        save();
        log("조이스틱 찾음: 가운데 (" + Math.round(fd.cx * sx) + ", " + Math.round(fd.cy * sy) + ") · 반지름 " + Math.round(fd.r * sy) + "px · 점수 " + Math.round(fd.score), 'g');
        showStick(3000);
    }

    /** 저장된 조이스틱 자리에 원을 잠깐 그려 줌 (맞게 찾았는지 눈으로 확인) */
    static void showStick(long ms) {
        double[] st = stick;
        main.post(() -> {
            TapService t = TapService.instance;
            if (t != null && st != null) t.showRing(st, ms);
        });
    }

    /** 조이스틱 테스트: W → A → S → D 를 0.8초씩 (조이스틱을 위 · 왼쪽 · 아래 · 오른쪽으로) */
    static void stickTest(StickPc sp) {
        showStick(1500);
        for (String k : new String[]{"w", "a", "s", "d"}) {
            state = "조이스틱 테스트 — " + k.toUpperCase();
            changed();
            sp.keyDown(k);
            long end = System.nanoTime() + 800_000_000L;
            while (System.nanoTime() < end) {
                CHECK.run();
                Pc.nap(20);
            }
            sp.keyUp(k);
            Pc.nap(400);
        }
        sp.releaseAll();
        log("조이스틱 테스트: W · A · S · D 0.8초씩 보냄 — 캐릭터가 앞 · 왼쪽 · 뒤 · 오른쪽으로 갔으면 성공", 'g');
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
            int sold = sl.run();
            stat("sells", 1);
            stat("sold", sold);
            log("판매 테스트: " + sold + "종류 판매", 'g');
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
        if (sl == null) {
            log("인벤토리 가득 — 판매는 Shizuku 연결이 필요함", 'y');
            return false;
        }
        String m = sl.missing();
        if (m != null) {
            log("인벤토리 가득 — 판매 못 함: " + m, 'y');
            return false;
        }
        try {
            int sold = sl.run();
            stat("sells", 1);
            stat("sold", sold);
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
        cfg.tapAbove = p.getBoolean("tapAbove", true);
        autoUpdate = p.getBoolean("autoUpdate", true);
        useMove = p.getBoolean("useMove", true);
        useSell = p.getBoolean("useSell", true);
        relDrag = p.getBoolean("relDrag", false);
        sellMax = p.getInt("sellMax", 100);
        sellDelay = p.getFloat("sellDelay", 0f);
        move.resetWait = p.getFloat("m_reset_wait", (float) move.resetWait);
        // V0.9.11 기준 장소 걷기가 바뀜 (W+A → A → A+W → 쉼 → D) — 예전 값(m_w_time …)은 안 씀
        move.waTime = p.getFloat("m2_wa", (float) move.waTime);
        move.aTime = p.getFloat("m2_a", (float) move.aTime);
        move.awTime = p.getFloat("m2_aw", (float) move.awTime);
        move.pauseTime = p.getFloat("m2_pause", (float) move.pauseTime);
        move.dTime = p.getFloat("m2_d", (float) move.dTime);
        move.oTime = p.getFloat("m_o_time", (float) move.oTime);
        move.margin = p.getFloat("m_margin", (float) move.margin);
        move.tiltPx = p.getInt("m_tilt_px", move.tiltPx);
        move.button = p.getString("m_button", move.button);
        stick = arr(p.getString("stick", ""));
        if (stick != null && stick.length != 3) stick = null;
        useStick = p.getBoolean("useStick", true);
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
                .putBoolean("adaptive", cfg.adaptive).putBoolean("tapAbove", cfg.tapAbove)
                .putBoolean("autoUpdate", autoUpdate).putBoolean("useMove", useMove).putBoolean("useSell", useSell).putBoolean("relDrag", relDrag)
                .putInt("sellMax", sellMax).putFloat("sellDelay", (float) sellDelay)
                .putFloat("m_reset_wait", (float) move.resetWait)
                .putFloat("m2_wa", (float) move.waTime).putFloat("m2_a", (float) move.aTime).putFloat("m2_aw", (float) move.awTime)
                .putFloat("m2_pause", (float) move.pauseTime).putFloat("m2_d", (float) move.dTime)
                .putFloat("m_o_time", (float) move.oTime).putFloat("m_margin", (float) move.margin)
                .putInt("m_tilt_px", move.tiltPx).putString("m_button", move.button)
                .putString("pos", ps.toString())
                .putString("stick", stick == null ? "" : arr(stick)).putBoolean("useStick", useStick)
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
