package com.acrux.mobile;

import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.media.Image;
import android.media.MediaCodec;
import android.media.MediaCodecInfo;
import android.media.MediaFormat;
import android.media.MediaMuxer;
import android.os.Build;
import android.os.SystemClock;

import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * 버그 보고 — 매크로가 막히면 (또는 직접 신고) 기록 · 스크린샷 · 최근 1분 클립을 acruxmacro.com 으로 보냄
 *  · 앱에는 웹후크 · 토큰이 없음 (웹사이트가 받아서 개발자 탭 · 봇이 디스코드 채널에 올림)
 *  · 가벼운 클립 (디스코드용): 매크로가 돌고 로블록스가 맨 앞일 때만 초당 4장을 작게 (가로 800) JPEG 로 1분치 → 보낼 때 MP4
 *  · 고화질 클립 (웹사이트용): HdClip — 화질 · FPS 는 자동 (폰 성능) 또는 직접 · 따로 올림 (와이파이에서만 · 설정)
 *  · 스크린샷: 기기 스크린샷처럼 원본 크기 PNG · 알림이 화면을 덮고 있으면 안 찍음 · 기록의 웹후크 주소 · 전화번호 · 이메일 · IP 는 지움
 *  · 무엇이든 실패하면 조용히 넘어감 (매크로에 영향 없게) · 못 보낸 건 최대 3개까지 두고 나중에 다시
 */
final class BugReport {
    private BugReport() {}

    static final String URL = "https://acruxmacro.com/api/report";
    /** 설정 (Hub 가 불러오고 저장) */
    static volatile boolean on = true, clip = true;

    static final int FPS = 4, CLIP_SEC = 60, FRAME_W = 800;
    static final long AUTO_GAP_MS = 5 * 60_000L, SAME_GAP_MS = 30 * 60_000L, MANUAL_GAP_MS = 60_000L;
    static final int AUTO_PER_DAY = 12, MAX_PENDING = 3;
    /** 웹사이트가 받는 고화질 클립 한도 (100MB) 안 */
    static final long HD_MAX = 95L << 20;

    static final class Frame {
        final long t;
        final int w, h;
        final byte[] jpg;

        Frame(long t, int w, int h, byte[] jpg) {
            this.t = t;
            this.w = w;
            this.h = h;
            this.jpg = jpg;
        }
    }

    static final ArrayDeque<Frame> ring = new ArrayDeque<>();
    static final ExecutorService SEND = Executors.newSingleThreadExecutor(r -> {
        Thread t = new Thread(r, "acrux-report");
        t.setPriority(Thread.MIN_PRIORITY);
        return t;
    });
    static Thread sampler;
    static volatile long lastAuto, lastManual, lastRetry;
    static final java.util.Map<String, Long> lastByReason = new java.util.HashMap<>();
    static volatile String lastResult = "";

    /** 앱을 켤 때 한 번 (Hub.init) */
    static synchronized void start() {
        if (sampler != null) return;
        sampler = new Thread(BugReport::sampleLoop, "acrux-clip");
        sampler.setPriority(Thread.MIN_PRIORITY);
        sampler.setDaemon(true);
        sampler.start();
        SEND.execute(() -> {
            sendCrash();
            flush();
        });
    }

    static File dir() {
        File d = new File(Hub.app.getFilesDir(), "reports");
        //noinspection ResultOfMethodCallIgnored
        d.mkdirs();
        return d;
    }

    // ---------------------------------------------------------------- 1분 클립 (화면 → 작은 JPEG)

