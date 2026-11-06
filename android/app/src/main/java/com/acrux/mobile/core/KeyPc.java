package com.acrux.mobile.core;

import java.util.HashSet;
import java.util.Set;

/**
 * 키보드 방식: 키(Esc R Enter W A S D E O / Space Shift)는 키보드(입력기)로 진짜 키 신호를 보냄
 * 마우스는 키보드로 못 보내서 화면 터치로: 클릭 → 그 자리 탭 (우클릭 = Click/Tap to Move) · 드래그 → 화면 끌기
 */
public final class KeyPc implements Pc {
    /** 키보드 — 보냈으면 true (보낼 곳이 없으면 false) */
    public interface Keys {
        boolean key(String key, boolean down);
    }

    /** 터치 (화면 px · 끝날 때까지 기다림) */
    public interface Touch {
        void tap(int x, int y, int ms);

        void swipe(int x1, int y1, int x2, int y2, int ms);
    }

    public static final String NO_KEYS = "키보드로 키를 보내지 못함 — 앱 → ③ 키보드 방식을 확인 (Acrux 키보드 선택 · 또는 접근성 입력기)";

    final Keys keys;
    final Touch t;
    final Fisher.Screen scr;
    public int tapMs = 120;
    public double dragScale = 0.5;

    final Set<String> held = new HashSet<>();
    int mx, my, downX, downY, dragX, dragY;
    long downAt;
    boolean btnDown;

    public KeyPc(Keys keys, Touch t, Fisher.Screen scr) {
        this.keys = keys;
        this.t = t;
        this.scr = scr;
    }

    @Override
    public void keyDown(String key) {
        if (!keys.key(key, true)) throw new RuntimeException(NO_KEYS);
        held.add(key);
    }

    @Override
    public void keyUp(String key) {
        if (!held.remove(key)) return;
        keys.key(key, false);
    }

    @Override
    public void moveTo(int x, int y) {
        mx = x;
        my = y;
    }

    @Override
    public void moveRel(int dx, int dy) {
        if (btnDown) {
            dragX += dx;
            dragY += dy;
        }
        mx += dx;
        my += dy;
    }

    @Override
    public void button(String button, boolean down) {
        if (down) {
            if (btnDown) return;
            btnDown = true;
            downX = mx;
            downY = my;
            dragX = dragY = 0;
            downAt = System.nanoTime();
            return;
        }
        if (!btnDown) return;
        btnDown = false;
        if (Math.abs(dragX) + Math.abs(dragY) > 6) {
            int w = scr.width(), h = scr.height();
            t.swipe(downX, downY, clamp(downX + (int) (dragX * dragScale), 2, w - 3), clamp(downY + (int) (dragY * dragScale), 2, h - 3), 600);
            return;
        }
        int ms = (int) ((System.nanoTime() - downAt) / 1_000_000);
        t.tap(downX, downY, Math.max(tapMs, ms));        // 짧게 누르면 호버만 되고 안 눌릴 때가 있음
    }

    static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
