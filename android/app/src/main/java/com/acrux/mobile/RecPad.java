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
 * 길 기록용 WASD 패드 · 점프 버튼 — 누르고 있는 동안 Acrux 가 로블록스 조이스틱을 그 방향으로 밂 · 점프 버튼을 누름
 *  · 대각선: 손가락 두 개로 W + A 를 같이 누르거나 키 사이 빈 모서리 칸을 누름
 *  · 안드로이드는 손가락이 화면에서 움직일 때마다 Acrux 가 누르던 터치를 끊음 → Joy 가 바로 다시 누름 (끄는 대신 가만히 누르는 거라 끊김이 적음)
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
            int jr = b * 3 / 4;
            add(t, jumpView, (int) (W * 0.66) - jr, (int) (H * 0.62) - jr, jr * 2, jr * 2);
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

    /** W A S D (십자) · 누르고 있는 동안만 · 손가락 두 개로 W + A 같이 누르기 · 빈 모서리 칸 = 대각선 */
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
            cell(c, fill, line, label, b * 2, b, b, d, "D");
            cell(c, fill, line, label, b, b * 2, b, s, "S");
        }

        @Override
        public boolean onTouchEvent(MotionEvent e) {
            int act = e.getActionMasked();
            int gone = act == MotionEvent.ACTION_UP || act == MotionEvent.ACTION_CANCEL ? -2
                    : act == MotionEvent.ACTION_POINTER_UP ? e.getActionIndex() : -1;
            boolean nw = false, na = false, ns = false, nd = false;
            if (gone != -2) {
                for (int i = 0; i < e.getPointerCount(); i++) {
                    if (i == gone) continue;
                    int col = (int) Math.floor(e.getX(i) / b), row = (int) Math.floor(e.getY(i) / b);
                    if (col < 0 || col > 2 || row < 0 || row > 2) continue;
                    if (row == 0) nw = true;
                    if (row == 2) ns = true;
                    if (col == 0) na = true;
                    if (col == 2) nd = true;
                }
            }
            if (nw && ns) nw = ns = false;
            if (na && nd) na = nd = false;
            if (nw != w || na != a || ns != s || nd != d) {
                w = nw;
                a = na;
                s = ns;
                d = nd;
                invalidate();
            }
            return true;
        }
    }

    /** 점프 (동그라미) · 누르고 있는 동안 점프 버튼을 누르고 있음 */
    final class JumpView extends View {
        final int b;
        final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG), line = new Paint(Paint.ANTI_ALIAS_FLAG), label;

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
            float r = getWidth() / 2f, m = b * 0.06f;
            fill.setColor(hold ? 0xDD3BA55D : 0x88202225);
            c.drawCircle(r, r, r - m, fill);
            c.drawCircle(r, r, r - m, line);
            c.drawText(Ui.tr("점프"), r, r + label.getTextSize() * 0.35f, label);
        }

        @Override
        public boolean onTouchEvent(MotionEvent e) {
            int act = e.getActionMasked();
            if (act == MotionEvent.ACTION_DOWN && !hold) {
                hold = true;
                jumps.add(1);
            } else if ((act == MotionEvent.ACTION_UP || act == MotionEvent.ACTION_CANCEL) && hold) {
                hold = false;
                jumps.add(-1);
            } else {
                return true;
            }
            invalidate();
            return true;
        }
    }
}
