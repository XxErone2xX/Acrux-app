package com.acrux.mobile;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.RemoteInput;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.os.Build;
import android.os.Bundle;

import java.io.BufferedReader;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.security.KeyFactory;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.PrivateKey;
import java.security.PublicKey;
import java.security.SecureRandom;
import java.security.cert.Certificate;
import java.security.cert.CertificateFactory;
import java.security.spec.PKCS8EncodedKeySpec;
import java.util.Date;
import java.util.Random;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Consumer;

import android.sun.security.x509.AlgorithmId;
import android.sun.security.x509.CertificateAlgorithmId;
import android.sun.security.x509.CertificateExtensions;
import android.sun.security.x509.CertificateIssuerName;
import android.sun.security.x509.CertificateSerialNumber;
import android.sun.security.x509.CertificateSubjectName;
import android.sun.security.x509.CertificateValidity;
import android.sun.security.x509.CertificateVersion;
import android.sun.security.x509.CertificateX509Key;
import android.sun.security.x509.KeyIdentifier;
import android.sun.security.x509.PrivateKeyUsageExtension;
import android.sun.security.x509.SubjectKeyIdentifierExtension;
import android.sun.security.x509.X500Name;
import android.sun.security.x509.X509CertImpl;
import android.sun.security.x509.X509CertInfo;

import io.github.muntashirakon.adb.AbsAdbConnectionManager;
import io.github.muntashirakon.adb.AdbPairingRequiredException;
import io.github.muntashirakon.adb.AdbStream;
import io.github.muntashirakon.adb.android.AdbMdns;

/**
 * 무선 디버깅으로 이 폰에 ADB 연결 (PC · 루팅 · Shizuku 없이 · 안드로이드 11 이상)
 *  · 처음 한 번: 설정 → 개발자 옵션 → 무선 디버깅 → '페어링 코드로 기기 페어링' → 알림 칸에 6자리 코드 입력
 *  · 그다음부터: 무선 디버깅만 켜져 있으면 저절로 연결 (재부팅 · 와이파이가 바뀌면 무선 디버깅 스위치만 다시)
 *  · 연결되면 ADB 권한 (shell) 으로 로블록스 로그를 읽음 → 바이옴
 * 연결 키 (RSA) 는 앱 안 폴더에만 저장 (다른 앱 · 기기로 안 나감)
 */
final class Adb extends AbsAdbConnectionManager {
    static final String CH = "acrux_adb";
    static final int NOTI = 7301;
    static final String KEY_CODE = "code";
    static final String ACT_PAIR = "com.acrux.mobile.ADB_PAIR";

    private static Adb inst;
    private final PrivateKey key;
    private final Certificate cert;

    static synchronized Adb get(Context c) throws Exception {
        if (inst == null) inst = new Adb(c.getApplicationContext());
        return inst;
    }

    private Adb(Context c) throws Exception {
        setApi(Build.VERSION.SDK_INT);
        File kf = new File(c.getFilesDir(), "adb.key"), cf = new File(c.getFilesDir(), "adb.crt");
        PrivateKey k = null;
        Certificate ct = null;
        if (kf.exists() && cf.exists()) {
            try (InputStream in = new FileInputStream(kf)) {
                byte[] b = new byte[(int) kf.length()];
                int n = 0;
                while (n < b.length) {
                    int r = in.read(b, n, b.length - n);
                    if (r < 0) break;
                    n += r;
                }
                k = KeyFactory.getInstance("RSA").generatePrivate(new PKCS8EncodedKeySpec(b));
            }
            try (InputStream in = new FileInputStream(cf)) {
                ct = CertificateFactory.getInstance("X.509").generateCertificate(in);
            }
        }
        if (k == null || ct == null) {
            KeyPairGenerator g = KeyPairGenerator.getInstance("RSA");
            g.initialize(2048, new SecureRandom());
            KeyPair kp = g.generateKeyPair();
            PublicKey pub = kp.getPublic();
            k = kp.getPrivate();
            String alg = "SHA512withRSA";
            Date from = new Date(), to = new Date(System.currentTimeMillis() + 3650L * 86400000L);
            CertificateExtensions ext = new CertificateExtensions();
            ext.set("SubjectKeyIdentifier", new SubjectKeyIdentifierExtension(new KeyIdentifier(pub).getIdentifier()));
            ext.set("PrivateKeyUsage", new PrivateKeyUsageExtension(from, to));
            X500Name name = new X500Name("CN=Acrux Mobile");
            X509CertInfo info = new X509CertInfo();
            info.set("version", new CertificateVersion(2));
            info.set("serialNumber", new CertificateSerialNumber(new Random().nextInt() & Integer.MAX_VALUE));
            info.set("algorithmID", new CertificateAlgorithmId(AlgorithmId.get(alg)));
            info.set("subject", new CertificateSubjectName(name));
            info.set("key", new CertificateX509Key(pub));
            info.set("validity", new CertificateValidity(from, to));
            info.set("issuer", new CertificateIssuerName(name));
            info.set("extensions", ext);
            X509CertImpl x = new X509CertImpl(info);
            x.sign(k, alg);
            ct = x;
            try (OutputStream o = new FileOutputStream(kf)) {
                o.write(k.getEncoded());
            }
            try (OutputStream o = new FileOutputStream(cf)) {
                o.write(ct.getEncoded());               // DER 그대로 (CertificateFactory 가 읽음)
            }
        }
        key = k;
        cert = ct;
    }

