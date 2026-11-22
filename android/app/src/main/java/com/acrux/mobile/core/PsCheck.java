package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.List;

/**
 * 서버 체크 (디스코드 봇 /서버 체크) — 지금 들어가 있는 서버가 내 브섭인지 (PC 의 pscheck.py 와 같음)
 * 1. 왼쪽 바의 게임 메뉴 버튼 (줄 세 개) 을 한 번 눌러 Menu 창을 엶
 * 2. Private Server Commands 를 열고 관리 패널에서 내 브섭인지 확인
 * 3. Menu 창을 닫음 (X · 안 닫히면 메뉴 버튼을 한 번 더)
 */
public final class PsCheck {
    /** Menu 창 버튼 글자 (SellText.norm 뒤: a → o · i → l) → 1080 높이 전체 화면에서 글자 가운데 y */
    static final String[] KEYS = {"seoson", "ochlev", "flshlng", "settlng", "robux", "develop"};
    static final int[] KEY_Y = {441, 494, 539, 586, 633, 679};
    static final int TITLE_Y = 389, CLOSE_DX = 163;

    public interface Io {
        /** 화면 전체 글자 (화면 비율) */
        List<SellText.Box> read();

        /** 화면 비율 자리를 누름 */
        void tap(double[] pos);

        /** 초 */
        void sleep(double sec);

        /** 화면 크기 {w, h} */
        int[] size();
    }

    /** 결과: menu = Menu 창이 보임 · own = 내 브섭 · close = X 자리 (화면 비율) */
    public static final class Res {
        public boolean menu, buttonRead;
        public double[] commands, panelClose;
        public double[] close;
    }

    static int key(SellText.Box b) {
        String n = SellText.norm(b.text);
        for (int i = 0; i < KEYS.length; i++) if (n.contains(KEYS[i])) return i;
        return -1;
    }

    /** Private Server Commands 버튼 글자 (게임 글꼴이라 'Privote Server Commonds' 처럼 읽혀도) */
    public static boolean isPsc(String text) {
        String n = SellText.norm(text);
        return n.contains("prlv") && (n.contains("serv") || n.contains("comm"));
    }

    public static Res measure(List<SellText.Box> boxes, int W, int H) {
        Res r = new Res();
        List<SellText.Box> items = new ArrayList<>();
        boolean[] found = new boolean[KEYS.length];
        int kinds = 0;
        for (SellText.Box b : boxes) {
            int k = key(b);
            if (k < 0) continue;
            items.add(b);
            if (!found[k]) kinds++;
            found[k] = true;
        }
        if (kinds < 2) return r;
        r.menu = true;
        SellText.Box top = null, bot = null;
        double bottom = 0;
        for (SellText.Box b : items) {
            bottom = Math.max(bottom, b.y);
            if (top == null || KEY_Y[key(b)] < KEY_Y[key(top)]) top = b;
            if (bot == null || KEY_Y[key(b)] > KEY_Y[key(bot)]) bot = b;
        }
        double y1 = top.y * H, y2 = bot.y * H;
        int r1 = KEY_Y[key(top)], r2 = KEY_Y[key(bot)];
        double k = r2 > r1 && y2 > y1 ? (y2 - y1) / (r2 - r1) : H / 1080.0;
        SellText.Box title = null;
        for (SellText.Box b : boxes) {
            String n = SellText.norm(b.text);
            if ((n.equals("menu") || n.equals("menux")) && b.y * H < y1) {
                title = b;
                break;
            }
        }
        // Menu 창은 화면 가운데 (버튼 글자 덩어리는 왼쪽 아이콘이 같이 읽혀 가운데가 밀리기도 해서 제목 · 화면 가운데를 씀)
        double cx = title != null ? title.x : 0.5;
        for (SellText.Box b : boxes) {
            // 버튼은 Menu 창 바로 아래 · 가운데 (월드의 글자 · 게시판에 걸리지 않게)
            if (isPsc(b.text) && b.y > bottom && Math.abs(b.x - cx) * W < 0.25 * H) {
                r.buttonRead = true;
                r.commands = new double[]{b.x, b.y};
                break;
            }
        }
        double ty = title != null ? title.y * H : y1 - (r1 - TITLE_Y) * k;
        if (r.commands == null && k > 0 && y2 > y1) {
            double commandsY = (ty + (735 - TITLE_Y) * k) / H;
            if (commandsY > bottom && commandsY < 1) r.commands = new double[]{cx, commandsY};
        }
        r.panelClose = new double[]{cx + 450 * k / W, (ty - 70 * k) / H};
        double x = cx * W + CLOSE_DX * k, y = ty - k;
        r.close = new double[]{Math.min(1, Math.max(0, x / W)), Math.min(1, Math.max(0, y / H))};
        return r;
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

    static Res look(Io io) {
        int[] s = io.size();
        return measure(io.read(), s[0], s[1]);
    }

    /** 메뉴를 열어 내 브섭인지 확인하고 닫음 → TRUE · FALSE · null (메뉴 창을 못 찾음) */
    public static Boolean check(Io io, double[] menuPos, Runnable warnNotClosed) {
        Res res = null;
        for (int attempt = 0; attempt < 2; attempt++) {     // 처음부터 메뉴가 열려 있었으면 첫 탭에 닫힘 → 한 번 더
            io.tap(menuPos);
            io.sleep(1.0);
            res = look(io);
            if (!res.menu) {
                io.sleep(0.7);                              // 렉으로 늦게 뜸
                res = look(io);
            }
            if (res.menu) break;
        }
        if (res == null || !res.menu) return null;
        Boolean own = null;
        boolean menuStillOpen = true;
        if (res.commands != null) {
            for (int attempt = 0; attempt < 2; attempt++) {
                io.tap(res.commands);
                for (int poll = 0; poll < 3; poll++) {
                    io.sleep(0.45);
                    List<SellText.Box> boxes = io.read();
                    if (panel(boxes)) {
                        io.tap(res.panelClose);
                        io.sleep(0.6);
                        boxes = io.read();
                        if (panel(boxes)) {
                            io.tap(res.panelClose);
                            io.sleep(0.6);
                            boxes = io.read();
                            if (panel(boxes) && warnNotClosed != null) warnNotClosed.run();
                        }
                        if (!measure(boxes, io.size()[0], io.size()[1]).menu) return true;
                        own = true;
                        break;
                    }
                    menuStillOpen = measure(boxes, io.size()[0], io.size()[1]).menu;
                }
                if (own != null || !menuStillOpen) break;
            }
            if (!menuStillOpen) return null;
            if (own == null && !res.buttonRead) own = false;
        }
        // 닫기: X → 그래도 열려 있으면 메뉴 버튼 한 번 더
        List<double[]> targets = new ArrayList<>();
        if (res.close != null) targets.add(res.close);
        targets.add(menuPos);
        boolean closed = false;
        for (double[] t : targets) {
            io.tap(t);
            io.sleep(0.7);
            if (!look(io).menu) {
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
