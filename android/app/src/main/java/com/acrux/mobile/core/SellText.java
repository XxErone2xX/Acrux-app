package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.TreeSet;

/**
 * 판매 화면 글자(OCR) 분석 — PC 버전 sell.py 의 순수 함수를 그대로 옮김
 * 글자 덩어리 Box = (글자, 가운데 x, 가운데 y, 너비, 높이) · 위치는 화면 비율
 */
public final class SellText {
    private SellText() {}

    public static final class Box {
        public final String text;
        public final double x, y, w, h;

        public Box(String text, double x, double y, double w, double h) {
            this.text = text;
            this.x = x;
            this.y = y;
            this.w = w;
            this.h = h;
        }
    }

    /** 영문만 · 소문자 · OCR 이 헷갈리는 글자는 하나로 (a ↔ o · i ↔ l) */
    public static String norm(String t) {
        StringBuilder s = new StringBuilder();
        for (char ch : String.valueOf(t).toLowerCase(java.util.Locale.ROOT).toCharArray()) {
            if (ch >= 'a' && ch <= 'z') s.append(ch == 'a' ? 'o' : ch == 'i' ? 'l' : ch);
        }
        return s.toString();
    }

    public static Box findText(List<Box> boxes, boolean exact, String... keys) {
        String[] ks = new String[keys.length];
        for (int i = 0; i < keys.length; i++) ks[i] = norm(keys[i]);
        for (Box b : boxes) {
            String n = norm(b.text);
            for (String k : ks) {
                boolean hit = exact ? (n.startsWith(k) && n.length() <= k.length() + 2) : n.contains(k);
                if (hit) return n.length() > k.length() + 2 ? subBox(b, k) : b;
            }
        }
        return null;
    }

    public static Box findText(List<Box> boxes, String... keys) {
        return findText(boxes, false, keys);
    }

    static Box subBox(Box b, String k) {
        String text = b.text;
        List<Integer> idx = new ArrayList<>();
        for (int i = 0; i < text.length(); i++) {
            char c = Character.toLowerCase(text.charAt(i));
            if (c >= 'a' && c <= 'z') idx.add(i);
        }
        String n = norm(text);
        int i = n.indexOf(k);
        if (i < 0 || idx.isEmpty() || text.length() < 2) return b;
        int a = idx.get(i), z = idx.get(i + k.length() - 1) + 1;
        double left = b.x - b.w / 2;
        return new Box(text.substring(a, z), left + b.w * (a + z) / 2.0 / text.length(), b.y, b.w * (z - a) / text.length(), b.h);
    }

    /** 파이썬 round(v, 4) 와 같게 (정확히 반일 때만 짝수 쪽) */
    static double r4(double v) {
        if (Double.isNaN(v) || Double.isInfinite(v)) return v;
        return new java.math.BigDecimal(v).setScale(4, java.math.RoundingMode.HALF_EVEN).doubleValue();
    }

    static double[] r(double x, double y) {
        return new double[]{r4(Math.min(1.0, Math.max(0.0, x))), r4(Math.min(1.0, Math.max(0.0, y)))};
    }

    /**
     * 대화 선택지 [Sell Fish] 를 글자로 찾기 — 선택지가 Roll 버튼 위에 겹쳐 그려져서 'SeRollsh' 처럼 섞여 읽힐 때가 많음
     *  1. sellfish 그대로 · 2. 'fish' · 'sellf' 가 들어간 글자 (대사 줄 아래) · 3. [Open Shop] 과 [Leave] 사이 가운데
     */
    public static Box sellFish(List<Box> boxes) {
        Box b = findText(boxes, "sellfish");
        if (b != null) return b;
        Box line = findText(boxes, "whotdoyou", "orrr", "clicktoskip", "toskip");
        double below = line != null ? line.y + line.h / 2 : 0;
        for (Box c : boxes) {
            String n = norm(c.text);
            if (c.y <= below || n.contains("coptoln")) continue;
            if (n.contains("flsh") || n.contains("sellf") || n.contains("ellfl")) return n.length() > 9 && n.contains("flsh") ? subBox(c, "flsh") : c;
        }
        Box open = findText(boxes, "openshop", "openshp"), leave = findText(boxes, "leove", "leave");
        if (open != null && leave != null && leave.x > open.x && Math.abs(open.y - leave.y) < 0.04)
            return new Box("[Sell Fish]", (open.x + leave.x) / 2, (open.y + leave.y) / 2, 0, 0);
        return null;
    }

