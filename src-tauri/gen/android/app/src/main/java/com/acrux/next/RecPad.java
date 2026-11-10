package com.acrux.next;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.PixelFormat;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowManager;




import java.util.concurrent.ConcurrentLinkedQueue;

/**
 * 길 기록용 WASD 패드 · 점프 버튼 — 키를 톡 누르면 켜지고 (홀드) 다시 누르면 꺼짐 · 켜 둔 동안 Acrux 가 로블록스 조이스틱을 그 방향으로 계속 밂
 *  · 꾹 누르기가 아닌 이유: 손가락이 화면에서 움직일 때마다 안드로이드가 Acrux 가 누르던 터치를 끊음 → 톡 누르는 순간만 끊기고 바로 다시 밂
 *  · W ↔ S, A ↔ D 는 반대라 하나를 켜면 반대쪽은 꺼짐 · 빈 모서리 칸 = 대각선 · 가운데 ■ = 전부 끄기
 *  · 점프: 동그라미 톡 = 한 번 뜀 · 오른쪽 아래 작은 [홀드] = 켜 둔 동안 점프 버튼을 계속 누르고 있음
 */
final class RecPad {
    final android.os.Handler main = new android.os.Handler(android.os.Looper.getMainLooper());
    static double[] dir(boolean w, boolean a, boolean s, boolean d) {
        double x = (d ? 1 : 0) - (a ? 1 : 0), y = (s ? 1 : 0) - (w ? 1 : 0);
        double length = Math.hypot(x, y);
        return length == 0 ? null : new double[] {x / length, y / length};
    }
    volatile boolean w, a, s, d, hold;
    /** [점프 W] 를 누른 뒤 이 시각까지는 앞 (W) 으로만 감 (켜 둔 키는 그 뒤에 다시) */
    volatile long dashUntil;
    static final long DASH_MS = 500;
    final ConcurrentLinkedQueue<Integer> jumps = new ConcurrentLinkedQueue<>();
    View keyView, jumpView;
    boolean keyDown, jumpDown;

    /**
     * 사람 손가락이 패드에 닿음 · 떨어짐 → Joy 에 알림 (닿아 있는 동안은 안드로이드가 Acrux 터치를 끊어서, 뗀 뒤에 한 번에 다시 누름)
     * 닿은 시각 = 터치가 실제로 들어온 시각 (그때 로블록스 쪽 손가락이 떨어짐)
     */
    void finger(boolean key, boolean down, MotionEvent e) {
        if (key) keyDown = down;
        else jumpDown = down;
        if (down) Joy.userDownAt = System.nanoTime() - Math.max(0, android.os.SystemClock.uptimeMillis() - e.getEventTime()) * 1_000_000L;
        else if (!keyDown && !jumpDown) Joy.userDownAt = 0;
    }

    public double[] stick() {
        if (System.currentTimeMillis() < dashUntil) return dir(true, false, false, false);
        return dir(w, a, s, d);
    }

    public int pollJump() {
        Integer e = jumps.poll();
        return e == null ? 0 : e;
    }

    public void show() {
        main.post(() -> {
            TapService t = TapService.Companion.getCurrent();
            if (t == null) return;
            int[] sc = t.screen();
            int W = sc[0], H = sc[1];
            int b = (int) Math.max(H * 0.10, 56 * t.getResources().getDisplayMetrics().density);
            keyView = new KeyView(t, b);
            int jr = b * 3 / 4;
            jumpView = new JumpView(t, b, jr);
            add(t, keyView, (int) (W * 0.32) - b * 3 / 2, (int) (H * 0.62) - b * 3 / 2, b * 3, b * 3);
            int js = jumpSize(b, jr);
            int top = jumpTop(b, jr);
            add(t, jumpView, (int) (W * 0.66) - jr, (int) (H * 0.62) - top, js, top + js - jr);
        });
    }

    public void hide() {
        main.post(() -> {
            TapService t = TapService.Companion.getCurrent();
            if (t == null) return;
            if (keyView != null) t.removePad(keyView);
            if (jumpView != null) t.removePad(jumpView);
            keyView = jumpView = null;
        });
        keyDown = jumpDown = false;
        Joy.userDownAt = 0;
        w = a = s = d = false;
        if (hold) {
            hold = false;
            jumps.add(-1);
        }
    }

