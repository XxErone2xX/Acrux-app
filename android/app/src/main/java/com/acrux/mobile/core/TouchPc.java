package com.acrux.mobile.core;

import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Shizuku 가 없을 때: PC 키보드 · 마우스 입력을 로블록스 모바일 터치 조작으로 바꿔서 누름
 *   W A S D        → 왼쪽 아래 조이스틱을 그 방향으로 밀고 있기 (여러 키 = 대각선)
 *   Esc → R → Enter → 메뉴 버튼 → Reset Character → Reset (글자로 찾음)
 *   E              → 상호작용 버튼 · Space → 점프 버튼
 *   / · Enter      → (채팅 키) 터치에선 필요 없음 · Shift 무시
 *   O 누르고 있기   → 두 손가락 모으기 (줌 아웃) · 누른 시간만큼 여러 번
 *   클릭 (좌 · 우)  → 그 자리 탭 (우클릭 = Tap to Move) · 최소 tapMs 동안 눌렀다 뗌 (짧으면 버튼이 안 눌릴 때가 있음)
 *   우클릭 드래그   → 한 손가락으로 화면 끌기 (카메라)
 * 키보드 앱 방식(keys)이 켜져 있고 보낼 수 있으면 키는 그쪽으로 먼저 보냄
 */
public final class TouchPc implements Pc {
    /** 터치 (화면 px) — 안드로이드에선 접근성 서비스 */
    public interface Touch {
        /** 누르고 ms 뒤에 뗌 (끝날 때까지 기다림) */
        void tap(int x, int y, int ms);

        void swipe(int x1, int y1, int x2, int y2, int ms);

        /** 두 손가락: far → near 로 모으기 (줌 아웃) */
        void pinch(int cx, int cy, int far, int near, int ms);

        /** 조이스틱: (cx, cy) 를 누른 채 (x, y) 로 밀어 둠 · null 이면 손 뗌 (기다리지 않음) */
        void stick(int cx, int cy, int[] to);
    }

    /** 키보드 앱 방식 — 보냈으면 true (입력칸이 없으면 false) */
    public interface Keys {
        boolean key(String key, boolean down);
    }

    public static final String[][] KEYS = {{"stick_pos", "조이스틱 가운데"}, {"jump_pos", "점프 버튼"}, {"menu_pos", "로블록스 메뉴 버튼"},
            {"interact_pos", "상호작용 버튼 (E 대신)"}};
    public static final Map<String, double[]> DEFAULTS = Map.of(
            "stick_pos", new double[]{0.16, 0.72}, "jump_pos", new double[]{0.88, 0.78}, "menu_pos", new double[]{0.035, 0.06});

    final Touch t;
    final Mover.Ocr ocr;
    final Map<String, double[]> pos;
    final Fisher.Screen scr;
    public Keys keys;
    public int tapMs = 120;
    public double tiltScale = 0.5, stickR = 0.09;
    public Fisher.Out out;

    final Set<String> held = new HashSet<>(), viaKeys = new HashSet<>();
    int mx, my, downX, downY, dragX, dragY;
    long downAt, oAt;
    boolean btnDown;
    int resetStep;           // 1 메뉴 열림 · 2 Reset Character 누름
    SellText.Box resetBox;

    public TouchPc(Touch t, Mover.Ocr ocr, Map<String, double[]> pos, Fisher.Screen scr) {
        this.t = t;
        this.ocr = ocr;
        this.pos = pos;
        this.scr = scr;
    }

    double[] at(String k) {
        double[] p = pos.get(k);
        return p != null ? p : DEFAULTS.get(k);
    }

    int[] px(double[] p) {
        return new int[]{(int) (p[0] * scr.width()), (int) (p[1] * scr.height())};
    }

    void tapAt(double[] p, int ms) {
        if (p == null) return;
        int[] q = px(p);
        t.tap(q[0], q[1], ms);
    }

    void log(String s) {
        if (out != null) out.log(s, 'd');
    }

    // ---------------------------------------------------------------- 키
    @Override
    public void keyDown(String key) {
        if (held.contains(key)) {
            if (viaKeys.contains(key) && keys != null) keys.key(key, true);     // 자동 반복
            return;
        }
        held.add(key);
        if (keys != null && keys.key(key, true)) {
            viaKeys.add(key);
            return;
        }
        try {
            press(key);
        } catch (RuntimeException e) {
            held.remove(key);
            throw e;
        }
    }

