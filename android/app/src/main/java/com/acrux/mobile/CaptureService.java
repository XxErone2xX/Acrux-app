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

import java.nio.ByteBuffer;

/**
 * 화면 캡처 (MediaProjection) — 자동 낚시가 화면 색을 보는 데 씀 · 녹화 · 저장은 안 함
 * 화면이 돌아가면(세로 → 가로) 캡처 크기도 맞춰 바꿈
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
                .setContentText(com.acrux.mobile.core.I18n.t("화면 캡처 중 · 자동 낚시 준비됨"))
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

    @Override
    public synchronized Img grab(int x, int y, int gw, int gh) {
        if (reader == null) return null;
        fitSize();
        Image img;
        try {
            img = reader.acquireLatestImage();
        } catch (IllegalStateException e) {
            img = null;
        }
        if (img != null) {
            if (cur != null) cur.close();
            cur = img;
        }
        if (cur == null) return null;
        int iw = cur.getWidth(), ih = cur.getHeight();
        int x0 = Math.max(0, x), y0 = Math.max(0, y), x1 = Math.min(iw, x + gw), y1 = Math.min(ih, y + gh);
        Img out = new Img(Math.max(0, x1 - x0), Math.max(0, y1 - y0));
        if (out.w == 0 || out.h == 0) return out;
        Image.Plane p = cur.getPlanes()[0];
        ByteBuffer buf = p.getBuffer().duplicate();
        int rs = p.getRowStride(), ps = p.getPixelStride();
        byte[] row = new byte[out.w * ps];
        for (int yy = 0; yy < out.h; yy++) {
            buf.position((y0 + yy) * rs + x0 * ps);
            buf.get(row, 0, Math.min(row.length, buf.remaining()));
            int k = yy * out.w;
            for (int xx = 0, o = 0; xx < out.w; xx++, o += ps, k++) {
                out.r[k] = row[o] & 255;
                out.g[k] = row[o + 1] & 255;
                out.b[k] = row[o + 2] & 255;
            }
        }
        return out;
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
