package com.acrux.mobile;

import android.animation.ValueAnimator;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Path;
import android.view.HapticFeedbackConstants;
import android.view.MotionEvent;
import android.view.View;
import android.view.animation.DecelerateInterpolator;

/**
 * 밀어서 켜기 · 끄기 — 오른쪽 손잡이를 왼쪽 끝까지 끌었다 놓으면 켜짐 ↔ 꺼짐 (톡 누르는 것으로는 안 바뀜 · 실수로 안 바뀌게)
 * 칸 안 글자: "이름 · 켜짐 / 꺼짐" + 왼쪽 끝에 밀 방향 (‹) · 끄는 동안 글자는 흐려지고 칸이 차오름 · 놓으면 손잡이는 오른쪽으로 돌아감
 * 디버깅 모드 (앱 맨 위 · 설정 · 떠 있는 Acrux 창) 에서 씀
 */
final class Slide extends View {
    interface On {
        void flip(boolean now);
    }

    static final float FLIP_AT = 0.85f;                 // 이만큼 (손잡이가 갈 수 있는 거리의 비율) 넘게 밀고 놓으면 바뀜

    final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG), text = new Paint(Paint.ANTI_ALIAS_FLAG);
    final Path arrow = new Path();
    final float u;
    final String label;
    final int onColor, offColor, trackColor, fgColor, dimColor;
    final On listener;
    boolean on;
    /** 민 정도 0 (손잡이가 오른쪽 끝) ~ 1 (왼쪽 끝) */
    float pos;
    float downX, downPos;
    boolean dragging, armed;
    ValueAnimator back;

    /** label: 한국어 이름 (그릴 때 지금 언어로) · sp: 글자 크기 · 색: 켜짐 · 꺼짐 손잡이 · 칸 · 글자 · 흐린 글자 */
    Slide(Context c, String label, float sp, int onColor, int offColor, int trackColor, int fgColor, int dimColor, On listener) {
        super(c);
        u = c.getResources().getDisplayMetrics().density;
        this.label = label;
        this.onColor = onColor;
        this.offColor = offColor;
        this.trackColor = trackColor;
        this.fgColor = fgColor;
        this.dimColor = dimColor;
        this.listener = listener;
        text.setTextSize(sp * c.getResources().getDisplayMetrics().scaledDensity);
        text.setTypeface(android.graphics.Typeface.DEFAULT_BOLD);
        setClickable(true);
        setContentDescription(Ui.tr(label));
    }

    /** 화면 갱신으로 맞춤 (끄는 중이면 놓을 때 맞춤) */
    void set(boolean v) {
        if (v == on) return;
        on = v;
        invalidate();
    }

    float knob() {
        return getHeight() - 4 * u;                     // 정사각형 손잡이 한 변
    }

    float travel() {
        return Math.max(1, getWidth() - knob() - 4 * u);
    }

    @Override
    protected void onDraw(Canvas c) {
        float w = getWidth(), h = getHeight(), i = 2 * u, k = knob(), x = w - i - k - pos * travel();
        int accent = on ? onColor : offColor;
        // 칸 · 밀어 온 만큼 차오름 (다음 상태 색 · 손잡이부터 오른쪽 끝까지)
        fill.setColor(trackColor);
        c.drawRect(0, 0, w, h, fill);
        int next = on ? offColor : onColor;
        if (pos > 0) {
            fill.setColor((next & 0x00FFFFFF) | ((int) (0x30 + 0x50 * pos) << 24));
            c.drawRect(x - i, 0, w, h, fill);
        }
        // 테두리 (켜져 있으면 켜짐 색)
        fill.setStyle(Paint.Style.STROKE);
        fill.setStrokeWidth(Math.max(1, u));
        fill.setColor(on ? onColor : (dimColor & 0x00FFFFFF) | 0x66000000);
        c.drawRect(u / 2, u / 2, w - u / 2, h - u / 2, fill);
        fill.setStyle(Paint.Style.FILL);
        // 글자: 이름 · 상태 (가운데 · 손잡이 왼쪽) — 밀수록 흐려짐
        String s = Ui.tr(label) + " · " + Ui.tr(on ? "켜짐" : "꺼짐");
        float left = h * 0.6f, right = w - i - k - 6 * u;
        float tw = text.measureText(s);
        int a = (int) (255 * Math.max(0, 1 - pos * 1.6f));
        text.setColor((on ? onColor : fgColor) & 0x00FFFFFF | (a << 24));
        float ty = h / 2 - (text.descent() + text.ascent()) / 2;
        c.drawText(s, Math.max(left, left + (right - left - tw) / 2), ty, text);
        // 왼쪽 끝: 미는 방향 ‹ (흐린 색)
        float ax = h * 0.42f, ah = h * 0.18f;
        arrow.reset();
        arrow.moveTo(ax + ah * 0.6f, h / 2 - ah);
        arrow.lineTo(ax - ah * 0.4f, h / 2);
        arrow.lineTo(ax + ah * 0.6f, h / 2 + ah);
        text.setStyle(Paint.Style.STROKE);
        text.setStrokeWidth(Math.max(1.2f, 1.4f * u));
        text.setColor(dimColor & 0x00FFFFFF | ((int) (0xAA * Math.max(0, 1 - pos)) << 24));
        c.drawPath(arrow, text);
        text.setStyle(Paint.Style.FILL);
        // 손잡이 (지금 상태 색 · 끝까지 밀면 다음 상태 색)
        fill.setColor(armed ? next : accent);
        c.drawRect(x, i, x + k, i + k, fill);
        // 손잡이 안 ‹ 두 개
        float cx = x + k / 2, ch = k * 0.16f;
        text.setStyle(Paint.Style.STROKE);
        text.setStrokeWidth(Math.max(1.2f, 1.3f * u));
        text.setColor(0xFF111214);                      // 손잡이는 밝은 색이라 어두운 ‹
        for (float dx : new float[]{ch * 0.9f, -ch * 0.5f}) {
            arrow.reset();
            arrow.moveTo(cx + dx + ch * 0.5f, k / 2 + i - ch);
            arrow.lineTo(cx + dx - ch * 0.5f, k / 2 + i);
            arrow.lineTo(cx + dx + ch * 0.5f, k / 2 + i + ch);
            c.drawPath(arrow, text);
        }
        text.setStyle(Paint.Style.FILL);
    }

    @Override
    public boolean onTouchEvent(MotionEvent e) {
        switch (e.getActionMasked()) {
            case MotionEvent.ACTION_DOWN:
                if (back != null) back.cancel();
                downX = e.getX();
                downPos = pos;
                dragging = true;
                getParent().requestDisallowInterceptTouchEvent(true);   // 칸 넘기기 · 스크롤이 가져가지 않게
                return true;
            case MotionEvent.ACTION_MOVE:
                if (!dragging) return false;
                pos = Math.max(0, Math.min(1, downPos + (downX - e.getX()) / travel()));   // 왼쪽으로 밀수록 +
                boolean a = pos >= FLIP_AT;
                if (a != armed) {
                    armed = a;
                    if (a) performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY);
                }
                invalidate();
                return true;
            case MotionEvent.ACTION_UP:
            case MotionEvent.ACTION_CANCEL:
                if (!dragging) return false;
                dragging = false;
                getParent().requestDisallowInterceptTouchEvent(false);
                if (e.getActionMasked() == MotionEvent.ACTION_UP && armed) {
                    on = !on;
                    performHapticFeedback(android.os.Build.VERSION.SDK_INT >= 30 ? HapticFeedbackConstants.CONFIRM : HapticFeedbackConstants.LONG_PRESS);
                    if (listener != null) listener.flip(on);
                }
                armed = false;
                settle();
                return true;
            default:
                return super.onTouchEvent(e);
        }
    }

    /** 손잡이를 오른쪽으로 되돌림 */
    void settle() {
        back = ValueAnimator.ofFloat(pos, 0);
        back.setDuration((long) (120 + 140 * pos));
        back.setInterpolator(new DecelerateInterpolator());
        back.addUpdateListener(an -> {
            pos = (float) an.getAnimatedValue();
            invalidate();
        });
        back.start();
    }
}
