package com.acrux.mobile;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.PixelFormat;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowManager;

import com.acrux.mobile.core.Mover;
import com.acrux.mobile.core.Stick;

import java.util.concurrent.ConcurrentLinkedQueue;

/**
 * 길 기록용 WASD 패드 · 점프 버튼 — 키를 톡 누르면 켜지고 다시 누르면 꺼짐 (켜 둔 동안 Acrux 가 로블록스 조이스틱을 그 방향으로 계속 밂)
 *  · 끌기가 아니라 톡 누르기인 이유: 손가락이 화면에 닿는 순간 안드로이드가 Acrux 가 누르던 터치를 끊음 → 닿는 순간만 끊기고 떼면 바로 다시 밂
 *  · W ↔ S, A ↔ D 는 반대라 하나를 켜면 반대쪽은 꺼짐 · W + A 같은 대각선은 둘 다 켜면 됨 · 가운데 ■ = 전부 끄기 (멈춤)
 *  · 점프: 톡 = 한 번 뜀 · 꾹 = 켜 두면 계속 뜀 (점프 버튼을 누르고 있음) · 다시 누르면 뗌
 */
final class RecPad implements Mover.Pad {
    volatile boolean w, a, s, d, hold;
    final ConcurrentLinkedQueue<Integer> jumps = new ConcurrentLinkedQueue<>();
    View keyView, jumpView;

    @Override
    public double[] stick() {
        return Stick.dir(w, a, s, d);
    }

    @Override
    public int pollJump() {
        Integer e = jumps.poll();
        return e == null ? 0 : e;
    }

    @Override
    public void show() {
        Hub.main.post(() -> {
            TapService t = TapService.instance;
            if (t == null) return;
            int[] sc = t.screen();
            int W = sc[0], H = sc[1];
            int b = (int) Math.max(H * 0.10, 56 * t.getResources().getDisplayMetrics().density);
            keyView = new KeyView(t, b);
            jumpView = new JumpView(t, b);
            add(t, keyView, (int) (W * 0.32) - b * 3 / 2, (int) (H * 0.62) - b * 3 / 2, b * 3, b * 3);
            add(t, jumpView, (int) (W * 0.66) - b, (int) (H * 0.62) - b / 2, b * 2, b);
        });
    }

    @Override
    public void hide() {
        Hub.main.post(() -> {
            TapService t = TapService.instance;
            if (t == null) return;
            if (keyView != null) t.removeView(keyView);
            if (jumpView != null) t.removeView(jumpView);
            keyView = jumpView = null;
        });
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
            t.wm.addView(view, p);
        } catch (RuntimeException e) {
            Hub.log("기록용 패드를 못 띄움: " + e, 'r');
        }
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

    /** W A S D (십자) · 가운데 ■ */
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
            if (e.getActionMasked() != MotionEvent.ACTION_DOWN) return true;
            int col = (int) (e.getX() / b), row = (int) (e.getY() / b);
            if (row == 0 && col == 1) {
                w = !w;
                if (w) s = false;
            } else if (row == 2 && col == 1) {
                s = !s;
                if (s) w = false;
            } else if (row == 1 && col == 0) {
                a = !a;
                if (a) d = false;
            } else if (row == 1 && col == 2) {
                d = !d;
                if (d) a = false;
            } else if (row == 1 && col == 1) {
                w = a = s = d = false;
            } else {
                return true;
            }
            invalidate();
            return true;
        }
    }

    /** 점프 (톡 = 한 번) · 꾹 (켜 두면 계속 누르고 있음) */
    final class JumpView extends View {
        final int b;
        final Paint fill = new Paint(), line = new Paint(), label;
        boolean flash;

        JumpView(Context c, int b) {
            super(c);
            this.b = b;
            line.setStyle(Paint.Style.STROKE);
            line.setStrokeWidth(Math.max(2, b / 24f));
            line.setColor(0xCCFFFFFF);
            label = text(b);
            label.setTextSize(Math.max(20, b * 0.26f));
        }

        @Override
        protected void onDraw(Canvas c) {
            cell(c, fill, line, label, 0, 0, b, flash, Ui.tr("점프"));
            cell(c, fill, line, label, b, 0, b, hold, Ui.tr("꾹"));
        }

        @Override
        public boolean onTouchEvent(MotionEvent e) {
            if (e.getActionMasked() != MotionEvent.ACTION_DOWN) return true;
            if (e.getX() < b) {
                if (hold) return true;                       // 이미 누르고 있음
                jumps.add(1);
                flash = true;
                postDelayed(() -> {                          // 0.2초 뒤 뗌 (로블록스가 누름을 볼 시간)
                    if (!hold) jumps.add(-1);
                    flash = false;
                    invalidate();
                }, 200);
            } else {
                hold = !hold;
                jumps.add(hold ? 1 : -1);
            }
            invalidate();
            return true;
        }
    }
}