    static void sampleLoop() {
        Bitmap full = null, small = null;
        int[] wh = new int[2];
        android.graphics.Canvas sc = null;
        android.graphics.Paint paint = new android.graphics.Paint(android.graphics.Paint.FILTER_BITMAP_FLAG);
        long lastLite = 0, idleSince = 0;
        while (true) {
            long t0 = SystemClock.uptimeMillis();
            int[] hd = null;
            long period = 1000 / FPS;
            try {
                hd = on && clip ? HdClip.target() : null;
                com.acrux.mobile.core.Fisher fs = Hub.fisher;
                if (hd != null && fs != null && fs.phase == 2) hd = new int[]{hd[0], Math.min(hd[1], HdClip.REEL_FPS)};   // 릴링 중엔 덜 찍음
                if (hd != null) period = Math.max(20, 1000 / Math.max(FPS, hd[1]));
                CaptureService cap = CaptureService.active;
                if (on && clip && cap != null && Hub.running() && TapService.robloxSure()) {      // 매크로가 돌 때만 (배터리)
                    idleSince = 0;
                    boolean lite = t0 - lastLite >= 1000 / FPS - 20;
                    if (hd != null) {
                        full = cap.frame(full, wh);                  // 고화질: 화면을 한 번 복사해서 둘 다 씀
                        if (full != null) {
                            HdClip.feed(full, wh[0], wh[1], hd);
                            if (lite) {
                                lastLite = t0;
                                int lw = Math.min(FRAME_W, wh[0]), lh = (int) Math.round((double) wh[1] * lw / wh[0]);
                                if (small == null || small.getWidth() != lw || small.getHeight() != lh) {
                                    if (small != null) small.recycle();
                                    small = Bitmap.createBitmap(lw, lh, Bitmap.Config.ARGB_8888);
                                    sc = new android.graphics.Canvas(small);
                                }
                                sc.drawBitmap(full, new android.graphics.Rect(0, 0, wh[0], wh[1]), new android.graphics.Rect(0, 0, lw, lh), paint);
                                addLite(small, lw, lh);
                            }
                        }
                    } else {
                        HdClip.stop();
                        if (full != null) {                          // 고화질을 안 쓰면 큰 복사본은 내려놓음
                            full.recycle();
                            full = null;
                        }
                        if (lite) {                                  // 가벼운 클립만: 화면을 건너뛰며 작게만 복사 (전체 복사 없이)
                            lastLite = t0;
                            int[] px = cap.sample(FRAME_W, wh);
                            if (px != null) {
                                if (small == null || small.getWidth() != wh[0] || small.getHeight() != wh[1]) {
                                    if (small != null) small.recycle();
                                    small = Bitmap.createBitmap(wh[0], wh[1], Bitmap.Config.ARGB_8888);
                                    sc = new android.graphics.Canvas(small);
                                }
                                small.setPixels(px, 0, wh[0], 0, 0, wh[0], wh[1]);
                                addLite(small, wh[0], wh[1]);
                            }
                        }
                    }
                } else {
                    if (idleSince == 0) idleSince = t0;
                    else if (t0 - idleSince > 60_000L) {          // 1분 넘게 쉼 → 고화질 녹화 · 화면 복사본 내려놓기 (메모리)
                        HdClip.stop();
                        if (full != null) {
                            full.recycle();
                            full = null;
                        }
                    }
                }
                synchronized (ring) {
                    long keep = SystemClock.uptimeMillis() - CLIP_SEC * 1000L;
                    while (!ring.isEmpty() && ring.peekFirst().t < keep) ring.pollFirst();
                    if (!on || !clip) ring.clear();
                }
                if (!on || !clip) HdClip.stop();
                if (t0 - lastRetry > 10 * 60_000L && hasPending()) {
                    lastRetry = t0;
                    SEND.execute(BugReport::flush);
                }
            } catch (Throwable ignored) {                 // 클립은 덤 — 무슨 일이 있어도 매크로엔 영향 없게
            }
            long work = SystemClock.uptimeMillis() - t0;
            if (hd != null) HdClip.load(work, period, t0);
            try {
                Thread.sleep(Math.max(hd != null ? 5 : 40, period - work));
            } catch (InterruptedException e) {
                return;
            }
        }
    }

    /** 가벼운 클립 한 장 (JPEG) */
    static void addLite(Bitmap b, int w, int h) {
        ByteArrayOutputStream o = new ByteArrayOutputStream(64 * 1024);
        b.compress(Bitmap.CompressFormat.JPEG, 55, o);
        Frame f = new Frame(SystemClock.uptimeMillis(), w, h, o.toByteArray());
        synchronized (ring) {
            ring.addLast(f);
        }
    }

    /** 기기 스크린샷처럼: 원본 크기 PNG (화면 그대로 · 손실 없음) */
    static byte[] screenshot(CaptureService cap) {
        int[] wh = new int[2];
        Bitmap full = cap.frame(null, wh);
        if (full == null) return null;
        Bitmap b = wh[0] == full.getWidth() ? full : Bitmap.createBitmap(full, 0, 0, wh[0], wh[1]);
        try {
            ByteArrayOutputStream o = new ByteArrayOutputStream(4 << 20);
            b.compress(Bitmap.CompressFormat.PNG, 100, o);
            return o.toByteArray();
        } finally {
            if (b != full) b.recycle();
            full.recycle();
        }
    }

    // ---------------------------------------------------------------- 보내기

    /** 매크로 기록 한 줄마다 (Hub.log) — 막힌 걸로 보이는 줄이면 자동 보고 */
    static void onLog(String msg, char color) {
        if (!on || msg == null || !Hub.running()) return;
        if (!(color == 'r' && !msg.startsWith("웹후크") && !msg.startsWith("ADB")) && !stuck(msg)) return;
        if (notBug(msg) || !TapService.robloxFront()) return;   // 로블록스가 앞에 없으면 클립에 찍힐 것도 없음
        auto(msg);
    }

