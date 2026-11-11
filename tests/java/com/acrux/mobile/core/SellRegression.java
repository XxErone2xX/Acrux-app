package com.acrux.mobile.core;

import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** 판매 (Seller) 를 가짜 화면 · 글자 읽기 · 터치로 돌려 봄 — 색 찾기는 검은 화면이라 안 되고 글자로만 찾음 */
public final class SellRegression {
    static double now() {
        return System.nanoTime() / 1e9;
    }

    /** 가짜 로블록스: 대화 선택지는 대화창을 누른 뒤 0.9초 지나야 보임 (서서히 떠오름) · 상점을 연 직후 첫 탭은 씹힘 */
    static final class Game implements Fisher.Screen, Mover.Ocr {
        double lastDialogTap = now();
        int dialogTaps, fish, sold, firstTaps;
        boolean shop, selected, confirm, swallowFirst;
        final List<String> taps = new ArrayList<>();

        public int width() { return 1000; }
        public int height() { return 500; }
        public Img grab(int x, int y, int w, int h) { return new Img(Math.max(1, w), Math.max(1, h)); }

        public List<SellText.Box> read(double[] region) {
            List<SellText.Box> b = new ArrayList<>();
            if (!shop) {
                b.add(new SellText.Box("Captain Flarg", 0.45, 0.62, 0.1, 0.03));
                b.add(new SellText.Box("Arrrrrrr... What do you", 0.5, 0.70, 0.3, 0.04));
                b.add(new SellText.Box("Click to skip", 0.42, 0.80, 0.05, 0.01));
                if (now() - lastDialogTap > 0.9) {
                    b.add(new SellText.Box("[Open Shop]", 0.36, 0.87, 0.1, 0.03));
                    b.add(new SellText.Box("[SeRollsh]", 0.5, 0.87, 0.1, 0.03));     // Roll 글자가 섞여 읽힘
                    b.add(new SellText.Box("[Leave]", 0.65, 0.87, 0.07, 0.03));
                }
                return b;
            }
            // 템플릿 1 (2400x1080) 의 자리 · 오른쪽 알림 · 상점 뒤로 비치는 NPC 이름도 같이 (이름으로 잘못 보면 안 됨)
            b.add(new SellText.Box("Fishing Shop", 0.293, 0.251, 0.09, 0.026));
            b.add(new SellText.Box("Buy", 0.4875, 0.29, 0.012, 0.011));
            b.add(new SellText.Box("Sell", 0.667, 0.288, 0.011, 0.011));
            b.add(new SellText.Box("Aura Auto Added", 0.89, 0.30, 0.08, 0.014));
            b.add(new SellText.Box("Captain Flarg", 0.55, 0.41, 0.06, 0.02));
            b.add(new SellText.Box("Sell", 0.282, 0.745, 0.015, 0.015));
            b.add(new SellText.Box("Sell All", 0.357, 0.745, 0.03, 0.015));
            b.add(new SellText.Box(selected ? "Weight: 19.8" : "Empty.", 0.27, 0.66, 0.05, 0.03));
            b.add(new SellText.Box(selected ? "Sells for 40" : "Sells for 0", 0.27, 0.716, 0.04, 0.02));
            if (fish > 0) b.add(new SellText.Box(fish > 1 ? "Spiraldrift Moonfish" : "Spiraldrift", fish > 1 ? 0.47 : 0.435, 0.43, fish > 1 ? 0.12 : 0.05, 0.017));
            if (confirm) {
                b.add(new SellText.Box("Sell", 0.4217, 0.59, 0.03, 0.03));
                b.add(new SellText.Box("Cancel", 0.578, 0.59, 0.04, 0.03));
            }
            return b;
        }