    /** NPC 대화창 가운데 (대사 줄 · Click to skip · 이름 줄) */
    public static double[] dialogPoint(List<Box> boxes) {
        Box line = findText(boxes, "orrr", "whotdoyou");
        Box skip = findText(boxes, "clicktoskip", "toskip");
        Box ref = line != null ? line : skip;
        if (ref == null) return null;
        Box name = null;
        for (Box b : boxes) {
            if (norm(b.text).contains("coptolnflorg") && b.y < ref.y && (name == null || b.y > name.y)) name = b;
        }
        double sum = ref.y;
        int n = 1;
        if (name != null) {
            sum += name.y;
            n++;
        }
        if (skip != null && line != null) {
            sum += skip.y;
            n++;
        }
        return r(0.5, sum / n);
    }

    /** 상점에 늘 있는 글자 (물고기 이름이 아님) — norm 을 거친 모양으로 (전에는 'sellall' 그대로라 norm 한 'selloll' 과 안 맞아서
     *  Sell All 글자를 물고기 이름으로 봤음 → 물고기가 없어도 '아직 있음' 으로 보거나 첫 칸 자리가 틀어졌음) */
    static final java.util.Set<String> SHOP_WORDS = new java.util.HashSet<>();

    static {
        for (String w : new String[]{"sell", "sell all", "buy", "fishing shop", "cancel", "exit", "close"}) SHOP_WORDS.add(norm(w));
    }

    /** 물고기 이름이 아닌 글자 (norm 모양 일부) — 상점 뒤로 비치는 NPC 이름 · 알림 · 왼쪽 칸 글자 등 */
    static final String[] NOT_FISH = {"coptoln", "florg", "tolk", "ouroouto", "ouro", "your", "worp", "potlon", "ployer", "rejoln",
            "sellsfor", "welght", "empty", "flshlngshop", "selloll"};

    /** 물고기 이름처럼 생긴 단어: 대문자로 시작 + 영문 소문자 3자 이상 (앞뒤 문장 부호는 무시) — 그림 · 칸 테두리를 글자로 잘못 읽은 조각은 뺌 */
    static boolean nameLike(String t) {
        String w = t.replaceAll("^[^A-Za-z]+|[^A-Za-z]+$", "");
        if (w.length() < 4 || !Character.isUpperCase(w.charAt(0))) return false;
        int lower = 0;
        for (int i = 1; i < w.length(); i++) {
            char c = w.charAt(i);
            if (c >= 'a' && c <= 'z') lower++;
            else if (!(c >= 'A' && c <= 'Z')) return false;
        }
        return lower >= 3;
    }

    /** 여러 단어가 한 줄로 읽힌 글자 → 단어마다 (글자 수 비율로 자리를 나눔) — 한 줄에 물고기 이름 여러 개가 같이 읽힘 */
    static List<Box> words(Box b) {
        List<Box> out = new ArrayList<>();
        String t = b.text;
        if (t == null || t.trim().indexOf(' ') < 0 || t.length() < 2) {
            out.add(b);
            return out;
        }
        double left = b.x - b.w / 2, per = b.w / t.length();
        int i = 0;
        while (i < t.length()) {
            while (i < t.length() && t.charAt(i) == ' ') i++;
            int j = i;
            while (j < t.length() && t.charAt(j) != ' ') j++;
            if (j > i) out.add(new Box(t.substring(i, j), left + per * (i + j) / 2.0, b.y, per * (j - i), b.h));
            i = j;
        }
        return out;
    }