    /** 버그가 아니라 사용자 쪽 상태 · 설정 문제 — 자동 보고 안 함 (로블록스가 앞에 없음 · 권한 꺼짐 · 위치 미지정 · 화면을 건드림 · "~해 주세요" 안내) */
    static final String[] NOT_BUG = {
            "맨 앞에 없음", "접근성 권한", "먼저 지정", "먼저 조이스틱", "지정되지 않음", "화면이 계속 눌려 있어서", "화면을 아직 못 받음", "주세요",
    };

    static boolean notBug(String msg) {
        for (String s : NOT_BUG) if (msg.contains(s)) return true;
        return false;
    }

    /** 자동 낚시가 막힌 줄 (PC · 모바일 공통 문구 일부) */
    static final String[] STUCK = {
            "초 동안 안 보임", "한동안 낚시가 진행되지 않음", "결과창을 닫은 뒤", "아우라 도감이 안 열림 — 아우라 도감 버튼 다시 누름 (3/3)",
            "끝나지 않음 — 멈춘 것으로 보고", "자동 낚시 오류", "Exit 버튼이 사라졌는데",
            "연속 실패", "낚시 장소로 다시 감 —", "서버 체크 오류", "내 서버로 복귀 오류", "낚시 장소 복구 실패",
            "확인창 Sell 을 눌러도 안 닫힘", "상점이 안 열림", "아우라 도감 [Exit] 가 씹힘 — 다시 누름 (5/5)",
    };

    static boolean stuck(String msg) {
        // 판매 실패: 마지막 시도까지 실패했을 때만 ("판매 실패 3번: …") · 중간 ("판매 실패 (1/3)") 은 다시 하는 중
        if (msg.startsWith("판매 실패 ") && !msg.startsWith("판매 실패 (")) return true;
        // "Fish 를 N번 눌러도 반응 없음 — 낚시 인벤토리 가득" 은 판매하러 가는 평소 흐름이라 안 셈
        for (String s : STUCK) if (msg.contains(s)) return true;
        return false;
    }

    static synchronized void auto(String reason) {
        long now = System.currentTimeMillis();
        String key = reason.replaceAll("[0-9]+", "#");
        Long same = lastByReason.get(key);
        if (now - lastAuto < AUTO_GAP_MS || (same != null && now - same < SAME_GAP_MS)) return;
        String day = new java.text.SimpleDateFormat("yyyyMMdd", Locale.ROOT).format(new java.util.Date());
        android.content.SharedPreferences p = Hub.prefs();
        int n = day.equals(p.getString("bugDay", "")) ? p.getInt("bugDayN", 0) : 0;
        if (n >= AUTO_PER_DAY) return;
        p.edit().putString("bugDay", day).putInt("bugDayN", n + 1).apply();
        lastAuto = now;
        lastByReason.put(key, now);
        submit(reason, false);
    }

    /** 직접 신고 (버튼) → 바로 보내기 시작했으면 true */
    static synchronized boolean manual() {
        long now = System.currentTimeMillis();
        if (now - lastManual < MANUAL_GAP_MS) {
            Hub.log("버그 신고는 1분에 한 번만 보낼 수 있음", 'y');
            return false;
        }
        lastManual = now;
        submit("직접 신고", true);
        return true;
    }

    static void submit(String reason, boolean manual) {
        final String state = Hub.state, mode = Hub.mode;
        SEND.execute(() -> {
            try {
                if (!manual) Thread.sleep(3000);                  // 막힌 뒤 화면도 조금 담기
                File d = make(reason, manual, state, mode);
                if (d != null) flush();
            } catch (Throwable e) {
                Hub.log("버그 보고를 만들지 못함: " + e, 'd');
            }
        });
    }

