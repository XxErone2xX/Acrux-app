package com.acrux.mobile.core;

import java.text.Normalizer;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * 오토 아이템 사용 — PC 버전 popping.py 의 ItemUser 를 그대로 옮김
 *   Inventory → Items → (아이템마다: Search → 이름 입력 + 엔터 → 글자 읽기로 이름 확인 (일치율 · 1번 더 시도)
 *   → 아이템 칸 → 수량칸 → 1 입력 + 엔터 → Use) → Inventory 닫기
 * 간격마다 1개씩 · 하나도 못 쓰면 1분 뒤 다시 · 자동 낚시가 던지기 전 대기 화면일 때 (낚시를 끊지 않게)
 * 위치는 플레이어가 직접 지정 · 글자 넣기는 접근성 (안 되면 무선 디버깅)
 */
public final class ItemUser {
    public static final String[][] KEYS = {{"item_inventory_pos", "Inventory 버튼"}, {"item_items_pos", "Items 버튼"},
            {"item_search_pos", "Search 버튼"}, {"item_slot_pos", "아이템 칸"}, {"item_amount_pos", "수량 입력칸"}, {"item_use_pos", "Use 버튼"}};
    /** 아이템 이름을 읽는 칸 (두 모서리) */
    public static final String[][] OCR = {{"item_ocr_a", "이름 읽는 칸 왼쪽 위"}, {"item_ocr_b", "이름 읽는 칸 오른쪽 아래"}};
    public static final String[] NAMES = {"Strange Controller", "Biome Randomizer"};
    static final double RETRY = 60;
    // PC 의 DELAY_DEFAULT 와 같음 (초)
    static final double D_INVENTORY = 0.5, D_ITEMS = 0.5, D_SEARCH = 0.5, D_TYPING = 0.5, D_ITEM_CLICK = 0.35, D_AFTER_DBL = 0.5, D_AFTER_ENTER = 0.35;

    /** 지금 입력칸에 글자를 넣음 (clear: 원래 글자를 지우고) + 엔터 → 됐는지 */
    public interface Typer {
        boolean type(String text, boolean clear);
    }

    public static final class Cfg {
        public boolean on = false;
        /** Strange Controller · Biome Randomizer 사용 */
        public final boolean[] use = {true, true};
        /** 간격 (분) — PC 와 같음 */
        public final double[] min = {10.5, 18.0};
        /** 다 쓰고 Inventory 버튼을 한 번 더 눌러 닫기 */
        public boolean close = true;
        /** 이름 일치율 (%) */
        public double match = 70;
    }

    public final Cfg cfg = new Cfg();
    /** 마지막으로 쓴 시각 (초 · 0 = 켜면 바로 한 번 씀) · 이번에 앱을 켠 뒤 쓴 횟수 */
    public final double[] last = {0, 0};
    public final int[] used = {0, 0};
    public double retryAt = 0;
    /** 위치가 덜 됐다고 알린 시각 (자주 안 알리게) */
    double missLogAt = -1e9;

    public static double now() {
        return System.currentTimeMillis() / 1000.0;
    }

    /** 위치가 덜 됐으면 이유 (null = 쓸 수 있음) */
    public static String missing(Map<String, double[]> pos) {
        List<String> miss = new ArrayList<>();
        for (String[] k : KEYS) if (!pos.containsKey(k[0])) miss.add(k[1]);
        if (!pos.containsKey(OCR[0][0]) || !pos.containsKey(OCR[1][0])) miss.add("이름 읽는 칸");
        return miss.isEmpty() ? null : "오토 아이템 위치 지정 필요: " + String.join(", ", miss) + " (Acrux 창 → 오토 아이템)";
    }

    public static int saved(Map<String, double[]> pos) {
        int n = 0;
        for (String[] k : KEYS) if (pos.containsKey(k[0])) n++;
        for (String[] k : OCR) if (pos.containsKey(k[0])) n++;
        return n;
    }

    /** 지금 쓸 차례인 아이템 (NAMES 번호) */
    public List<Integer> due(double t) {
        List<Integer> out = new ArrayList<>();
        if (!cfg.on || t < retryAt) return out;
        for (int k = 0; k < NAMES.length; k++) if (cfg.use[k] && t - last[k] >= cfg.min[k] * 60) out.add(k);
        return out;
    }

    /** 다음에 쓰기까지 남은 초 */
    public double left(int k, double t) {
        return Math.max(0, Math.max(last[k] + cfg.min[k] * 60, retryAt) - t);
    }

    /** 위치가 덜 됐을 때 자동 낚시 중에는 5분에 한 번만 알림 */
    public boolean missLogDue(double t) {
        if (t - missLogAt < 300) return false;
        missLogAt = t;
        return true;
    }

