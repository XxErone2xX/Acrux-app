package com.acrux.mobile;

import android.content.Context;
import android.content.Intent;
import android.net.Uri;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.List;

/**
 * 디스코드 연결 (매크로 봇 /상태 · /통계 · /로그) — PC 버전 app.py 의 디스코드 연결과 같은 서버 · 같은 방식
 *  · [디스코드 로그인] → 브라우저로 acruxmacro.com/link 를 열어 디스코드로 로그인 → [연결] → 앱 전용 열쇠를 받아 둠 (2초마다 확인 · 10분까지)
 *  · 연결된 동안만 15초 (작동 중) · 60초 (꺼짐) 마다 상태를 보냄: 작동 여부 · 지금 하는 일 · 바이옴 · 통계 · 최근 기록 30줄
 *    (주소는 지움 · 화면은 안 보냄) · 매크로를 켜고 끌 때는 바로
 */
final class DiscordLink {
    private DiscordLink() {}

    static final String URL = "https://acruxmacro.com/api";
    static volatile String code, poll, error = "";
    static volatile long until;
    static volatile long since;              // 이번에 매크로를 켠 시각 (꺼져 있으면 0)
    static boolean lastRun;
    static final Object wake = new Object();
    static Thread loop;

    static String token() {
        return Hub.prefs().getString("dcToken", "");
    }

    static String name() {
        return Hub.prefs().getString("dcName", "");
    }

    static boolean linked() {
        return token().matches("[0-9a-f]{64}");
    }

    static synchronized void start() {
        if (loop != null) return;
        // 매크로를 켜고 끌 때 바로 보냄 (화면 갱신 때마다 확인 · 메인 스레드)
        Hub.listeners.add(() -> {
            boolean r = Hub.running();
            if (r == lastRun) return;
            lastRun = r;
            since = r ? System.currentTimeMillis() : 0;
            poke();
        });
        loop = new Thread(DiscordLink::run, "acrux-discord");
        loop.setDaemon(true);
        loop.start();
    }

    static void poke() {
        synchronized (wake) {
            wake.notifyAll();
        }
    }

    /** acruxmacro.com/api 호출 → {상태 번호, JSON} */
    static Object[] call(String path, JSONObject body, String tok) throws Exception {
        HttpURLConnection h = (HttpURLConnection) new URL(URL + path).openConnection();
        h.setConnectTimeout(10000);
        h.setReadTimeout(10000);
        h.setRequestProperty("User-Agent", "AcruxMobile/" + Online.BuildConfigVersion.get());
        h.setRequestProperty("Content-Type", "application/json");
        if (tok != null) h.setRequestProperty("Authorization", "Bearer " + tok);
        try {
            if (body != null) {
                h.setRequestMethod("POST");
                h.setDoOutput(true);
                try (OutputStream o = h.getOutputStream()) {
                    o.write(body.toString().getBytes(StandardCharsets.UTF_8));
                }
            }
            int st = h.getResponseCode();
            InputStream in = st >= 400 ? h.getErrorStream() : h.getInputStream();
            String txt = "";
            if (in != null) {
                try (InputStream s = in) {
                    byte[] buf = new byte[8192];
                    int n = 0, r;
                    while (n < buf.length && (r = s.read(buf, n, buf.length - n)) > 0) n += r;
                    txt = new String(buf, 0, n, StandardCharsets.UTF_8);
                }
            }
            JSONObject j;
            try {
                j = new JSONObject(txt.isEmpty() ? "{}" : txt);
            } catch (Exception e) {
                j = new JSONObject();
            }
            return new Object[]{st, j};
        } finally {
            h.disconnect();
        }
    }

    /** 연결 시작 (메인 스레드에서 불러도 됨 · 네트워크는 뒤에서) */
    static void login(Context ctx) {
        if (poll != null && System.currentTimeMillis() < until) return;
        error = "";
        new Thread(() -> {
            try {
                Object[] r = call("/link/start", new JSONObject().put("platform", "mobile"), null);
                JSONObject j = (JSONObject) r[1];
                if ((int) r[0] != 200 || !j.has("code")) throw new IllegalStateException();
                String p = j.getString("poll");
                code = j.getString("code");
                poll = p;
                until = System.currentTimeMillis() + j.optLong("expires", 600) * 1000L;
                Hub.log("디스코드 연결 — 브라우저에서 로그인한 뒤 [연결] 을 눌러 주세요 (코드 " + code.substring(0, 4) + "-" + code.substring(4) + ")", 'c');
                String url = j.getString("url");
                Hub.main.post(() -> {
                    try {
                        ctx.startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
                    } catch (RuntimeException e) {
                        error = "브라우저를 열 수 없음";
                        Hub.changed();
                    }
                });
                Hub.changed();
                waitLink(p);
            } catch (Exception e) {
                error = "서버에 연결할 수 없음";
                code = null;
                poll = null;
                Hub.changed();
            }
        }, "acrux-discord-link").start();
    }