    static void add(TapService t, View view, int x, int y, int w, int h) {
        WindowManager.LayoutParams p = new WindowManager.LayoutParams(w, h,
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS
                        | WindowManager.LayoutParams.FLAG_SPLIT_TOUCH,
                PixelFormat.TRANSLUCENT);
        p.gravity = Gravity.TOP | Gravity.START;
        p.x = x;
        p.y = y;
        try {
            t.addPad(view, p);
        } catch (RuntimeException e) {
            Native.platformLog("기록용 패드를 못 띄움: " + e, 'r');
        }
    }

    /** 점프 동그라미 가운데의 창 안 높이 (위쪽 [점프 W] 동그라미가 들어갈 자리) */
    static int jumpTop(int b, int jr) {
        float R = jr, h = jr * 0.6f;
        return (int) Math.ceil(Math.max(R, (R + h + b * 0.08f) * 0.7071f + h)) + 2;
    }

    /** 점프 창 너비 (큰 동그라미 + 오른쪽 위 · 아래 작은 동그라미가 다 들어가게) */
    static int jumpSize(int b, int jr) {
        float R = jr, h = jr * 0.6f;
        return (int) Math.ceil(R + (R + h + b * 0.08f) * 0.7071f + h) + 2;
    }

    static Paint text(int b) {
        Paint p = new Paint(Paint.ANTI_ALIAS_FLAG);
        p.setColor(0xFFFFFFFF);
        p.setTextAlign(Paint.Align.CENTER);
        p.setTextSize(Math.max(22, b * 0.38f));
        p.setFakeBoldText(true);
        p.setShadowLayer(4, 0, 0, 0xFF000000);
        return p;
    }

    /** 네모 칸 (켜짐 = 초록) */
    static void cell(Canvas c, Paint fill, Paint line, Paint label, float x, float y, float b, boolean on, String s) {
        float m = b * 0.06f;
        fill.setColor(on ? 0xDD3BA55D : 0x88202225);
        c.drawRect(x + m, y + m, x + b - m, y + b - m, fill);
        c.drawRect(x + m, y + m, x + b - m, y + b - m, line);
        c.drawText(s, x + b / 2, y + b / 2 + label.getTextSize() * 0.35f, label);
    }

    /** W A S D (십자) · 톡 = 켜기 / 끄기 · 빈 모서리 칸 = 대각선 두 키 같이 · 가운데 ■ = 전부 끄기 */
    final class KeyView extends View {
        final int b;
        final Paint fill = new Paint(), line = new Paint(), label;

        KeyView(Context c, int b) {
            super(c);
            this.b = b;
            line.setStyle(Paint.Style.STROKE);
            line.setStrokeWidth(Math.max(2, b / 24f));
            line.setColor(0xCCFFFFFF);
            label = text(b);
        }

        @Override
        protected void onDraw(Canvas c) {
            cell(c, fill, line, label, b, 0, b, w, "W");
            cell(c, fill, line, label, 0, b, b, a, "A");
            cell(c, fill, line, label, b, b, b, false, "■");
            cell(c, fill, line, label, b * 2, b, b, d, "D");
            cell(c, fill, line, label, b, b * 2, b, s, "S");
        }

        @Override
        public boolean onTouchEvent(MotionEvent e) {
            int act = e.getActionMasked();
            if (act == MotionEvent.ACTION_UP || act == MotionEvent.ACTION_CANCEL) finger(true, false, e);
            if (act != MotionEvent.ACTION_DOWN && act != MotionEvent.ACTION_POINTER_DOWN) return true;
            finger(true, true, e);
            int i = e.getActionIndex();
            int col = (int) Math.floor(e.getX(i) / b), row = (int) Math.floor(e.getY(i) / b);
            if (col < 0 || col > 2 || row < 0 || row > 2) return true;
            if (row == 1 && col == 1) {
                w = a = s = d = false;
            } else {
                boolean kw = row == 0, ks = row == 2, ka = col == 0, kd = col == 2;
                boolean on = !((!kw || w) && (!ks || s) && (!ka || a) && (!kd || d));   // 누른 키가 다 켜져 있으면 끔 · 아니면 켬
                if (kw) { w = on; if (on) s = false; }
                if (ks) { s = on; if (on) w = false; }
                if (ka) { a = on; if (on) d = false; }
                if (kd) { d = on; if (on) a = false; }
            }
            invalidate();
            return true;
        }
    }

