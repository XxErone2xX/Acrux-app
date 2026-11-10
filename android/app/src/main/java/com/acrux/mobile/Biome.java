package com.acrux.mobile;

import android.content.Context;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.text.Normalizer;
import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Date;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.TimeZone;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * 바이옴 매크로 (PC biome.py 와 같은 방식) — 무선 디버깅 (Adb) 으로 로블록스 로그를 읽어서 [BloxstrapRPC] 의 hoverText = 지금 바이옴
 *  · 로그캣 (실시간) + 로블록스 폴더의 최신 .log 파일 둘 다 봄 (기종마다 남는 곳이 다를 수 있어서) · 바뀔 때만 처리
 *  · 디스코드 웹후크: 켜짐 / 꺼짐 · 바이옴 시작 / 끝 · 접속 끊김 (PC 와 같은 임베드 · 아래에 Acrux Mobile 버전)
 *  · 직접 켰을 때 이미 나와 있는 바이옴이 알림 대상이면 바로 Started 를 보냄 (앱이 다시 켜질 때는 알림 없이 맞추기만)
 *  · 로그캣은 줄마다 시각을 같이 받음 → 연결이 끊겼다 붙어도 마지막으로 본 줄부터 이어서 읽음 (그 사이 바뀐 바이옴을 놓치지 않음)
 *  · 로그캣이 2분 동안 한 줄도 안 오면 끊긴 걸로 보고 다시 연결
 */
final class Biome {
    private Biome() {}

    static final String DISCORD_URL = "https://discord.gg/B9QzJuseHR";
    static final int EMBED_COLOR = 0xFFFFFF, EXIT_COLOR = 0xFF0000;
    static final String THUMB_URL = "https://raw.githubusercontent.com/rngenesis0-coder/Acrux-macro/refs/heads/main/%s.png";
    static final Set<String> RARE = new HashSet<>(Arrays.asList("GLITCHED", "DREAMSPACE", "CYBERSPACE"));
    static final Set<String> TARGET = new HashSet<>(Arrays.asList("WINDY", "SNOWY", "RAINY", "SANDSTORM", "HELL", "STARFALL", "HEAVEN",
            "CORRUPTION", "NULL", "GLITCHED", "DREAMSPACE", "CYBERSPACE", "SINGULARITY", "LOSTFOREST", "RADIANTFOREST",
            "PUMPKINMOON", "GRAVEYARD", "BLOODRAIN"));
    static final Set<String> THUMB = new HashSet<>(Arrays.asList("WINDY", "SNOWY", "RAINY", "SANDSTORM", "HELL", "STARFALL", "HEAVEN",
            "CORRUPTION", "NULL", "GLITCHED", "DREAMSPACE", "CYBERSPACE", "SINGULARITY"));
    static final Map<String, String> SHOW = new HashMap<>();

    static {
        SHOW.put("LOSTFOREST", "LOST FOREST");
        SHOW.put("RADIANTFOREST", "RADIANT FOREST");
        SHOW.put("PUMPKINMOON", "PUMPKIN MOON");
        SHOW.put("BLOODRAIN", "BLOOD RAIN");
    }

    static final Pattern RPC = Pattern.compile("\"largeImage\"\\s*:\\s*\\{[^}]*\"hoverText\"\\s*:\\s*\"([^\"]+)\"");
    static final Pattern HOOK = Pattern.compile("https://(?:(?:ptb|canary)\\.)?discord(?:app)?\\.com/api/(?:v\\d+/)?webhooks/\\d+/[A-Za-z0-9_\\-]+/?(?:\\?[A-Za-z0-9_=&\\-]*)?");
    static final String DISCONNECT = "Client:Disconnect";

    // ---- 설정 (Hub.prefs)
    static boolean on() {
        return Hub.prefs().getBoolean("biomeOn", false);
    }

    static List<String> hooks() {
        List<String> l = new ArrayList<>();
        for (String h : Hub.prefs().getString("biomeHooks", "").split("[\\s,]+")) if (!h.trim().isEmpty()) l.add(h.trim());
        return l;
    }

