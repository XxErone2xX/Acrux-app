package com.acrux.mobile;

import android.content.ComponentName;
import android.content.ServiceConnection;
import android.content.pm.PackageManager;
import android.os.IBinder;
import android.view.KeyEvent;
import android.view.MotionEvent;

import com.acrux.mobile.core.Pc;

import java.util.HashMap;
import java.util.Map;

import rikka.shizuku.Shizuku;

/** PC 와 같은 키보드 · 마우스 (Shizuku 로 띄운 InjectService 를 통해) — 연결 상태 · 키 이름 → 키 코드 · 마우스 버튼 상태 */
final class ShizukuPc implements Pc {
    static volatile IInject svc;
    static volatile String why = "";
    static final int REQ = 77;
    static boolean listening;

    static final Map<String, Integer> KEYS = new HashMap<>();

    static {
        KEYS.put("esc", KeyEvent.KEYCODE_ESCAPE);
        KEYS.put("enter", KeyEvent.KEYCODE_ENTER);
        KEYS.put("/", KeyEvent.KEYCODE_SLASH);
        KEYS.put("space", KeyEvent.KEYCODE_SPACE);
        KEYS.put("shift", KeyEvent.KEYCODE_SHIFT_LEFT);
        for (char c = 'a'; c <= 'z'; c++) KEYS.put(String.valueOf(c), KeyEvent.KEYCODE_A + (c - 'a'));
    }

    static final Shizuku.UserServiceArgs ARGS = new Shizuku.UserServiceArgs(
            new ComponentName("com.acrux.mobile", InjectService.class.getName()))
            .daemon(false).processNameSuffix("inject").debuggable(false).version(3);

    static final ServiceConnection CONN = new ServiceConnection() {
        @Override
        public void onServiceConnected(ComponentName n, IBinder b) {
            IInject s = IInject.Stub.asInterface(b);
            String p;
            try {
                p = s.ping();
            } catch (Exception e) {
                p = String.valueOf(e);
            }
            if ("ok".equals(p)) {
                svc = s;
                why = "";
                Hub.log("Shizuku 연결됨 — 키보드 · 마우스 입력 준비됨", 'g');
            } else {
                svc = null;
                why = p;
                Hub.log("Shizuku 입력 서비스 오류: " + p, 'r');
            }
            Hub.changed();
        }

        @Override
        public void onServiceDisconnected(ComponentName n) {
            svc = null;
            why = "연결이 끊김";
            Hub.changed();
        }
    };

    static boolean ready() {
        IInject s = svc;
        if (s == null) return false;
        try {
            return s.asBinder().pingBinder();
        } catch (RuntimeException e) {
            return false;
        }
    }

    static String status() {
        if (ready()) return "✓ 연결됨";
        try {
            if (!Shizuku.pingBinder()) return "Shizuku 가 안 켜져 있음";
            if (Shizuku.isPreV11()) return "Shizuku 를 최신으로 업데이트해야 함";
            if (Shizuku.checkSelfPermission() != PackageManager.PERMISSION_GRANTED) return "권한 허용 필요";
        } catch (RuntimeException e) {
            return "Shizuku 없음";
        }
        return why.isEmpty() ? "연결 안 됨" : why;
    }

    /** 연결: Shizuku 실행 확인 → 권한 요청 → 입력 서비스 띄우기 */
    static void connect() {
        if (!listening) {
            listening = true;
            Shizuku.addRequestPermissionResultListener((code, result) -> {
                if (code == REQ && result == PackageManager.PERMISSION_GRANTED) bind();
                else if (code == REQ) Hub.log("Shizuku 권한을 허용하지 않음", 'y');
                Hub.changed();
            });
            Shizuku.addBinderDeadListener(() -> {
                svc = null;
                why = "Shizuku 가 꺼짐";
                Hub.changed();
            });
        }
        try {
            if (!Shizuku.pingBinder()) {
                Hub.log("Shizuku 앱을 켜고 '시작'을 먼저 해야 함 (무선 디버깅)", 'y');
                return;
            }
            if (Shizuku.checkSelfPermission() != PackageManager.PERMISSION_GRANTED) {
                Shizuku.requestPermission(REQ);
                return;
            }
            bind();
        } catch (RuntimeException e) {
            Hub.log("Shizuku 연결 실패: " + e.getMessage(), 'r');
        }
    }

    /** 앱을 켤 때: Shizuku 가 켜져 있고 이미 허용했으면 조용히 연결 */
    static void autoConnect() {
        try {
            if (svc == null && Shizuku.pingBinder() && !Shizuku.isPreV11()
                    && Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED) connect();
        } catch (RuntimeException ignored) {
        }
    }

    static void bind() {
        try {
            Shizuku.bindUserService(ARGS, CONN);
            Hub.log("Shizuku 입력 서비스 시작 중…", 'd');
        } catch (RuntimeException e) {
            Hub.log("Shizuku 입력 서비스를 띄우지 못함: " + e.getMessage(), 'r');
        }
    }

    // ---- 입력
    int x, y, buttons;
    final boolean relativeDrag;

    ShizukuPc(boolean relativeDrag) {
        this.relativeDrag = relativeDrag;
    }

    void key(String k, boolean down) {
        Integer code = KEYS.get(k);
        IInject s = svc;
        if (code == null || s == null) return;
        try {
            s.key(code, down);
        } catch (Exception ignored) {
        }
    }

    void mouse(int action, float mx, float my, int b, int actionButton, boolean rel) {
        IInject s = svc;
        if (s == null) return;
        try {
            s.mouse(action, mx, my, b, actionButton, rel);
        } catch (Exception ignored) {
        }
    }

    @Override
    public void keyDown(String k) {
        key(k, true);
    }

    @Override
    public void keyUp(String k) {
        key(k, false);
    }

    void at(int nx, int ny) {
        x = nx;
        y = ny;
        mouse(buttons == 0 ? MotionEvent.ACTION_HOVER_MOVE : MotionEvent.ACTION_MOVE, x, y, buttons, 0, false);
    }

    @Override
    public void moveTo(int nx, int ny) {
        at(nx, ny);
        Pc.nap(10);
        at(nx + 1, ny);                       // 로블록스가 위치를 인식하도록 1픽셀 흔들기 (PC 와 같음)
        at(nx, ny);
    }

    @Override
    public void moveRel(int dx, int dy) {
        if (relativeDrag) {
            mouse(MotionEvent.ACTION_MOVE, dx, dy, buttons, 0, true);
        } else {
            at(x + dx, y + dy);
        }
    }

    @Override
    public void button(String b, boolean down) {
        int mask = "right".equals(b) ? MotionEvent.BUTTON_SECONDARY : MotionEvent.BUTTON_PRIMARY;
        if (down) {
            if ((buttons & mask) != 0) return;
            boolean first = buttons == 0;
            buttons |= mask;
            if (first) mouse(MotionEvent.ACTION_DOWN, x, y, buttons, 0, false);
            mouse(MotionEvent.ACTION_BUTTON_PRESS, x, y, buttons, mask, false);
        } else {
            if ((buttons & mask) == 0) return;
            buttons &= ~mask;
            mouse(MotionEvent.ACTION_BUTTON_RELEASE, x, y, buttons, mask, false);
            if (buttons == 0) {
                mouse(MotionEvent.ACTION_UP, x, y, 0, 0, false);
                mouse(MotionEvent.ACTION_HOVER_MOVE, x, y, 0, 0, false);
            }
        }
    }
}
