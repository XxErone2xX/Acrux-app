package com.acrux.mobile.core;

/**
 * PC 와 같은 키보드 · 마우스 입력 (PC 버전 macro.py 의 key_down · key_up · key_tap · move_to · move_rel · mouse_button · click · mouse_click_here)
 * 모바일에서는 Shizuku(ADB 권한)로 로블록스에 진짜 키 · 마우스 신호를 보냄
 * 키 이름: esc r enter w a s d e o / space shift · 버튼: left · right
 */
public interface Pc {
    void keyDown(String key);

    void keyUp(String key);

    /** 화면 좌표(px)로 마우스 이동 (로블록스가 위치를 인식하도록 1px 흔듦) */
    void moveTo(int x, int y);

    /** 지금 자리에서 dx, dy 만큼 (우클릭 드래그로 카메라 돌리기) */
    void moveRel(int dx, int dy);

    void button(String button, boolean down);

    static void nap(long ms) {
        try {
            Thread.sleep(Math.max(0, ms));
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    default void keyTap(String key) {
        keyDown(key);
        nap(80);                 // 느린 기기(30fps 이하)에서도 한 프레임 넘게 눌려 있게
        keyUp(key);
    }

    /** PC macro.click: 옮기고 0.03초 → 누름 → 0.03초 → 뗌 */
    default void click(int x, int y, String button) {
        moveTo(x, y);
        nap(30);
        button(button, true);
        nap(30);
        button(button, false);
    }

    /** PC macro.mouse_click_here: 마우스를 안 움직이고 지금 자리에서 (릴링처럼 빠르게 연속으로) */
    default void clickHere(String button, int holdMs) {
        button(button, true);
        nap(holdMs);
        button(button, false);
    }
}