    static String ps() {
        return Hub.prefs().getString("biomePs", "").trim();
    }

    static boolean everyone() {
        return Hub.prefs().getBoolean("biomeEveryone", true);
    }

    // ---- 상태
    static volatile String current;
    static volatile boolean watching;
    static volatile String status = "꺼짐";
    static long activeSince;
    static boolean leftSent;
    static Thread loop;
    /** 로그캣에서 마지막으로 본 줄의 시각 ("MM-DD HH:MM:SS.mmm") · 그 줄을 받은 때 — 다시 연결하면 여기부터 */
    static volatile String resumeTs;
    static volatile long lastLineAt;
    /** 바이옴 줄이 나오는 곳 (0 아직 모름 · 1 로그캣 · 2 로그 파일) — 먼저 나온 한 곳만 믿음 (두 곳이 엇갈리면 지난 바이옴을 다시 알릴 수 있어서) */
    static volatile int primary;
    /** 직접 켰음 → 지금 나와 있는 바이옴도 알림 */
    static volatile boolean announceNow;
    static final Pattern TS = Pattern.compile("^(\\d\\d-\\d\\d \\d\\d:\\d\\d:\\d\\d\\.\\d{3})");

    /** 켜기 / 끄기 (설정 저장 + 감시 시작 / 멈춤 + 켜짐 · 꺼짐 웹후크) */
    static synchronized void set(Context c, boolean on) {
        boolean running = loop != null && loop.isAlive();
        if (on == on() && on == running) return;            // 이미 그 상태 (미니 UI 버튼 · 앱 스위치가 서로 맞춰질 때 알림이 두 번 가지 않게)
        Hub.prefs().edit().putBoolean("biomeOn", on).apply();
        if (on) {
            start(c, true);
            Hub.changed();
        } else {
            stop(true);
        }
    }

    /** 앱이 켜질 때: 켜져 있었으면 다시 감시 (웹후크 켜짐 알림은 안 보냄) */
    static synchronized void resume(Context c) {
        if (on() && (loop == null || !loop.isAlive())) start(c, false);
    }

    static synchronized void start(Context c, boolean announce) {
        if (loop != null && loop.isAlive()) return;
        Context ctx = c.getApplicationContext();
        activeSince = System.currentTimeMillis();
        current = null;
        leftSent = false;
        resumeTs = null;
        primary = 0;
        announceNow = announce;
        if (announce) {
            sendAll(toggle(true));
            Hub.log("바이옴 매크로 켜짐", 'c');
        }
        loop = new Thread(() -> run(ctx), "acrux-biome");
        loop.setDaemon(true);
        loop.start();
    }

    static synchronized void stop(boolean say) {
        Thread t = loop;
        loop = null;
        watching = false;
        status = "꺼짐";
        if (t != null) t.interrupt();
        if (say) {
            sendAll(toggle(false));
            Hub.log("바이옴 매크로 꺼짐", 'c');
        }
        Hub.changed();
    }

    static boolean alive() {
        return loop == Thread.currentThread() && on();
    }

    static void setStatus(String s) {
        if (!s.equals(status)) {
            status = s;
            Hub.changed();
        }
    }

    static final String FILES = "p=; f=; s=0; while :; do l=$(find /sdcard/Android/data/com.roblox.client -iname '*.log' 2>/dev/null); n=; "
            + "[ -n \"$l\" ] && n=$(ls -t $l 2>/dev/null | head -n 1); "
            + "if [ -n \"$n\" ] && [ \"$n\" != \"$f\" ]; then [ -n \"$p\" ] && kill $p 2>/dev/null; f=\"$n\"; "
            + "if [ $s = 0 ]; then tail -n 0 -f \"$f\" & else tail -n +1 -f \"$f\" & fi; p=$!; s=1; fi; sleep 5; done";

