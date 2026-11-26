package com.acrux.mobile;

import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Rect;
import android.media.MediaCodec;
import android.media.MediaCodecInfo;
import android.media.MediaFormat;
import android.media.MediaMuxer;
import android.os.Build;
import android.view.Surface;

import java.io.File;
import java.nio.ByteBuffer;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.Locale;

/**
 * 버그 보고 고화질 클립 — 화면을 하드웨어 인코더 (H.264) 로 계속 녹화하고 최근 1분치 압축된 조각만 들고 있음
 *  · 보낼 때는 다시 압축하지 않고 MP4 로 묶기만 함 (빠름)
 *  · 화질 (가로 길이) · FPS 는 자동 (이 폰의 램 · 성능 등급으로 정하고, 따라가지 못하거나 뜨거우면 낮춤) 또는 직접
 *  · 웹사이트 (개발자 탭) 에만 올라감 — 디스코드에는 BugReport 의 가벼운 클립 (4fps · 가로 800)
 *  · BugReport 의 화면 샘플 스레드에서만 부름 (feed · stop) · 내보내기 (export) 는 보내는 스레드에서
 */
final class HdClip {
    private HdClip() {}

    // ---- 설정 (Hub 가 불러오고 저장)
    /** 화질: 0 자동 · 1 낮음 (가로 800) · 2 보통 (1280) · 3 높음 (1600) · 4 최고 (원본 · 최대 2560) */
    static volatile int quality = 0;
    /** FPS: 0 자동 · 아니면 그 값 */
    static volatile int fps = 0;
    /** 고화질 클립은 와이파이에서만 보냄 (크기가 커서) */
    static volatile boolean wifiOnly = true;

    static final int[] LONG = {0, 800, 1280, 1600, 2560};
    static final String[] QUALITY_NAMES = {"자동", "낮음 (가로 800)", "보통 (가로 1280)", "높음 (가로 1600)", "최고 (원본 · 최대 2560)"};
    static final int[] FPS_CHOICES = {0, 4, 10, 15, 20, 30};
    /** 자동 단계 (가로 길이 · FPS) — 0 은 고화질 클립 없음 (가벼운 클립과 같음) */
    static final int[][] AUTO = {{800, 4}, {1280, 10}, {1280, 15}, {1600, 20}};
    static final long WINDOW_US = 60_000_000L;
    /** 압축률 (픽셀 · 장면당 비트) · 최대 비트레이트 (1분에 90MB 아래 → 웹사이트 한도 100MB 안) */
    static final double BPP = 0.12;
    static final int MAX_BITRATE = 12_000_000, MIN_BITRATE = 1_000_000;

    static volatile int autoLevel = -1;
    /** 인코더가 안 됨 (이번 실행 동안 고화질 클립 없음) */
    static volatile boolean broken;
    /** 지금 녹화 중인 크기 · FPS ("" = 안 함) */
    static volatile String now = "";
    static volatile String lastDrop = "";

    // ---- 인코더
    static MediaCodec codec;
    static Surface input;
    static MediaFormat outFmt;
    static int encW, encH, encFps, bitrate;
    static final Paint PAINT = new Paint(Paint.FILTER_BITMAP_FLAG);
    static final Rect SRC = new Rect(), DST = new Rect();
    static final MediaCodec.BufferInfo INFO = new MediaCodec.BufferInfo();

    static final class Pkt {
        final long pts;
        final boolean key;
        final byte[] data;

        Pkt(long pts, boolean key, byte[] data) {
            this.pts = pts;
            this.key = key;
            this.data = data;
        }
    }

    static final ArrayDeque<Pkt> ring = new ArrayDeque<>();
    static long ringBytes;

    // ---------------------------------------------------------------- 무엇으로 녹화할지

    /** 이 폰의 자동 단계 (처음 한 번) — 미디어 성능 등급 · 램 · 코어 */
    static int autoTier() {
        if (autoLevel >= 0) return autoLevel;
        double ram = 0;
        try {
            android.app.ActivityManager am = (android.app.ActivityManager) Hub.app.getSystemService(android.content.Context.ACTIVITY_SERVICE);
            android.app.ActivityManager.MemoryInfo mi = new android.app.ActivityManager.MemoryInfo();
            am.getMemoryInfo(mi);
            ram = mi.totalMem / 1073741824.0;
        } catch (Throwable ignored) {
        }
        int pc = Build.VERSION.SDK_INT >= 31 ? Build.VERSION.MEDIA_PERFORMANCE_CLASS : 0;
        int cores = Runtime.getRuntime().availableProcessors();
        int lv = pc >= 33 || ram >= 11.5 ? 3 : pc >= 31 || ram >= 7.5 ? 2 : ram >= 5.5 && cores >= 8 ? 1 : 0;
        autoLevel = lv;
        return lv;
    }

