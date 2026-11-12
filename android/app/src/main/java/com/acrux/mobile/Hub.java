package com.acrux.mobile;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.Handler;
import android.os.Looper;
import android.os.Process;

import com.acrux.mobile.core.Fisher;
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
    /** 로블록스 클래식 조이스틱 {가운데 x / 화면 너비, 가운데 y / 화면 높이, 반지름 / 화면 높이} (못 찾았으면 null) · W A S D 를 조이스틱 터치로 */
    static volatile double[] stick;
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

    /** 자동 낚시가 누르는 입력: 화면 터치 */
    static Fisher.Input fishInput() {
        return (x, y, ms) -> {
            TapService t = TapService.instance;
            if (t != null) t.tap(x, y, Math.max(ms, 40));
        };
    }

    /** m: run 자동 낚시 · cal 낚시 자동 보정 · sellcal 판매 자동 보정 · sell 판매 한 번 · base / fish / sellgo 이동 테스트
     *  · rec_fish / rec_sell 길 기록 · stickfind / sticktest 조이스틱 → 못 하면 이유 */
    static synchronized String start(String m) {
        if (running()) return "이미 실행 중";
        lastError = null;
        // 새로 실행할 때는 리셋을 꼭 함 ('방금 리셋함' 은 한 실행 안에서만 — 그 사이 손으로 움직였을 수 있음)
        Mover.markMoved();
        if (TapService.instance == null) return "접근성 권한(Acrux Mobile)을 먼저 켜야 함";
        CaptureService cap = CaptureService.active;
        if (cap == null) return "앱에서 [화면 캡처 시작]을 먼저 눌러야 함";
        if (m.equals("run") && !cfg.ready()) return "먼저 낚시 → 낚시 자동 보정을 해야 함 (낚시 자리에서 Fish 버튼이 보일 때)";
        if (m.equals("sticktest") && stick == null) return "먼저 조이스틱 찾기 (로블록스 설정 → 이동 방식: 조이스틱 · 조이스틱 찾기)";
        TouchIn touch = new TouchIn(cap.width(), cap.height());
        Fisher f = new Fisher(cap, fishInput(), OUT, cfg);
        Mover mv = new Mover(cap, touch, new MlOcr(), pos, move, OUT, CHECK);
        Seller sl = seller(cap, mv);
        // 필요한 위치 · 길이 없으면 시작 전에 바로 알려 줌
        if (m.equals("sell")) {
            String miss = sl.missing();
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
                    Touch.nap(20);
                }
                if (m.equals("stickfind")) stickFind(cap);
                else if (m.equals("sticktest")) stickTest(touch);
                else if (m.equals("jumptest")) {
                    int fb = Joy.pilot.jumpFallbacks;
                    mv.jumpTest();
                    log("점프 테스트 끝 — 제자리 1번 · 걸으면서 1번 · 꾹 누르는 1.5초 동안 계속 뛰었으면 성공" + (Joy.pilot.jumpFallbacks > fb ? " (점프를 따로 다시 누른 적 있음)" : ""), 'g');
                }
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
                mv.releaseKeys();
                Joy.set(null);
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

    static Seller seller(CaptureService cap, Mover mv) {
        Seller s = new Seller(cap, mv, new MlOcr(), pos, sellRoute, fishRoute, OUT);
        s.sellMax = sellMax;
        s.sellDelay = sellDelay;
        s.shot = why -> saveShot(cap, why);
        return s;
    }

    /**
     * 판매에서 버튼 찾기에 실패한 화면을 사진으로 남김 (사진첩 → Pictures/Acrux) — 보내 주면 왜 못 찾았는지 볼 수 있음
     * 안드로이드 10 이상: 권한 없이 사진첩에 · 그 아래: 앱 폴더 (Android/data/com.acrux.mobile/files/Pictures)
     */
    static void saveShot(CaptureService cap, String why) {
        try {
            com.acrux.mobile.core.Img im = cap.grab(0, 0, cap.width(), cap.height());
            if (im == null || im.w * im.h == 0) return;
            int[] px = new int[im.w * im.h];
            for (int k = 0; k < px.length; k++) px[k] = 0xFF000000 | (im.r[k] << 16) | (im.g[k] << 8) | im.b[k];
            android.graphics.Bitmap bm = android.graphics.Bitmap.createBitmap(px, im.w, im.h, android.graphics.Bitmap.Config.ARGB_8888);
            String name = "acrux_" + why + "_" + new SimpleDateFormat("MMdd_HHmmss", Locale.ROOT).format(new Date()) + ".jpg";
            String where;
            if (android.os.Build.VERSION.SDK_INT >= 29) {
                android.content.ContentValues v = new android.content.ContentValues();
                v.put(android.provider.MediaStore.MediaColumns.DISPLAY_NAME, name);
                v.put(android.provider.MediaStore.MediaColumns.MIME_TYPE, "image/jpeg");
                v.put(android.provider.MediaStore.MediaColumns.RELATIVE_PATH, "Pictures/Acrux");
                android.net.Uri uri = app.getContentResolver().insert(android.provider.MediaStore.Images.Media.EXTERNAL_CONTENT_URI, v);
                if (uri == null) return;
                try (java.io.OutputStream o = app.getContentResolver().openOutputStream(uri)) {
                    if (o == null) return;
                    bm.compress(android.graphics.Bitmap.CompressFormat.JPEG, 90, o);
                }
                where = "사진첩 Pictures/Acrux/" + name;
            } else {
                java.io.File dir = app.getExternalFilesDir(android.os.Environment.DIRECTORY_PICTURES);
                if (dir == null) return;
                java.io.File f = new java.io.File(dir, name);
                try (java.io.OutputStream o = new java.io.FileOutputStream(f)) {
                    bm.compress(android.graphics.Bitmap.CompressFormat.JPEG, 90, o);
                }
                where = f.getAbsolutePath();
            }
            bm.recycle();
            log("못 찾은 화면을 사진으로 남김: " + where + " — 문제를 알릴 때 같이 보내 주세요", 'd');
        } catch (Exception | OutOfMemoryError e) {
            log("화면 사진을 남기지 못함: " + e, 'd');
        }
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
            mv.goPlace("낚시 장소", fishRoute);
        } else if (m.equals("sellgo")) {
            mv.goPlace("물고기 판매 장소", sellRoute);
        } else if (m.startsWith("rec_")) {
            record(mv, m.endsWith("sell"));
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
                if (zeroSells >= 2) {
                    log("판매하러 갔는데 두 번 연속 팔 물고기가 없음 — 낚시 자리 문제로 보고 멈춤", 'y');
                    return false;
                }
                log("판매하러 갔는데 팔 물고기가 없음 — 낚시 자리를 다시 잡고 이어감", 'y');
                return true;
            } catch (Fisher.Stopped e) {
                throw e;
            } catch (RuntimeException e) {
                if (attempt >= SELL_TRIES) {
                    log("판매 실패 " + SELL_TRIES + "번 — 멈춤: " + e.getMessage() + " (판매 장소 길 · 위치 확인)", 'r');
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
        autoUpdate = p.getBoolean("autoUpdate", true);
        sellMax = p.getInt("sellMax", 100);
        sellDelay = num(p, "sellDelay", 0);
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
                .putBoolean("autoUpdate", autoUpdate)
                .putInt("sellMax", sellMax).putFloat("sellDelay", (float) sellDelay)
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
        save();
        changed();
    }
}
