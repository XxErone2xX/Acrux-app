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

import java.util.concurrent.ConcurrentLinkedQueue;

/**
 * 길 기록용 Acrux 조이스틱 · 점프 버튼 — 손가락 좌표를 그대로 받음 (화면 감지 없음 · 오차 없음)
 *  · 로블록스 조이스틱 · 점프 버튼과 겹치지 않는 자리에 띄움 (그 자리로는 Acrux 가 로블록스에 터치를 대신 보냄)
 *  · 조이스틱: 판 안 아무 데나 누르면 거기가 가운데 → 끄는 방향 · 거리 (판 반지름 = 끝까지)
 *  · 두 창이라 조이스틱 · 점프를 동시에 누를 수 있음
 */
final class RecPad implements Mover.Pad {
    volatile double[] v;
    final ConcurrentLinkedQueue<Integer> jumps = new ConcurrentLinkedQueue<>();
    View stickView, jumpView;

    @Override
    public double[] stick() {
        return v;
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
            int[] s = t.screen();
            int W = s[0], H = s[1];
            double[] st = Hub.stick;
            int r = (int) Math.max(H * 0.11, st != null ? st[2] * H * 1.3 : 0);
            stickView = new StickPadView(t, r);
            jumpView = new JumpPadView(t, (int) (r * 0.75));
            add(t, stickView, (int) (W * 0.30) - r * 2, (int) (H * 0.70) - r * 2, r * 4, r * 4);
            int jr = (int) (r * 0.75);
            add(t, jumpView, (int) (W * 0.70) - jr, (int) (H * 0.70) - jr, jr * 2, jr * 2);
        });
    }

    @Override
    public void hide() {
        Hub.main.post(() -> {
            TapService t = TapService.instance;
            if (t == null) return;
            if (stickView != null) t.removeView(stickView);
            if (jumpView != null) t.removeView(jumpView);
            stickView = jumpView = null;
        });
        v = null;
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
            Hub.log("기록용 조이스틱을 못 띄움: " + e, 'r');
        }
    }

    /** 기록용 조이스틱: 누른 자리가 가운데 · 끄는 만큼 (반지름 r = 끝까지) */
    final class StickPadView extends View {
        final int r;
        final Paint ring = new Paint(Paint.ANTI_ALIAS_FLAG), knob = new Paint(Paint.ANTI_ALIAS_FLAG), label = new Paint(Paint.ANTI_ALIAS_FLAG);
        float ox = -1, oy, kx, ky;

        StickPadView(Context c, int r) {
            super(c);
            this.r = r;
            ring.setStyle(Paint.Style.STROKE);
            ring.setStrokeWidth(Math.max(3, r / 18f));
            ring.setColor(0xCC3BA55D);
            knob.setColor(0x993BA55D);
            label.setColor(0xFFFFFFFF);
            label.setTextAlign(Paint.Align.CENTER);
            label.setTextSize(Math.max(22, r / 4f));
            label.setShadowLayer(4, 0, 0, 0xFF000000);
        }

        @Override
        protected void onDraw(Canvas c) {
            float cx = getWidth() / 2f, cy = getHeight() / 2f;
            if (ox < 0) {
                ring.setAlpha(150);
                c.drawCircle(cx, cy, r, ring);
                c.drawCircle(cx, cy, r * 0.45f, knob);
                c.drawText(Ui.tr("이동"), cx, cy + label.getTextSize() * 0.35f, label);
            } else {
                ring.setAlpha(220);
                c.drawCircle(ox, oy, r, ring);
                c.drawCircle(kx, ky, r * 0.45f, knob);
            }
        }

        @Override
        public boolean onTouchEvent(MotionEvent e) {
            switch (e.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                    ox = e.getX();
                    oy = e.getY();
                    kx = ox;
                    ky = oy;
                    v = new double[]{0, 0};
                    break;
                case MotionEvent.ACTION_MOVE: {
                    double dx = e.getX() - ox, dy = e.getY() - oy, d = Math.hypot(dx, dy);
                    double m = Math.min(1, d / r);
                    if (d > 0) {
                        kx = (float) (ox + dx / d * m * r);
                        ky = (float) (oy + dy / d * m * r);
                        v = new double[]{dx / d * m, dy / d * m};
                    }
                    break;
                }
                case MotionEvent.ACTION_UP:
                case MotionEvent.ACTION_CANCEL:
                    ox = -1;
                    v = null;
                    break;
                default:
                    return true;
            }
            invalidate();
            return true;
        }
    }

    /** 기록용 점프 버튼: 누르는 동안 눌림 */
    final class JumpPadView extends View {
        final int r;
        final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG), label = new Paint(Paint.ANTI_ALIAS_FLAG);
        boolean down;

        JumpPadView(Context c, int r) {
            super(c);
            this.r = r;
            label.setColor(0xFFFFFFFF);
            label.setTextAlign(Paint.Align.CENTER);
            label.setTextSize(Math.max(22, r / 3f));
            label.setShadowLayer(4, 0, 0, 0xFF000000);
        }

        @Override
        protected void onDraw(Canvas c) {
            float cx = getWidth() / 2f, cy = getHeight() / 2f;
            fill.setColor(down ? 0xCCFAA61A : 0x88FAA61A);
            c.drawCircle(cx, cy, r * 0.95f, fill);
            c.drawText(Ui.tr("점프"), cx, cy + label.getTextSize() * 0.35f, label);
        }

        @Override
        public boolean onTouchEvent(MotionEvent e) {
            int a = e.getActionMasked();
            if (a == MotionEvent.ACTION_DOWN && !down) {
                down = true;
                jumps.add(1);
            } else if ((a == MotionEvent.ACTION_UP || a == MotionEvent.ACTION_CANCEL) && down) {
                down = false;
                jumps.add(-1);
            } else {
                return true;
            }
            invalidate();
            return true;
        }
    }
}
