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
 * 길 기록용 WASD 패드 · 점프 버튼 — 키를 톡 누르면 켜지고 (홀드) 다시 누르면 꺼짐 · 켜 둔 동안 Acrux 가 로블록스 조이스틱을 그 방향으로 계속 밂
 *  · 꾹 누르기가 아닌 이유: 손가락이 화면에서 움직일 때마다 안드로이드가 Acrux 가 누르던 터치를 끊음 → 톡 누르는 순간만 끊기고 바로 다시 밂
 *  · W ↔ S, A ↔ D 는 반대라 하나를 켜면 반대쪽은 꺼짐 · 빈 모서리 칸 = 대각선 · 가운데 ■ = 전부 끄기
 *  · 점프: 동그라미 톡 = 한 번 뜀 · 오른쪽으로 이어진 [홀드] = 켜 둔 동안 점프 버튼을 계속 누르고 있음
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
            add(t, jumpView, (int) (W * 0.66) - jr, (int) (H * 0.62) - jr, jr * 2 + (int) (b * 1.3), jr * 2);
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
            if (act != MotionEvent.ACTION_DOWN && act != MotionEvent.ACTION_POINTER_DOWN) return true;
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

    /** 점프 (동그라미 · 톡 = 한 번 뜀) + 오른쪽으로 이어진 [홀드] (톡 = 점프 버튼을 계속 누르고 있기 / 떼기) */
    final class JumpView extends View {
        final int b;
        final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG), line = new Paint(Paint.ANTI_ALIAS_FLAG), label, small;
        boolean flash;

        JumpView(Context c, int b) {
            super(c);
            this.b = b;
            line.setStyle(Paint.Style.STROKE);
            line.setStrokeWidth(Math.max(2, b / 24f));
            line.setColor(0xCCFFFFFF);
            label = text(b);
            label.setTextSize(Math.max(20, b * 0.26f));
            small = text(b);
            small.setTextSize(Math.max(18, b * 0.22f));
        }

        @Override
        protected void onDraw(Canvas c) {
            float h = getHeight(), r = h / 2f, m = b * 0.06f, bh = h * 0.30f;
            // 오른쪽 홀드 막대 (동그라미 가운데부터 이어짐)
            fill.setColor(hold ? 0xDD3BA55D : 0x88202225);
            c.drawRect(r, r - bh, getWidth() - m, r + bh, fill);
            c.drawRect(r, r - bh, getWidth() - m, r + bh, line);
            c.drawText(Ui.tr("홀드"), (r * 2 + getWidth() - m) / 2f, r + small.getTextSize() * 0.35f, small);
            // 점프 동그라미
            fill.setColor(flash || hold ? 0xFF3BA55D : 0xEE202225);
            c.drawCircle(r, r, r - m, fill);
            c.drawCircle(r, r, r - m, line);
            c.drawText(Ui.tr("점프"), r, r + label.getTextSize() * 0.35f, label);
        }

        @Override
        public boolean onTouchEvent(MotionEvent e) {
            if (e.getActionMasked() != MotionEvent.ACTION_DOWN) return true;
            float r = getHeight() / 2f;
            if (e.getX() < r * 2) {
                if (hold || flash) return true;              // 이미 누르고 있음
                jumps.add(1);
                flash = true;
                postDelayed(() -> {                          // 0.2초 뒤 뗌 (로블록스가 누름을 볼 시간)
                    if (!hold) jumps.add(-1);
                    flash = false;
                    invalidate();
                }, 200);
            } else {
                hold = !hold;
                if (!flash) jumps.add(hold ? 1 : -1);        // 톡 점프 중에 켜면 그대로 이어서 누르고 있음
                else if (!hold) jumps.add(-1);
            }
            invalidate();
            return true;
        }
    }
}