    static void waitLink(String p) {
        while (p.equals(poll) && System.currentTimeMillis() < until) {
            try {
                Thread.sleep(2000);
                Object[] r = call("/link/poll", new JSONObject().put("poll", p), null);
                JSONObject j = (JSONObject) r[1];
                String s = j.optString("status");
                if ("ok".equals(s) && j.optString("token").matches("[0-9a-f]{64}")) {
                    String n = j.optJSONObject("user") != null ? j.getJSONObject("user").optString("name") : "";
                    Hub.prefs().edit().putString("dcToken", j.getString("token")).putString("dcName", n).apply();
                    code = null;
                    poll = null;
                    Hub.log("디스코드 연결됨 — " + n, 'g');
                    Hub.changed();
                    poke();
                    return;
                }
                if ("expired".equals(s) || (int) r[0] == 400) break;
            } catch (InterruptedException e) {
                return;
            } catch (Exception ignored) {
                // 잠깐 끊겼으면 다음에 다시
            }
        }
        if (p.equals(poll)) {
            code = null;
            poll = null;
            error = "연결 시간이 지남 — 다시 눌러 주세요";
            Hub.changed();
        }
    }

    static void cancel() {
        code = null;
        poll = null;
        error = "";
        Hub.changed();
    }

    static void logout() {
        String tok = token();
        Hub.prefs().edit().remove("dcToken").remove("dcName").apply();
        cancel();
        Hub.log("디스코드 연결 끊음", 'y');
        if (!tok.isEmpty()) new Thread(() -> {
            try {
                call("/macro/unlink", new JSONObject(), tok);
            } catch (Exception ignored) {
            }
        }, "acrux-discord-unlink").start();
    }

    static final String[][] STAT_MAP = {{"success", "fish_success"}, {"junk", "fish_junk"}, {"fail", "fish_fail"}, {"sells", "sell_runs"}, {"sold", "sold"}};

    /** 봇에 보낼 지금 상태 (글자는 앱 언어로) */
    static JSONObject state() throws Exception {
        boolean run = Hub.running();
        JSONObject s = new JSONObject();
        s.put("v", "V" + Online.BuildConfigVersion.get());
        s.put("running", run);
        String st = Hub.state == null ? "" : Hub.state;
        s.put("action", com.acrux.mobile.core.I18n.t(run ? st : "꺼짐"));
        if (run && since > 0) s.put("started", since);
        if (Biome.on()) {
            s.put("features", new JSONArray().put(com.acrux.mobile.core.I18n.t("바이옴 매크로")));
            if (Biome.current != null && !Biome.current.isEmpty()) s.put("biome", Biome.current);
        }
        JSONObject ses = new JSONObject(), all = new JSONObject();
        for (int i = 0; i < Hub.STAT_KEYS.length; i++) {
            for (String[] m : STAT_MAP) {
                if (!m[0].equals(Hub.STAT_KEYS[i])) continue;
                ses.put(m[1], Hub.statRun[i]);
                all.put(m[1], Hub.statAll(i));
            }
        }
        s.put("stats", new JSONObject().put("session", ses).put("all", all));
        JSONArray logs = new JSONArray();
        List<String> l = Hub.recentLogsShown(30);
        for (String line : l) {
            int k = line.indexOf("  ");
            String t = k > 0 ? line.substring(0, k) : "", m = k > 0 ? line.substring(k + 2) : line;
            m = m.replaceAll("https?://\\S+", "[link]");
            logs.put(new JSONArray().put(t).put(m.length() > 200 ? m.substring(0, 200) : m));
        }
        s.put("logs", logs);
        return s;
    }

    static void run() {
        long wait = 5000;
        while (true) {
            synchronized (wake) {
                try {
                    wake.wait(wait);
                } catch (InterruptedException e) {
                    return;
                }
            }
            String tok = token();
            if (!linked()) {
                wait = 30_000;
                continue;
            }
            try {
                Object[] r = call("/macro/state", state(), tok);
                if ((int) r[0] == 401) {               // 사이트에서 연결을 끊었음
                    if (tok.equals(token())) Hub.prefs().edit().remove("dcToken").remove("dcName").apply();
                    Hub.log("디스코드 연결이 끊김 — 다시 연결하려면 설정 → 디스코드 연결", 'y');
                    Hub.changed();
                    wait = 30_000;
                    continue;
                }
                wait = Math.min(300, Math.max(10, ((JSONObject) r[1]).optInt("next", 60))) * 1000L;
            } catch (Throwable e) {
                wait = 60_000;
            }
        }
    }
}
