package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * 물고기 판매 (모바일) — PC 버전 sell.py 의 Seller · autocal 을 옮김
 *   기준 장소 → 판매 장소(Captain Flarg) → 카메라 정렬 → 상호작용(E 대신) → 대화 넘기기 → [Sell Fish]
 *   → (첫 칸 → Sell All → 확인 Sell) 을 확인창이 안 뜰 때까지 반복 → X 로 상점 닫기 → 기준 장소 → 낚시 장소
 * 다 팔았는지: Sell All 을 눌렀는데 확인창(초록 Sell 버튼)이 안 뜸 → 물고기가 없음
 */
public final class Seller {
    public static final String[][] KEYS = {{"dialog_pos", "대화창"}, {"sell_fish_pos", "Sell Fish 버튼"}, {"first_fish_pos", "첫 번째 물고기 칸"},
            {"sell_all_pos", "Sell All 버튼"}, {"confirm_sell_pos", "확인 Sell 버튼"}, {"shop_close_pos", "상점 닫기 X"}};
    static final double CONFIRM_GREEN = 0.04;
    static final double[] DIALOG_AREA = {0.0, 0.4, 1.0, 1.0};
    static final double SKIP_SETTLE = 0.35;

    public interface Routes {
        String base();

        String fish();

        String sell();
    }

    final Fisher.Screen scr;
    final Mover mv;
    final Mover.Ocr ocr;
    final Map<String, double[]> pos;
    final Routes routes;
    final Fisher.Out out;
    public int sellMax = 100;
    public double sellDelay = 0;

    public Seller(Fisher.Screen scr, Mover mv, Mover.Ocr ocr, Map<String, double[]> pos, Routes routes, Fisher.Out out) {
        this.scr = scr;
        this.mv = mv;
        this.ocr = ocr;
        this.pos = pos;
        this.routes = routes;
        this.out = out;
    }

    static boolean empty(String route) {
        return Mover.parse(route).isEmpty();
    }

    String routeMissing(String... names) {
        for (String n : names) {
            String text = n.equals("기준 장소") ? routes.base() : n.equals("낚시 장소") ? routes.fish() : routes.sell();
            List<Mover.Step> steps;
            try {
                steps = Mover.parse(text);
            } catch (Mover.ParseError e) {
                return n + " 경로 " + e.getMessage();
            }
            if (steps.isEmpty()) return n + " 경로가 비어 있음";
            List<String> m = mv.missing(steps);
            if (!m.isEmpty()) return n + " 경로: 위치 지정 필요 — " + String.join(", ", m);
        }
        return null;
    }

    /** 낚시 장소로 못 가는 이유 (null = 갈 수 있음) */
    public String fishMissing() {
        return routeMissing("기준 장소", "낚시 장소");
    }

    /** 판매를 못 하는 이유 (null = 팔 수 있음) */
    public String missing() {
        List<String> miss = new ArrayList<>();
        for (String[] k : KEYS) if (!pos.containsKey(k[0])) miss.add(k[1]);
        if (!miss.isEmpty()) return "판매 보정 필요: " + String.join(", ", miss) + " (Acrux 창 → ⚙ → 판매 보정)";
        if (!pos.containsKey("상호작용버튼")) return "상호작용버튼 위치 지정 필요 (Acrux 창 → ⚙ → 위치 지정)";
        return routeMissing("기준 장소", "판매 장소", "낚시 장소");
    }

    public void goFish() {
        out.log("낚시 장소로 이동", 'd');
        mv.run(routes.base(), "기준 장소로");
        mv.run(routes.fish(), "낚시 장소로");
        out.log("낚시 장소 도착", 'g');
    }

    double[] p(String key) {
        return pos.get(key);
    }

    /** 확인창 Sell 버튼 자리의 밝은 초록 비율 0~1 · 못 보면 -1 */
    double green(double[] at) {
        int W = scr.width(), H = scr.height();
        int x = (int) (at[0] * W), y = (int) (at[1] * H), w = Math.max(8, (int) (H * 0.08)), h = Math.max(6, (int) (H * 0.04));
        Img im = scr.grab(x - w / 2, y - h / 2, w, h);
        if (im == null || im.w * im.h == 0) return -1;
        int c = 0;
        for (int k = 0; k < im.w * im.h; k++) {
            int r = im.r[k], g = im.g[k], b = im.b[k];
            if (g > 120 && g > r + 30 && g > b + 30) c++;
        }
        return (double) c / (im.w * im.h);
    }

