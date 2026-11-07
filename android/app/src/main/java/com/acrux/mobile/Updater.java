package com.acrux.mobile;

import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageInstaller;
import android.net.Uri;
import android.os.Build;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.security.MessageDigest;

/**
 * 자동 업데이트 — GitHub 릴리스(mobile-v<버전>)에서 새 버전을 찾아 받고, 받은 파일을 sha256 으로 확인한 뒤 바로 설치 화면을 띄움
 * 앱을 켤 때 한 번 확인 (끌 수 있음) · 앱에서 [확인] / [업데이트] 로도
 * 서명 키가 고정이라 덮어쓰기 설치 (V0.7.0 부터 · 그 전 버전은 한 번만 지우고 새로 설치)
 */
final class Updater {
    private Updater() {}

    static final String REPO = "XxErone2xX/Acrux-app";
    static final String API = "https://api.github.com/repos/" + REPO + "/releases?per_page=50";
    static final String ACTION = "com.acrux.mobile.UPDATE_STATUS";

    static volatile String status = "";
    static volatile String newVer, notes, apkUrl, shaUrl;
    static volatile boolean busy, checkedOnce;

    static String current(Context c) {
        try {
            return c.getPackageManager().getPackageInfo(c.getPackageName(), 0).versionName;
        } catch (Exception e) {
            return "0";
        }
    }

    /** "0.6.10" > "0.6.9" */
    static int cmp(String a, String b) {
        String[] x = a.split("[^0-9]+"), y = b.split("[^0-9]+");
        for (int i = 0; i < Math.max(x.length, y.length); i++) {
            int p = i < x.length && !x[i].isEmpty() ? Integer.parseInt(x[i]) : 0;
            int q = i < y.length && !y[i].isEmpty() ? Integer.parseInt(y[i]) : 0;
            if (p != q) return Integer.compare(p, q);
        }
        return 0;
    }

    static void set(String s) {
        status = s;
        Hub.changed();
    }

    static HttpURLConnection open(String url) throws Exception {
        HttpURLConnection h = (HttpURLConnection) new URL(url).openConnection();
        h.setConnectTimeout(10000);
        h.setReadTimeout(20000);
        h.setInstanceFollowRedirects(true);
        h.setRequestProperty("User-Agent", "AcruxMobile-Updater");
        h.setRequestProperty("Accept", "application/vnd.github+json");
        return h;
    }

    static String text(String url) throws Exception {
        HttpURLConnection h = open(url);
        // InputStream.readAllBytes() 는 안드로이드 13 이상에만 있음 (8 ~ 12 에선 앱이 꺼졌음) → 직접 읽음
        try (InputStream in = h.getInputStream()) {
            java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
            byte[] buf = new byte[1 << 14];
            int n;
            while ((n = in.read(buf)) > 0 && out.size() < (4 << 20)) out.write(buf, 0, n);
            return out.toString("UTF-8");
        } finally {
            h.disconnect();
        }
    }

    /** 새 버전 확인 · auto: 앱을 켤 때 (새 버전이 있으면 바로 받아서 설치 화면까지) */
    static void check(Context ctx, boolean auto) {
        if (busy) return;
        Context c = ctx.getApplicationContext();
        busy = true;
        set("업데이트 확인 중…");
        new Thread(() -> {
            boolean get = false;
            try {
                String cur = current(c);
                JSONArray rels = new JSONArray(text(API));
                String best = null, bestApk = null, bestSha = null, bestNotes = null;
                for (int i = 0; i < rels.length(); i++) {
                    JSONObject r = rels.getJSONObject(i);
                    String tag = r.optString("tag_name");
                    if (!tag.startsWith("mobile-v") || r.optBoolean("draft")) continue;
                    String v = tag.substring("mobile-v".length());
                    String apk = null, sha = null;
                    JSONArray as = r.optJSONArray("assets");
                    for (int j = 0; as != null && j < as.length(); j++) {
                        JSONObject a = as.getJSONObject(j);
                        String n = a.optString("name");
                        if (n.equals("Acrux-Mobile.apk")) apk = a.optString("browser_download_url");
                        else if (n.equals("Acrux-Mobile.apk.sha256")) sha = a.optString("browser_download_url");
                    }
                    if (apk == null || sha == null) continue;
                    if (best == null || cmp(v, best) > 0) {
                        best = v;
                        bestApk = apk;
                        bestSha = sha;
                        bestNotes = r.optString("body");
                    }
                }
                if (best == null) {
                    newVer = null;
                    set("릴리스를 못 찾음");
                } else if (cmp(best, cur) > 0) {
                    newVer = best;
                    apkUrl = bestApk;
                    shaUrl = bestSha;
                    notes = firstSection(bestNotes);
                    set("새 버전 V" + best + " 있음");
                    Hub.log("새 버전 V" + best + " 있음 (지금 V" + cur + ")", 'c');
                    get = auto && Hub.autoUpdate;
                } else {
                    newVer = null;
                    set("최신 버전 (V" + cur + ")");
                }
            } catch (Throwable e) {
                set("업데이트 확인 실패 (인터넷 연결 확인): " + e.getClass().getSimpleName());
            } finally {
                busy = false;
                Hub.changed();
            }
            if (get) download(c);
        }, "acrux-update").start();
    }