    static void run(Context c) {
        Thread me = Thread.currentThread();
        // 로블록스 폴더의 최신 로그 파일도 같이 (따로 스레드 · 새 파일이 생기면 그 파일로 넘어감 — 새 파일은 처음부터 읽음)
        Thread files = new Thread(() -> {
            while (on() && loop == me) {
                if (watching) {
                    try {
                        Adb.shell(c, FILES, 30 * 60 * 1000L, l -> line(l, 2));
                    } catch (Throwable ignored) {
                    }
                }
                if (!sleep(5000)) return;
            }
        }, "acrux-biome-file");
        files.setDaemon(true);
        files.start();
        try {
            while (alive()) {
                String r = Adb.connect(c);
                if (!r.equals("ok")) {
                    watching = false;
                    String why = r.equals("pair") ? "무선 디버깅 페어링 필요" : "무선 디버깅 연결 안 됨 — 다시 시도 중";
                    if (!why.equals(status)) {                   // 처음 한 번만 기록 · 화면 알림 (미니 UI 의 [바이옴] 이 노랑인 이유)
                        String msg = "바이옴 매크로: " + why;
                        Hub.log(msg, 'y');
                        Hub.main.post(() -> android.widget.Toast.makeText(Hub.app, com.acrux.mobile.core.I18n.t(msg), android.widget.Toast.LENGTH_LONG).show());
                    }
                    setStatus(why);
                    if (!sleep(10000)) return;
                    continue;
                }
                String from = resumeTs;
                if (from == null || System.currentTimeMillis() - lastLineAt > 5 * 60 * 1000L) {
                    // 처음 · 오래 끊겼음: 로그에 남은 마지막 바이옴으로 맞추고 지금부터 읽음 (오래 지난 바이옴을 줄줄이 알리지 않게)
                    String[] last = {null}, ts = {null};
                    try {
                        Adb.shell(c, "logcat -d -v time 2>/dev/null | tail -n 1; logcat -d -v brief 2>/dev/null | grep -a -e 'BloxstrapRPC' -e '" + DISCONNECT + "' | tail -n 3", 20000, l -> {
                            Matcher m = TS.matcher(l);
                            if (m.find()) {
                                if (ts[0] == null) ts[0] = m.group(1);
                                return;
                            }
                            if (l.contains(DISCONNECT)) {
                                last[0] = null;                  // 그 뒤로 나갔음 → 지금 바이옴은 모름
                            } else {
                                String b = parse(l);
                                if (b != null) last[0] = b;
                            }
                        });
                    } catch (Throwable ignored) {
                    }
                    from = ts[0];
                    sync(last[0]);
                }
                announceNow = false;
                watching = true;
                setStatus("감시 중");
                try {
                    // 줄마다 시각 (-v time) · 2분 동안 아무 줄도 없으면 끊긴 것 · 30분마다 다시 연결 (끊긴 자리부터 이어서)
                    Adb.shell(c, "logcat -v time -T " + (from == null ? "1" : "'" + from + "'"), 30 * 60 * 1000L, 120000, l -> {
                        Matcher m = TS.matcher(l);
                        if (m.find()) resumeTs = m.group(1);
                        lastLineAt = System.currentTimeMillis();
                        line(l, 1);
                    });
                } catch (Throwable t) {
                    Hub.log("바이옴 로그 읽기 끊김: " + t, 'y');
                }
                watching = false;
                if (!sleep(1000)) return;
            }
        } finally {
            files.interrupt();
        }
    }

    /** 로그에 남아 있던 마지막 바이옴으로 맞춤 */
    static synchronized void sync(String b) {
        if (b == null || !on() || loop == null) return;
        if (current == null) {
            current = b;
            leftSent = false;
            Hub.log("지금 바이옴: " + show(b), 'c');
            Hub.changed();
            if (announceNow && TARGET.contains(b)) sendAll(biome(b, true));     // 켰을 때 이미 나와 있는 바이옴
        } else if (!b.equals(current)) {
            change(b);
        }
    }

    static boolean sleep(long ms) {
        try {
            Thread.sleep(ms);
            return true;
        } catch (InterruptedException e) {
            return false;
        }
    }