    /** 점프 (큰 동그라미 · 톡 = 한 번 뜀) + 오른쪽 아래 작은 동그라미 [홀드] (톡 = 점프 버튼을 계속 누르고 있기 / 떼기) */
    /** 점프 (큰 동그라미 · 톡 = 한 번 뜀) + 오른쪽 위 [점프 W] (뛰면서 0.5초 앞으로) + 오른쪽 아래 [홀드] (톡 = 계속 누르고 있기 / 떼기) */
    final class JumpView extends View {
        final int b;
        final float R, h, hc, T;       // 점프 반지름 · 작은 동그라미 반지름 · 작은 동그라미까지 (대각선) · 점프 가운데 높이
        final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG), line = new Paint(Paint.ANTI_ALIAS_FLAG), label, small;
        boolean flash, dash, dashArmed;

        JumpView(Context c, int b, int jr) {
            super(c);
            this.b = b;
            R = jr;
            h = jr * 0.6f;
            hc = (R + h + b * 0.08f) * 0.7071f;
            T = jumpTop(b, jr);
            line.setStyle(Paint.Style.STROKE);
            line.setStrokeWidth(Math.max(2, b / 24f));
            line.setColor(0xCCFFFFFF);
            label = text(b);
            label.setTextSize(Math.max(20, b * 0.26f));
            small = text(b);
            small.setTextSize(Math.max(16, b * 0.18f));
        }

        void circle(Canvas c, float x, float y, float r, boolean on, String s, Paint p, int off) {
            float m = b * 0.04f;
            fill.setColor(on ? 0xEE3BA55D : off);
            c.drawCircle(x, y, r - m, fill);
            c.drawCircle(x, y, r - m, line);
            c.drawText(LogService.tr(getContext(), s), x, y + p.getTextSize() * 0.35f, p);
        }

        @Override
        protected void onDraw(Canvas c) {
            circle(c, R, T, R, flash || hold || dash, "점프", label, 0xEE202225);
            circle(c, R + hc, T - hc, h, dash, "점프 W", small, 0xCC202225);
            circle(c, R + hc, T + hc, h, hold, "홀드", small, 0xCC202225);
        }

        /** 톡 점프 (0.2초 뒤 뗌) */
        void tapJump() {
            if (hold || flash) return;                       // 이미 누르고 있음
            jumps.add(1);
            flash = true;
            postDelayed(() -> {                              // 0.2초 뒤 뗌 (로블록스가 누름을 볼 시간)
                if (!hold) jumps.add(-1);
                flash = false;
                invalidate();
            }, 200);
        }

        @Override
        public boolean onTouchEvent(MotionEvent e) {
            int act = e.getActionMasked();
            if (act == MotionEvent.ACTION_UP || act == MotionEvent.ACTION_CANCEL) {
                finger(false, false, e);
                if (dashArmed) {                             // 손가락을 뗀 때부터 0.5초 (닿아 있는 동안은 Acrux 가 못 누름)
                    dashArmed = false;
                    dashUntil = System.currentTimeMillis() + DASH_MS;
                    postDelayed(() -> {
                        dash = false;
                        invalidate();
                    }, DASH_MS);
                }
                return true;
            }
            if (act != MotionEvent.ACTION_DOWN) return true;
            float ex = e.getX(), ey = e.getY();
            if (Math.hypot(ex - R, ey - T) <= R) {
                tapJump();
            } else if (Math.hypot(ex - R - hc, ey - T + hc) <= h * 1.15) {
                // 점프 W: 뛰면서 0.5초 앞으로 (W 를 누른 채 점프보다 안정적)
                tapJump();
                dashUntil = System.currentTimeMillis() + DASH_MS + 1000;
                dash = true;
                dashArmed = true;
            } else if (Math.hypot(ex - R - hc, ey - T - hc) <= h * 1.15) {
                hold = !hold;
                if (!flash) jumps.add(hold ? 1 : -1);        // 톡 점프 중에 켜면 그대로 이어서 누르고 있음
                else if (!hold) jumps.add(-1);
            } else {
                return false;                                // 빈 곳은 그냥 통과
            }
            finger(false, true, e);
            invalidate();
            return true;
        }
    }
}
