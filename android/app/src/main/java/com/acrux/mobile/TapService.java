package com.acrux.mobile;

import android.accessibilityservice.AccessibilityService;
import android.accessibilityservice.GestureDescription;
import android.content.Intent;
import android.graphics.Path;
import android.graphics.PixelFormat;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowManager;
import android.view.accessibility.AccessibilityEvent;
import android.widget.LinearLayout;
import android.widget.TextView;

/**
 * 접근성 서비스 — 자동 낚시의 터치를 대신 보냄 (dispatchGesture) + 로블록스 위에 떠 있는 작은 조작 창
 * 화면 내용은 읽지 않음 (canRetrieveWindowContent = false)
 */
public class TapService extends AccessibilityService {
    static volatile TapService instance;

    WindowManager wm;
    LinearLayout panel;
    WindowManager.LayoutParams lp;
    TextView status, calBtn, runBtn;
    final Runnable refresh = this::refresh;

    @Override
    protected void onServiceConnected() {
        super.onServiceConnected();
        Hub.init(this);
        instance = this;
        Hub.listeners.add(refresh);
        showPanel();
        Hub.log("접근성 권한 켜짐 · 터치 준비됨", 'g');
        Hub.changed();
    }

    @Override
    public void onAccessibilityEvent(AccessibilityEvent event) {}

    @Override
    public void onInterrupt() {}

    @Override
    public boolean onUnbind(Intent intent) {
        cleanup();
        return super.onUnbind(intent);
    }

    @Override
    public void onDestroy() {
        cleanup();
        super.onDestroy();
    }

    void cleanup() {
        if (instance == this) instance = null;
        Hub.listeners.remove(refresh);
        Hub.stop();
        if (panel != null && wm != null) {
            try {
                wm.removeView(panel);
            } catch (RuntimeException ignored) {
            }
        }
        panel = null;
        Hub.changed();
    }

    /** (x, y) 를 ms 동안 누름 (화면 좌표 · 지금 화면 방향 기준) */
    void tap(int x, int y, int ms) {
        if (x < 0 || y < 0) return;
        Path p = new Path();
        p.moveTo(x, y);
        GestureDescription g = new GestureDescription.Builder()
                .addStroke(new GestureDescription.StrokeDescription(p, 0, Math.max(1, ms)))
                .build();
        dispatchGesture(g, null, null);
    }

    // ---------------------------------------------------------------- 떠 있는 조작 창
    void showPanel() {
        if (panel != null) return;
        wm = (WindowManager) getSystemService(WINDOW_SERVICE);
        panel = new LinearLayout(this);
        panel.setOrientation(LinearLayout.VERTICAL);
        int pad = Ui.dp(this, 8);
        panel.setPadding(pad, pad, pad, pad);
        panel.setBackground(new Ui.Deco(this, 0xE62B2D31, Ui.LINE, Ui.WHITE, false));

        TextView title = Ui.text(this, "ACRUX  ✥", 12, Ui.FG, true);
        title.setPadding(0, 0, 0, Ui.dp(this, 2));
        status = Ui.text(this, "대기", 11, Ui.MUTED, false);
        status.setMaxWidth(Ui.dp(this, 150));
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        calBtn = small("보정", Ui.YELLOW, v -> toggle(true));
        runBtn = small("시작", Ui.ACCENT, v -> toggle(false));
        row.addView(calBtn, Ui.wrap());
        LinearLayout.LayoutParams gap = Ui.wrap();
        gap.leftMargin = Ui.dp(this, 6);
        row.addView(runBtn, gap);
        panel.addView(title, Ui.wrap());
        panel.addView(status, Ui.wrap());
        LinearLayout.LayoutParams rp = Ui.wrap();
        rp.topMargin = Ui.dp(this, 6);
        panel.addView(row, rp);

        lp = new WindowManager.LayoutParams(
                WindowManager.LayoutParams.WRAP_CONTENT, WindowManager.LayoutParams.WRAP_CONTENT,
                WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
                PixelFormat.TRANSLUCENT);
        lp.gravity = Gravity.TOP | Gravity.START;
        lp.x = Hub.prefs().getInt("panelX", Ui.dp(this, 12));
        lp.y = Hub.prefs().getInt("panelY", Ui.dp(this, 12));
        // 제목을 끌어서 옮김 (낚시 창 · 결과창을 가리지 않는 곳으로)
        title.setOnTouchListener(new View.OnTouchListener() {
            float sx, sy;
            int ox, oy;

            @Override
            public boolean onTouch(View v, MotionEvent e) {
                switch (e.getActionMasked()) {
                    case MotionEvent.ACTION_DOWN:
                        sx = e.getRawX();
                        sy = e.getRawY();
                        ox = lp.x;
                        oy = lp.y;
                        return true;
                    case MotionEvent.ACTION_MOVE:
                        lp.x = ox + Math.round(e.getRawX() - sx);
                        lp.y = oy + Math.round(e.getRawY() - sy);
                        wm.updateViewLayout(panel, lp);
                        return true;
                    case MotionEvent.ACTION_UP:
                        Hub.prefs().edit().putInt("panelX", lp.x).putInt("panelY", lp.y).apply();
                        return true;
                    default:
                        return false;
                }
            }
        });
        try {
            wm.addView(panel, lp);
        } catch (RuntimeException e) {
            Hub.log("조작 창을 띄우지 못함: " + e, 'r');
            panel = null;
        }
        refresh();
    }

    TextView small(String s, int color, View.OnClickListener on) {
        TextView b = Ui.button(this, s, color, on);
        b.setTextSize(13);
        b.setPadding(Ui.dp(this, 12), Ui.dp(this, 6), Ui.dp(this, 12), Ui.dp(this, 6));
        return b;
    }

    void toggle(boolean cal) {
        if (Hub.running()) {
            Hub.stop();
            return;
        }
        String err = Hub.start(cal);
        if (!err.isEmpty()) Hub.log(err, 'y');
        refresh();
    }

    void refresh() {
        if (panel == null) return;
        boolean run = Hub.running();
        String mode = Hub.mode;
        status.setText(CaptureService.active == null ? "화면 캡처 꺼짐 · 앱에서 시작" : Hub.state);
        calBtn.setText(run && "cal".equals(mode) ? "취소" : "보정");
        runBtn.setText(run && "run".equals(mode) ? "정지" : "시작");
        calBtn.setEnabled(!run || "cal".equals(mode));
        runBtn.setEnabled(!run || "run".equals(mode));
        calBtn.setAlpha(calBtn.isEnabled() ? 1f : 0.4f);
        runBtn.setAlpha(runBtn.isEnabled() ? 1f : 0.4f);
    }
}