        void tap(double x, double y) {
            if (!shop) {
                if (Math.abs(y - 0.87) < 0.03 && Math.abs(x - 0.5) < 0.05 && now() - lastDialogTap > 0.9) {
                    shop = true;
                    taps.add("sellfish");
                } else {
                    lastDialogTap = now();
                    dialogTaps++;
                    taps.add("dialog");
                }
                return;
            }
            if (confirm) {
                if (Math.abs(x - 0.4217) < 0.03 && Math.abs(y - 0.59) < 0.03) {
                    confirm = selected = false;
                    fish--;
                    sold++;
                    taps.add("confirm");
                }
                return;
            }
            if (x >= 0.405 && x <= 0.465 && y >= 0.322 && y <= 0.456) {      // 첫 번째 칸
                firstTaps++;
                if (swallowFirst && firstTaps == 1) {
                    taps.add("first(swallowed)");
                    return;
                }
                if (fish > 0) selected = true;
                taps.add("first");
            } else if (Math.abs(x - 0.357) < 0.035 && Math.abs(y - 0.745) < 0.02) {
                taps.add("sellall");
                if (selected) confirm = true;
            } else {
                taps.add(String.format("miss(%.3f,%.3f)", x, y));
            }
        }
    }

    static Seller seller(Game g, Map<String, double[]> pos) {
        Touch t = (Touch) Proxy.newProxyInstance(Touch.class.getClassLoader(), new Class<?>[]{Touch.class}, (p, m, a) -> {
            if (m.getName().equals("tap")) g.tap((Integer) a[0] / 1000.0, (Integer) a[1] / 500.0);
            return m.getReturnType() == boolean.class ? Boolean.TRUE : null;
        });
        Fisher.Out out = new Fisher.Out() {
            public void log(String msg, char color) { System.out.println("    log: " + msg); }
            public void state(String msg) {}
        };
        Mover mv = new Mover(g, t, g, new HashMap<>(), new Mover.Cfg(), out, () -> {});
        return new Seller(g, mv, g, pos, new Route(), new Route(), out);
    }

    static void check(boolean ok, String what) {
        if (!ok) throw new AssertionError(what);
    }

    public static void main(String[] a) {
        // 1. 선택지가 서서히 떠오름 + 글자가 섞여 읽힘 → 그래도 [Sell Fish] 를 찾음 (전에는 0.35초마다 대화창을 눌러서 못 봄)
        Game g = new Game();
        Map<String, double[]> pos = new HashMap<>();
        pos.put("dialog_pos", new double[]{0.5, 0.75});
        Seller s = seller(g, pos);
        SellText.Box sf = s.skipDialog(() -> s.ui(pos.get("dialog_pos"), 0), 6.0, "sellfish");
        check(sf != null && Math.abs(sf.x - 0.5) < 0.03, "sell fish not found, taps " + g.taps);
        System.out.println("  fading choices: found after " + g.dialogTaps + " dialog taps");

        // 2. 상점을 연 직후 첫 칸 탭이 씹힘 → 그래도 다 팖 (전에는 '물고기 없음' 으로 보고 닫았음)
        g = new Game();
        g.shop = true;
        g.fish = 3;
        g.swallowFirst = true;
        pos.put("first_fish_pos", new double[]{0.435, 0.39});
        pos.put("sell_all_pos", new double[]{0.357, 0.745});
        pos.put("confirm_sell_pos", new double[]{0.4217, 0.59});
        Seller s2 = seller(g, pos);
        int sold = s2.sellLoop(0);
        check(sold == 3 && g.fish == 0, "sold " + sold + " left " + g.fish + " taps " + g.taps);
        System.out.println("  swallowed first tap: sold " + sold);

        // 3. 저장한 첫 칸 자리가 틀림 (물고기 칸이 아님) → 화면 글자로 다시 찾아서 팖
        g = new Game();
        g.shop = true;
        g.fish = 2;
        pos.put("first_fish_pos", new double[]{0.55, 0.39});      // 두 번째 칸 쪽 (첫 칸이 아님)
        Seller s3 = seller(g, pos);
        sold = s3.sellLoop(0);
        check(sold == 2, "wrong first slot: sold " + sold + " taps " + g.taps);
        System.out.println("  wrong first slot: sold " + sold);
        System.out.println("3 sell regression cases passed");
    }
}
