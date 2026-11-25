package com.acrux.mobile;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.Handler;
import android.os.Looper;
import android.os.Process;

import com.acrux.mobile.core.Fisher;
import com.acrux.mobile.core.ItemUser;
import com.acrux.mobile.core.Mover;
import com.acrux.mobile.core.Route;
import com.acrux.mobile.core.Touch;
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
    /** 위치 (화면 비율): 공통 위치(menu_btn · reset_btn · reset_confirm · collection_pos · collection_close) + 대화 버튼 + 판매 자동 보정(dialog_pos …) */
    static final Map<String, double[]> pos = new ConcurrentHashMap<>();
    static final Mover.Cfg move = new Mover.Cfg();
    /** 기준 장소 → 낚시 장소 · 판매 장소: 플레이어가 조이스틱으로 직접 걸어간 길 */
    static volatile Route fishRoute = new Route(), sellRoute = new Route();
    static boolean autoUpdate = true;
    /** 디버깅 모드: 모든 기능 (자동 낚시 · 보정 · 판매 · 이동 · 길 기록 · 조이스틱 · 바이옴 매크로) 을 돌릴 때 매크로가 보는 칸 · 읽은 것 · 누른 자리를 화면에 그림 (BoxOverlay) */
    static volatile boolean debug;
    /** 이번 실행이 끝난 결과 한 줄 (디버깅 화면 표시용 · endWarn: 잘 안 됨) · 이번 실행의 마지막 빨간 기록 (멈춘 이유) */
    static volatile String endNote, lastRed;
    static volatile boolean endWarn;
    /** 로블록스 클래식 조이스틱 {가운데 x / 화면 너비, 가운데 y / 화면 높이, 반지름 / 화면 높이} (못 찾았으면 null) · W A S D 를 조이스틱 터치로 */
    static volatile double[] stick;
    static int sellMax = 100;
    static double sellDelay = 0;
    /** 오토 아이템 사용 (설정 + 마지막으로 쓴 시각 · 앱이 켜져 있는 동안 그대로) */
    static final ItemUser items = new ItemUser();

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
    /** 판매하러 갔는데 팔 물고기가 없었던 횟수 (한 판이라도 끝나면 0) · 그래서 판매하러 가지 않고 넘긴 횟수 */
    static int zeroSells, sellSkips;

    static void init(Context c) {
        if (app == null) app = c.getApplicationContext();
        if (!loaded) {
            loaded = true;
            loadLang();
            load();
            Online.start();
            DiscordLink.start();
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
    static final java.util.concurrent.atomic.AtomicBoolean refreshQueued = new java.util.concurrent.atomic.AtomicBoolean();
    static void changed() {
        if (listeners.isEmpty() || !refreshQueued.compareAndSet(false, true)) return;
        main.post(() -> {
            refreshQueued.set(false);
            for (Runnable r : listeners) r.run();
        });
    }

    static void log(String msg, char color) {
        if (color == 'r' && running()) lastRed = msg;
        String t = new SimpleDateFormat("HH:mm:ss", Locale.ROOT).format(new Date());
        synchronized (logs) {
            logs.add(t + "  " + msg);
            logRevision++;
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

    static volatile long logRevision;

    static void clearLogs() {
        synchronized (logs) {
            logs.clear();
            logRevision++;
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

    /** 자동 낚시가 누르는 입력: 화면 터치 */
    static Fisher.Input fishInput() {
        return new Fisher.Input() {
            @Override
            public void tap(int x, int y, int ms) {
                TapService t = TapService.instance;
                if (t != null) t.tap(x, y, Math.max(ms, 40));
            }

            @Override
            public long cancelled() {
                return TapService.cancelled.get();
            }
        };
    }

    /** 낚시가 누르는 입력: 화면 터치 + 누른 자리를 화면에 잠깐 표시 (낚시 테스트 · 디버깅 모드로 칸을 띄웠을 때만 보임) */
    static Fisher.Input testInput() {
        Fisher.Input in = fishInput();
        return new Fisher.Input() {
            @Override
            public void tap(int x, int y, int ms) {
                BoxOverlay.tapped(x, y);
                in.tap(x, y, ms);
            }

            @Override
            public long cancelled() {
                return in.cancelled();
            }
        };
    }

    /** 이동 · 판매 터치 + 누른 자리를 화면에 잠깐 표시 (디버깅 모드로 칸을 띄웠을 때만 보임) */
    static Touch markTouch(TouchIn t) {
        return new Touch() {
            @Override
            public void tap(int x, int y, int ms) {
                BoxOverlay.tapped(x, y);
                t.tap(x, y, ms);
            }

            @Override
            public void swipe(int x0, int y0, int x1, int y1, int ms) {
                BoxOverlay.tapped(x0, y0);
                BoxOverlay.tapped(x1, y1);
                t.swipe(x0, y0, x1, y1, ms);
            }

            @Override
            public void pinch(int cx, int cy, int d0, int d1, int ms) {
                BoxOverlay.tapped(cx, cy);
                t.pinch(cx, cy, d0, d1, ms);
            }

            @Override
            public void stick(double[] v) {
                t.stick(v);
            }

            @Override
            public void jumpDown(int x, int y) {
                t.jumpDown(x, y);
            }

            @Override
            public void jumpUp() {
                t.jumpUp();
            }

            @Override
            public boolean stickHeld() {
                return t.stickHeld();
            }

            @Override
            public void play(List<Route.Ev> evs, int[] jump, Runnable check) {
                t.play(evs, jump, check);
            }

            @Override
            public void traceStart() {
                t.traceStart();
            }

            @Override
            public List<Route.Ev> traceEnd() {
                return t.traceEnd();
            }

            @Override
            public double[] stickAt() {
                return t.stickAt();
            }
        };
    }

    /** 디버깅 모드 켜기 · 끄기 — 실행 중이면 바로 칸을 띄우거나 지움 (메인 스레드) */
    static void setDebug(boolean on) {
        debug = on;
        prefs().edit().putBoolean("debug", on).apply();
        Fisher f = fisher;
        if (f != null && running()) {
            boolean test = "fishtest".equals(mode);          // 낚시 테스트는 디버깅 모드와 상관없이 늘 그림
            f.debug = on || test;
            f.probe = on || test ? BoxOverlay.PROBE : null;
        }
        BoxOverlay.sync();
        log(on ? "디버깅 모드 켬 — 모든 기능에서 매크로가 보는 칸 · 읽은 것 · 누른 자리를 화면에 그림" : "디버깅 모드 끔", 'd');
        changed();
    }

    /** m: run 자동 낚시 · cal 낚시 자동 보정 · fishtest 칸을 띄우며 낚시 테스트 · sellcal 판매 자동 보정 · sell 판매 한 번 · base / fish / sellgo 이동 테스트
     *  · rec_fish / rec_sell 길 기록 · stickfind / sticktest 조이스틱 → 못 하면 이유 */
    static synchronized String start(String m) {
        if (running()) return "이미 실행 중";
        if (psBusy) return "서버 체크 중 — 잠시 후 다시";
        psResume = false;
        lastError = null;
        // 새로 실행할 때는 리셋을 꼭 함 ('방금 리셋함' 은 한 실행 안에서만 — 그 사이 손으로 움직였을 수 있음)
        Mover.markMoved();
        if (TapService.instance == null) return "접근성 권한(Acrux Mobile)을 먼저 켜야 함";
        CaptureService cap = CaptureService.active;
        if (cap == null) return "앱에서 [화면 캡처 시작]을 먼저 눌러야 함";
        if ((m.equals("run") || m.equals("fishtest")) && !cfg.ready()) return "먼저 낚시 → 낚시 자동 보정을 해야 함 (낚시 자리에서 Fish 버튼이 보일 때)";
        if (m.equals("sticktest") && stick == null) return "먼저 조이스틱 찾기 (로블록스 설정 → 이동 방식: 조이스틱 · 조이스틱 찾기)";
        TouchIn touch = new TouchIn(cap.width(), cap.height());
        boolean test = m.equals("fishtest"), show = test || debug;
        // 누른 자리는 늘 넘김 (디버깅 모드를 실행 중에 켜도 보이게 · 칸을 안 띄웠으면 안 그림)
        Fisher f = new Fisher(cap, testInput(), OUT, cfg);
        Mover mv = new Mover(cap, markTouch(touch), new MlOcr(), pos, move, OUT, CHECK);
        Seller sl = seller(cap, mv);
        // 필요한 위치 · 길이 없으면 시작 전에 바로 알려 줌
        if (m.equals("sell")) {
            String miss = sl.missing();
            if (miss != null) return miss;
        }
        if (m.equals("itemtest")) {
            String miss = ItemUser.missing(pos);
            if (miss != null) return miss;
        }
        if (m.equals("base") || m.equals("fish") || m.equals("sellgo") || m.startsWith("rec_")) {
            String miss = mv.baseMissing();
            if (miss != null) return miss;
        }
        if (m.equals("run")) {
            // 이동 · 판매 설정을 전부 마쳐야 자동 낚시 (전에는 설정 없이도 돌아서 낚시만 하다가 가득 차면 멈췄음)
            String miss = sl.missing();
            if (miss != null) return "자동 낚시는 이동 · 판매 설정을 모두 마쳐야 시작됨 — " + miss;
            f.onStart = sl::goFish;
            zeroSells = sellSkips = 0;
            f.onFull = () -> sellOnce(sl);
            // 던지기 전 대기 화면에서 (낚시를 끊지 않게): 디스코드 봇 서버 체크 → 오토 아이템 사용
            f.onIdle = () -> psAtIdle(sl, f) || itemsAtIdle(mv);
            items.retryAt = 0;
            if (items.cfg.on && ItemUser.missing(pos) != null) log(ItemUser.missing(pos), 'y');
        }
        TapService ts = TapService.instance;
        if (ts != null && ts.coversGame() && (m.equals("cal") || m.equals("run") || test))
            log("Acrux 창이 낚시 창 쪽을 가리고 있음 — 위쪽 구석으로 옮겨 주세요 (화면 캡처에 같이 찍혀서 인식이 틀릴 수 있음)", 'y');
        stopFlag = false;
        f.onResult = kind -> {
            stat(kind, 1);
            zeroSells = sellSkips = 0;                         // 한 판이라도 끝났으면 Fish 는 먹히는 것 (빈 판매 기록을 지움)
        };
        f.onLearn = Hub::saveLearned;
        if (show) {
            f.debug = true;
            f.probe = BoxOverlay.PROBE;
        }
        endNote = lastRed = null;
        endWarn = false;
        fisher = f;
        mode = m;
        worker = new Thread(() -> {
            Process.setThreadPriority(Process.THREAD_PRIORITY_DISPLAY);
            boolean done = false;
            try {
                // 시작하고 1초 기다림 — [시작]을 누른 손가락이 떨어지고 게임 화면으로 돌아온 뒤에 키 · 클릭을 보냄
                state = "1초 뒤 시작";
                changed();
                long t0 = System.nanoTime();
                while (System.nanoTime() - t0 < 1_000_000_000L) {
                    CHECK.run();
                    Touch.nap(20);
                }
                if (m.equals("stickfind")) {
                    stickFind(cap);
                    endNote = "조이스틱 찾음";
                } else if (m.equals("sticktest")) {
                    stickTest(touch);
                    endNote = "조이스틱 테스트 끝";
                } else if (m.equals("jumptest")) {
                    int fb = Joy.pilot.jumpFallbacks;
                    mv.jumpTest();
                    log("점프 테스트 끝 — 제자리 1번 · 걸으면서 1번 · 꾹 누르는 1.5초 동안 계속 뛰었으면 성공" + (Joy.pilot.jumpFallbacks > fb ? " (점프를 따로 다시 누른 적 있음)" : ""), 'g');
                    endNote = "점프 테스트 끝";
                }
                else work(m, f, mv, sl);
                done = true;
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
                // 칸을 띄웠으면 끝난 이유를 잠깐 보여 주고 지움 (안 띄웠으면 아무것도 안 함)
                //  낚시 테스트 결과 · 오류 · [정지] · 결과 한 줄 · 스스로 멈춘 이유 (빨간 기록 · 자동 낚시) · 끝
                //  색: 잘 끝남 초록 · [정지] 흰색 · 문제 · 오류 노랑
                String tr = test ? f.testEnd : null;
                boolean round = tr != null && (tr.equals("성공") || tr.equals("쓰레기") || tr.equals("실패"));
                String end = tr != null ? tr : lastError != null ? lastError : stopFlag ? "멈춤"
                        : endNote != null ? endNote : lastRed != null ? lastRed : done ? "끝" : "멈춤";
                int col = tr != null ? (round ? com.acrux.mobile.core.TestBoxes.C_FISH : com.acrux.mobile.core.TestBoxes.C_REEL)
                        : lastError != null ? com.acrux.mobile.core.TestBoxes.C_REEL : stopFlag || end.equals("멈춤") ? BoxOverlay.C_TEXT
                        : endNote != null ? (endWarn ? com.acrux.mobile.core.TestBoxes.C_REEL : com.acrux.mobile.core.TestBoxes.C_FISH)
                        : lastRed != null ? com.acrux.mobile.core.TestBoxes.C_REEL
                        : com.acrux.mobile.core.TestBoxes.C_FISH;
                BoxOverlay.ending = true;
                main.post(() -> BoxOverlay.end(end, col));
                mv.releaseKeys();
                Joy.set(null);
                measuring = false;
                synchronized (Hub.class) {
                    mode = "";
                    java.util.concurrent.CompletableFuture<org.json.JSONObject> pending = psWant.getAndSet(null);
                    if (pending != null) pending.complete(psRes(null, "other"));
                }
                Quiet.off(app);                                       // 켜 둔 방해 금지를 되돌림
                state = lastError != null ? "멈춤 — " + lastError : "대기";     // 멈춘 이유를 떠 있는 창에도
                String why = lastError;
                if (why != null) main.post(() -> android.widget.Toast.makeText(app, com.acrux.mobile.core.I18n.t("멈춤 — " + why), android.widget.Toast.LENGTH_LONG).show());
                save();
                changed();
            }
        }, "acrux-worker");
        Quiet.on(app);                                            // 매크로 중 알림 끄기 (설정에서 켰을 때 · 일이 바로 끝나도 되돌리게 시작 전에)
        worker.start();
        main.post(BoxOverlay::started);                        // 디버깅 모드 · 낚시 테스트: 보정된 칸 · 상태를 바로 그림 (시작 전 1초 동안도 보임)
        changed();
        return "";
    }

    // ---- 서버 체크 (디스코드 봇 /서버 체크 · PC 와 같음)
    //  자동 낚시 중이면 던지기 전 대기 화면에서 (Fisher.onIdle) · 아무것도 안 돌면 바로 · 다른 기능 (테스트 · 보정 · 기록) 중이면 안 함
    //  이유 코드 (봇이 디스코드 언어로 보여 줌): no_roblox · perm · menu_pos · menu · other · busy · stuck · error · no_link
    static final String PS_MENU = "game_menu";
    static final String PS_COMMANDS = "private_server_commands";
    static final String PS_MENU_CLOSE = "ps_menu_close", PS_PANEL_CLOSE = "ps_panel_close";
    static final String[][] PS_CLOSE_KEYS = {{PS_MENU_CLOSE, "게임 메뉴 X"}, {PS_PANEL_CLOSE, "내 서버 관리창 X"}};
    static final String[] PS_MENU_KEY = {PS_MENU, "게임 메뉴 버튼 (왼쪽 줄 세 개)"};
    static volatile boolean psBusy;
    /** 복귀 뒤 자동 낚시를 다시 켤 예정 (그 사이 [시작] · [정지] 를 누르면 취소) */
    static volatile boolean psResume;
    static final class ServerCheck extends java.util.concurrent.CompletableFuture<org.json.JSONObject> {
        final java.util.function.Consumer<String> progress;
        ServerCheck(java.util.function.Consumer<String> progress) { this.progress = progress; }
    }
    static final java.util.concurrent.atomic.AtomicReference<ServerCheck> psWant =
            new java.util.concurrent.atomic.AtomicReference<>();
    static final long PS_WAIT_MS = 60_000;

    static org.json.JSONObject psRes(Boolean own, String why) {
        org.json.JSONObject o = new org.json.JSONObject();
        try {
            o.put("own", own == null ? org.json.JSONObject.NULL : own);
            o.put("why", why);
            o.put("link", !Biome.ps().isEmpty());
        } catch (org.json.JSONException ignored) {
        }
        if (own == null) log("서버 체크 — 확인 못 함 (" + psWhy(why) + ")", 'y');
        return o;
    }

    static String psWhy(String why) {
        switch (why) {
            case "no_roblox": return "로블록스를 못 찾음 — 로블록스에 들어가 있는 상태로 다시";
            case "perm": return "접근성 권한 · 화면 캡처가 꺼져 있음";
            case "menu_pos": return "서버 체크 버튼 위치를 지정하세요.";
            case "menu": return "메뉴 창을 못 찾음 — 게임 메뉴 버튼 위치 확인";
            case "other": return "다른 기능 (테스트 · 보정 · 기록) 중";
            case "busy": return "이미 서버 체크 중";
            case "stuck": return "다른 기능이 1분 안에 끝나지 않음";
            case "no_link": return "내 서버 링크가 없음 — 바이옴 매크로 설정에서 입력";
            default: return "오류";
        }
    }

    /** 봇 요청 (디스코드 연결 스레드) → {own, why, link} */
    static org.json.JSONObject psCheck(java.util.function.Consumer<String> progress) {
        if (TapService.instance == null || CaptureService.active == null) return psRes(null, "perm");
        if (pos.get(PS_MENU) == null || pos.get(PS_COMMANDS) == null
                || pos.get(PS_MENU_CLOSE) == null || pos.get(PS_PANEL_CLOSE) == null) return psRes(null, "menu_pos");
        for (String[] k : Mover.RESET_KEYS) {
            if (pos.get(k[0]) == null) {
                log("서버 체크 — 봇 명령어 → 서버 체크에서 재설정 버튼 위치를 지정해 주세요", 'y');
                return psRes(null, "menu_pos");                // 위치 지정이 빠짐 (서버 체크 위치와 같은 화면)
            }
        }
        ServerCheck want = null;
        Thread other = null;
        synchronized (Hub.class) {
            if (psBusy || psWant.get() != null) return psRes(null, "busy");
            if (running() && "run".equals(mode)) {
                want = new ServerCheck(progress);
                progress.accept("waiting");
                psWant.set(want);
            } else {
                psBusy = true;
                if (running()) {
                    other = worker;
                    progress.accept("waiting");
                }
            }
        }
        if (want != null) {
            log("서버 체크 — 자동 낚시가 대기 화면에 오면 확인", 'c');
            try {
                return want.get(PS_WAIT_MS, TimeUnit.MILLISECONDS);
            } catch (Exception e) {
                if (psWant.compareAndSet(want, null)) return psRes(null, running() ? "stuck" : "other");
                try {
                    return want.get(35, TimeUnit.SECONDS);   // 막 시작함 → 끝날 때까지
                } catch (Exception e2) {
                    return psRes(null, "error");
                }
            }
        }
        String back = null;
        try {
            if (other != null) {
                try {
                    other.join(PS_WAIT_MS);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    return psRes(null, "other");
                }
                if (other.isAlive()) return psRes(null, "stuck");
            }
            stopFlag = false;
            progress.accept("checking");
            // 폰의 디스코드에서 명령을 보냈으면 로블록스가 뒤에 있음 → 로블록스를 앞으로 가져와서 확인하고, 끝나면 그 앱으로 돌려보냄
            if (!TapService.robloxFront()) {
                String rb = TapService.roblox, was = TapService.front;
                if (rb == null) {
                    // Acrux (접근성) 가 로블록스보다 나중에 켜졌으면 로블록스를 앞에서 본 적이 없음 → 기본 로블록스 앱으로
                    log("서버 체크 — 맨 앞 앱: " + was + " · 로블록스를 아직 못 봄", 'd');
                    rb = "com.roblox.client";
                }
                if (!TapService.bringFront(rb)) return psRes(null, "no_roblox");
                back = was;
                log("서버 체크 — 로블록스를 앞으로 가져옴 (맨 앞 앱: " + was + ")", 'c');
                for (int i = 0; i < 20 && !TapService.robloxFront(); i++) Touch.nap(100);
                if (!TapService.robloxFront()) return psRes(null, "no_roblox");
                Touch.nap(1500);                               // 화면이 다 그려질 때까지
            }
            return psRun();
        } catch (Fisher.Stopped e) {
            return psRes(null, "other");
        } finally {
            try {
                if (back != null && TapService.robloxFront()) {
                    Touch.nap(300);
                    TapService.bringFront(back);
                }
            } finally {
                psBusy = false;
                changed();
            }
        }
    }

    /** 자동 낚시가 던지기 전 대기 화면에서 부름 (워커 스레드) → 서버 체크 요청이 있었으면 하고 true */
    static boolean psAtIdle(Seller seller, Fisher fisher) {
        ServerCheck want;
        synchronized (Hub.class) {
            want = psWant.getAndSet(null);
            if (want == null) return false;
            psBusy = true;
        }
        String was = state;
        OUT.state("서버 체크 중");
        try {
            if (!TapService.robloxFront()) {
                want.complete(psRes(null, "no_roblox"));
            } else {
                want.progress.accept("checking");
                want.complete(psRun());
                CHECK.run();
                // 서버 체크는 캐릭터를 재설정함 → 낚시 장소로 돌아감 · 못 돌아가면 자동 낚시를 끄지 않고 Fisher 가 쉬었다가 다시 감
                if (!TapService.robloxFront()) {
                    log("서버 체크 뒤 로블록스가 맨 앞에 없음 — 잠시 뒤 낚시 장소로 다시 감", 'y');
                    fisher.lostSpot = true;
                } else {
                    OUT.state("낚시 장소로 돌아가는 중");
                    seller.goFish();
                }
            }
        } catch (Fisher.Stopped e) {
            want.complete(psRes(null, "other"));
            throw e;
        } catch (RuntimeException e) {
            want.complete(psRes(null, "error"));
            log("서버 체크 후 낚시 장소 복구 실패: " + e.getMessage() + " — 잠시 뒤 다시 감", 'y');
            fisher.lostSpot = true;
        } finally {
            psBusy = false;
            OUT.state(was);
        }
        return true;
    }

    /** 자동 낚시의 던지기 전 대기 화면: 쓸 차례인 아이템이 있으면 사용 → 했는지 (그동안 낚시는 쉼) */
    static boolean itemsAtIdle(Mover mv) {
        double t = ItemUser.now();
        List<Integer> keys = items.due(t);
        if (keys.isEmpty()) return false;
        if (ItemUser.missing(pos) != null) {
            items.retryAt = t + 60;
            if (items.missLogDue(t)) log(ItemUser.missing(pos), 'y');
            return false;
        }
        if (!TapService.robloxFront()) return false;
        String was = state;
        try {
            items.run(keys, mv, new MlOcr(), pos, Hub::typeText, OUT, false);
        } catch (Fisher.Stopped e) {
            throw e;
        } catch (RuntimeException e) {
            items.retryAt = ItemUser.now() + 60;
            log("오토 아이템 오류: " + e.getMessage() + " — 1분 뒤 다시 시도", 'y');
        } finally {
            OUT.state(was);
        }
        return true;
    }

    /**
     * 오토 아이템: 키보드가 뜬 입력칸에 글자 넣기 + 엔터 (clear: 원래 글자를 지우고)
     *  ① 접근성으로 입력칸 글자를 바로 바꿈 (키보드가 뜰 때까지 1.2초 기다림) ② 안 되면 무선 디버깅 (input text · 엔터 키)
     */
    static boolean typeText(String text, boolean clear) {
        TapService ts = TapService.instance;
        if (ts != null) {
            long end = System.nanoTime() + 1_200_000_000L;
            while (true) {
                CHECK.run();
                if (ts.setInputText(text)) {
                    Touch.nap(150);
                    if (!ts.inputEnter() && !adbShell("input keyevent 66")) log("엔터를 못 보냄 (안드로이드 11 미만 · 무선 디버깅 없음) — 그대로 진행", 'd');
                    return true;
                }
                if (System.nanoTime() > end) break;
                Touch.nap(100);
            }
        }
        StringBuilder c = new StringBuilder();
        if (clear) {
            c.append("input keyevent 123");                        // 끝으로 → 지우기 30번
            for (int i = 0; i < 30; i++) c.append(" 67");
            c.append("; ");
        }
        c.append("input text '").append(text.replace("'", "").replace(" ", "%s")).append("'; input keyevent 66");
        return adbShell(c.toString());
    }

    /** 무선 디버깅으로 명령 하나 (연결 안 됐으면 false) */
    static boolean adbShell(String cmd) {
        if (!Adb.connected || app == null) return false;
        try {
            Adb.shell(app, cmd, 10000, l -> { });
            return true;
        } catch (Exception e) {
            log("무선 디버깅 명령 실패: " + e.getMessage(), 'd');
            return false;
        }
    }

    /** 메뉴를 열어 확인하고 닫음 (화면을 쓰는 다른 일이 없을 때만 부름) */
    static org.json.JSONObject psRun() {
        CaptureService cap = CaptureService.active;
        TapService ts = TapService.instance;
        double[] menu = pos.get(PS_MENU);
        if (cap == null || ts == null) return psRes(null, "perm");
        double[] commands = pos.get(PS_COMMANDS);
        double[] menuClose = pos.get(PS_MENU_CLOSE), panelClose = pos.get(PS_PANEL_CLOSE);
        if (menu == null || commands == null || menuClose == null || panelClose == null) return psRes(null, "menu_pos");
        log("서버 체크 — 메뉴를 열어 내 서버인지 확인", 'c');
        MlOcr ocr = new MlOcr();
        Runnable guard = () -> {
            CHECK.run();
            if (!TapService.robloxFront()) throw new IllegalStateException("로블록스가 맨 앞에 없음");
        };
        try {
            guard.run();
            Mover mover = new Mover(cap, markTouch(new TouchIn(cap.width(), cap.height())), ocr, pos, move, OUT, guard);
            mover.resetForServerCheck();
            Boolean own = com.acrux.mobile.core.PsCheck.check(new com.acrux.mobile.core.PsCheck.Io() {
                @Override
                public List<com.acrux.mobile.core.SellText.Box> read() {
                    guard.run();
                    return ocr.read(null);
                }

                @Override
                public void tap(double[] p) {
                    guard.run();
                    int x = (int) (p[0] * cap.width()), y = (int) (p[1] * cap.height());
                    BoxOverlay.tapped(x, y);
                    ts.tap(x, y, 80);
                }

                @Override
                public void sleep(double sec) {
                    Touch.nap((int) (sec * 1000));
                }

            }, menu, commands, menuClose, panelClose, () -> log("서버 체크 — Menu 창이 안 닫힘 (직접 닫아 주세요)", 'y'));
            if (own == null) return psRes(null, "menu");
            log(own ? "서버 체크 — 내 서버" : "서버 체크 — 내 서버가 아님", own ? 'g' : 'y');
            return psRes(own, "");
        } catch (Fisher.Stopped e) {
            throw e;
        } catch (RuntimeException e) {
            log("서버 체크 오류: " + e, 'y');
            return psRes(null, "error");
        }
    }

    /** 봇의 [내 서버로 복귀] → 자동 낚시를 멈추고 (ADB 가 연결돼 있으면 로블록스를 끄고) 내 서버 링크를 엶 → 45초 뒤 자동 낚시 다시 시작 → {ok, why} */
    static org.json.JSONObject psReturn() {
        org.json.JSONObject o = new org.json.JSONObject();
        String link = Biome.ps();
        try {
            if (link.isEmpty()) return o.put("ok", false).put("why", "no_link");
            if (TapService.instance == null) return o.put("ok", false).put("why", "perm");
            boolean wasRun;
            synchronized (Hub.class) {
                if (psBusy || psWant.get() != null) return o.put("ok", false).put("why", "busy");
                wasRun = running() && "run".equals(mode);
                if (running() && !wasRun) return o.put("ok", false).put("why", "other");
                psBusy = true;
            }
            try {
                log("디스코드 봇 — 내 서버로 복귀", 'c');
                if (wasRun) {
                    stop();
                    for (int i = 0; i < 100 && running(); i++) Touch.nap(100);
                    if (running()) return o.put("ok", false).put("why", "busy");
                }
                if (Adb.connected) {
                    try {
                        Adb.shell(app, "am force-stop com.roblox.client", 5000, l -> { });
                        Touch.nap(1500);
                    } catch (Exception e) {
                        log("로블록스를 끄지 못함 (ADB) — 링크만 엶", 'd');
                    }
                }
                TapService ts = TapService.instance;
                Context c = ts != null ? ts : app;
                android.content.Intent view = new android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(link))
                        .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK);
                try {
                    c.startActivity(new android.content.Intent(view).setPackage("com.roblox.client"));
                } catch (android.content.ActivityNotFoundException e) {
                    c.startActivity(view);
                }
                if (wasRun) {
                    log("내 서버로 복귀 — 45초 뒤 자동 낚시 다시 시작", 'c');
                    psResume = true;
                    main.postDelayed(() -> {
                        if (!psResume || running()) return;
                        psResume = false;
                        String err = start("run");
                        if (!err.isEmpty()) log("자동 낚시 다시 시작 못 함 — " + err, 'y');
                    }, 45_000);
                }
                return o.put("ok", true).put("why", "");
            } finally {
                psBusy = false;
            }
        } catch (org.json.JSONException e) {
            return o;
        } catch (RuntimeException e) {
            log("내 서버로 복귀 오류: " + e, 'y');
            try {
                return o.put("ok", false).put("why", "error");
            } catch (org.json.JSONException ignored) {
                return o;
            }
        }
    }

    static Seller seller(CaptureService cap, Mover mv) {
        Seller s = new Seller(cap, mv, new MlOcr(), pos, sellRoute, fishRoute, OUT);
        s.sellMax = sellMax;
        s.sellDelay = sellDelay;
        return s;
    }

    // ---------------------------------------------------------------- 저장 위치는 항상 화면 안
    // (화면 밖 좌표를 누르면 안드로이드가 'Path bounds must not be negative' 로 거절해서 매크로가 멈춤
    //  · 다른 화면 크기 · 방향에서 저장했거나 조이스틱 가장자리를 화면 밖 쪽으로 찍은 경우)
    /** 저장한 숫자 읽기 — float 로 저장해서 0.3 이 0.30000001192… 로 돌아옴 (설정 칸에 그대로 보였음) → 소수 넷째 자리로 맞춤 */
    static double num(android.content.SharedPreferences p, String key, double def) {
        return Math.round(p.getFloat(key, (float) def) * 10000.0) / 10000.0;
    }

    static double in01(double v) {
        return Double.isNaN(v) ? 0.5 : Math.max(0, Math.min(0.9999, v));
    }

    /** 위치 [x, y] (화면 비율) → 화면 안으로 */
    static double[] fitPos(double[] v) {
        return v == null || v.length < 2 ? v : new double[]{in01(v[0]), in01(v[1])};
    }

    /** 조이스틱 [가운데 x, y, 반지름(높이 비율)] → 가운데는 화면 안 · 판이 화면 밖으로 나가지 않게 반지름을 줄임 (w, h: 화면 px · 0 = 모름) */
    static double[] fitStick(double[] st, int w, int h) {
        if (st == null || st.length < 3) return st;
        double cx = in01(st[0]), cy = in01(st[1]), r = Math.max(0, st[2]);
        if (w > 0 && h > 0) r = Math.min(r, Math.min(Math.min(cx * w, (1 - cx) * w), Math.min(cy * h, (1 - cy) * h)) / h);
        else r = Math.min(r, Math.min(cy, 1 - cy));
        return new double[]{cx, cy, r};
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
            throw new RuntimeException("조이스틱을 못 찾음 — 로블록스 설정에서 이동 방식을 '조이스틱'(클래식)으로 바꾸고, 조이스틱이 보일 때 다시 (안 되면 조이스틱 직접 지정)");
        double sx = (double) w / im.w, sy = (double) h / im.h;
        stick = fitStick(new double[]{fd.cx * sx / w, fd.cy * sy / h, fd.r * sy / h}, w, h);
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
    static void stickTest(TouchIn t) {
        showStick(1500);
        double[][] dirs = {{0, -1}, {-1, 0}, {0, 1}, {1, 0}};
        String[] keys = {"W", "A", "S", "D"};
        for (int i = 0; i < 4; i++) {
            state = "조이스틱 테스트 — " + keys[i];
            changed();
            t.stick(dirs[i]);
            long end = System.nanoTime() + 800_000_000L;
            while (System.nanoTime() < end) {
                CHECK.run();
                Touch.nap(20);
            }
            t.stick(null);
            Touch.nap(400);
        }
        log("조이스틱 테스트: W · A · S · D 0.8초씩 보냄 — 캐릭터가 앞 · 왼쪽 · 뒤 · 오른쪽으로 갔으면 성공", 'g');
    }

    static void work(String m, Fisher f, Mover mv, Seller sl) {
        if (m.equals("run")) {
            f.run();
        } else if (m.equals("cal")) {
            String r = f.calibrate();
            log("자동 보정: " + r, cfg.ready() ? 'g' : 'y');
            endNote = "자동 보정: " + r;
            endWarn = !r.endsWith("맞춤");                    // 다 맞추지 못함 (창을 못 찾음 · 입질 없음 · 결과창 못 잼 …)
        } else if (m.equals("fishtest")) {
            f.test();
        } else if (m.equals("sellcal")) {
            List<String> notes = sl.autocal();
            endWarn = !notes.isEmpty();
            endNote = "판매 자동 보정 완료" + (endWarn ? " · " + String.join(" · ", notes) : "");
            log(endNote, endWarn ? 'y' : 'g');
        } else if (m.equals("sell")) {
            int sold = sl.run();
            stat("sells", 1);
            stat("sold", sold);
            log("판매 테스트: " + sold + "종류 판매", 'g');
            endNote = "판매 테스트: " + sold + "종류 판매";
        } else if (m.equals("itemtest")) {
            // 테스트: 쿨타임 · 켜짐과 상관없이 지금 바로 (고른 아이템 · 하나도 안 골랐으면 둘 다)
            if (!TapService.robloxFront()) throw new RuntimeException("로블록스가 맨 앞에 없음 — 테스트 안 함");
            List<Integer> keys = new ArrayList<>();
            for (int k = 0; k < ItemUser.NAMES.length; k++) if (items.cfg.use[k]) keys.add(k);
            if (keys.isEmpty()) for (int k = 0; k < ItemUser.NAMES.length; k++) keys.add(k);
            double retry = items.retryAt;
            int n = items.run(keys, mv, new MlOcr(), pos, Hub::typeText, OUT, true);
            items.retryAt = retry;                                 // 테스트 결과로 자동 낚시 중 다음 시도를 미루지 않음
            endWarn = n < keys.size();
            endNote = "오토 아이템 테스트: " + n + " / " + keys.size() + "개 사용";
            log(endNote, endWarn ? 'y' : 'g');
        } else if (m.equals("base")) {
            mv.goBase();
            log("기준 장소 도착", 'g');
            endNote = "기준 장소 도착";
        } else if (m.equals("fish")) {
            mv.goPlace("낚시 장소", fishRoute);
            endNote = "낚시 장소까지 길 따라 이동 끝";
        } else if (m.equals("sellgo")) {
            mv.goPlace("물고기 판매 장소", sellRoute);
            endNote = "물고기 판매 장소까지 길 따라 이동 끝";
        } else if (m.startsWith("rec_")) {
            record(mv, m.endsWith("sell"));
            endNote = (m.endsWith("sell") ? "물고기 판매 장소" : "낚시 장소") + " 길 저장됨";
        }
    }

    /** 길 기록: 기준 장소로 간 뒤 → 플레이어가 조이스틱으로 직접 걸어감 → [도착] → 저장 */
    static void record(Mover mv, boolean sell) {
        String name = sell ? "물고기 판매 장소" : "낚시 장소";
        arrived = false;
        try {
            Route r = mv.recordPad(name, new RecPad(), () -> {
                if (!measuring) {
                    measuring = true;              // 기준 장소에 도착해서 기록을 시작하면 [도착] 버튼을 보여 줌
                    changed();
                }
                return arrived;
            });
            if (sell) sellRoute = r;
            else fishRoute = r;
            save();
            String msg = name + " 길 저장됨 (" + Math.round(r.seconds() * 10) / 10.0 + "초 · 점프 " + r.jumps() + "번)";
            log(msg, 'g');
            main.post(() -> android.widget.Toast.makeText(app, com.acrux.mobile.core.I18n.t(msg), android.widget.Toast.LENGTH_LONG).show());
        } finally {
            measuring = false;
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
        String m = sl.missing();
        if (m != null) {
            log("인벤토리 가득 — 판매 못 함: " + m, 'y');
            return false;
        }
        if (zeroSells >= 1 && sellSkips < 2) {
            // 지난번 판매 때 팔 물고기가 없었고 그 뒤로 한 판도 못 끝냈는데 또 '가득' → 인벤토리 문제가 아닐 수 있음 (Fish 버튼 자리 · 화면 문제)
            //  → 두 번까지는 판매하러 가지 않고 낚시 장소부터 다시 잡음 (낚시가 다시 감 · 멈추지 않음) · 그래도 안 되면 다시 판매하러 감 (지난번에 물고기를 잘못 읽었을 수도)
            sellSkips++;
            log("Fish 를 눌러도 반응이 없는데 지난번 판매 때 팔 물고기가 없었음 — 판매하러 가지 않고 낚시 장소부터 다시 잡음 (" + sellSkips + "/2 · 계속 그러면 낚시 보정 → 칸을 띄우며 낚시 테스트로 확인)", 'y');
            return false;
        }
        sellSkips = 0;
        // 이동이 한 번 빗나가면 (판매 장소에 못 감 → 대화창 · 상점을 못 찾음) 바로 멈추지 않고 처음(리셋)부터 다시 감
        for (int attempt = 1; ; attempt++) {
            try {
                int sold = sl.run();
                stat("sells", 1);
                stat("sold", sold);
                if (sold > 0) {
                    zeroSells = 0;
                    return true;
                }
                zeroSells++;
                log("판매하러 갔는데 팔 물고기가 없음 — 낚시 자리를 다시 잡고 이어감", 'y');
                return true;
            } catch (Fisher.Stopped e) {
                throw e;
            } catch (RuntimeException e) {
                if (attempt >= SELL_TRIES) {
                    // 멈추지 않음 — 낚시가 낚시 장소로 다시 가서 이어감 (다음에 가득 차면 또 판매하러 감)
                    log("판매 실패 " + SELL_TRIES + "번: " + e.getMessage() + " — 낚시 장소로 돌아가서 이어감 (계속 그러면 판매 장소 길 · 위치 확인)", 'y');
                    return false;
                }
                log("판매 실패 (" + attempt + "/" + SELL_TRIES + "): " + e.getMessage() + " — 처음부터 다시 이동", 'y');
                state = "판매 장소로 다시 가는 중";
                changed();
            }
        }
    }

    static final int SELL_TRIES = 3;

    /** 길 기록 [취소] — 걸어온 길을 버리고 멈춤 (기록해 둔 예전 길은 그대로) · [도착] 을 이미 눌렀으면 무시 */
    static void cancelRecord() {
        if (!measuring || !mode.startsWith("rec_") || arrived) return;
        log("길 기록 취소 — 저장 안 함", 'y');
        stopFlag = true;
        Thread w = worker;
        if (w != null) w.interrupt();
    }

    static void stop() {
        // 길 기록 중에 [멈춤] = [도착] (걸어온 길을 저장) · 저장하는 중에 한 번 더 누르면 그때 멈춤
        if (measuring && mode.startsWith("rec_") && !arrived) {
            arrived = true;
            log("도착 — 길 저장", 'g');
            return;
        }
        stopFlag = true;
        psResume = false;
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
        cfg.biteMax = num(p, "biteMax", cfg.biteMax);
        cfg.clickMs = p.getInt("clickMs", cfg.clickMs);
        cfg.leadMs = num(p, "leadMs", cfg.leadMs);
        cfg.targetPct = num(p, "targetPct", cfg.targetPct);
        cfg.adaptive = p.getBoolean("adaptive", cfg.adaptive);
        cfg.tapAbove = p.getBoolean("tapAbove", true);
        cfg.reelFall = num(p, "reelFall", 0);
        cfg.reelJump = num(p, "reelJump", 0);
        cfg.reelDelay = num(p, "reelDelay", 0);
        cfg.reelSpot = p.getInt("reelSpot", 0);
        autoUpdate = p.getBoolean("autoUpdate", true);
        debug = p.getBoolean("debug", false);
        sellMax = p.getInt("sellMax", 100);
        sellDelay = num(p, "sellDelay", 0);
        ItemUser.Cfg ic = items.cfg;
        ic.on = p.getBoolean("it_on", false);
        ic.use[0] = p.getBoolean("it_strange", true);
        ic.use[1] = p.getBoolean("it_random", true);
        ic.min[0] = num(p, "it_strange_min", 10.5);
        ic.min[1] = num(p, "it_random_min", 18.0);
        ic.close = p.getBoolean("it_close", true);
        ic.match = num(p, "it_match", 70);
        move.resetWait = num(p, "m_reset_wait", move.resetWait);
        // V0.9.11 기준 장소 걷기가 바뀜 (W+A → A → A+W → 쉼 → D) — 예전 값(m_w_time …)은 안 씀
        move.waTime = num(p, "m2_wa", move.waTime);
        move.vip = p.getBoolean("vip", true);
        move.waVip = num(p, "m2_wa_vip", move.waVip);
        move.aTime = num(p, "m2_a", move.aTime);
        move.awTime = num(p, "m2_aw", move.awTime);
        move.pauseTime = num(p, "m2_pause", move.pauseTime);
        move.dTime = num(p, "m2_d", move.dTime);
        move.tiltSwipes = p.getInt("m3_tilt", move.tiltSwipes);
        move.zoomPinches = p.getInt("m3_zoom", move.zoomPinches);
        move.margin = num(p, "m_margin", move.margin);
        stick = arr(p.getString("stick", ""));
        if (stick != null && stick.length != 3) stick = null;
        stick = fitStick(stick, 0, 0);
        pos.clear();
        for (String kv : p.getString("pos", "").split(";")) {
            int e = kv.indexOf('=');
            if (e <= 0) continue;
            double[] v = arr(kv.substring(e + 1));
            if (v != null && v.length == 2) pos.put(kv.substring(0, e), fitPos(v));
        }
        fishRoute = Route.load(p.getString("fishRoute", ""));
        sellRoute = Route.load(p.getString("sellRoute", ""));
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
                .putFloat("reelFall", (float) cfg.reelFall).putFloat("reelJump", (float) cfg.reelJump).putFloat("reelDelay", (float) cfg.reelDelay)
                .putInt("reelSpot", cfg.reelSpot).remove("closeLearn")              // 1.8.5: 결과창 X 자리 기억 없앰 (예전 값 지움)
                .putBoolean("autoUpdate", autoUpdate)
                .putInt("sellMax", sellMax).putFloat("sellDelay", (float) sellDelay)
                .putBoolean("it_on", items.cfg.on).putBoolean("it_strange", items.cfg.use[0]).putBoolean("it_random", items.cfg.use[1])
                .putFloat("it_strange_min", (float) items.cfg.min[0]).putFloat("it_random_min", (float) items.cfg.min[1])
                .putBoolean("it_close", items.cfg.close).putFloat("it_match", (float) items.cfg.match)
                .putFloat("m_reset_wait", (float) move.resetWait)
                .putFloat("m2_wa", (float) move.waTime).putBoolean("vip", move.vip).putFloat("m2_wa_vip", (float) move.waVip).putFloat("m2_a", (float) move.aTime).putFloat("m2_aw", (float) move.awTime)
                .putFloat("m2_pause", (float) move.pauseTime).putFloat("m2_d", (float) move.dTime)
                .putFloat("m_margin", (float) move.margin).putInt("m3_tilt", move.tiltSwipes).putInt("m3_zoom", move.zoomPinches)
                .putString("pos", ps.toString())
                .putString("stick", stick == null ? "" : arr(stick))
                .putString("fishRoute", fishRoute.save()).putString("sellRoute", sellRoute.save())
                .apply();
    }

    static void resetCalibration() {
        cfg.panelRegion = cfg.reelRegion = cfg.resultRegion = null;
        cfg.fishBtn = cfg.barRegion = cfg.closePos = cfg.titlePos = null;
        cfg.reelDerived = true;
        cfg.reelSpot = 0;
        save();
        changed();
    }

    /** 자동 낚시가 잰 값만 저장 (릴링 지연 · 속도 · 누르는 자리) — 낚시 중에 판마다 부름 */
    static void saveLearned() {
        prefs().edit()
                .putFloat("reelFall", (float) cfg.reelFall).putFloat("reelJump", (float) cfg.reelJump).putFloat("reelDelay", (float) cfg.reelDelay)
                .putInt("reelSpot", cfg.reelSpot).remove("closeLearn")
                .apply();
        changed();
    }

    /** 앱 → 낚시 → 릴링 측정값 [초기화] — 다음 판부터 처음 값으로 다시 잼 */
    static void resetReel() {
        cfg.reelFall = cfg.reelJump = cfg.reelDelay = 0;
        cfg.reelSpot = 0;
        saveLearned();
        log("릴링 측정값을 지움", 'd');
    }
}
