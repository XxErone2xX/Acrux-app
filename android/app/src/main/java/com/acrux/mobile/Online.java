package com.acrux.mobile;

import org.json.JSONObject;

import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;

/**
 * 사용자 수 (Now Active) — PC 버전 app.py _online_loop 와 같은 서버 · 같은 방식 (PC · 모바일을 합친 수)
 *  · 1분마다 GET /count → {"online": 사용자 수} 를 읽어 표시
 *  · 항상 집계에 참여: 서버가 알려준 간격(기본 5분)마다 POST /ping {"id": 설치마다 만든 무작위 32자리}
 *  · 보내는 건 무작위 번호뿐 (개인 정보 없음)
 */
final class Online {
    private Online() {}

    static final String URL = "https://acrux.pentagration.com";
    static volatile Integer count;          // 모르면 null
    static Thread loop;

    static synchronized void start() {
        if (loop != null) return;
        loop = new Thread(Online::run, "acrux-online");
        loop.setDaemon(true);
        loop.start();
    }

    /** 집계 참여는 항상 켜짐 (V0.10.7 부터 끄는 스위치 없음) */
    static boolean share() {
        return true;
    }

    static String id() {
        String id = Hub.prefs().getString("installId", "");
        if (!id.matches("[0-9a-f]{32}")) {
            byte[] b = new byte[16];
            new SecureRandom().nextBytes(b);
            StringBuilder s = new StringBuilder();
            for (byte x : b) s.append(String.format("%02x", x));
            id = s.toString();
            Hub.prefs().edit().putString("installId", id).apply();
        }
        return id;
    }

    static JSONObject call(String path, String body) throws Exception {
        HttpURLConnection h = (HttpURLConnection) new URL(URL + path).openConnection();
        h.setConnectTimeout(10000);
        h.setReadTimeout(10000);
        h.setRequestProperty("User-Agent", "AcruxMobile/" + BuildConfigVersion.get());
        h.setRequestProperty("Content-Type", "application/json");
        try {
            if (body != null) {
                h.setRequestMethod("POST");
                h.setDoOutput(true);
                try (OutputStream o = h.getOutputStream()) {
                    o.write(body.getBytes(StandardCharsets.UTF_8));
                }
            }
            try (InputStream in = h.getInputStream()) {
                byte[] buf = new byte[4096];
                int n = 0, r;
                while (n < buf.length && (r = in.read(buf, n, buf.length - n)) > 0) n += r;
                return new JSONObject(new String(buf, 0, n, StandardCharsets.UTF_8));
            }
        } finally {
            h.disconnect();
        }
    }

    static void run() {
        try {
            Thread.sleep(2000);
        } catch (InterruptedException e) {
            return;
        }
        long pingEvery = 300_000, lastPing = 0;
        while (true) {
            try {
                if (share() && System.currentTimeMillis() - lastPing >= pingEvery) {
                    JSONObject r = call("/ping", new JSONObject().put("id", id()).toString());
                    lastPing = System.currentTimeMillis();
                    pingEvery = Math.min(3600, Math.max(60, r.optInt("next", 300))) * 1000L;
                }
                count = call("/count", null).getInt("online");
            } catch (Throwable e) {
                count = null;
            }
            Hub.changed();
            try {
                Thread.sleep(60_000);
            } catch (InterruptedException e) {
                return;
            }
        }
    }

    /** 앱 버전 (User-Agent 용) */
    static final class BuildConfigVersion {
        static String v;

        static String get() {
            if (v == null) {
                try {
                    v = Hub.app.getPackageManager().getPackageInfo(Hub.app.getPackageName(), 0).versionName;
                } catch (Exception e) {
                    v = "?";
                }
            }
            return v;
        }
    }
}