    /** 지금 설정으로 녹화할 {가로 길이, FPS} · 고화질 클립이 필요 없으면 null (가벼운 클립과 같은 4fps · 800) */
    static int[] target() {
        if (broken) return null;
        int lv = autoTier();
        int side = quality <= 0 ? AUTO[lv][0] : LONG[Math.min(quality, LONG.length - 1)];
        int f = fps <= 0 ? AUTO[lv][1] : fps;
        if (side <= 800 && f <= 4) return null;
        return new int[]{side, f};
    }

    static boolean isAuto() {
        return quality <= 0 || fps <= 0;
    }

    /** 설정 화면에 보일 한 줄 */
    static String plan() {
        int[] t = target();
        if (t == null) return Ui.tr("고화질 클립 없음 (가벼운 클립만)");
        int mbMin = (int) Math.round(Math.max(MIN_BITRATE, Math.min(MAX_BITRATE, t[0] * t[0] * 9 / 20.0 * t[1] * BPP)) * 60 / 8 / 1048576);
        String s = com.acrux.mobile.core.I18n.f("가로 {0} · {1}fps · 1분에 약 {2}MB", t[0], t[1], mbMin);
        if (isAuto()) s += " (" + Ui.tr("자동") + ")";
        if (!lastDrop.isEmpty()) s += "\n" + lastDrop;
        return s;
    }

    // ---------------------------------------------------------------- 녹화 (샘플 스레드)

    /** 화면 한 장 (src 의 왼쪽 위 sw × sh 가 실제 화면) */
    static synchronized void feed(Bitmap src, int sw, int sh, int[] tgt) {
        int side = Math.min(tgt[0], Math.max(sw, sh));
        int w, h;
        if (sw >= sh) {
            w = side;
            h = (int) Math.round((double) sh * side / sw);
        } else {
            h = side;
            w = (int) Math.round((double) sw * side / sh);
        }
        w &= ~15;
        h &= ~15;
        if (w < 64 || h < 64) return;
        try {
            if (codec == null || w != encW || h != encH || tgt[1] != encFps) start(w, h, tgt[1]);
            if (codec == null) return;
            Canvas c = input.lockHardwareCanvas();
            try {
                SRC.set(0, 0, sw, sh);
                DST.set(0, 0, encW, encH);
                c.drawBitmap(src, SRC, DST, PAINT);
            } finally {
                input.unlockCanvasAndPost(c);
            }
            drain();
        } catch (Throwable e) {
            Hub.log("버그 보고 고화질 클립을 멈춤: " + e, 'd');
            stop();
            lastDrop = Ui.tr("이 폰에서 고화질 녹화가 안 됨 — 가벼운 클립만");
            broken = true;                                        // 앱을 다시 켤 때까지 다시 안 함
        }
    }

