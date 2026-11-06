package com.acrux.mobile;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.ColorFilter;
import android.graphics.Paint;
import android.graphics.PixelFormat;
import android.graphics.Typeface;
import android.graphics.drawable.Drawable;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.TextView;

/** PC 버전과 같은 모양: 어두운 배경 · 각진 칸 · 네 모서리 ㄱ 장식 */
final class Ui {
    static final int BG = 0xFF1E1F22, PANEL = 0xFF2B2D31, FIELD = 0xFF383A40, LINE = 0xFF3F4147, FG = 0xFFDBDEE1,
            MUTED = 0xFF949BA4, ACCENT = 0xFF5865F2, GREEN = 0xFF23A55A, YELLOW = 0xFFF0B232, RED = 0xFFF23F43, WHITE = 0xFFFFFFFF;

    private Ui() {}

    static int dp(Context c, float v) {
        return Math.round(TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, v, c.getResources().getDisplayMetrics()));
    }

    /** 칸 배경: 채움 + (선) + 네 모서리 ㄱ */
    static final class Deco extends Drawable {
        final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG), line = new Paint(Paint.ANTI_ALIAS_FLAG), corner = new Paint(Paint.ANTI_ALIAS_FLAG);
        final float arm, w;
        final boolean border;

        Deco(Context c, int fillColor, int lineColor, int cornerColor, boolean border) {
            fill.setColor(fillColor);
            line.setColor(lineColor);
            line.setStyle(Paint.Style.STROKE);
            line.setStrokeWidth(dp(c, 1));
            corner.setColor(cornerColor);
            corner.setStyle(Paint.Style.FILL);
            arm = dp(c, 10);
            w = dp(c, 2);
            this.border = border;
        }

        @Override
        public void draw(Canvas cv) {
            float l = getBounds().left, t = getBounds().top, r = getBounds().right, b = getBounds().bottom;
            cv.drawRect(l, t, r, b, fill);
            if (border) cv.drawRect(l + 0.5f, t + 0.5f, r - 0.5f, b - 0.5f, line);
            // ㄱ 장식 (가로 · 세로 팔)
            cv.drawRect(l, t, l + arm, t + w, corner);
            cv.drawRect(l, t, l + w, t + arm, corner);
            cv.drawRect(r - arm, t, r, t + w, corner);
            cv.drawRect(r - w, t, r, t + arm, corner);
            cv.drawRect(l, b - w, l + arm, b, corner);
            cv.drawRect(l, b - arm, l + w, b, corner);
            cv.drawRect(r - arm, b - w, r, b, corner);
            cv.drawRect(r - w, b - arm, r, b, corner);
        }

        @Override
        public void setAlpha(int a) {}

        @Override
        public void setColorFilter(ColorFilter cf) {}

        @Override
        public int getOpacity() {
            return PixelFormat.TRANSLUCENT;
        }
    }

    static TextView text(Context c, String s, float sp, int color, boolean bold) {
        TextView t = new TextView(c);
        t.setText(s);
        t.setTextSize(TypedValue.COMPLEX_UNIT_SP, sp);
        t.setTextColor(color);
        if (bold) t.setTypeface(Typeface.DEFAULT_BOLD);
        return t;
    }

    /** 각진 버튼 (테두리 색 = 글자 색) */
    static TextView button(Context c, String s, int color, View.OnClickListener on) {
        TextView b = text(c, s, 14, color, true);
        b.setGravity(Gravity.CENTER);
        b.setPadding(dp(c, 16), dp(c, 9), dp(c, 16), dp(c, 9));
        b.setBackground(new Deco(c, 0x00000000, color, color, true));
        b.setOnClickListener(on);
        b.setClickable(true);
        return b;
    }

    static LinearLayout card(Context c) {
        LinearLayout l = new LinearLayout(c);
        l.setOrientation(LinearLayout.VERTICAL);
        l.setPadding(dp(c, 16), dp(c, 14), dp(c, 16), dp(c, 14));
        l.setBackground(new Deco(c, PANEL, LINE, WHITE, false));
        return l;
    }

    static LinearLayout.LayoutParams wrap() {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT);
    }

    static LinearLayout.LayoutParams fill(Context c, int topDp) {
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        p.topMargin = dp(c, topDp);
        return p;
    }

    static LinearLayout.LayoutParams weight() {
        return new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f);
    }
}