    /** 보고 한 건을 파일로 (reports/<시각>/meta.json · log.txt · shot.jpg · clip.mp4) */
    static File make(String reason, boolean manual, String state, String mode) throws Exception {
        List<Frame> frames;
        synchronized (ring) {
            frames = new ArrayList<>(ring);
        }
        File d = new File(dir(), String.valueOf(System.currentTimeMillis()));
        if (!d.mkdirs()) return null;
        JSONObject m = new JSONObject();
        m.put("install", Online.id());
        m.put("app", "mobile");
        m.put("v", Online.BuildConfigVersion.get());
        m.put("code", code());
        m.put("reason", reason.length() > 280 ? reason.substring(0, 280) : reason);
        m.put("manual", manual);
        m.put("model", (Build.MANUFACTURER + " " + Build.MODEL).trim());
        m.put("android", Build.VERSION.RELEASE + " (" + Build.VERSION.SDK_INT + ")");
        CaptureService cap = CaptureService.active;
        if (cap != null) m.put("screen", cap.width() + "x" + cap.height());
        m.put("lang", com.acrux.mobile.core.I18n.lang());
        m.put("mode", mode == null ? "" : mode);
        m.put("state", state == null ? "" : state);
        m.put("device", device());
        m.put("at", System.currentTimeMillis());
        // 스크린샷 (지금 화면 · 로블록스가 맨 앞일 때만 · 기기 스크린샷처럼 원본 PNG)
        if (cap != null && TapService.robloxSure()) {
            byte[] png = screenshot(cap);
            if (png != null) write(new File(d, "shot.png"), png);
        }
        // 고화질 클립 (최근 1분 · 웹사이트용)
        if (clip) {
            try {
                File hdF = new File(d, "hd.mp4");
                int sec = HdClip.export(hdF);
                if (sec >= 0) {
                    m.put("hd", HdClip.describeNow());
                    m.put("hdSec", sec);
                    if (hdF.length() > HD_MAX) {
                        //noinspection ResultOfMethodCallIgnored
                        hdF.delete();
                        Hub.log("버그 보고 고화질 클립이 너무 커서 뺌 (" + (hdF.length() >> 20) + "MB)", 'd');
                    }
                }
            } catch (Throwable e) {
                //noinspection ResultOfMethodCallIgnored
                new File(d, "hd.mp4").delete();
                Hub.log("버그 보고 고화질 클립을 만들지 못함: " + e, 'd');
            }
        }
        // 클립 (최근 1분)
        if (clip && frames.size() >= 8) {
            try {
                long t0 = SystemClock.elapsedRealtime();
                int sec = encode(frames, new File(d, "clip.mp4"));
                m.put("clipSec", sec);
                Hub.log("버그 보고 클립 " + sec + "초 (" + (SystemClock.elapsedRealtime() - t0) + "ms)", 'd');
            } catch (Throwable e) {
                //noinspection ResultOfMethodCallIgnored
                new File(d, "clip.mp4").delete();
                Hub.log("버그 보고 클립을 만들지 못함 — 클립 없이 보냄: " + e, 'd');
            }
        }
        StringBuilder log = new StringBuilder();
        for (String l : Hub.recentLogs(250)) log.append(l).append('\n');
        write(new File(d, "log.txt"), scrub(log.toString()).getBytes(StandardCharsets.UTF_8));
        write(new File(d, "meta.json"), m.toString().getBytes(StandardCharsets.UTF_8));
        trim();
        return d;
    }

    /** 지난번에 앱이 튕겼으면 그 기록을 한 번 보냄 (클립 없음 · 같은 기록은 한 번만) */
    static void sendCrash() {
        try {
            if (!on) return;
            String c = Crash.read();
            if (c == null || c.isEmpty()) return;
            String h = Integer.toHexString(c.hashCode());
            if (h.equals(Hub.prefs().getString("bugCrash", ""))) return;
            Hub.prefs().edit().putString("bugCrash", h).apply();
            File d = new File(dir(), String.valueOf(System.currentTimeMillis()));
            if (!d.mkdirs()) return;
            JSONObject m = new JSONObject();
            m.put("install", Online.id());
            m.put("app", "mobile");
            m.put("v", Online.BuildConfigVersion.get());
            m.put("code", code());
            m.put("reason", "앱이 튕김 (지난번 실행)");
            m.put("model", (Build.MANUFACTURER + " " + Build.MODEL).trim());
            m.put("android", Build.VERSION.RELEASE + " (" + Build.VERSION.SDK_INT + ")");
            m.put("lang", com.acrux.mobile.core.I18n.lang());
            m.put("device", device());
            m.put("at", System.currentTimeMillis());
            write(new File(d, "log.txt"), scrub(c).getBytes(StandardCharsets.UTF_8));
            write(new File(d, "meta.json"), m.toString().getBytes(StandardCharsets.UTF_8));
            trim();
        } catch (Throwable ignored) {
        }
    }

