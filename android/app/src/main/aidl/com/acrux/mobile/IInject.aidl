// Shizuku(ADB 권한)로 도는 입력 서비스 — 로블록스에 PC 와 같은 키보드 · 마우스 신호를 보냄
package com.acrux.mobile;

interface IInject {
    void destroy() = 16777114;
    String ping() = 1;
    boolean key(int keyCode, boolean down) = 2;
    boolean mouse(int action, float x, float y, int buttons, int actionButton, boolean relative) = 3;
    /** 키를 어떻게 보내는지: "uhid" (진짜 키보드 장치) · "inject …" (입력 주입 — 이유) */
    String keyMode() = 4;
}
