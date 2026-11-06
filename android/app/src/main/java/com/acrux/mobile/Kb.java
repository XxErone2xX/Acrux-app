package com.acrux.mobile;

import android.os.Build;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.KeyCharacterMap;
import android.view.KeyEvent;
import android.view.inputmethod.InputConnection;

import java.util.HashMap;
import java.util.Map;

/**
 * 키보드 방식 — 키를 입력기(키보드)로 보냄. 보낼 곳 두 군데를 차례로 시도:
 *   1. Acrux 키보드 (이 앱의 키보드 · 설정에서 켜고 지금 키보드로 선택)
 *   2. 접근성 입력기 (안드로이드 13+ · 키보드를 안 바꿔도 됨)
 */
final class Kb {
    private Kb() {}

    static final Map<String, Integer> CODES = new HashMap<>();

    static {
        CODES.put("esc", KeyEvent.KEYCODE_ESCAPE);
        CODES.put("enter", KeyEvent.KEYCODE_ENTER);
        CODES.put("/", KeyEvent.KEYCODE_SLASH);
        CODES.put("space", KeyEvent.KEYCODE_SPACE);
        CODES.put("shift", KeyEvent.KEYCODE_SHIFT_LEFT);
        for (char c = 'a'; c <= 'z'; c++) CODES.put(String.valueOf(c), KeyEvent.KEYCODE_A + (c - 'a'));
    }

    static final Map<String, long[]> down = new HashMap<>();     // 키 → {누른 시각, 반복 횟수}
    static volatile String last = "";

    static KeyEvent event(String key, boolean press) {
        Integer code = CODES.get(key);
        if (code == null) return null;
        long now = SystemClock.uptimeMillis();
        long[] d;
        synchronized (down) {
            d = down.get(key);
            if (press) {
                if (d == null) down.put(key, d = new long[]{now, 0});
                else d[1]++;
            } else {
                down.remove(key);
                if (d == null) d = new long[]{now, 0};
            }
        }
        return new KeyEvent(d[0], now, press ? KeyEvent.ACTION_DOWN : KeyEvent.ACTION_UP, code, press ? (int) d[1] : 0, 0,
                KeyCharacterMap.VIRTUAL_KEYBOARD, 0, KeyEvent.FLAG_SOFT_KEYBOARD | KeyEvent.FLAG_KEEP_TOUCH_MODE, InputDevice.SOURCE_KEYBOARD);
    }

    /** 키 하나 보내기 → 보냈으면 true */
    static boolean key(String key, boolean press) {
        KeyEvent e = event(key, press);
        if (e == null) return true;                 // 모르는 키는 무시
        InputConnection ic = AcruxKeyboard.connection();
        if (ic != null && ic.sendKeyEvent(e)) {
            last = "Acrux 키보드";
            return true;
        }
        TapService t = TapService.instance;
        if (t != null && t.imeKey(e)) {
            last = "접근성 입력기";
            return true;
        }
        return false;
    }

    static boolean keyboardOn() {
        return AcruxKeyboard.instance != null;
    }

    static boolean accessibilityImeOk() {
        return Build.VERSION.SDK_INT >= 33 && TapService.instance != null;
    }

    static String status() {
        String a = keyboardOn() ? "Acrux 키보드 선택됨" : "Acrux 키보드 선택 안 됨";
        String b = Build.VERSION.SDK_INT >= 33 ? (TapService.instance != null ? "접근성 입력기 켜짐" : "접근성 입력기 꺼짐 (접근성 권한)")
                : "접근성 입력기는 안드로이드 13+";
        return a + " · " + b + (last.isEmpty() ? "" : " · 마지막으로 보낸 곳: " + last);
    }
}
