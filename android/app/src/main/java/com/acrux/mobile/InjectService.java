package com.acrux.mobile;

import android.os.Build;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.InputEvent;
import android.view.KeyCharacterMap;
import android.view.KeyEvent;
import android.view.MotionEvent;

import org.lsposed.hiddenapibypass.HiddenApiBypass;

import java.io.RandomAccessFile;
import java.lang.reflect.Method;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashSet;
import java.util.Set;

/**
 * Shizuku 가 ADB(shell) 권한으로 따로 띄우는 서비스 — 진짜 키보드 · 마우스처럼 입력을 넣음 (scrcpy 와 같은 방식)
 * 앱(Acrux)은 이 서비스에 '이 키를 눌러' · '마우스를 여기로' 만 보냄
 *
 * 키보드: /dev/uhid 로 진짜 USB 키보드 장치("Acrux Keyboard")를 만들어서 보냄 (블루투스 키보드가 쓰는 것과 같은 커널 기능)
 *   → 안드로이드 · 로블록스가 키보드를 꽂은 것과 똑같이 봄 (로블록스 모바일은 진짜 키보드가 있어야 W A S D 이동을 켬)
 *   입력 주입(장치 번호 -1 '가상 키보드')으로 보낸 키는 로블록스가 무시했음
 *   uhid 를 못 열면 (기종 · 보안 설정) 예전처럼 입력 주입으로
 */
public class InjectService extends IInject.Stub {
    Object im;
    Method inject, setActionButton;
    long downTime;
    String err = "";
    RandomAccessFile uhid;
    String uhidErr = "";
    final Set<Integer> held = new LinkedHashSet<>();
    int mods;
    final long[] keyDown = new long[400];
    final int[] repeat = new int[400];

    public InjectService() {
        if (Build.VERSION.SDK_INT >= 28) {
            try {
                HiddenApiBypass.addHiddenApiExemptions("");
            } catch (Throwable ignored) {
            }
        }
        try {
            Class<?> g = Class.forName("android.hardware.input.InputManagerGlobal");
            im = g.getMethod("getInstance").invoke(null);
            inject = g.getMethod("injectInputEvent", InputEvent.class, int.class);
        } catch (Throwable t) {
            try {
                Class<?> c = Class.forName("android.hardware.input.InputManager");
                im = c.getMethod("getInstance").invoke(null);
                inject = c.getMethod("injectInputEvent", InputEvent.class, int.class);
            } catch (Throwable t2) {
                err = String.valueOf(t2);
            }
        }
        try {
            setActionButton = MotionEvent.class.getMethod("setActionButton", int.class);
        } catch (Throwable ignored) {
        }
        uhidOpen();         // 연결하자마자 키보드 장치를 만들어 둠 (게임 중에 '키보드 꽂힘' 이 생기지 않게)
    }

    // ---------------------------------------------------------------- 진짜 키보드 장치 (uhid)
    static final int UHID_DESTROY = 1, UHID_CREATE2 = 11, UHID_INPUT2 = 12, EV = 4376;
    /** 일반 USB 키보드 (부트 키보드) 설명서: 수정 키 8개 + 예약 1바이트 + 키 6개 */
    static final byte[] DESC = hex("05010906a101050719e029e71500250175019508810295017508810195067508150025650507190029658100c0");

    static byte[] hex(String s) {
        byte[] b = new byte[s.length() / 2];
        for (int i = 0; i < b.length; i++) b[i] = (byte) Integer.parseInt(s.substring(i * 2, i * 2 + 2), 16);
        return b;
    }

    void uhidOpen() {
        if (uhid != null) return;
        try {
            RandomAccessFile f = new RandomAccessFile("/dev/uhid", "rw");
            ByteBuffer b = ByteBuffer.allocate(EV).order(ByteOrder.LITTLE_ENDIAN);
            b.putInt(0, UHID_CREATE2);
            byte[] name = "Acrux Keyboard".getBytes(StandardCharsets.US_ASCII);
            for (int i = 0; i < name.length; i++) b.put(4 + i, name[i]);
            b.putShort(260, (short) DESC.length);
            b.putShort(262, (short) 0x03);          // BUS_USB
            b.putInt(264, 0x1209);                  // vendor
            b.putInt(268, 0xAC10);                  // product
            b.putInt(272, 1);                       // version
            b.putInt(276, 0);                       // country
            for (int i = 0; i < DESC.length; i++) b.put(280 + i, DESC[i]);
            f.write(b.array());
            uhid = f;
            // 커널이 보내는 알림(START · OPEN · LED)을 계속 읽어서 버림 (안 읽으면 쌓임)
            Thread rd = new Thread(() -> {
                byte[] buf = new byte[EV];
                try {
                    while (uhid == f) {
                        if (f.read(buf) <= 0) break;
                    }
                } catch (Throwable ignored) {
                }
            }, "uhid-read");
            rd.setDaemon(true);
            rd.start();
            SystemClock.sleep(300);                 // 안드로이드가 새 키보드를 등록할 시간
        } catch (Throwable t) {
            uhidErr = String.valueOf(t);
            uhid = null;
        }
    }