    /** 로그 한 줄 (src: 1 로그캣 · 2 로그 파일) */
    static synchronized void line(String l, int src) {
        if (!on() || loop == null) return;
        if (l.contains(DISCONNECT)) {
            if (!leftSent) {
                leftSent = true;
                Hub.log("로블록스 접속 끊김", 'y');
                sendAll(exit());
            }
            return;
        }
        String b = parse(l);
        if (b == null) return;
        if (primary == 0) primary = src;
        if (src != primary || b.equals(current)) return;
        change(b);
    }

    static void change(String b) {
        String prev = current;
        current = b;
        leftSent = false;
        Hub.log("바이옴: " + (prev == null ? "-" : show(prev)) + " → " + show(b), RARE.contains(b) ? 'g' : 'c');
        Hub.changed();
        if (prev != null && TARGET.contains(prev)) sendAll(biome(prev, false));
        if (TARGET.contains(b)) sendAll(biome(b, true));
    }

    static String parse(String l) {
        if (!l.contains("BloxstrapRPC") || !l.contains("largeImage")) return null;
        Matcher m = RPC.matcher(l.replace("\\\"", "\""));
        return m.find() ? norm(m.group(1)) : null;
    }

    /** 바이옴 이름 → 키 ('Sand Storm' → 'SANDSTORM' · 'The Lost Forest' → 'LOSTFOREST' · 한두 글자 틀려도 아는 바이옴으로) */
    static String norm(String name) {
        String s = Normalizer.normalize(name == null ? "" : name, Normalizer.Form.NFKD).toUpperCase(Locale.ROOT).replaceAll("[^A-Z0-9]", "");
        if (s.isEmpty()) return null;
        String k = known(s);
        if (k == null && s.startsWith("THE")) k = known(s.substring(3));
        return k != null ? k : s;
    }

    static String known(String s) {
        if (TARGET.contains(s)) return s;
        if (s.length() < 5) return null;
        String best = null;
        int bd = 99;
        for (String t : TARGET) {
            int d = lev(s, t);
            if (d < bd) {
                bd = d;
                best = t;
            }
        }
        return best != null && bd <= Math.max(1, best.length() / 5) ? best : null;
    }

    static int lev(String a, String b) {
        int[] p = new int[b.length() + 1], q = new int[b.length() + 1];
        for (int j = 0; j <= b.length(); j++) p[j] = j;
        for (int i = 1; i <= a.length(); i++) {
            q[0] = i;
            for (int j = 1; j <= b.length(); j++) {
                q[j] = Math.min(Math.min(q[j - 1] + 1, p[j] + 1), p[j - 1] + (a.charAt(i - 1) == b.charAt(j - 1) ? 0 : 1));
            }
            int[] t = p;
            p = q;
            q = t;
        }
        return p[b.length()];
    }

    static String show(String b) {
        return b == null ? "-" : SHOW.getOrDefault(b, b);
    }

    // ---- 웹후크 (PC 와 같은 임베드)
    static String label() {
        return "Acrux Mobile V" + Updater.current(Hub.app);
    }

    static String now() {
        SimpleDateFormat f = new SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss.SSS'Z'", Locale.ROOT);
        f.setTimeZone(TimeZone.getTimeZone("UTC"));
        return f.format(new Date());
    }

    static String uptime() {
        long s = Math.max(0, (System.currentTimeMillis() - activeSince) / 1000);
        return (s / 3600) + "hr " + (s % 3600 / 60) + "mn " + (s % 60) + "sc";
    }

    static JSONObject embed(String desc, int color) throws Exception {
        JSONObject e = new JSONObject();
        e.put("description", desc);
        e.put("color", color);
        e.put("timestamp", now());
        e.put("footer", new JSONObject().put("text", label()));
        return e;
    }

    static JSONObject wrap(JSONObject e, String content) throws Exception {
        JSONObject p = new JSONObject();
        if (content != null) p.put("content", content);
        p.put("embeds", new JSONArray().put(e));
        p.put("allowed_mentions", new JSONObject().put("parse", new JSONArray().put("everyone").put("roles")));
        return p;
    }