    /** keys 아이템을 하나씩 사용 → 쓴 개수 (멈추면 Fisher.Stopped) */
    public int run(List<Integer> keys, Mover mv, Mover.Ocr ocr, Map<String, double[]> pos, Typer typer, Fisher.Out out, boolean test) {
        String miss = missing(pos);
        if (miss != null) {
            retryAt = now() + RETRY;
            out.log(miss, 'y');
            return 0;
        }
        List<String> names = new ArrayList<>();
        for (int k : keys) names.add(NAMES[k]);
        out.log((test ? "오토 아이템 사용 테스트 — " : "오토 아이템 사용 — ") + String.join(", ", names), 'd');
        out.state("오토 아이템: Inventory 열기");
        mv.tapUi(pos.get("item_inventory_pos"));
        mv.sleep(D_INVENTORY);
        mv.tapUi(pos.get("item_items_pos"));
        mv.sleep(D_ITEMS);
        int ok = 0;
        boolean noInput = false;
        try {
            for (int k : keys) {
                if (useOne(NAMES[k], mv, ocr, pos, typer, out)) {
                    last[k] = now();
                    used[k]++;
                    ok++;
                }
                mv.sleep(D_AFTER_ENTER);
            }
        } catch (NoInput e) {
            noInput = true;
            out.log("오토 아이템: 검색칸에 글자를 못 넣음 — 무선 디버깅을 연결하면 그걸로 입력함 (앱 → 바이옴 → 무선 디버깅 연결) · 5분 뒤 다시 시도", 'y');
        }
        if (noInput) {
            retryAt = now() + 5 * RETRY;                           // 글자를 못 넣으면 5분 뒤 다시 (인벤토리를 1분마다 열지 않게)
        } else if (ok == 0) {
            retryAt = now() + RETRY;
            out.log("오토 아이템 — 쓴 아이템 없음 · 1분 뒤 다시 시도", 'y');
        }
        if (cfg.close) {
            out.state("오토 아이템: Inventory 닫기");
            mv.tapUi(pos.get("item_inventory_pos"));
            mv.sleep(D_INVENTORY);
        }
        return ok;
    }

    static final class NoInput extends RuntimeException {
        NoInput() {
            super("no input", null, false, false);
        }
    }

    boolean useOne(String name, Mover mv, Mover.Ocr ocr, Map<String, double[]> pos, Typer typer, Fisher.Out out) {
        double th = cfg.match;
        Integer count = null;
        String text = "";
        for (int attempt = 1; attempt <= 2; attempt++) {           // 일치율 미달이면 Search 부터 1번 더
            out.state("오토 아이템: " + name + " 검색" + (attempt == 2 ? " (다시)" : ""));
            mv.tapUi(pos.get("item_search_pos"));
            mv.sleep(D_SEARCH);
            if (!typer.type(name, true)) throw new NoInput();
            mv.sleep(D_TYPING);
            text = read(ocr, pos);
            Object[] pr = parse(text);
            String ocrName = (String) pr[0];
            count = (Integer) pr[1];
            double score = similarity(ocrName, name);
            out.log("글자 읽기: '" + cut(text, 40) + "' · " + name + " 일치율 " + fmt(score) + "%", 'd');
            if (score >= th) break;
            count = null;
            if (attempt == 2) {
                out.log(name + " — 일치율 " + fmt(th) + "% 미만, 건너뜀", 'y');
                return false;
            }
        }
        if (count == null) {
            // 게임은 1개뿐인 아이템엔 'x1' 을 안 띄움 (개수 표시가 없으면 1개)
            if (!text.matches(".*\\d.*")) count = 1;
            else {
                out.log(name + " — 개수를 읽지 못해 건너뜀", 'y');
                return false;
            }
        }
        if (count < 1) {
            out.log(name + " — 없음, 건너뜀", 'y');
            return false;
        }
        out.state("오토 아이템: " + name + " 1개 사용");
        mv.tapUi(pos.get("item_slot_pos"));
        mv.sleep(D_ITEM_CLICK);
        mv.tapUi(pos.get("item_amount_pos"));
        mv.sleep(D_AFTER_DBL);
        if (!typer.type("1", true)) out.log("오토 아이템: 수량칸에 1 을 못 넣음 — 그대로 Use", 'd');
        mv.sleep(D_AFTER_ENTER);
        mv.tapUi(pos.get("item_use_pos"));
        out.log(name + " 1개 사용 (보유 " + count + "개)", 'g');
        return true;
    }

    /** 이름 칸 글자 (위 → 아래 · 왼쪽 → 오른쪽 순서로 이어 붙임) */
    static String read(Mover.Ocr ocr, Map<String, double[]> pos) {
        double[] a = pos.get(OCR[0][0]), b = pos.get(OCR[1][0]);
        List<SellText.Box> bs = new ArrayList<>(ocr.read(new double[]{a[0], a[1], b[0], b[1]}));
        bs.sort(Comparator.<SellText.Box>comparingDouble(x -> Math.round(x.y * 40)).thenComparingDouble(x -> x.x));
        StringBuilder s = new StringBuilder();
        for (SellText.Box x : bs) {
            if (s.length() > 0) s.append(' ');
            s.append(x.text);
        }
        return s.toString();
    }

    static String cut(String s, int n) {
        return s.length() <= n ? s : s.substring(0, n);
    }

    static String fmt(double v) {
        return v == Math.rint(v) ? String.valueOf((long) v) : String.valueOf(Math.round(v * 10) / 10.0);
    }