    @Override
    protected PrivateKey getPrivateKey() {
        return key;
    }

    @Override
    protected Certificate getCertificate() {
        return cert;
    }

    @Override
    protected String getDeviceName() {
        return "Acrux Mobile";
    }

    // ---- 바깥에서 쓰는 것 (전부 백그라운드 스레드에서)

    static boolean supported() {
        return Build.VERSION.SDK_INT >= 30;
    }

    static volatile boolean connected;

    /** 무선 디버깅이 켜져 있으면 연결 · 페어링이 필요하면 'pair' · 꺼져 있으면 'off' · 되면 'ok' */
    static String connect(Context c) {
        if (!supported()) return "old";
        try {
            Adb a = get(c);
            if (a.isConnected()) return "ok";
            boolean ok = a.autoConnect(c.getApplicationContext(), 6000);
            connected = ok;
            return ok ? "ok" : "off";
        } catch (AdbPairingRequiredException e) {
            return "pair";
        } catch (InterruptedException e) {
            return "off";                                    // 무선 디버깅 서비스를 못 찾음 (꺼져 있음)
        } catch (Throwable t) {
            Hub.log("ADB 연결 실패: " + t, 'r');
            return "off";
        }
    }

    /** 페어링: 지금 열려 있는 '페어링 코드' 창의 포트를 찾아서 코드로 페어링 → 바로 연결 */
    static String pair(Context c, String code) {
        if (!supported()) return "old";
        try {
            AtomicInteger port = new AtomicInteger(-1);
            AtomicReference<String> host = new AtomicReference<>("127.0.0.1");
            CountDownLatch got = new CountDownLatch(1);
            AdbMdns m = new AdbMdns(c.getApplicationContext(), AdbMdns.SERVICE_TYPE_TLS_PAIRING, (addr, p) -> {
                if (addr != null) host.set(addr.getHostAddress());
                port.set(p);
                got.countDown();
            });
            m.start();
            try {
                got.await(12, TimeUnit.SECONDS);
            } finally {
                m.stop();
            }
            if (port.get() <= 0) return "noport";
            get(c).pair(host.get(), port.get(), code.trim());
            String r = connect(c);
            return r.equals("ok") ? "ok" : "paired";
        } catch (Throwable t) {
            Hub.log("ADB 페어링 실패: " + t, 'r');
            return "fail";
        }
    }

    /** shell 명령 → 한 줄씩 (끝날 때까지 · 최대 ms) */
    static void shell(Context c, String cmd, long ms, Consumer<String> line) throws Exception {
        shell(c, cmd, ms, 0, line);
    }

    /** idleMs > 0: 그 시간 동안 한 줄도 안 오면 끊긴 걸로 보고 닫음 (계속 흘러나오는 로그용) */
    static void shell(Context c, String cmd, long ms, long idleMs, Consumer<String> line) throws Exception {
        AdbStream s = get(c).openStream("shell:" + cmd);
        long end = System.currentTimeMillis() + ms;
        long[] seen = {System.currentTimeMillis()};
        Thread killer = new Thread(() -> {
            try {
                while (true) {
                    Thread.sleep(idleMs > 0 ? Math.min(5000, idleMs) : Math.max(1, end - System.currentTimeMillis()));
                    long now = System.currentTimeMillis();
                    if (now >= end || (idleMs > 0 && now - seen[0] > idleMs)) break;
                }
                s.close();
            } catch (Exception ignored) {
            }
        });
        killer.setDaemon(true);
        killer.start();
        try (BufferedReader r = new BufferedReader(new InputStreamReader(s.openInputStream(), StandardCharsets.UTF_8))) {
            String l;
            while (System.currentTimeMillis() < end && (l = r.readLine()) != null) {
                seen[0] = System.currentTimeMillis();
                line.accept(l);
            }
        } catch (Exception e) {
            if (!s.isClosed()) throw e;
        } finally {
            killer.interrupt();
            try {
                s.close();
            } catch (Exception ignored) {
            }
        }
    }

