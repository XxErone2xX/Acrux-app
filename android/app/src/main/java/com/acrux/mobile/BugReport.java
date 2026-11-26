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
 *  · 클립: 매크로가 돌고 로블록스가 맨 앞일 때만 초당 4장을 작게 (가로 800) JPEG 로 1분치만 들고 있다가 → 보낼 때 MP4 로 만듦
 *  · 화면 위쪽 (알림이 뜰 수 있는 곳) 은 모자이크 · 기록의 웹후크 주소 · 전화번호 · 이메일 · IP 는 지움
 *  · 무엇이든 실패하면 조용히 넘어감 (매크로에 영향 없게) · 못 보낸 건 최대 3개까지 두고 나중에 다시
 */
final class BugReport {
    private BugReport() {}

    static final String URL = "https://acruxmacro.com/api/report";
    /** 설정 (Hub 가 불러오고 저장) */
    static volatile boolean on = true, clip = true;

    static final int FPS = 4, CLIP_SEC = 60, FRAME_W = 800;
    /** 화면 위쪽 이 비율만큼 모자이크 (알림 · 상태 표시줄) */
    static final double TOP_BLUR = 0.16;
    static final long AUTO_GAP_MS = 5 * 60_000L, SAME_GAP_MS = 30 * 60_000L, MANUAL_GAP_MS = 60_000L;
    static final int AUTO_PER_DAY = 12, MAX_PENDING = 3;

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
        long period = 1000 / FPS;
        while (true) {
            long t0 = SystemClock.uptimeMillis();
            try {
                CaptureService cap = CaptureService.active;
                if (on && clip && cap != null && Hub.running() && TapService.robloxSure()) {      // 매크로가 돌 때만 (배터리)
                    Frame f = grab(cap, FRAME_W, 55);
                    if (f != null) synchronized (ring) {
                        ring.addLast(f);
                    }
                }
                synchronized (ring) {
                    long keep = SystemClock.uptimeMillis() - CLIP_SEC * 1000L;
                    while (!ring.isEmpty() && ring.peekFirst().t < keep) ring.pollFirst();
                    if (!on || !clip) ring.clear();
                }
                if (t0 - lastRetry > 10 * 60_000L && hasPending()) {
                    lastRetry = t0;
                    SEND.execute(BugReport::flush);
                }
            } catch (Throwable ignored) {                 // 클립은 덤 — 무슨 일이 있어도 매크로엔 영향 없게
            }
            long wait = period - (SystemClock.uptimeMillis() - t0);
            try {
                Thread.sleep(Math.max(40, wait));
            } catch (InterruptedException e) {
                return;
            }
        }
    }

    /** 지금 화면을 가로 maxW 이하로 줄여서 (위쪽 모자이크) JPEG */
    static Frame grab(CaptureService cap, int maxW, int quality) {
        int[] wh = new int[2];
        int[] px = cap.sample(maxW, wh);
        if (px == null) return null;
        Bitmap b = Bitmap.createBitmap(wh[0], wh[1], Bitmap.Config.ARGB_8888);
        try {
            blurTop(px, wh[0], wh[1]);
            b.setPixels(px, 0, wh[0], 0, 0, wh[0], wh[1]);
            ByteArrayOutputStream o = new ByteArrayOutputStream(64 * 1024);
            b.compress(Bitmap.CompressFormat.JPEG, quality, o);
            return new Frame(SystemClock.uptimeMillis(), wh[0], wh[1], o.toByteArray());
        } finally {
            b.recycle();
        }
    }

    /** 화면 위쪽을 큰 칸 모자이크 (알림 내용 · 상태 표시줄이 안 읽히게) */
    static void blurTop(int[] px, int w, int h) {
        int band = (int) Math.ceil(h * TOP_BLUR), s = Math.max(8, w / 60);
        for (int y0 = 0; y0 < band; y0 += s) {
            for (int x0 = 0; x0 < w; x0 += s) {
                int y1 = Math.min(band, y0 + s), x1 = Math.min(w, x0 + s);
                long r = 0, g = 0, bl = 0, n = 0;
                for (int y = y0; y < y1; y++)
                    for (int x = x0; x < x1; x++) {
                        int c = px[y * w + x];
                        r += (c >> 16) & 255;
                        g += (c >> 8) & 255;
                        bl += c & 255;
                        n++;
                    }
                int c = 0xff000000 | (int) (r / n) << 16 | (int) (g / n) << 8 | (int) (bl / n);
                for (int y = y0; y < y1; y++) Arrays.fill(px, y * w + x0, y * w + x1, c);
            }
        }
    }

    // ---------------------------------------------------------------- 보내기

    /** 매크로 기록 한 줄마다 (Hub.log) — 막힌 걸로 보이는 줄이면 자동 보고 */
    static void onLog(String msg, char color) {
        if (!on || msg == null || !Hub.running()) return;
        if (!(color == 'r' && !msg.startsWith("웹후크") && !msg.startsWith("ADB")) && !stuck(msg)) return;
        auto(msg);
    }

    /** 자동 낚시가 막힌 줄 (PC · 모바일 공통 문구 일부) */
    static final String[] STUCK = {
            "초 동안 안 보임", "한동안 낚시가 진행되지 않음", "결과창을 닫은 뒤", "아우라 도감이 안 열림 — 아우라 도감 버튼 다시 누름 (3/3)",
            "판매 실패", "번 눌러도 반응 없음", "끝나지 않음 — 멈춘 것으로 보고", "자동 낚시 오류", "Exit 버튼이 사라졌는데",
            "연속 실패", "낚시 장소로 다시 감 —", "서버 체크 오류", "내 서버로 복귀 오류", "낚시 장소 복구 실패",
            "확인창 Sell 을 눌러도 안 닫힘", "상점이 안 열림", "아우라 도감 [Exit] 가 씹힘 — 다시 누름 (5/5)",
    };

    static boolean stuck(String msg) {
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
        m.put("stats", stats());
        m.put("device", device());
        m.put("at", System.currentTimeMillis());
        // 스크린샷 (지금 화면 · 로블록스가 맨 앞일 때만)
        if (cap != null && TapService.robloxSure()) {
            Frame shot = grab(cap, 1600, 80);
            if (shot != null) write(new File(d, "shot.jpg"), shot.jpg);
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

    static String stats() {
        StringBuilder s = new StringBuilder();
        for (int i = 0; i < Hub.STAT_KEYS.length; i++) {
            if (s.length() > 0) s.append(" · ");
            s.append(Hub.STAT_KEYS[i]).append(' ').append(Hub.statRun[i]);
        }
        return s.toString();
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

    /** 쌓인 보고를 오래된 것부터 보냄 (SEND 스레드에서) */
    static void flush() {
        for (File d : pending()) {
            int code = upload(d);
            if (code == 200) {
                delete(d);
                lastResult = "ok";
            } else if (code == 400 || code == 413) {
                delete(d);                                     // 웹사이트가 받을 수 없는 보고 → 다시 보내도 같음
                lastResult = "bad";
            } else {
                lastResult = code == 429 ? "rate" : "net";
                break;                                         // 네트워크 · 횟수 제한 → 나중에 다시
            }
        }
    }

    /** 보고 한 건 → HTTP 코드 (0 = 연결 실패) */
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
                File shot = new File(d, "shot.jpg");
                if (shot.exists()) part(o, b, "shot", "shot.jpg", "image/jpeg", shot);
                File clipF = new File(d, "clip.mp4");
                if (clipF.exists()) part(o, b, "clip", "clip.mp4", "video/mp4", clipF);
                o.writeBytes("--" + b + "--\r\n");
            }
            int code = c.getResponseCode();
            String body = read(code < 400 ? c.getInputStream() : c.getErrorStream());
            if (code == 200) {
                String id = new JSONObject(body).optString("id", "");
                Hub.log("버그 보고를 보냄" + (id.isEmpty() ? "" : " (#" + id + ")"), 'd');
            } else Hub.log("버그 보고를 못 보냄 (" + code + ")", 'd');
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