    void press(String key) {
        switch (key) {
            case "w": case "a": case "s": case "d":
                stick();
                break;
            case "space":
                tapAt(at("jump_pos"), 80);
                break;
            case "e":
                if (at("interact_pos") == null) throw new RuntimeException("터치 모드: 상호작용 버튼 위치를 먼저 지정 (Acrux 창 → ⚙ → 터치 조작)");
                tapAt(at("interact_pos"), tapMs);
                break;
            case "esc":
                tapAt(at("menu_pos"), 80);
                resetStep = 1;
                break;
            case "r":
                if (resetStep == 1) resetCharacter();
                break;
            case "enter":
                if (resetStep == 2) resetConfirm();
                break;
            case "o":
                oAt = System.nanoTime();
                break;
            default:
                break;                 // / · shift
        }
    }

    @Override
    public void keyUp(String key) {
        if (!held.remove(key)) return;
        if (viaKeys.remove(key)) {
            if (keys != null) keys.key(key, false);
            return;
        }
        switch (key) {
            case "w": case "a": case "s": case "d":
                stick();
                break;
            case "o":
                zoomOut((System.nanoTime() - oAt) / 1e9);
                break;
            default:
                break;
        }
    }

    void stick() {
        int dx = (held.contains("d") && !viaKeys.contains("d") ? 1 : 0) - (held.contains("a") && !viaKeys.contains("a") ? 1 : 0);
        int dy = (held.contains("s") && !viaKeys.contains("s") ? 1 : 0) - (held.contains("w") && !viaKeys.contains("w") ? 1 : 0);
        int[] c = px(at("stick_pos"));
        if (dx == 0 && dy == 0) {
            t.stick(c[0], c[1], null);
            return;
        }
        double len = Math.hypot(dx, dy), r = stickR * scr.height();
        t.stick(c[0], c[1], new int[]{(int) Math.round(c[0] + r * dx / len), (int) Math.round(c[1] + r * dy / len)});
    }

    /** O 를 누른 시간만큼 줌 아웃 (0.5초마다 한 번 모으기) */
    void zoomOut(double sec) {
        int n = Math.max(1, Math.min(10, (int) Math.round(sec / 0.5)));
        int w = scr.width(), h = scr.height();
        for (int i = 0; i < n; i++) t.pinch(w / 2, h / 2, (int) (h * 0.35), (int) (h * 0.06), 300);
    }

    SellText.Box find(boolean exact, String... k) {
        if (ocr == null) return null;
        for (int i = 0; i < 4; i++) {
            List<SellText.Box> b = ocr.read(null);
            SellText.Box f = SellText.findText(b, exact, k);
            if (f != null) return f;
            Pc.nap(350);
        }
        return null;
    }

    void resetCharacter() {
        Pc.nap(300);
        SellText.Box b = find(false, "resetcharacter", "resetchar");
        if (b == null && pos.containsKey("reset_pos")) {
            tapAt(pos.get("reset_pos"), 80);
            resetStep = 2;
            return;
        }
        if (b == null) {
            tapAt(at("menu_pos"), 80);          // 메뉴 닫기
            resetStep = 0;
            throw new RuntimeException("터치 모드: 메뉴에서 Reset Character 를 못 찾음 (메뉴 버튼 위치를 확인)");
        }
        tapAt(new double[]{b.x, b.y}, 80);
        resetBox = b;
        resetStep = 2;
    }

    void resetConfirm() {
        resetStep = 0;
        Pc.nap(300);
        SellText.Box ok = find(true, "reset");
        if (ok != null) tapAt(new double[]{ok.x, ok.y}, 80);
        else if (pos.containsKey("reset_ok_pos")) tapAt(pos.get("reset_ok_pos"), 80);
        else log("리셋 확인 버튼을 못 찾음 (확인 없이 리셋됐을 수 있음)");
    }

    // ---------------------------------------------------------------- 마우스
    @Override
    public void moveTo(int x, int y) {
        mx = x;
        my = y;
    }

    @Override
    public void moveRel(int dx, int dy) {
        if (btnDown) {
            dragX += dx;
            dragY += dy;
        }
        mx += dx;
        my += dy;
    }

    @Override
    public void button(String button, boolean down) {
        if (down) {
            if (btnDown) return;
            btnDown = true;
            downX = mx;
            downY = my;
            dragX = dragY = 0;
            downAt = System.nanoTime();
            return;
        }
        if (!btnDown) return;
        btnDown = false;
        if (Math.abs(dragX) + Math.abs(dragY) > 6) {
            int w = scr.width(), h = scr.height();
            int ex = clamp(downX + (int) (dragX * tiltScale), 2, w - 3), ey = clamp(downY + (int) (dragY * tiltScale), 2, h - 3);
            t.swipe(downX, downY, ex, ey, 600);
            return;
        }
        int held = (int) ((System.nanoTime() - downAt) / 1_000_000);
        t.tap(downX, downY, Math.max(tapMs, held));
    }

    static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