    /** 인코더 시작 — 크기가 안 되면 줄여서 */
    static void start(int w, int h, int f) throws Exception {
        stop();
        MediaCodec c = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_VIDEO_AVC);
        try {
            MediaCodecInfo.VideoCapabilities vc = c.getCodecInfo().getCapabilitiesForType(MediaFormat.MIMETYPE_VIDEO_AVC).getVideoCapabilities();
            while (vc != null && !vc.isSizeSupported(w, h) && Math.max(w, h) > 640) {
                w = (w * 3 / 4) & ~15;
                h = (h * 3 / 4) & ~15;
            }
            int br = (int) Math.max(MIN_BITRATE, Math.min(MAX_BITRATE, (double) w * h * f * BPP));
            MediaFormat fmt = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, w, h);
            fmt.setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface);
            fmt.setInteger(MediaFormat.KEY_BIT_RATE, br);
            fmt.setInteger(MediaFormat.KEY_FRAME_RATE, f);
            fmt.setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 1);
            c.configure(fmt, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE);
            input = c.createInputSurface();
            c.start();
            codec = c;
            encW = w;
            encH = h;
            encFps = f;
            bitrate = br;
            now = encW + "x" + encH + " · " + encFps + "fps";
            synchronized (ring) {
                ring.clear();
                ringBytes = 0;
            }
            outFmt = null;
        } catch (Exception e) {
            c.release();
            throw e;
        }
    }

    /** 인코더에서 나온 조각 → 최근 1분 */
    static void drain() {
        while (true) {
            int o = codec.dequeueOutputBuffer(INFO, 0);
            if (o == MediaCodec.INFO_TRY_AGAIN_LATER) return;
            if (o == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {
                outFmt = codec.getOutputFormat();
                continue;
            }
            if (o < 0) continue;
            ByteBuffer b = codec.getOutputBuffer(o);
            if (b != null && INFO.size > 0 && (INFO.flags & MediaCodec.BUFFER_FLAG_CODEC_CONFIG) == 0) {
                byte[] d = new byte[INFO.size];
                b.position(INFO.offset);
                b.get(d, 0, INFO.size);
                synchronized (ring) {
                    ring.addLast(new Pkt(INFO.presentationTimeUs, (INFO.flags & MediaCodec.BUFFER_FLAG_KEY_FRAME) != 0, d));
                    ringBytes += d.length;
                    trim();
                }
            }
            codec.releaseOutputBuffer(o, false);
        }
    }

    /** 1분 · 메모리 한도를 넘으면 앞에서부터 키 프레임 단위로 버림 (클립은 늘 키 프레임으로 시작) */
    static void trim() {
        long cap = Math.min(Runtime.getRuntime().maxMemory() / 4, 120L << 20);
        while (ring.size() > 1) {
            Pkt first = ring.peekFirst(), last = ring.peekLast();
            boolean old = last.pts - first.pts > WINDOW_US, big = ringBytes > cap;
            if (!first.key) {                                     // 시작이 키 프레임이 아니면 키 프레임까지 버림
                dropOne();
                continue;
            }
            if (!old && !big) return;
            // 다음 키 프레임 찾기 — 그 앞까지 버려도 1분이 남거나 메모리가 넘으면 버림
            Pkt next = null;
            Iterator<Pkt> it = ring.iterator();
            it.next();
            while (it.hasNext()) {
                Pkt p = it.next();
                if (p.key) {
                    next = p;
                    break;
                }
            }
            if (next == null) return;
            if (!big && last.pts - next.pts < WINDOW_US) return;
            while (ring.peekFirst() != next) dropOne();
        }
    }

    static void dropOne() {
        Pkt p = ring.pollFirst();
        if (p != null) ringBytes -= p.data.length;
    }

    static synchronized void stop() {
        if (codec != null) {
            try {
                codec.stop();
            } catch (Exception ignored) {
            }
            codec.release();
            codec = null;
        }
        if (input != null) {
            input.release();
            input = null;
        }
        now = "";
        synchronized (ring) {
            ring.clear();
            ringBytes = 0;
        }
    }

    // ---- 자동: 폰이 못 따라가거나 뜨거우면 한 단계 낮춤
    static double ema;
    static long overSince, thermalAt;

    /** 샘플 한 번에 걸린 시간 (workMs) · 한 장 간격 (periodMs) */
    static void load(long workMs, long periodMs, long nowMs) {
        if (!isAuto() || codec == null) return;
        ema = ema * 0.9 + workMs * 0.1;
        boolean over = ema > periodMs * 0.75;
        if (nowMs - thermalAt > 10_000) {
            thermalAt = nowMs;
            try {
                android.os.PowerManager pm = (android.os.PowerManager) Hub.app.getSystemService(android.content.Context.POWER_SERVICE);
                if (Build.VERSION.SDK_INT >= 29 && pm.getCurrentThermalStatus() >= android.os.PowerManager.THERMAL_STATUS_SEVERE) over = true;
            } catch (Throwable ignored) {
            }
        }
        if (!over) {
            overSince = 0;
            return;
        }
        if (overSince == 0) overSince = nowMs;
        if (nowMs - overSince < 3000 || autoLevel <= 0) return;
        autoLevel--;
        overSince = 0;
        ema = 0;
        int[] t = target();
        lastDrop = com.acrux.mobile.core.I18n.f("폰이 따라가지 못해 낮춤 → {0}", t == null ? Ui.tr("가벼운 클립만") : "가로 " + t[0] + " · " + t[1] + "fps");
        Hub.log("버그 보고 클립: " + lastDrop, 'd');
    }

    // ---------------------------------------------------------------- 내보내기 (보내는 스레드)

    /** 최근 1분 → MP4 → 길이 (초) · 없으면 -1 */
    static int export(File out) throws Exception {
        List<Pkt> ps;
        MediaFormat fmt;
        synchronized (ring) {
            ps = new ArrayList<>(ring);
            fmt = outFmt;
        }
        if (fmt == null || ps.size() < 2 || !ps.get(0).key) return -1;
        MediaMuxer mux = new MediaMuxer(out.getAbsolutePath(), MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4);
        boolean started = false;
        try {
            int track = mux.addTrack(fmt);
            mux.start();
            started = true;
            long t0 = ps.get(0).pts, last = -1;
            MediaCodec.BufferInfo bi = new MediaCodec.BufferInfo();
            for (Pkt p : ps) {
                long pts = Math.max(last + 1, p.pts - t0);
                last = pts;
                bi.set(0, p.data.length, pts, p.key ? MediaCodec.BUFFER_FLAG_KEY_FRAME : 0);
                mux.writeSampleData(track, ByteBuffer.wrap(p.data), bi);
            }
            return (int) Math.round((ps.get(ps.size() - 1).pts - t0) / 1e6);
        } finally {
            try {
                if (started) mux.stop();
            } catch (Exception ignored) {
            }
            mux.release();
        }
    }

    static String describeNow() {
        return now.isEmpty() ? "" : String.format(Locale.ROOT, "%s · %.1fMbps", now, bitrate / 1e6);
    }
}
