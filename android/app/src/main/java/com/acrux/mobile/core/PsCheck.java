package com.acrux.mobile.core;

import java.util.List;

/**
 * 서버 체크 (디스코드 봇 /서버 체크) — 지금 들어가 있는 서버가 내 브섭인지 (PC 의 pscheck.py 와 같음)
 * 1. 왼쪽 바의 게임 메뉴 버튼 (줄 세 개) 을 한 번 눌러 Menu 창을 엶
 * 2. Private Server Commands 를 열고 관리 패널에서 내 브섭인지 확인
 * 3. Menu 창을 닫음 (지정한 X 위치 사용)
 */
public final class PsCheck {
    /** Menu 창 버튼 글자 (SellText.norm 뒤: a → o · i → l) → 1080 높이 전체 화면에서 글자 가운데 y */
    static final String[] KEYS = {"seoson", "ochlev", "flshlng", "settlng", "robux", "develop"};

    public interface Io {
        /** 화면 전체 글자 (화면 비율) */
        List<SellText.Box> read();

        /** 화면 비율 자리를 누름 */
        void tap(double[] pos);

        /** 초 */
        void sleep(double sec);

    }

    static boolean menu(List<SellText.Box> boxes) {
        boolean[] seen = new boolean[KEYS.length];
        int count = 0;
        for (SellText.Box box : boxes) {
            String label = SellText.norm(box.text);
            for (int i = 0; i < KEYS.length; i++) {
                if (!label.contains(KEYS[i])) continue;
                if (!seen[i] && ++count >= 2) return true;
                seen[i] = true;
                break;
            }
        }
        return false;
    }

    static boolean panel(List<SellText.Box> boxes) {
        StringBuilder labels = new StringBuilder();
        for (SellText.Box b : boxes) {
            if (b.x > 0.22 && b.x < 0.78 && b.y > 0.20 && b.y < 0.80)
                labels.append(SellText.norm(b.text));
        }
        String text = labels.toString();
        int sections = 0;
        for (String heading : new String[]{"server information", "server logs", "server control", "player control", "player list"})
            if (text.contains(SellText.norm(heading))) sections++;
        return sections >= 2 || (sections >= 1 && text.contains(SellText.norm("Private Server Management")));
    }

    /** 메뉴를 열어 내 브섭인지 확인하고 닫음 → TRUE · FALSE · null (메뉴 창을 못 찾음) */
    public static Boolean check(Io io, double[] menuPos, double[] savedCommands, double[] menuClose, double[] panelClose, Runnable warnNotClosed) {
        if (menuPos == null || savedCommands == null || menuClose == null || panelClose == null) return null;
        boolean opened = false;
        for (int attempt = 0; attempt < 2; attempt++) {     // 처음부터 메뉴가 열려 있었으면 첫 탭에 닫힘 → 한 번 더
            io.tap(menuPos);
            io.sleep(1.0);
            opened = menu(io.read());
            if (!opened) {
                io.sleep(0.7);                              // 렉으로 늦게 뜸
                opened = menu(io.read());
            }
            if (opened) break;
        }
        if (!opened) return null;
        Boolean own = null;
        boolean menuStillOpen = true;
        for (int attempt = 0; attempt < 2; attempt++) {
            io.tap(savedCommands);
            for (int poll = 0; poll < 3; poll++) {
                io.sleep(0.45);
                List<SellText.Box> boxes = io.read();
                if (panel(boxes)) {
                    io.tap(panelClose);
                    io.sleep(0.6);
                    boxes = io.read();
                    if (panel(boxes)) {
                        io.tap(panelClose);
                        io.sleep(0.6);
                        boxes = io.read();
                        if (panel(boxes)) {
                            if (warnNotClosed != null) warnNotClosed.run();
                            return true;
                        }
                    }
                    if (!menu(boxes)) return true;
                    own = true;
                    break;
                }
                menuStillOpen = menu(boxes);
            }
            if (own != null || !menuStillOpen) break;
        }
        if (!menuStillOpen) return null;
        if (own == null) own = false;
        boolean closed = false;
        for (int attempt = 0; attempt < 2; attempt++) {
            io.tap(menuClose);
            io.sleep(0.7);
            if (!menu(io.read())) {
                closed = true;
                break;
            }
        }
        if (!closed && warnNotClosed != null) warnNotClosed.run();
        return own;
    }

    private PsCheck() {
    }
}
