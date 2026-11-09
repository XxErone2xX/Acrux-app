package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * 물고기 판매 — PC 버전 sell.py 의 Seller · autocal 을 그대로 옮김
 *   기준 장소 → 판매 장소 길 → 카메라 정렬 → 대화 버튼 (E) 탭 → 대화 넘기기 → [Sell Fish]
 *   → (첫 칸 → Sell All → 확인 Sell) 을 확인창이 안 뜰 때까지 반복 → X 로 상점 닫기 → 기준 장소 → 낚시 장소 지점들
 * 다 팔았는지: Sell All 을 눌렀는데 확인창(초록 Sell 버튼)이 안 뜸 → 물고기가 없음
 * 모바일: 버튼은 전부 손가락 탭 (씹히면 짧게 · 길게 바꿔서 다시)
 */
public final class Seller {
    public static final String[][] KEYS = {{"dialog_pos", "대화창"}, {"sell_fish_pos", "Sell Fish 버튼"}, {"first_fish_pos", "첫 번째 물고기 칸"},
            {"sell_all_pos", "Sell All 버튼"}, {"confirm_sell_pos", "확인 Sell 버튼"}, {"shop_close_pos", "상점 닫기 X"}};
    /** 직접 지정하는 위치: 판매 장소에 도착했을 때 화면에 뜨는 대화 버튼 (PC 의 E) */
    public static final String[][] TALK = {{"talk_btn", "대화 버튼 (E)"}};
    static final double CONFIRM_GREEN = 0.04;
    static final double[] DIALOG_AREA = {0.0, 0.4, 1.0, 1.0};
    static final double SKIP_SETTLE = 0.35;

    final Fisher.Screen scr;
    final Mover mv;
    final Mover.Ocr ocr;
    final Map<String, double[]> pos;          // 판매 위치 (판매 보정)
    final Route sellRoute, fishRoute;
    final Fisher.Out out;
    public int sellMax = 100;
    public double sellDelay = 0;

    public Seller(Fisher.Screen scr, Mover mv, Mover.Ocr ocr, Map<String, double[]> pos, Route sellRoute, Route fishRoute, Fisher.Out out) {
        this.scr = scr;
        this.mv = mv;
        this.ocr = ocr;
        this.pos = pos;
        this.sellRoute = sellRoute;
        this.fishRoute = fishRoute;
        this.out = out;
    }

    double[] p(String k) {
        return pos.get(k);
    }

    /** 낚시 장소로 못 가는 이유 (null = 갈 수 있음) */
    public String fishMissing() {
        String m = mv.baseMissing();
        if (m != null) return m;
        if (fishRoute == null || fishRoute.isEmpty()) return "낚시 장소 길이 없음 (Acrux 창 → ⚙ → 낚시 장소 길 기록)";
        return null;
    }

    /** 판매를 못 하는 이유 (null = 팔 수 있음) */
    public String missing() {
        List<String> miss = new ArrayList<>();
        for (String[] k : KEYS) if (!pos.containsKey(k[0])) miss.add(k[1]);
        if (!miss.isEmpty()) return "판매 자동 보정 필요: " + String.join(", ", miss) + " (Acrux 창 → ⚙ → 판매 자동 보정)";
        for (String[] k : TALK) if (!pos.containsKey(k[0])) return "먼저 지정 필요: " + k[1] + " (Acrux 창 → ⚙ → 판매)";
        String m = mv.baseMissing();
        if (m != null) return m;
        if (sellRoute == null || sellRoute.isEmpty()) return "물고기 판매 장소 길이 없음 (Acrux 창 → ⚙ → 판매 장소 길 기록)";
        return fishMissing();
    }

    public void goFish() {
        mv.goPlace("낚시 장소", fishRoute);
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
        SellText.Box b = options(keys);
        while (b == null && System.nanoTime() < end) {
            click.run();
            mv.sleep(SKIP_SETTLE);
            b = options(keys);
        }
        return b;
    }

    /** 선택지가 보이나: [Sell Fish] 는 색으로 먼저 (Roll 글자가 비쳐서 글자 읽기가 틀릴 때가 있음) → 글자 */
    SellText.Box options(String... keys) {
        if (keys.length == 1 && "sellfish".equals(keys[0])) {
            double[] at = findSellFish();
            if (at != null) return new SellText.Box("[Sell Fish]", at[0], at[1], 0, 0);
        }
        return SellText.findText(ocr.read(DIALOG_AREA), keys);
    }

    /** 로블록스 버튼 누르기: t 번째 시도 — 0 탭 (0.15초) · 1 짧은 탭 (0.08초) · 2 길게 (0.3초) */
    void ui(double[] at, int t) {
        mv.tapUi(at, t == 0 ? 150 : t == 1 ? 80 : 300);
    }

    Img full() {
        return scr.grab(0, 0, scr.width(), scr.height());
    }

    /** 화면에서 대화 선택지 [Sell Fish] 를 색으로 찾음 (글자 읽기보다 먼저 · 선택지가 Roll 버튼과 겹쳐도 됨) */
    double[] findSellFish() {
        Img im = full();
        if (im == null || im.w == 0) return null;
        Buttons.Box[] d = Buttons.dialog(im);
        return d == null ? null : Buttons.center(d[1], im);
    }