    /**
     * 기기 정보 (버그가 특정 기종에서만 나는지 보려고) — 이름 · 칩 · 메모리 · 화면 · 로블록스 버전 · 배터리 · 발열 · 네트워크 종류
     *  · 기기 고유 번호 (IMEI · 시리얼 · 광고 ID 등) 는 안 넣음
     */
    static JSONObject device() {
        JSONObject d = new JSONObject();
        android.content.Context c = Hub.app;
        put(d, "brand", Build.BRAND);
        put(d, "manufacturer", Build.MANUFACTURER);
        put(d, "model", Build.MODEL);
        put(d, "device", Build.DEVICE);
        put(d, "hardware", Build.HARDWARE);
        if (Build.VERSION.SDK_INT >= 31) put(d, "soc", (Build.SOC_MANUFACTURER + " " + Build.SOC_MODEL).trim());
        put(d, "abi", Build.SUPPORTED_ABIS.length > 0 ? Build.SUPPORTED_ABIS[0] : "");
        put(d, "android", Build.VERSION.RELEASE);
        put(d, "sdk", String.valueOf(Build.VERSION.SDK_INT));
        put(d, "build", Build.DISPLAY);
        put(d, "cores", String.valueOf(Runtime.getRuntime().availableProcessors()));
        put(d, "heapMax", (Runtime.getRuntime().maxMemory() >> 20) + "MB");
        try {
            android.app.ActivityManager am = (android.app.ActivityManager) c.getSystemService(android.content.Context.ACTIVITY_SERVICE);
            android.app.ActivityManager.MemoryInfo mi = new android.app.ActivityManager.MemoryInfo();
            am.getMemoryInfo(mi);
            put(d, "ram", String.format(Locale.ROOT, "%.1fGB", mi.totalMem / 1073741824.0));
            put(d, "ramFree", (mi.availMem >> 20) + "MB" + (mi.lowMemory ? " (low)" : ""));
        } catch (Throwable ignored) {
        }
        try {
            android.util.DisplayMetrics dm = c.getResources().getDisplayMetrics();
            put(d, "display", Math.max(dm.widthPixels, dm.heightPixels) + "x" + Math.min(dm.widthPixels, dm.heightPixels) + " · " + dm.densityDpi + "dpi");
            android.hardware.display.DisplayManager dmg = (android.hardware.display.DisplayManager) c.getSystemService(android.content.Context.DISPLAY_SERVICE);
            android.view.Display disp = dmg.getDisplay(android.view.Display.DEFAULT_DISPLAY);
            if (disp != null) put(d, "refresh", Math.round(disp.getRefreshRate()) + "Hz");
        } catch (Throwable ignored) {
        }
        CaptureService cap = CaptureService.active;
        if (cap != null) put(d, "capture", cap.width() + "x" + cap.height());
        try {
            String rp = TapService.roblox != null ? TapService.roblox : "com.roblox.client";
            put(d, "roblox", c.getPackageManager().getPackageInfo(rp, 0).versionName);
        } catch (Throwable e) {
            put(d, "roblox", "?");
        }
        try {
            android.os.BatteryManager bm = (android.os.BatteryManager) c.getSystemService(android.content.Context.BATTERY_SERVICE);
            put(d, "battery", bm.getIntProperty(android.os.BatteryManager.BATTERY_PROPERTY_CAPACITY) + "%" + (Build.VERSION.SDK_INT >= 23 && bm.isCharging() ? " (charging)" : ""));
        } catch (Throwable ignored) {
        }
        try {
            if (Build.VERSION.SDK_INT >= 29) {
                android.os.PowerManager pm = (android.os.PowerManager) c.getSystemService(android.content.Context.POWER_SERVICE);
                String[] th = {"none", "light", "moderate", "severe", "critical", "emergency", "shutdown"};
                int t = pm.getCurrentThermalStatus();
                put(d, "thermal", t >= 0 && t < th.length ? th[t] : String.valueOf(t));
                put(d, "powerSave", pm.isPowerSaveMode() ? "on" : "off");
            }
        } catch (Throwable ignored) {
        }
        try {
            android.net.ConnectivityManager cm = (android.net.ConnectivityManager) c.getSystemService(android.content.Context.CONNECTIVITY_SERVICE);
            android.net.NetworkCapabilities nc = cm.getNetworkCapabilities(cm.getActiveNetwork());
            put(d, "network", nc == null ? "none" : nc.hasTransport(android.net.NetworkCapabilities.TRANSPORT_WIFI) ? "wifi"
                    : nc.hasTransport(android.net.NetworkCapabilities.TRANSPORT_CELLULAR) ? "mobile" : "other");
        } catch (Throwable ignored) {
        }
        try {
            put(d, "storageFree", (c.getFilesDir().getUsableSpace() >> 20) + "MB");
        } catch (Throwable ignored) {
        }
        put(d, "accessibility", TapService.instance != null ? "on" : "off");
        put(d, "adb", Adb.connected ? "on" : "off");
        synchronized (ring) {
            put(d, "clipFrames", String.valueOf(ring.size()));
        }
        return d;
    }

    static void put(JSONObject o, String k, String v) {
        try {
            if (v != null && !v.trim().isEmpty()) o.put(k, v.trim().length() > 80 ? v.trim().substring(0, 80) : v.trim());
        } catch (Exception ignored) {
        }
    }

    static int code() {
        try {
            android.content.pm.PackageInfo i = Hub.app.getPackageManager().getPackageInfo(Hub.app.getPackageName(), 0);
            return Build.VERSION.SDK_INT >= 28 ? (int) i.getLongVersionCode() : i.versionCode;
        } catch (Exception e) {
            return 0;
        }
    }

