package com.acrux.mobile;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.graphics.PixelFormat;
import android.hardware.display.DisplayManager;
import android.hardware.display.VirtualDisplay;
import android.media.Image;
import android.media.ImageReader;
import android.media.projection.MediaProjection;
import android.media.projection.MediaProjectionManager;
import android.os.Build;
import android.os.Handler;
import android.os.HandlerThread;
import android.os.IBinder;
import android.os.SystemClock;
import android.util.DisplayMetrics;
import android.view.Display;

import com.acrux.mobile.core.Fisher;
import com.acrux.mobile.core.Img;
import com.acrux.mobile.core.Mask;

import java.nio.ByteBuffer;

/**
 * 화면 캡처 (MediaProjection) — 자동 낚시가 화면 색을 보는 데 씀 · 녹화 · 저장은 안 함
 * 화면이 돌아가면(세로 → 가로) 캡처 크기도 맞춰 바꿈
 * 캡처가 끊겼는지 지켜봄: 다른 화면 녹화 앱이 켜지면 기종에 따라 검은 화면만 오거나 새 화면이 안 옴 (마지막 화면에 멈춤)
 *  → 매크로가 '낚시 창을 못 찾음' 으로만 끝나서 이유를 몰랐음 → 기록 · 알림으로 알려 줌
 */
public class CaptureService extends Service implements Fisher.Screen {
    static volatile CaptureService active;
    static final String CHANNEL = "acrux";

