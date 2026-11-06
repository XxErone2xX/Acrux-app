package com.acrux.mobile;

import android.os.Build;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.InputEvent;
import android.view.KeyCharacterMap;
import android.view.KeyEvent;
import android.view.MotionEvent;

import org.lsposed.hiddenapibypass.HiddenApiBypass;

import java.lang.reflect.Method;

/**
 * Shizuku 가 ADB(shell) 권한으로 따로 띄우는 서비스 — 진짜 키보드 · 마우스처럼 입력을 넣음 (scrcpy 와 같은 방식)
 * 앱(Acrux)은 이 서비스에 '이 키를 눌러' · '마우스를 여기로' 만 보냄
 */
public class InjectService extends IInject.Stub {
    Object im;
    Method inject, setActionButton;
    long downTime;
    String err = "";

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
    }

    @Override
    public void destroy() {
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
        long now = SystemClock.uptimeMillis();
        KeyEvent e = new KeyEvent(now, now, down ? KeyEvent.ACTION_DOWN : KeyEvent.ACTION_UP, code, 0, 0,
                KeyCharacterMap.VIRTUAL_KEYBOARD, 0, 0, InputDevice.SOURCE_KEYBOARD);
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