    static JSONObject toggle(boolean on) {
        try {
            return wrap(embed("> ## Biome Macro " + (on ? "Enabled" : "Disabled") + "\nJoin discord to download macro\n" + DISCORD_URL, EMBED_COLOR), null);
        } catch (Exception e) {
            return null;
        }
    }

    static JSONObject biome(String b, boolean started) {
        try {
            String w = started ? "Started" : "Ended";
            JSONObject e = embed("> ## Biome " + w + " - " + show(b) + "\n> \n> **" + w + ":** <t:" + (System.currentTimeMillis() / 1000) + ":R>", EMBED_COLOR);
            JSONArray f = new JSONArray().put(new JSONObject().put("name", "Uptime").put("value", uptime()).put("inline", true));
            if (started) f.put(new JSONObject().put("name", "Private Server Link").put("value", ps().isEmpty() ? "None" : ps()).put("inline", false));
            e.put("fields", f);
            if (THUMB.contains(b)) e.put("thumbnail", new JSONObject().put("url", String.format(Locale.ROOT, THUMB_URL, b)));
            String mention = started && RARE.contains(b) && everyone() ? "@everyone" : "";
            return wrap(e, mention);
        } catch (Exception e) {
            return null;
        }
    }

    static JSONObject exit() {
        try {
            return wrap(embed("> ## Player has left the game (Disconnected/Left)", EXIT_COLOR), null);
        } catch (Exception e) {
            return null;
        }
    }

    static JSONObject test() {
        try {
            JSONObject e = embed("> ## Webhook Test\n> Webhook connection test", EMBED_COLOR);
            e.put("fields", new JSONArray().put(new JSONObject().put("name", "Private Server Link").put("value", ps().isEmpty() ? "Not set" : ps()).put("inline", false)));
            return wrap(e, null);
        } catch (Exception e) {
            return null;
        }
    }

    static boolean isHook(String u) {
        return u != null && HOOK.matcher(u.trim()).matches();
    }

    /** 웹후크는 한 줄로 차례대로 (끝 → 시작 순서가 뒤바뀌지 않게 · 전에는 알림마다 따로 보내서 가끔 시작이 먼저 도착) */
    static final java.util.concurrent.ExecutorService QUEUE = java.util.concurrent.Executors.newSingleThreadExecutor(r -> {
        Thread t = new Thread(r, "acrux-webhook");
        t.setDaemon(true);
        return t;
    });

    /** 모든 웹후크로 (보낸 순서 그대로) · 디스코드 웹후크 주소가 아니면 안 보냄 */
    static void sendAll(JSONObject payload) {
        if (payload == null) return;
        List<String> hs = hooks();
        if (hs.isEmpty()) return;
        QUEUE.execute(() -> {
            for (String h : hs) {
                if (!isHook(h)) {
                    Hub.log("디스코드 웹후크 주소가 아니라서 안 보냄 (…" + h.substring(Math.max(0, h.length() - 6)) + ")", 'y');
                    continue;
                }
                int code = post(h, payload);
                for (int tries = 0; code == 429 && tries < 3; tries++) {     // 디스코드가 너무 빠르다고 하면 잠깐 기다렸다 다시 (순서 유지)
                    sleep(1500L * (tries + 1));
                    code = post(h, payload);
                }
                if (code != 200 && code != 204) Hub.log("웹후크 전송 실패 (" + code + ")", 'r');
            }
        });
    }

    static int post(String url, JSONObject payload) {
        HttpURLConnection c = null;
        try {
            c = (HttpURLConnection) new URL(url).openConnection();
            c.setRequestMethod("POST");
            c.setConnectTimeout(8000);
            c.setReadTimeout(8000);
            c.setDoOutput(true);
            c.setRequestProperty("Content-Type", "application/json");
            c.setRequestProperty("User-Agent", "AcruxMobile/1.0");
            try (OutputStream o = c.getOutputStream()) {
                o.write(payload.toString().getBytes(StandardCharsets.UTF_8));
            }
            return c.getResponseCode();
        } catch (Exception e) {
            return -1;
        } finally {
            if (c != null) c.disconnect();
        }
    }
}