    /** 릴리스 설명에서 맨 위 한국어 부분만 (## … 업데이트 ~ 다음 ##) */
    static String firstSection(String body) {
        if (body == null) return "";
        String[] lines = body.replace("\r", "").split("\n");
        StringBuilder s = new StringBuilder();
        int heads = 0;
        for (String l : lines) {
            if (l.startsWith("## ")) {
                if (++heads > 1) break;
                continue;
            }
            if (heads == 1) s.append(l).append('\n');
        }
        return s.toString().trim();
    }

    static void download(Context ctx) {
        if (busy || apkUrl == null) return;
        Context c = ctx.getApplicationContext();
        busy = true;
        set("V" + newVer + " 받는 중…");
        new Thread(() -> {
            try {
                String want = text(shaUrl).trim().split("\\s+")[0].toLowerCase();
                File f = new File(c.getCacheDir(), "update.apk");
                HttpURLConnection h = open(apkUrl);
                long total = h.getContentLengthLong(), got = 0;
                MessageDigest md = MessageDigest.getInstance("SHA-256");
                int lastPct = -1;
                try (InputStream in = h.getInputStream(); OutputStream out = new FileOutputStream(f)) {
                    byte[] buf = new byte[1 << 16];
                    int n;
                    while ((n = in.read(buf)) > 0) {
                        out.write(buf, 0, n);
                        md.update(buf, 0, n);
                        got += n;
                        int pct = total > 0 ? (int) (got * 100 / total) : -1;
                        if (pct != lastPct && pct % 5 == 0) {
                            lastPct = pct;
                            set("V" + newVer + " 받는 중… " + pct + "%");
                        }
                    }
                } finally {
                    h.disconnect();
                }
                StringBuilder hex = new StringBuilder();
                for (byte b : md.digest()) hex.append(String.format("%02x", b));
                if (!hex.toString().equals(want)) {
                    f.delete();
                    set("받은 파일이 깨짐 (sha256 다름) — 다시 [업데이트]");
                    return;
                }
                install(c, f);
            } catch (Throwable e) {
                set("받기 실패: " + e.getClass().getSimpleName() + " — 다시 [업데이트]");
            } finally {
                busy = false;
                Hub.changed();
            }
        }, "acrux-update-dl").start();
    }

    static boolean canInstall(Context c) {
        return Build.VERSION.SDK_INT < 26 || c.getPackageManager().canRequestPackageInstalls();
    }

    static void openInstallPermission(Context c) {
        try {
            Intent i = new Intent(android.provider.Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:" + c.getPackageName()));
            i.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            c.startActivity(i);
        } catch (RuntimeException ignored) {
        }
    }

    static void install(Context c, File f) throws Exception {
        if (!canInstall(c)) {
            set("설치 권한 필요 — 'Acrux Mobile 에서 앱 설치 허용'을 켜고 돌아와서 [업데이트]");
            Hub.main.post(() -> openInstallPermission(c));
            return;
        }
        set("설치 준비 중…");
        PackageInstaller pi = c.getPackageManager().getPackageInstaller();
        PackageInstaller.SessionParams p = new PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL);
        p.setAppPackageName(c.getPackageName());
        if (Build.VERSION.SDK_INT >= 31) p.setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED);
        int id = pi.createSession(p);
        try (PackageInstaller.Session s = pi.openSession(id)) {
            try (InputStream in = new FileInputStream(f); OutputStream out = s.openWrite("acrux.apk", 0, f.length())) {
                byte[] buf = new byte[1 << 16];
                int n;
                while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
                s.fsync(out);
            }
            Intent i = new Intent(c, UpdateReceiver.class).setAction(ACTION);
            int flags = PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= 31 ? PendingIntent.FLAG_MUTABLE : 0);
            s.commit(PendingIntent.getBroadcast(c, id, i, flags).getIntentSender());
        }
        set("설치 화면에서 [업데이트] 를 누르세요");
    }

    /** 설치 결과 (설치 확인 화면 띄우기 · 성공 · 실패) */
    public static class UpdateReceiver extends BroadcastReceiver {
        @Override
        public void onReceive(Context c, Intent in) {
            int st = in.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE);
            if (st == PackageInstaller.STATUS_PENDING_USER_ACTION) {
                Intent confirm = in.getParcelableExtra(Intent.EXTRA_INTENT);
                if (confirm != null) {
                    confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                    try {
                        c.startActivity(confirm);
                    } catch (RuntimeException e) {
                        set("설치 화면을 못 띄움 — 앱을 열고 다시 [업데이트]");
                    }
                }
                return;
            }
            if (st == PackageInstaller.STATUS_SUCCESS) {
                set("업데이트 완료");
                return;
            }
            String msg = in.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE);
            if (st == PackageInstaller.STATUS_FAILURE_CONFLICT || st == PackageInstaller.STATUS_FAILURE_INCOMPATIBLE
                    || (msg != null && msg.toUpperCase().contains("SIGNATURE"))) {
                set("서명이 달라서 덮어쓸 수 없음 — 지금 앱을 한 번만 지우고 새 버전을 설치 (그 뒤로는 자동 업데이트)");
            } else if (st == PackageInstaller.STATUS_FAILURE_ABORTED) {
                set("설치 취소됨 — 다시 [업데이트]");
            } else {
                set("설치 실패: " + (msg != null ? msg : String.valueOf(st)));
            }
        }
    }
}