    double[] waitConfirm(double[] at, double before, double timeout) {
        long end = System.nanoTime() + (long) (timeout * 1e9), ocrAt = System.nanoTime() + 300_000_000L;
        while (true) {
            double g = green(at);
            if (g >= 0 && g >= CONFIRM_GREEN && g >= Math.max(0, before) + 0.03) return at;
            if (System.nanoTime() >= ocrAt) {
                ocrAt = System.nanoTime() + 500_000_000L;
                double[][] conf = SellText.findConfirm(ocr.read(null));
                if (conf != null) return conf[0];
            }
            if (System.nanoTime() >= end) return null;
            mv.sleep(0.05);
        }
    }

    boolean confirmOpen(double[] at, double before) {
        double g = green(at);
        if (g >= 0 && g >= CONFIRM_GREEN && g >= Math.max(0, before) + 0.03) return true;
        return SellText.findConfirm(ocr.read(null)) != null;
    }

    /** 대화 넘기기 — 선택지(keys)가 보일 때까지: 글자 확인 → 없으면 대화창 한 번 누름 → 잠깐 → 다시 확인 */
    SellText.Box skipDialog(Runnable click, double timeout, String... keys) {
        long end = System.nanoTime() + (long) (timeout * 1e9);
        SellText.Box b = SellText.findText(ocr.read(DIALOG_AREA), keys);
        while (b == null && System.nanoTime() < end) {
            click.run();
            mv.sleep(SKIP_SETTLE);
            b = SellText.findText(ocr.read(DIALOG_AREA), keys);
        }
        return b;
    }

    /** 판매 한 번 → 판 종류 수 */
    public int run() {
        String m = missing();
        if (m != null) throw new RuntimeException(m);
        double d = sellDelay;
        out.log("물고기 팔러 감", 'd');
        mv.run(routes.base(), "판매 · 기준 장소로");
        mv.run(routes.sell(), "판매 · 판매 장소로");
        out.state("판매 · 대화");
        if (pos.containsKey("채팅버튼") && pos.containsKey("도감버튼") && pos.containsKey("도감닫기")) mv.run("카메라정렬", "판매 · 카메라 정렬");
        mv.tapAt(p("상호작용버튼"));
        mv.sleep(0.2);
        SellText.Box sf = skipDialog(() -> mv.tapAt(p("dialog_pos")), 12.0, "sellfish");
        mv.sleep(0.15 + d);
        mv.tapAt(sf != null ? new double[]{sf.x, sf.y} : p("sell_fish_pos"));
        mv.sleep(1.0 + d);
        int sold = 0;
        boolean warned = false;
        for (int i = 0; i < sellMax; i++) {
            out.state("판매 · 파는 중 (" + sold + "종류)");
            mv.tapAt(p("first_fish_pos"));
            mv.sleep(0.5 + d);
            double before = green(p("confirm_sell_pos"));
            mv.tapAt(p("sell_all_pos"));
            double[] sellAt = waitConfirm(p("confirm_sell_pos"), before, 2.5 + d);
            if (sellAt == null) break;                    // 확인창이 없음 = 팔 물고기가 없음
            if (sellAt != p("confirm_sell_pos") && !warned) {
                warned = true;
                out.log("확인창 Sell 이 보정한 자리와 달라서 글자로 찾아 누름 — 판매 보정을 다시 해 주세요", 'y');
            }
            mv.sleep(0.15 + d);
            boolean closed = false;
            for (int t = 0; t < 3 && !closed; t++) {
                mv.tapAt(sellAt);
                mv.sleep(1.2 + d);
                closed = !confirmOpen(sellAt, before);
            }
            if (!closed) {
                out.log("확인창 Sell 을 눌러도 안 닫힘 — 판매 보정을 확인해 주세요", 'y');
                break;
            }
            mv.sleep(0.3 + d);
            sold++;
        }
        out.log("물고기 판매 완료 (" + sold + "종류)", 'g');
        mv.tapAt(p("shop_close_pos"));
        mv.sleep(0.5 + d);
        mv.run(routes.base(), "판매 · 기준 장소로");
        mv.run(routes.fish(), "판매 · 낚시 장소로");
        out.log("낚시 장소 도착 — 낚시 이어감", 'g');
        return sold;
    }