    /** 화면에서 상점 [Sell All] 을 색으로 찾음 */
    double[] findSellAll() {
        Img im = full();
        if (im == null || im.w == 0) return null;
        Buttons.Box[] d = Buttons.shop(im);
        return d == null ? null : Buttons.center(d[1], im);
    }

    /** 상점이 열렸나: [Sell] · [Sell All] 색 → 없으면 글자 (Fishing Shop · Sell All) */
    boolean shopOpen() {
        if (findSellAll() != null) return true;
        List<SellText.Box> b = ocr.read(null);
        return SellText.findText(b, "fishingshop") != null || SellText.findText(b, true, "sellall") != null;
    }

    /** 대화창에서 [Sell Fish] 를 눌러 상점 열기 — 최대 3번 (탭 · 짧은 탭 · 길게) · 매번 상점이 열렸는지 확인 */
    boolean openShop(double[] sf, double d) {
        for (int t = 0; t < 3; t++) {
            double[] at = findSellFish();
            if (at == null) at = sf;
            else pos.put("sell_fish_pos", at);
            ui(at, t);
            for (int i = 0; i < 4; i++) {
                mv.sleep(0.5 + d / 2);
                if (shopOpen()) {
                    if (t > 0) out.log("[Sell Fish] 가 " + (t + 1) + "번째에 눌림", 'd');
                    return true;
                }
            }
        }
        return false;
    }

    /**
     * Sell All → 확인창이 뜰 때까지 최대 3번: 탭 → 짧은 탭 → 길게
     * 상점을 연 뒤 처음 누르는 Sell All 이 씹히는 경우가 있음 (한 번 눌리고 나면 그 뒤로는 잘 됨)
     * → 3번 다 안 떠야 '팔 물고기 없음'
     */
    double[] sellAll(double before, double d) {
        double[] at = p("confirm_sell_pos");
        for (int t = 0; t < 3; t++) {
            // 누르기 직전에 화면에서 [Sell All] 을 다시 찾음 (저장한 자리가 어긋나 있어도)
            double[] btn = findSellAll();
            if (btn != null) pos.put("sell_all_pos", btn);
            else btn = p("sell_all_pos");
            ui(btn, t);
            double[] got = waitConfirm(at, before, (t == 0 ? 1.5 : 2.0) + d);
            if (got != null) {
                if (t > 0) out.log("Sell All 이 " + (t + 1) + "번째에 눌림", 'd');
                return got;
            }
        }
        return null;
    }

    /** 판매 한 번 → 판 종류 수 */
    public int run() {
        String m = missing();
        if (m != null) throw new RuntimeException(m);
        double d = sellDelay;
        out.log("물고기 팔러 감", 'c');
        out.state("판매 · 물고기 판매 장소로 가는 중");
        mv.goBase();
        mv.walk("물고기 판매 장소", sellRoute);
        out.state("판매 · 카메라 정렬 (도감)");
        mv.alignCamera();                              // 대화 버튼을 누르기 전에 화면부터 맞춤
        out.state("판매 · 대화");
        mv.tapUi(p("talk_btn"));                       // PC 의 E — 화면에 뜨는 대화 버튼
        mv.sleep(0.2);
        SellText.Box sf = skipDialog(() -> ui(p("dialog_pos"), 0), 12.0, "sellfish");
        mv.sleep(0.15 + d);
        if (!openShop(sf != null ? new double[]{sf.x, sf.y} : p("sell_fish_pos"), d)) out.log("[Sell Fish] 를 3번 눌러도 상점이 안 열림 — 그대로 이어감", 'y');
        mv.sleep(0.5 + d);
        int sold = 0;
        boolean warned = false;
        for (int i = 0; i < sellMax; i++) {
            out.state("판매 · 파는 중 (" + sold + "종류)");
            ui(p("first_fish_pos"), 0);
            mv.sleep(0.5 + d);
            double before = green(p("confirm_sell_pos"));
            double[] sellAt = sellAll(before, d);
            if (sellAt == null) break;                  // Sell All 을 3번 눌러도 확인창이 없음 = 팔 물고기가 없음
            if (sellAt != p("confirm_sell_pos") && !warned) {
                warned = true;
                out.log("확인창 Sell 이 지정한 자리와 달라서 글자로 찾아 누름 — 판매 자동 보정을 다시 해주세요", 'y');
            }
            mv.sleep(0.15 + d);
            boolean closed = false;
            for (int t = 0; t < 3 && !closed; t++) {
                ui(sellAt, t);                          // 탭 · 짧은 탭 · 길게
                mv.sleep(1.2 + d);
                closed = !confirmOpen(sellAt, before);
            }
            if (!closed) {
                out.log("확인창 Sell 을 눌러도 안 닫힘 — 판매 위치를 확인해 주세요", 'y');
                break;
            }
            mv.sleep(0.3 + d);
            sold++;
        }
        out.log("물고기 판매 완료 (" + sold + "종류)", 'g');
        ui(p("shop_close_pos"), 0);
        mv.sleep(0.5 + d);
        out.state("판매 · 낚시 장소로 돌아가는 중");
        mv.goBase();
        mv.walk("낚시 장소", fishRoute);
        out.log("낚시 장소 도착 — 낚시 이어감", 'g');
        return sold;
    }

