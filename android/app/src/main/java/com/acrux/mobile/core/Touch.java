package com.acrux.mobile.core;

/**
 * 화면 터치 (안드로이드: 접근성 터치 · 루팅 · Shizuku 없이) — 이동 · 판매에 쓰는 입력 전부
 *  · tap: 버튼 누르기 · swipe: 한 손가락 끌기 (화면 돌리기) · pinch: 두 손가락 (줌)
 *  · stick: 로블록스 클래식 조이스틱을 그 방향으로 밀고 있기 (길이 1 = 끝까지 · null = 뗌)
 * 좌표는 화면 px · 시간은 ms · 누르기만 하고 바로 돌아옴 (끝날 때까지 기다리는 건 부르는 쪽)
 */
public interface Touch {
    void tap(int x, int y, int ms);

    void swipe(int x0, int y0, int x1, int y1, int ms);

    /** (cx, cy) 를 가운데로 두 손가락을 위아래로 d0 → d1 간격으로 (d1 < d0: 오므리기 = 줌 아웃) · 처음에 잠깐 가만히 댄 뒤 움직임 */
    void pinch(int cx, int cy, int d0, int d1, int ms);

    /** 조이스틱 방향 (화면 좌표: 오른쪽 +x · 아래 +y) · null = 손가락 뗌 */
    void stick(double[] v);

    /** (x, y) 의 점프 버튼을 누름 (떼라고 할 때까지 누르고 있음 · 조이스틱 손가락과 따로) */
    void jumpDown(int x, int y);

    /** 점프 버튼 뗌 */
    void jumpUp();

    /** 조이스틱 {가운데 x, 가운데 y, 반지름} (px) · 없으면 null */
    double[] stickAt();

    static void nap(long ms) {
        try {
            Thread.sleep(Math.max(0, ms));
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }
}