    // ---------------------------------------------------------------- 판매 보정 (PC autocal)
    /** 플레이어가 Captain Flarg 앞에서 상호작용 버튼을 직접 누르면: 대화창 → [Sell Fish] → 상점 → 확인창 위치를 글자로 찾음 → 안내 글 */
    public List<String> autocal() {
        List<String> notes = new ArrayList<>();
        out.state("Captain Flarg 앞에서 상호작용 버튼을 눌러 주세요");
        long end = System.nanoTime() + 120_000_000_000L;
        double[] dlg = null;
        List<SellText.Box> boxes;
        while (true) {
            if (System.nanoTime() > end) throw new RuntimeException("대화창이 안 보임 — Captain Flarg 앞에서 상호작용 버튼을 눌러 주세요");
            boxes = ocr.read(DIALOG_AREA);
            dlg = SellText.dialogPoint(boxes);
            if (dlg != null || SellText.findText(boxes, "sellfish") != null) break;
            mv.sleep(0.4);
        }
        out.state("대화창 찾음 — 이제 만지지 마세요");
        mv.sleep(0.6);
        boxes = ocr.read(DIALOG_AREA);
        double[] d2 = SellText.dialogPoint(boxes);
        if (d2 != null) dlg = d2;
        if (dlg != null) pos.put("dialog_pos", dlg);
        SellText.Box sf = SellText.findText(boxes, "sellfish");
        if (sf == null) {
            if (!pos.containsKey("dialog_pos")) throw new RuntimeException("대화창을 못 찾음");
            sf = skipDialog(() -> mv.tapAt(p("dialog_pos")), 12.0, "sellfish");
        }
        if (sf == null) throw new RuntimeException("[Sell Fish] 버튼을 못 찾음");
        pos.put("sell_fish_pos", new double[]{SellText.r4(sf.x), SellText.r4(sf.y)});
        if (dlg == null) notes.add("대화창은 못 찾음");
        out.state("상점 여는 중");
        mv.tapAt(p("sell_fish_pos"));
        SellText.Box title = null, sa = null;
        for (int i = 0; i < 8; i++) {
            mv.sleep(0.6);
            boxes = ocr.read(null);
            title = SellText.findText(boxes, "fishingshop");
            sa = SellText.findText(boxes, true, "sellall");
            if (title != null && sa != null) break;
        }
        if (title == null || sa == null) throw new RuntimeException("상점(Fishing Shop · Sell All)을 못 찾음");
        mv.sleep(0.4);
        boxes = ocr.read(null);
        SellText.Box t2 = SellText.findText(boxes, "fishingshop"), s2 = SellText.findText(boxes, true, "sellall");
        if (t2 != null) title = t2;
        if (s2 != null) sa = s2;
        double[][] geo = SellText.shopLayout(boxes, title, sa, (double) scr.width() / Math.max(1, scr.height()));
        if (geo == null) throw new RuntimeException("상점 Buy · Sell 탭을 못 찾음");
        pos.put("sell_all_pos", new double[]{SellText.r4(sa.x), SellText.r4(sa.y)});
        pos.put("first_fish_pos", geo[1]);
        pos.put("shop_close_pos", geo[0]);
        out.state("확인창 확인 중 (팔지는 않음)");
        mv.tapAt(p("first_fish_pos"));
        mv.sleep(0.6);
        mv.tapAt(p("sell_all_pos"));
        double[][] conf = null;
        for (int i = 0; i < 4 && conf == null; i++) {
            mv.sleep(0.6);
            conf = SellText.findConfirm(ocr.read(null));
        }
        if (conf != null) {
            pos.put("confirm_sell_pos", conf[0]);
            mv.tapAt(conf[1]);                       // Cancel
            mv.sleep(0.6);
        } else {
            pos.put("confirm_sell_pos", geo[2]);
            notes.add("확인창이 안 떠서(물고기 없음?) 확인 Sell 은 계산한 자리");
        }
        out.state("상점 닫는 중");
        mv.tapAt(p("shop_close_pos"));
        mv.sleep(0.8);
        if (SellText.findText(ocr.read(null), "fishingshop") != null) notes.add("상점이 안 닫힘 — 상점 닫기 X 위치를 직접 지정해 주세요");
        return notes;
    }
}