    MediaProjection mp;
    VirtualDisplay vd;
    ImageReader reader;
    Image cur;
    int w, h, dpi;
    long checkedAt;
    HandlerThread thread;
    Handler handler;
    // 캡처 상태: 마지막으로 새 화면이 온 때 · 검은 화면인지 (1초에 한 번 확인) · 마지막 경고
    volatile long frameAt, blackAt, blackSince;
    volatile boolean black;
    long warnedAt;
    /** 새 화면을 받은 횟수 (릴링이 같은 화면을 두 번 분석하지 않게) · 한 줄 복사용 버퍼 (매번 새로 안 만듦) */
    long frames;
    byte[] rowBuf = new byte[0];
    // 검은 화면은 6초 넘게 이어질 때만 (로블록스 로딩 · 리셋 때 잠깐 검은 화면은 괜찮음)
    static final long STALE_MS = 8000, BLACK_MS = 6000, WARN_GAP_MS = 30000;

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        Hub.init(this);
        NotificationManager nm = getSystemService(NotificationManager.class);
        nm.createNotificationChannel(new NotificationChannel(CHANNEL, "Acrux Mobile", NotificationManager.IMPORTANCE_LOW));
        PendingIntent open = PendingIntent.getActivity(this, 0, new Intent(this, MainActivity.class), PendingIntent.FLAG_IMMUTABLE);
        Notification n = new Notification.Builder(this, CHANNEL)
                .setContentTitle("Acrux Mobile")
                .setContentText(com.acrux.mobile.core.I18n.t("화면 캡처 중 · 매크로 준비됨"))
                .setSmallIcon(R.mipmap.ic_launcher)
                .setContentIntent(open)
                .setOngoing(true)
                .build();
        if (Build.VERSION.SDK_INT >= 29) startForeground(1, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION);
        else startForeground(1, n);
        if (intent == null || mp != null) return START_NOT_STICKY;
        int code = intent.getIntExtra("code", 0);
        Intent data = Build.VERSION.SDK_INT >= 33 ? intent.getParcelableExtra("data", Intent.class) : intent.getParcelableExtra("data");
        if (data == null) {
            stopSelf();
            return START_NOT_STICKY;
        }
        thread = new HandlerThread("acrux-capture");
        thread.start();
        handler = new Handler(thread.getLooper());
        try {
            MediaProjectionManager mpm = getSystemService(MediaProjectionManager.class);
            mp = mpm.getMediaProjection(code, data);
            mp.registerCallback(new MediaProjection.Callback() {
                @Override
                public void onStop() {
                    Hub.log("화면 캡처가 끝남", 'y');
                    Hub.stop();
                    stopSelf();
                }
            }, handler);
            int[] s = realSize();
            w = s[0];
            h = s[1];
            dpi = s[2];
            reader = ImageReader.newInstance(w, h, PixelFormat.RGBA_8888, 3);
            vd = mp.createVirtualDisplay("acrux", w, h, dpi, DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR, reader.getSurface(), null, handler);
            active = this;
            frameAt = SystemClock.uptimeMillis();
            Hub.log("화면 캡처 시작 (" + w + "x" + h + ")", 'g');
        } catch (RuntimeException e) {
            Hub.log("화면 캡처를 시작하지 못함: " + e, 'r');
            stopSelf();
        }
        Hub.changed();
        return START_NOT_STICKY;
    }

    int[] realSize() {
        DisplayManager dm = getSystemService(DisplayManager.class);
        Display d = dm.getDisplay(Display.DEFAULT_DISPLAY);
        DisplayMetrics m = new DisplayMetrics();
        d.getRealMetrics(m);
        return new int[]{m.widthPixels, m.heightPixels, m.densityDpi};
    }

    /** 화면이 돌아갔으면 캡처 크기를 바꿈 (0.5초에 한 번만 확인) */
    void fitSize() {
        long t = SystemClock.uptimeMillis();
        if (t - checkedAt < 500 || vd == null) return;
        checkedAt = t;
        int[] s = realSize();
        if (s[0] == w && s[1] == h) return;
        if (cur != null) {
            cur.close();
            cur = null;
        }
        ImageReader old = reader;
        reader = ImageReader.newInstance(s[0], s[1], PixelFormat.RGBA_8888, 3);
        vd.resize(s[0], s[1], s[2]);
        vd.setSurface(reader.getSurface());
        old.close();
        w = s[0];
        h = s[1];
        dpi = s[2];
        frameAt = t;                                 // 새 크기의 첫 화면을 기다리는 동안은 멈춤으로 보지 않음
        Hub.log("화면 크기 바뀜 → " + w + "x" + h, 'd');
    }

    @Override
    public synchronized int width() {
        fitSize();
        return w;
    }

    @Override
    public synchronized int height() {
        fitSize();
        return h;
    }

    /** 새 화면이 왔으면 바꿔 끼움 (+ 검은 화면 · 멈춤 확인) */
    void acquire() {
        Image img;
        try {
            img = reader.acquireLatestImage();
        } catch (IllegalStateException e) {
            img = null;
        }
        long now = SystemClock.uptimeMillis();
        if (img != null) {
            if (cur != null) cur.close();
            cur = img;
            frames++;
            frameAt = now;
            if (now - blackAt >= 1000) {
                blackAt = now;
                boolean b = isBlack(img);
                if (b && !black) blackSince = now;
                black = b;
            }
        }
        watch(now);
    }

    @Override
    public synchronized Img grab(int x, int y, int gw, int gh) {
        if (reader == null) return null;
        fitSize();
        acquire();
        if (cur == null) return null;
        int iw = cur.getWidth(), ih = cur.getHeight();
        int x0 = Math.max(0, x), y0 = Math.max(0, y), x1 = Math.min(iw, x + gw), y1 = Math.min(ih, y + gh);
        Img out = new Img(Math.max(0, x1 - x0), Math.max(0, y1 - y0));
        if (out.w == 0 || out.h == 0) return out;
        return copy(out, x0, y0, 0, 0, out.w, out.h, 1) ? out : null;
    }

    /** 릴링용: dst 크기만큼 (x, y) 부분을 dst 에 바로 복사 (새 이미지를 안 만듦) · 같은 화면이면 (known) 복사도 안 함 */
    @Override
    public synchronized long grabInto(Img dst, int x, int y, long known) {
        if (reader == null) return -1;
        fitSize();
        acquire();
        if (cur == null) return -1;
        if (frames == known) return known;
        int iw = cur.getWidth(), ih = cur.getHeight();
        int x0 = Math.max(0, x), y0 = Math.max(0, y), x1 = Math.min(iw, x + dst.w), y1 = Math.min(ih, y + dst.h);
        if (x0 != x || y0 != y || x1 - x0 != dst.w || y1 - y0 != dst.h) dst.clear();      // 화면 밖은 검은색
        if (x1 > x0 && y1 > y0 && !copy(dst, x0, y0, x0 - x, y0 - y, x1 - x0, y1 - y0, 1)) return -1;
        return frames;
    }

    /** 줄인 화면: step 칸마다 한 칸 (결과창 테두리 찾기 · 전체 화면을 그대로 복사하면 폰에선 한 번에 수십 MB) */
    @Override
    public synchronized Img grabStep(int x, int y, int gw, int gh, int step) {
        if (reader == null) return null;
        fitSize();
        acquire();
        if (cur == null) return null;
        step = Math.max(1, step);
        int iw = cur.getWidth(), ih = cur.getHeight();
        int x0 = Math.max(0, x), y0 = Math.max(0, y), x1 = Math.min(iw, x + gw), y1 = Math.min(ih, y + gh);
        Img out = new Img(Math.max(0, (x1 - x0 + step - 1) / step), Math.max(0, (y1 - y0 + step - 1) / step));
        if (out.w == 0 || out.h == 0) return out;
        return copy(out, x0, y0, 0, 0, out.w, out.h, step) ? out : null;
    }

    synchronized android.graphics.Bitmap ocrBitmap(int x, int y, int width, int height) {
        if (reader == null) return null;
        fitSize();
        acquire();
        if (cur == null) return null;
        int left = Math.max(0, x), top = Math.max(0, y);
        int cols = Math.min(cur.getWidth(), x + width) - left;
        int rows = Math.min(cur.getHeight(), y + height) - top;
        if (cols < 8 || rows < 8) return null;
        android.graphics.Bitmap bitmap = android.graphics.Bitmap.createBitmap(cols, rows, android.graphics.Bitmap.Config.ARGB_8888);
        boolean complete = false;
        try {
            Img strip = new Img(cols, Math.min(32, rows));
            int[] packed = new int[strip.w * strip.h];
            for (int offset = 0; offset < rows; offset += strip.h) {
                int count = Math.min(strip.h, rows - offset);
                if (!copy(strip, left, top + offset, 0, 0, cols, count, 1)) return null;
                for (int k = 0; k < cols * count; k++)
                    packed[k] = 0xff000000 | (strip.r[k] << 16) | (strip.g[k] << 8) | strip.b[k];
                bitmap.setPixels(packed, 0, cols, 0, offset, cols, count);
            }
            complete = true;
            return bitmap;
        } finally {
            if (!complete) bitmap.recycle();
        }
    }

    /** 지금 화면 (sx, sy) 부터 step 칸마다 w × h 칸 → dst 의 (dx, dy) 부터 · 실패하면 false (화면이 바뀌는 순간 닫힌 이미지) */
    boolean copy(Img dst, int sx, int sy, int dx, int dy, int w, int h, int step) {
        Image.Plane p;
        ByteBuffer buf;
        try {
            p = cur.getPlanes()[0];
            buf = p.getBuffer().duplicate();
        } catch (RuntimeException e) {               // 화면이 바뀌는 순간 닫힌 이미지
            cur.close();
            cur = null;
            return false;
        }
        int rs = p.getRowStride(), ps = Math.max(4, p.getPixelStride()), lim = buf.limit();
        int need = ((w - 1) * step + 1) * ps;
        if (rowBuf.length < need) rowBuf = new byte[need];
        byte[] row = rowBuf;
        for (int yy = 0; yy < h; yy++) {
            int at = (sy + yy * step) * rs + sx * ps;
            if (at < 0 || at >= lim) {                   // 기종에 따라 버퍼가 화면보다 짧을 때 → 나머지 줄은 검은색
                for (int y2 = yy; y2 < h; y2++) {
                    int k = (dy + y2) * dst.w + dx;
                    java.util.Arrays.fill(dst.r, k, k + w, 0);
                    java.util.Arrays.fill(dst.g, k, k + w, 0);
                    java.util.Arrays.fill(dst.b, k, k + w, 0);
                }
                break;
            }
            buf.position(at);
            int n = Math.min(need, lim - at);
            buf.get(row, 0, n);
            if (n < need) java.util.Arrays.fill(row, n, need, (byte) 0);
            int k = (dy + yy) * dst.w + dx, adv = ps * step;
            for (int xx = 0, o = 0; xx < w; xx++, o += adv, k++) {
                dst.r[k] = row[o] & 255;
                dst.g[k] = row[o + 1] & 255;
                dst.b[k] = row[o + 2] & 255;
            }
        }
        // 낚시 테스트로 화면에 그린 칸 · 글자도 같이 찍힘 → 그 자리는 바로 바깥 칸 색으로 (인식에 안 섞이게)
        if (Mask.on()) {
            try {
                final ByteBuffer src = buf;
                final int iw = cur.getWidth(), ih = cur.getHeight(), rs2 = rs, ps2 = ps, lim2 = lim;
                Mask.fill(dst, sx, sy, dx, dy, w, h, step, (x, y) -> {
                    if (x < 0 || y < 0 || x >= iw || y >= ih) return -1;
                    int at = y * rs2 + x * ps2;
                    if (at < 0 || at + 2 >= lim2) return -1;
                    return ((src.get(at) & 255) << 16) | ((src.get(at + 1) & 255) << 8) | (src.get(at + 2) & 255);
                });
            } catch (RuntimeException ignored) {                // 화면이 바뀌는 순간 닫힌 이미지 · 그대로 씀
            }
        }
        return true;
    }

    /** 거의 다 검은 화면인지 (드문드문 찍어 봄) */
    static boolean isBlack(Image img) {
        try {
            Image.Plane p = img.getPlanes()[0];
            ByteBuffer buf = p.getBuffer();
            int iw = img.getWidth(), ih = img.getHeight(), rs = p.getRowStride(), ps = Math.max(4, p.getPixelStride()), lim = buf.limit();
            int seen = 0, lit = 0;
            for (int yy = ih / 40; yy < ih; yy += Math.max(1, ih / 20)) {
                for (int xx = iw / 60; xx < iw; xx += Math.max(1, iw / 30)) {
                    int at = yy * rs + xx * ps;
                    if (at + 2 >= lim) continue;
                    seen++;
                    if ((buf.get(at) & 255) + (buf.get(at + 1) & 255) + (buf.get(at + 2) & 255) > 36) lit++;
                }
            }
            return seen > 100 && lit < seen * 0.01;
        } catch (RuntimeException e) {
            return false;
        }
    }

    /** 지금 캡처 문제 (null = 괜찮음) */
    String problem(long now) {
        if (black && now - blackSince > BLACK_MS) return "화면 캡처가 검은 화면만 받음";
        if (frameAt > 0 && now - frameAt > STALE_MS) return "화면 캡처가 멈춤 (새 화면이 안 들어옴)";
        return null;
    }

    /** 매크로가 화면을 보는 중에 캡처가 끊겼으면 알림 (30초에 한 번) */
    void watch(long now) {
        String why = problem(now);
        if (why == null || now - warnedAt < WARN_GAP_MS) return;
        warnedAt = now;
        String msg = why + " — 다른 화면 녹화 앱이 켜져 있으면 끄고, 앱에서 [화면 캡처 시작] 을 다시 눌러 주세요";
        Hub.log(msg, 'r');
        Hub.main.post(() -> android.widget.Toast.makeText(this, com.acrux.mobile.core.I18n.t(msg), android.widget.Toast.LENGTH_LONG).show());
    }

    @Override
    public void onDestroy() {
        active = null;
        Hub.stop();
        synchronized (this) {
            if (cur != null) cur.close();
            cur = null;
            if (vd != null) vd.release();
            vd = null;
            if (reader != null) reader.close();
            reader = null;
        }
        if (mp != null) mp.stop();
        mp = null;
        if (thread != null) thread.quitSafely();
        Hub.changed();
        super.onDestroy();
    }
}