    /** 기록에서 비밀 · 개인 정보 지우기 (웹사이트에서도 한 번 더 지움) */
    static String scrub(String s) {
        return s.replaceAll("https?://(?:(?:ptb|canary)\\.)?discord(?:app)?\\.com/api/(?:v\\d+/)?webhooks/\\S+", "[webhook]")
                .replaceAll("[\\w-]{24,26}\\.[\\w-]{6}\\.[\\w-]{27,38}", "[token]")
                .replaceAll("[\\w.+-]+@[\\w-]+\\.[\\w.-]+", "[email]")
                .replaceAll("\\+\\d{1,3}[- ]?\\d{1,4}[- ]?\\d{3,4}[- ]?\\d{4}\\b", "[phone]")
                .replaceAll("\\b0\\d{1,2}[- ]\\d{3,4}[- ]\\d{4}\\b", "[phone]")
                .replaceAll("\\b01[016789]\\d{7,8}\\b", "[phone]")
                .replaceAll("\\b(?:\\d{1,3}\\.){3}\\d{1,3}\\b", "[ip]");
    }

    static void write(File f, byte[] b) throws java.io.IOException {
        try (FileOutputStream o = new FileOutputStream(f)) {
            o.write(b);
        }
    }

    static File[] pending() {
        File[] fs = dir().listFiles(File::isDirectory);
        if (fs == null) return new File[0];
        Arrays.sort(fs, (a, b) -> a.getName().compareTo(b.getName()));
        return fs;
    }

    static boolean hasPending() {
        return pending().length > 0;
    }

    /** 못 보낸 게 너무 많거나 오래됐으면 오래된 것부터 지움 */
    static void trim() {
        File[] fs = pending();
        long old = System.currentTimeMillis() - 24 * 3600_000L;
        for (int i = 0; i < fs.length; i++) {
            long t;
            try {
                t = Long.parseLong(fs[i].getName());
            } catch (NumberFormatException e) {
                t = 0;
            }
            if (i < fs.length - MAX_PENDING || t < old) delete(fs[i]);
        }
    }

    static void delete(File d) {
        File[] fs = d.listFiles();
        if (fs != null) for (File f : fs) //noinspection ResultOfMethodCallIgnored
            f.delete();
        //noinspection ResultOfMethodCallIgnored
        d.delete();
    }

    /** 쌓인 보고를 오래된 것부터 보냄 (SEND 스레드에서) — 보고 → 고화질 클립 (따로 · 와이파이에서만 설정이면 와이파이일 때) */
    static void flush() {
        for (File d : pending()) {
            File sent = new File(d, "sent.json");
            if (!sent.exists()) {
                int code = upload(d);
                if (code == 400 || code == 413) {
                    delete(d);                                 // 웹사이트가 받을 수 없는 보고 → 다시 보내도 같음
                    lastResult = "bad";
                    continue;
                }
                if (code != 200) {
                    lastResult = code == 429 ? "rate" : "net";
                    break;                                     // 네트워크 · 횟수 제한 → 나중에 다시
                }
                lastResult = "ok";
            }
            File hd = new File(d, "hd.mp4");
            if (!hd.exists()) {
                delete(d);
                continue;
            }
            if (HdClip.wifiOnly && !wifi()) continue;          // 고화질 클립은 와이파이가 될 때 (하루 안)
            int code = uploadHd(d, hd);
            if (code == 200 || code == 403 || code == 400 || code == 413) delete(d);   // 끝 · 열쇠 만료 · 못 받는 파일
        }
    }

    static boolean wifi() {
        try {
            android.net.ConnectivityManager cm = (android.net.ConnectivityManager) Hub.app.getSystemService(android.content.Context.CONNECTIVITY_SERVICE);
            android.net.NetworkCapabilities nc = cm.getNetworkCapabilities(cm.getActiveNetwork());
            return nc != null && (nc.hasTransport(android.net.NetworkCapabilities.TRANSPORT_WIFI) || nc.hasTransport(android.net.NetworkCapabilities.TRANSPORT_ETHERNET));
        } catch (Throwable e) {
            return false;
        }
    }