    // ---- 페어링 알림 (설정의 '페어링 코드' 창을 띄운 채로 알림 칸에서 코드 입력)

    static void showPairNotice(Context c, String text) {
        NotificationManager nm = c.getSystemService(NotificationManager.class);
        if (nm == null) return;
        nm.createNotificationChannel(new NotificationChannel(CH, Ui.tr("무선 디버깅 페어링"), NotificationManager.IMPORTANCE_HIGH));
        Intent i = new Intent(c, PairReceiver.class).setAction(ACT_PAIR);
        int fl = PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= 31 ? PendingIntent.FLAG_MUTABLE : 0);
        PendingIntent pi = PendingIntent.getBroadcast(c, 1, i, fl);
        RemoteInput ri = new RemoteInput.Builder(KEY_CODE).setLabel(Ui.tr("6자리 페어링 코드")).build();
        Notification.Action act = new Notification.Action.Builder(null, Ui.tr("코드 입력"), pi).addRemoteInput(ri).build();
        Notification n = new Notification.Builder(c, CH)
                .setSmallIcon(android.R.drawable.stat_sys_data_bluetooth)
                .setContentTitle(Ui.tr("Acrux — 무선 디버깅 페어링"))
                .setContentText(Ui.tr(text))
                .setStyle(new Notification.BigTextStyle().bigText(Ui.tr(text)))
                .addAction(act)
                .setOnlyAlertOnce(true)
                .build();
        nm.notify(NOTI, n);
    }

    /** 바이옴 로그 테스트가 끝났을 때 (로블록스 화면에 있어도 보이게) */
    static void notifyDone(Context c, String text) {
        NotificationManager nm = c.getSystemService(NotificationManager.class);
        if (nm == null) return;
        nm.createNotificationChannel(new NotificationChannel(CH, Ui.tr("무선 디버깅 페어링"), NotificationManager.IMPORTANCE_HIGH));
        Intent open = new Intent(c, MainActivity.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_SINGLE_TOP);
        PendingIntent pi = PendingIntent.getActivity(c, 2, open, PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        Notification n = new Notification.Builder(c, CH)
                .setSmallIcon(android.R.drawable.stat_sys_data_bluetooth)
                .setContentTitle(Ui.tr("바이옴 로그 테스트"))
                .setContentText(Ui.tr(text))
                .setContentIntent(pi)
                .setAutoCancel(true)
                .build();
        nm.notify(NOTI + 1, n);
    }

    static void hidePairNotice(Context c) {
        NotificationManager nm = c.getSystemService(NotificationManager.class);
        if (nm != null) nm.cancel(NOTI);
    }

    /** 알림 칸에서 코드를 입력하면 */
    public static final class PairReceiver extends BroadcastReceiver {
        @Override
        public void onReceive(Context c, Intent intent) {
            Bundle b = RemoteInput.getResultsFromIntent(intent);
            CharSequence cs = b == null ? null : b.getCharSequence(KEY_CODE);
            if (cs == null) return;
            String code = cs.toString().replaceAll("[^0-9]", "");
            PendingResult pr = goAsync();
            showPairNotice(c, "페어링 중…");
            new Thread(() -> {
                try {
                    String r = pair(c, code);
                    switch (r) {
                        case "ok":
                            hidePairNotice(c);
                            Hub.log("무선 디버깅 연결됨 — 이제 바이옴 로그 테스트를 할 수 있어요", 'g');
                            break;
                        case "paired":
                            showPairNotice(c, "페어링은 됐는데 연결을 못 함 — 무선 디버깅이 켜져 있는지 확인하고 앱에서 [연결]");
                            break;
                        case "noport":
                            showPairNotice(c, "페어링 창을 못 찾음 — 무선 디버깅 → '페어링 코드로 기기 페어링' 창을 띄운 채로 다시 입력");
                            break;
                        default:
                            showPairNotice(c, "페어링 실패 — 코드가 맞는지 확인하고 창을 다시 띄워서 새 코드로 입력");
                            break;
                    }
                    Hub.changed();
                } finally {
                    pr.finish();
                }
            }, "acrux-adb-pair").start();
        }
    }
}
