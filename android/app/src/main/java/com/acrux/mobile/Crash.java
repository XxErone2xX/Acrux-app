package com.acrux.mobile;

import android.content.Context;
import android.os.Build;

import java.io.File;
import java.io.FileOutputStream;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.Locale;

/**
 * 크래시 기록 — 앱이 꺼질 만한 오류를 파일(crash.txt)로 남김 → 다음에 앱을 켜면 홈 · 기록 칸에 보임 (복사해서 보내기)
 * 기종 · 안드로이드 버전 · 앱 버전 · 오류 내용만 (개인 정보 없음)
 */
final class Crash {
    private Crash() {}

    static File file;
    static Thread.UncaughtExceptionHandler prev;

    static void install(Context c) {
        if (file != null) return;
        file = new File(c.getFilesDir(), "crash.txt");
        prev = Thread.getDefaultUncaughtExceptionHandler();
        Thread.setDefaultUncaughtExceptionHandler((t, e) -> {
            save(e, t.getName());
            if (prev != null) prev.uncaughtException(t, e);
        });
    }

    static synchronized void save(Throwable e, String where) {
        if (file == null) return;
        try {
            StringWriter sw = new StringWriter();
            PrintWriter pw = new PrintWriter(sw);
            pw.println(new SimpleDateFormat("yyyy-MM-dd HH:mm:ss", Locale.ROOT).format(new Date()) + " · " + where);
            pw.println("Acrux Mobile " + Online.BuildConfigVersion.get() + " · Android " + Build.VERSION.RELEASE + " (API " + Build.VERSION.SDK_INT + ")");
            pw.println(Build.MANUFACTURER + " " + Build.MODEL + " · " + String.join(",", Build.SUPPORTED_ABIS));
            pw.println("heap max " + (Runtime.getRuntime().maxMemory() >> 20) + "MB");
            e.printStackTrace(pw);
            pw.flush();
            try (FileOutputStream o = new FileOutputStream(file, false)) {
                o.write(sw.toString().getBytes(StandardCharsets.UTF_8));
            }
        } catch (Throwable ignored) {
        }
    }

    static String read() {
        if (file == null || !file.exists()) return null;
        try (java.io.FileInputStream in = new java.io.FileInputStream(file)) {
            java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
            byte[] b = new byte[8192];
            int n;
            while ((n = in.read(b)) > 0 && out.size() < 64 * 1024) out.write(b, 0, n);
            return out.toString("UTF-8");
        } catch (Throwable e) {
            return null;
        }
    }

    static void clear() {
        if (file != null) file.delete();
    }
}