    /** 보고 한 건 → HTTP 코드 (0 = 연결 실패) · 받으면 sent.json 에 번호 · 고화질 클립 열쇠 */
    static int upload(File d) {
        File meta = new File(d, "meta.json");
        if (!meta.exists()) return 400;
        String b = "----acrux" + Long.toHexString(System.nanoTime());
        HttpURLConnection c = null;
        try {
            c = (HttpURLConnection) new URL(URL).openConnection();
            c.setConnectTimeout(15_000);
            c.setReadTimeout(60_000);
            c.setDoOutput(true);
            c.setRequestMethod("POST");
            c.setChunkedStreamingMode(64 * 1024);
            c.setRequestProperty("Content-Type", "multipart/form-data; boundary=" + b);
            c.setRequestProperty("User-Agent", "AcruxMobile/" + Online.BuildConfigVersion.get());
            try (DataOutputStream o = new DataOutputStream(c.getOutputStream())) {
                part(o, b, "meta", null, null, meta);
                File log = new File(d, "log.txt");
                if (log.exists()) part(o, b, "log", null, null, log);
                File png = new File(d, "shot.png"), jpg = new File(d, "shot.jpg");
                if (png.exists()) part(o, b, "shot", "shot.png", "image/png", png);
                else if (jpg.exists()) part(o, b, "shot", "shot.jpg", "image/jpeg", jpg);
                File clipF = new File(d, "clip.mp4");
                if (clipF.exists()) part(o, b, "clip", "clip.mp4", "video/mp4", clipF);
                o.writeBytes("--" + b + "--\r\n");
            }
            int code = c.getResponseCode();
            String body = read(code < 400 ? c.getInputStream() : c.getErrorStream());
            if (code == 200) {
                JSONObject r = new JSONObject(body);
                String id = r.optString("id", "");
                write(new File(d, "sent.json"), r.toString().getBytes(StandardCharsets.UTF_8));
                Hub.log("버그 보고를 보냄" + (id.isEmpty() ? "" : " (#" + id + ")"), 'd');
            } else Hub.log("버그 보고를 못 보냄 (" + code + ")", 'd');
            return code;
        } catch (Exception e) {
            return 0;
        } finally {
            if (c != null) c.disconnect();
        }
    }

    /** 고화질 클립 (PUT · 받은 번호 · 열쇠로) → HTTP 코드 */
    static int uploadHd(File d, File hd) {
        HttpURLConnection c = null;
        try {
            JSONObject r = new JSONObject(new String(java.nio.file.Files.readAllBytes(new File(d, "sent.json").toPath()), StandardCharsets.UTF_8));
            String id = r.optString("id", ""), key = r.optString("hd", "");
            if (id.isEmpty() || key.isEmpty()) return 400;
            c = (HttpURLConnection) new URL(URL + "/" + id + "/hd?k=" + key).openConnection();
            c.setConnectTimeout(15_000);
            c.setReadTimeout(120_000);
            c.setDoOutput(true);
            c.setRequestMethod("PUT");
            c.setFixedLengthStreamingMode(hd.length());
            c.setRequestProperty("Content-Type", "video/mp4");
            c.setRequestProperty("User-Agent", "AcruxMobile/" + Online.BuildConfigVersion.get());
            try (OutputStream o = c.getOutputStream(); InputStream in = new FileInputStream(hd)) {
                byte[] buf = new byte[64 * 1024];
                int n;
                while ((n = in.read(buf)) > 0) o.write(buf, 0, n);
            }
            int code = c.getResponseCode();
            read(code < 400 ? c.getInputStream() : c.getErrorStream());
            Hub.log(code == 200 ? "버그 보고 고화질 클립을 보냄 (#" + id + " · " + (hd.length() >> 20) + "MB)" : "버그 보고 고화질 클립을 못 보냄 (" + code + ")", 'd');
            return code;
        } catch (Exception e) {
            return 0;
        } finally {
            if (c != null) c.disconnect();
        }
    }

    static void part(DataOutputStream o, String b, String name, String file, String mime, File f) throws java.io.IOException {
        o.writeBytes("--" + b + "\r\n");
        o.writeBytes("Content-Disposition: form-data; name=\"" + name + "\"" + (file != null ? "; filename=\"" + file + "\"" : "") + "\r\n");
        o.writeBytes("Content-Type: " + (mime != null ? mime : "text/plain; charset=utf-8") + "\r\n\r\n");
        try (InputStream in = new FileInputStream(f)) {
            byte[] buf = new byte[32 * 1024];
            int n;
            while ((n = in.read(buf)) > 0) o.write(buf, 0, n);
        }
        o.writeBytes("\r\n");
    }

    static String read(InputStream in) {
        if (in == null) return "";
        try (InputStream s = in) {
            ByteArrayOutputStream o = new ByteArrayOutputStream();
            byte[] b = new byte[4096];
            int n;
            while ((n = s.read(b)) > 0 && o.size() < 16 * 1024) o.write(b, 0, n);
            return o.toString("UTF-8");
        } catch (Exception e) {
            return "";
        }
    }

    // ---------------------------------------------------------------- MP4 (H.264)