    /**
     * 첫 번째 물고기 칸 (Buy · Sell 탭 없이) — 물고기 칸들(제목 아래 · Sell All 위 · Sell All 오른쪽)의 이름 중 맨 윗줄 맨 왼쪽 → 그 이름 바로 위
     * (이름은 칸 아래쪽에 있음 · 이름 바로 위는 칸 안이라 칸 크기와 상관없이 눌림) · 이름을 못 읽으면 null
     * 전에는 상점 밖 글자도 이름으로 봐서 엉뚱한 곳을 눌렀음 (오른쪽 알림 'Aura Auto Added' · 상점 뒤로 비치는 'Captain Flarg' · 'Sell All' 등)
     * → 첫 칸을 못 눌러서 물고기가 안 골라지고, Sell All 을 눌러도 확인창이 안 떠서 '다 팔았음' 으로 보고 상점을 닫았음
     */
    public static double[] firstFish(List<Box> boxes, Box title, Box sellAll, double aspect) {
        double left = sellAll.x + sellAll.w / 2, right = 0.8;
        // 오른쪽 끝: 위쪽 [Sell] 탭 · 제목 줄의 X (있으면) — 그 오른쪽은 상점 밖 (알림 · 순위표)
        Box buy = null, tab = null;
        for (Box b : boxes) {
            String n = norm(b.text);
            if (b.y < title.y - title.h || b.y > title.y + 0.12) continue;
            if (n.equals("buy") && b.x > left) buy = b;
            if (n.equals("sell") && b.x > left && (tab == null || b.x > tab.x)) tab = b;
        }
        if (buy != null && tab != null && tab.x > buy.x) right = Math.min(right, tab.x + (tab.x - buy.x) * 0.6);
        double[] cx = closeX(boxes, title);
        if (cx != null && cx[0] > left) right = Math.min(right, cx[0] + 0.01);
        List<Box> names = new ArrayList<>();
        for (Box line : boxes) {
            for (Box b : words(line)) {
                String n = norm(b.text);
                if (n.length() < 3 || SHOP_WORDS.contains(n) || !nameLike(b.text)) continue;
                boolean bad = false;
                for (String w : NOT_FISH) if (n.contains(w)) bad = true;
                if (bad) continue;
                if (b.x <= left || b.x >= right) continue;
                if (b.y > title.y + title.h + 0.02 && b.y < sellAll.y) names.add(b);
            }
        }
        if (names.isEmpty()) return null;
        double top = 1;
        for (Box b : names) top = Math.min(top, b.y);
        Box f = null;
        for (Box b : names) {
            if (b.y - top > Math.max(b.h, 0.02)) continue;          // 맨 윗줄만
            if (f == null || b.x < f.x) f = b;
        }
        return r(f.x, f.y - Math.min(0.03, Math.max(0.012, f.h * 1.5)));
    }

    /** 상점 닫기 X — 제목 줄 오른쪽의 'X' 글자 · 못 읽으면 null */
    public static double[] closeX(List<Box> boxes, Box title) {
        Box best = null;
        for (Box b : boxes) {
            String t = String.valueOf(b.text).trim();
            boolean x = t.equalsIgnoreCase("x") || t.equals("×") || t.equals("✕") || t.equals("✖");
            if (x && b.x > title.x && Math.abs(b.y - title.y) < 0.08 && (best == null || b.x > best.x)) best = b;
        }
        return best == null ? null : r(best.x, best.y);
    }

    /** 확인창의 초록 Sell · 빨강 Cancel → {sell 위치, cancel 위치} 또는 null */
    public static double[][] findConfirm(List<Box> boxes) {
        Box cancel = findText(boxes, true, "cancel");
        if (cancel == null) return null;
        Box s = null;
        for (Box b : boxes) {
            if (norm(b.text).equals("sell") && Math.abs(b.y - cancel.y) < 0.04 && b.x < cancel.x && (s == null || b.x > s.x)) s = b;
        }
        if (s == null) return null;
        return new double[][]{{r4(s.x), r4(s.y)}, {r4(cancel.x), r4(cancel.y)}};
    }
}