    // ---------------------------------------------------------------- 이름 · 개수 (PC 의 parse_ocr · similarity)
    static final Pattern COUNT_RE = Pattern.compile("[x×X]\\s*([0-9][0-9,.]*)\\s*$");
    /** 개수 부분을 조금 틀리게 읽은 경우 (x1O · xl3 등) — 끝에 붙은 'x…' 덩어리 */
    static final Pattern LOOSE_COUNT_RE = Pattern.compile("\\s[x×X]\\s*([0-9lIiO|o,.]+)\\s*$");
    static final Pattern JUNK = Pattern.compile("^(?=.*[0-9])[0-9SsOoIl|]*[x×X][0-9SsOoIl|,.]*$|^[0-9,.]+$");
    static final Pattern ZERO_WIDTH = Pattern.compile("[\\u200b-\\u200f\\u2060\\ufeff\\u00ad]");

    /** 이름에 섞여 들어온 개수 조각 (8X · S6X · 23 등) 제거 */
    static String cleanName(String name) {
        StringBuilder s = new StringBuilder();
        for (String t : name.trim().split("\\s+")) {
            if (t.isEmpty() || JUNK.matcher(t).matches()) continue;
            if (s.length() > 0) s.append(' ');
            s.append(t);
        }
        return s.toString();
    }

    /** 'Warp Potion x23' → {"Warp Potion", 23} · 개수를 못 읽으면 null */
    public static Object[] parse(String text) {
        String t = String.join(" ", (text == null ? "" : text).trim().split("\\s+"));
        Matcher m = COUNT_RE.matcher(t);
        if (m.find()) return new Object[]{cleanName(t.substring(0, m.start())), digits(m.group(1))};
        m = LOOSE_COUNT_RE.matcher(t);
        if (!m.find()) return new Object[]{cleanName(t), null};
        String g = m.group(1).replace('l', '1').replace('I', '1').replace('i', '1').replace('|', '1').replace('O', '0').replace('o', '0');
        return new Object[]{cleanName(t.substring(0, m.start())), digits(g)};
    }

    static Integer digits(String s) {
        String d = s.replaceAll("[^0-9]", "");
        if (d.isEmpty()) return null;
        try {
            return Integer.parseInt(d.length() > 9 ? d.substring(0, 9) : d);
        } catch (NumberFormatException e) {
            return null;
        }
    }

    static String nfkc(String s) {
        return Normalizer.normalize(ZERO_WIDTH.matcher(s == null ? "" : s).replaceAll(""), Normalizer.Form.NFKC).toLowerCase(Locale.ROOT);
    }

    /** 정규화 + 소문자 + 글자 · 숫자만 */
    static String norm(String s) {
        StringBuilder b = new StringBuilder();
        String n = nfkc(s);
        for (int i = 0; i < n.length(); i++) {
            char c = n.charAt(i);
            if (Character.isLetterOrDigit(c)) b.append(c);
        }
        return b.toString();
    }

    static List<String> words(String s) {
        List<String> out = new ArrayList<>();
        for (String w : nfkc(s).split("[^0-9a-z가-힣]+")) if (!w.isEmpty()) out.add(w);
        return out;
    }

    /** 0~100 (%) — 정규화 뒤 같으면 100 · 찾는 이름에 단어가 더 붙은 다른 아이템은 0 (정확 일치 원칙) · 아니면 비슷한 정도 */
    public static double similarity(String ocrName, String target) {
        String a = norm(ocrName), b = norm(target);
        if (a.isEmpty() || b.isEmpty()) return 0;
        if (a.equals(b)) return 100;
        List<String> ow = words(ocrName), tw = words(target);
        int n = tw.size();
        if (n > 0 && ow.size() > n) {
            for (int i = 0; i + n <= ow.size(); i++) if (ow.subList(i, i + n).equals(tw)) return 0;
        }
        return Math.round(2.0 * matched(a, 0, a.length(), b, 0, b.length()) / (a.length() + b.length()) * 1000) / 10.0;
    }

    /** 파이썬 SequenceMatcher 와 같은 방식: 가장 긴 공통 조각 → 양쪽 나머지에서 다시 (맞은 글자 수) */
    static int matched(String a, int alo, int ahi, String b, int blo, int bhi) {
        if (alo >= ahi || blo >= bhi) return 0;
        int bi = alo, bj = blo, bk = 0;
        int[] prev = new int[bhi - blo + 1];
        for (int i = alo; i < ahi; i++) {
            int[] cur = new int[bhi - blo + 1];
            for (int j = blo; j < bhi; j++) {
                if (a.charAt(i) == b.charAt(j)) {
                    int k = prev[j - blo] + 1;
                    cur[j - blo + 1] = k;
                    if (k > bk) {
                        bi = i - k + 1;
                        bj = j - k + 1;
                        bk = k;
                    }
                }
            }
            prev = cur;
        }
        if (bk == 0) return 0;
        return bk + matched(a, alo, bi, b, blo, bj) + matched(a, bi + bk, ahi, b, bj + bk, bhi);
    }
}
