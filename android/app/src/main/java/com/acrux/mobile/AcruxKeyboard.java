package com.acrux.mobile;

import android.inputmethodservice.InputMethodService;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.widget.TextView;

/** Acrux 키보드 — 지금 키보드로 선택해 두면 이동 · 판매의 키(W A S D E Esc R Enter O /)를 이 키보드가 로블록스에 보냄 */
public class AcruxKeyboard extends InputMethodService {
    static volatile AcruxKeyboard instance;

    @Override
    public void onCreate() {
        super.onCreate();
        instance = this;
        Hub.changed();
    }

    @Override
    public void onDestroy() {
        if (instance == this) instance = null;
        Hub.changed();
        super.onDestroy();
    }

    static InputConnection connection() {
        AcruxKeyboard k = instance;
        return k == null ? null : k.getCurrentInputConnection();
    }

    @Override
    public View onCreateInputView() {
        TextView t = Ui.text(this, "Acrux 키보드 — 이동 · 판매 키를 보내는 중 · 글자를 치려면 다른 키보드로 바꾸기", 13, Ui.MUTED, false);
        t.setBackgroundColor(Ui.BG);
        int p = Ui.dp(this, 12);
        t.setPadding(p, p, p, p);
        return t;
    }

    @Override
    public boolean onEvaluateFullscreenMode() {
        return false;
    }

    @Override
    public void onStartInput(EditorInfo info, boolean restarting) {
        super.onStartInput(info, restarting);
        Hub.changed();
    }
}