    // ---------------------------------------------------------------- 판매 자동 보정 (PC autocal)
    /** 플레이어가 Captain Flarg 앞에서 E 를 직접 누르면: 대화창 → [Sell Fish] → Fishing Shop → 확인창 위치를 글자로 찾음 */
    public List<String> autocal() {
        List<String> notes = new ArrayList<>();
        out.state("Captain Flarg 앞에서 대화 버튼을 눌러 대화를 여세요");
        long end = System.nanoTime() + 120_000_000_000L;
        double[] dlg;
        List<SellText.Box> boxes;
        while (true) {
            if (System.nanoTime() > end) throw new RuntimeException("대화창이 안 보임 — Captain Flarg 앞에서 대화 버튼을 눌러 주세요");
            boxes = ocr.read(DIALOG_AREA);
            dlg = SellText.dialogPoint(boxes);
            if (dlg != null || SellText.findText(boxes, "sellfish") != null) break;
            mv.sleep(0.4);
        }
        out.state("대화창 찾음 — 이제 화면을 만지지 마세요");
        mv.sleep(0.6);
        boxes = ocr.read(DIALOG_AREA);
        double[] d2 = SellText.dialogPoint(boxes);
        if (d2 != null) dlg = d2;
        if (dlg != null) pos.put("dialog_pos", dlg);
        SellText.Box sf = options("sellfish");
        if (sf == null) {
            if (!pos.containsKey("dialog_pos")) throw new RuntimeException("대화창을 못 찾음");
            sf = skipDialog(() -> ui(p("dialog_pos"), 0), 12.0, "sellfish");
        }
        if (sf == null) throw new RuntimeException("[Sell Fish] 버튼을 못 찾음");
        pos.put("sell_fish_pos", new double[]{SellText.r4(sf.x), SellText.r4(sf.y)});
        if (dlg == null) notes.add("대화창은 못 찾음");
        out.state("상점 여는 중");
        openShop(p("sell_fish_pos"), 0);
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
        // Buy · Sell 탭은 안 찾음 — 물고기 이름 · X 글자로 바로 찾고 · 못 찾으면 저장된 자리 (⚙ → 직접 지정) 그대로
        double aspect = (double) scr.width() / Math.max(1, scr.height());
        double[] first = SellText.firstFish(boxes, title, sa, aspect), close = SellText.closeX(boxes, title);
        double[] saAt = findSellAll();                  // 색으로 찾은 자리가 있으면 그걸로 (글자 자리보다 버튼 한가운데)
        pos.put("sell_all_pos", saAt != null ? saAt : new double[]{SellText.r4(sa.x), SellText.r4(sa.y)});
        if (first != null) pos.put("first_fish_pos", first);
        else if (p("first_fish_pos") != null) notes.add("물고기 이름을 못 읽어서 첫 번째 물고기 칸은 저장된 자리 그대로");
        else throw new RuntimeException("첫 번째 물고기 칸을 못 찾음 (물고기가 있을 때 다시 하거나 ⚙ → 첫 번째 물고기 칸 직접 지정)");
        if (close != null) pos.put("shop_close_pos", close);
        else if (p("shop_close_pos") != null) notes.add("X 를 못 읽어서 상점 닫기는 저장된 자리 그대로");
        else notes.add("상점 닫기 X 를 못 찾음 — ⚙ → 상점 닫기 X 직접 지정");
        out.state("확인창 확인 중 (팔지는 않음)");
        ui(p("first_fish_pos"), 0);
        mv.sleep(0.6);
        double[][] conf = null;
        for (int t = 0; t < 3 && conf == null; t++) {          // 처음 누르는 Sell All 이 씹힐 수 있음 → 탭 · 짧은 탭 · 길게
            ui(p("sell_all_pos"), t);
            for (int i = 0; i < 3 && conf == null; i++) {
                mv.sleep(0.6);
                conf = SellText.findConfirm(ocr.read(null));
            }
        }
        if (conf != null) {
            pos.put("confirm_sell_pos", conf[0]);
            ui(conf[1], 0);                           // Cancel
            mv.sleep(0.6);
        } else {
            notes.add(p("confirm_sell_pos") != null ? "확인창이 안 떠서(물고기 없음?) 확인 Sell 은 저장된 자리 그대로"
                    : "확인창이 안 떠서(물고기 없음?) 확인 Sell 을 못 찾음 — 물고기가 있을 때 다시 보정해 주세요");
        }
        if (p("shop_close_pos") != null) {
            out.state("상점 닫는 중");
            ui(p("shop_close_pos"), 0);
            mv.sleep(0.8);
            if (SellText.findText(ocr.read(null), "fishingshop") != null) notes.add("상점이 안 닫힘 — 상점 닫기 X 위치를 확인해 주세요");
        }
        return notes;
    }
}
