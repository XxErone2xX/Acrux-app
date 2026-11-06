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

    /** 상점 글자들 → {닫기 X, 첫 칸, 확인 Sell} 위치 · aspect = 화면 너비 / 높이 · 못 재면 null */
    public static double[][] shopLayout(List<Box> boxes, Box title, Box sellAll, double aspect) {
        Box buy = null, tab = null;
        for (Box b : boxes) {
            if (norm(b.text).equals("buy") && b.y > title.y) {
                buy = b;
                break;
            }
        }
        if (buy != null) {
            for (Box b : boxes) {
                if (norm(b.text).equals("sell") && Math.abs(b.y - buy.y) < 0.03 && b.x > buy.x) {
                    tab = b;
                    break;
                }
            }
        }
        if (buy == null || tab == null) return null;
        double half = (tab.x - buy.x) / 2, left = buy.x - half, right = tab.x + half, span = sellAll.y - title.y;
        double[] close = r(right - 0.85 * title.h / aspect, title.y);
        List<Box> names = new ArrayList<>();
        for (Box b : boxes) {
            if (left < b.x && b.x < right && buy.y + 0.02 < b.y && b.y < sellAll.y + 0.05 && norm(b.text).length() >= 3) names.add(b);
        }
        names.sort(Comparator.comparingDouble(b -> b.y));
        double[] first;
        if (!names.isEmpty()) {
            double top = names.get(0).y;
            TreeSet<Double> rows = new TreeSet<>();
            for (Box b : names) rows.add(Math.round(b.y * 100.0) / 100.0);
            Double pitch = null;
            for (double y : rows) {
                if (y - top > names.get(0).h * 1.5) {
                    pitch = y - top;
                    break;
                }
            }
            if (pitch == null) {
                List<Double> xs = new ArrayList<>();
                for (Box b : names) if (Math.abs(b.y - top) < names.get(0).h) xs.add(b.x);
                xs.sort(null);
                pitch = xs.size() > 1 ? (xs.get(1) - xs.get(0)) * aspect : names.get(0).h * 3;
            }
            double col = Double.MAX_VALUE;
            for (Box b : names) col = Math.min(col, b.x);
            first = r(col, top - 0.3 * pitch);
        } else {
            double w = (right - left) / 5;
            first = r(left + w * 0.55, buy.y + w * aspect * 0.8);
        }
        double x1 = title.x - title.w / 2 - 0.005;
        double[] confirm = r((x1 + right) / 2 - 0.155 * (right - x1), title.y + 0.66 * span);
        return new double[][]{close, first, confirm};
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