    /** 안드로이드 키 코드 → USB 키보드 키 번호 (0 = 모름 · -1 = 왼쪽 Shift) */
    static int usage(int code) {
        if (code >= KeyEvent.KEYCODE_A && code <= KeyEvent.KEYCODE_Z) return 0x04 + (code - KeyEvent.KEYCODE_A);
        if (code >= KeyEvent.KEYCODE_1 && code <= KeyEvent.KEYCODE_9) return 0x1E + (code - KeyEvent.KEYCODE_1);
        switch (code) {
            case KeyEvent.KEYCODE_0: return 0x27;
            case KeyEvent.KEYCODE_ENTER: return 0x28;
            case KeyEvent.KEYCODE_ESCAPE: return 0x29;
            case KeyEvent.KEYCODE_DEL: return 0x2A;
            case KeyEvent.KEYCODE_TAB: return 0x2B;
            case KeyEvent.KEYCODE_SPACE: return 0x2C;
            case KeyEvent.KEYCODE_SLASH: return 0x38;
            case KeyEvent.KEYCODE_SHIFT_LEFT: return -1;
            default: return 0;
        }
    }

    synchronized boolean uhidKey(int code, boolean down) {
        int u = usage(code);
        if (uhid == null || u == 0) return false;
        if (u == -1) mods = down ? mods | 0x02 : mods & ~0x02;
        else if (down) held.add(u);
        else held.remove(u);
        byte[] r = new byte[8];
        r[0] = (byte) mods;
        int i = 2;
        for (int k : held) {
            if (i >= 8) break;
            r[i++] = (byte) k;
        }
        ByteBuffer b = ByteBuffer.allocate(6 + r.length).order(ByteOrder.LITTLE_ENDIAN);
        b.putInt(UHID_INPUT2).putShort((short) r.length).put(r);
        try {
            uhid.write(b.array());
            return true;
        } catch (Throwable t) {
            uhidErr = String.valueOf(t);
            try {
                uhid.close();
            } catch (Throwable ignored) {
            }
            uhid = null;
            return false;
        }
    }

    @Override
    public String keyMode() {
        return uhid != null ? "uhid" : "inject (" + uhidErr + ")";
    }

    @Override
    public void destroy() {
        try {
            if (uhid != null) {
                ByteBuffer b = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(UHID_DESTROY);
                uhid.write(b.array());
                uhid.close();
            }
        } catch (Throwable ignored) {
        }
        System.exit(0);
    }

    @Override
    public String ping() {
        return im != null && inject != null ? "ok" : "입력 기능을 못 찾음 " + err;
    }

    boolean send(InputEvent e) {
        try {
            Object r = inject.invoke(im, e, 0);
            return Boolean.TRUE.equals(r);
        } catch (Throwable t) {
            return false;
        }
    }

    @Override
    public boolean key(int code, boolean down) {
        if (uhidKey(code, down)) return true;
        // 예전 방식: 입력 주입 (누르고 있는 동안은 같은 누른 시각 + 반복 횟수 — 진짜 키보드의 자동 반복처럼)
        long now = SystemClock.uptimeMillis();
        int c = Math.max(0, Math.min(399, code));
        if (down && keyDown[c] == 0) {
            keyDown[c] = now;
            repeat[c] = 0;
        } else if (down) {
            repeat[c]++;
        }
        long dt = keyDown[c] != 0 ? keyDown[c] : now;
        if (!down) keyDown[c] = 0;
        KeyEvent e = new KeyEvent(dt, now, down ? KeyEvent.ACTION_DOWN : KeyEvent.ACTION_UP, code, down ? repeat[c] : 0, 0,
                KeyCharacterMap.VIRTUAL_KEYBOARD, 0, KeyEvent.FLAG_FROM_SYSTEM, InputDevice.SOURCE_KEYBOARD);
        return send(e);
    }

    @Override
    public boolean mouse(int action, float x, float y, int buttons, int actionButton, boolean relative) {
        long now = SystemClock.uptimeMillis();
        if (action == MotionEvent.ACTION_DOWN || downTime == 0) downTime = now;
        MotionEvent.PointerProperties pp = new MotionEvent.PointerProperties();
        pp.id = 0;
        pp.toolType = MotionEvent.TOOL_TYPE_MOUSE;
        MotionEvent.PointerCoords c = new MotionEvent.PointerCoords();
        c.x = x;
        c.y = y;
        c.pressure = buttons != 0 ? 1f : 0f;
        c.size = 1f;
        int source = InputDevice.SOURCE_MOUSE;
        if (relative) {
            source = InputDevice.SOURCE_MOUSE_RELATIVE;
            c.setAxisValue(MotionEvent.AXIS_RELATIVE_X, x);
            c.setAxisValue(MotionEvent.AXIS_RELATIVE_Y, y);
        }
        MotionEvent e = MotionEvent.obtain(downTime, now, action, 1, new MotionEvent.PointerProperties[]{pp},
                new MotionEvent.PointerCoords[]{c}, 0, buttons, 1f, 1f, 0, 0, source, 0);
        if (actionButton != 0 && setActionButton != null) {
            try {
                setActionButton.invoke(e, actionButton);
            } catch (Throwable ignored) {
            }
        }
        boolean ok = send(e);
        e.recycle();
        return ok;
    }
}