    /** JPEG 장면들 → MP4 (실제 찍힌 시각대로) → 길이 (초) */
    static int encode(List<Frame> all, File out) throws Exception {
        // 마지막 장면과 크기가 같은 것만 (중간에 화면을 돌렸으면 그 앞은 버림)
        Frame lastF = all.get(all.size() - 1);
        List<Frame> fs = new ArrayList<>();
        for (Frame f : all) if (f.w == lastF.w && f.h == lastF.h) fs.add(f);
        if (fs.size() < 4) throw new IllegalStateException("frames");
        int w = lastF.w & ~15, h = lastF.h & ~15;
        if (w < 64 || h < 64) throw new IllegalStateException("size");
        MediaFormat fmt = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, w, h);
        fmt.setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatYUV420Flexible);
        fmt.setInteger(MediaFormat.KEY_BIT_RATE, 900_000);
        fmt.setInteger(MediaFormat.KEY_FRAME_RATE, FPS);
        fmt.setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 2);
        MediaCodec codec = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_VIDEO_AVC);
        MediaMuxer mux = null;
        boolean started = false;
        try {
            codec.configure(fmt, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE);
            codec.start();
            mux = new MediaMuxer(out.getAbsolutePath(), MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4);
            MediaCodec.BufferInfo info = new MediaCodec.BufferInfo();
            int track = -1;
            long t0 = fs.get(0).t, lastPts = -1;
            int[] px = new int[w * h];
            int i = 0;
            boolean inputDone = false, outputDone = false;
            long deadline = SystemClock.elapsedRealtime() + 120_000;
            while (!outputDone) {
                if (SystemClock.elapsedRealtime() > deadline) throw new IllegalStateException("timeout");
                if (!inputDone) {
                    int in = codec.dequeueInputBuffer(10_000);
                    if (in >= 0) {
                        if (i >= fs.size()) {
                            codec.queueInputBuffer(in, 0, 0, Math.max(0, lastPts + 1), MediaCodec.BUFFER_FLAG_END_OF_STREAM);
                            inputDone = true;
                        } else {
                            Frame f = fs.get(i++);
                            Bitmap bm = BitmapFactory.decodeByteArray(f.jpg, 0, f.jpg.length);
                            if (bm == null) throw new IllegalStateException("decode");
                            try {
                                bm.getPixels(px, 0, w, 0, 0, Math.min(w, bm.getWidth()), Math.min(h, bm.getHeight()));
                            } finally {
                                bm.recycle();
                            }
                            Image img = codec.getInputImage(in);
                            if (img == null) throw new IllegalStateException("image");
                            fillYuv(img, px, w, h);
                            long pts = Math.max(lastPts + 1, (f.t - t0) * 1000);
                            lastPts = pts;
                            codec.queueInputBuffer(in, 0, w * h * 3 / 2, pts, 0);
                        }
                    }
                }
                int o = codec.dequeueOutputBuffer(info, 10_000);
                if (o == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {
                    track = mux.addTrack(codec.getOutputFormat());
                    mux.start();
                    started = true;
                } else if (o >= 0) {
                    ByteBuffer buf = codec.getOutputBuffer(o);
                    if ((info.flags & MediaCodec.BUFFER_FLAG_CODEC_CONFIG) != 0) info.size = 0;
                    if (info.size > 0 && started && buf != null) {
                        buf.position(info.offset);
                        buf.limit(info.offset + info.size);
                        mux.writeSampleData(track, buf, info);
                    }
                    codec.releaseOutputBuffer(o, false);
                    if ((info.flags & MediaCodec.BUFFER_FLAG_END_OF_STREAM) != 0) outputDone = true;
                }
            }
            return (int) Math.round((fs.get(fs.size() - 1).t - t0) / 1000.0);
        } finally {
            try {
                codec.stop();
            } catch (Exception ignored) {
            }
            codec.release();
            if (mux != null) {
                try {
                    if (started) mux.stop();
                } catch (Exception ignored) {
                }
                mux.release();
            }
        }
    }

    /** ARGB → YUV420 (인코더 입력 이미지의 줄 간격 · 칸 간격대로) */
    static void fillYuv(Image img, int[] px, int w, int h) {
        Image.Plane[] p = img.getPlanes();
        ByteBuffer yb = p[0].getBuffer(), ub = p[1].getBuffer(), vb = p[2].getBuffer();
        int yr = p[0].getRowStride(), yp = p[0].getPixelStride();
        int ur = p[1].getRowStride(), up = p[1].getPixelStride(), vr = p[2].getRowStride(), vp = p[2].getPixelStride();
        for (int y = 0; y < h; y++) {
            int row = y * w;
            for (int x = 0; x < w; x++) {
                int c = px[row + x], r = (c >> 16) & 255, g = (c >> 8) & 255, b = c & 255;
                yb.put(y * yr + x * yp, (byte) (((66 * r + 129 * g + 25 * b + 128) >> 8) + 16));
                if ((y & 1) == 0 && (x & 1) == 0) {
                    int k = (y >> 1);
                    ub.put(k * ur + (x >> 1) * up, (byte) (((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128));
                    vb.put(k * vr + (x >> 1) * vp, (byte) (((112 * r - 94 * g - 18 * b + 128) >> 8) + 128));
                }
            }
        }
    }
}
