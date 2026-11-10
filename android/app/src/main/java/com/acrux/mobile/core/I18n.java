package com.acrux.mobile.core;

import java.io.BufferedReader;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * 언어 (한국어 / English / 日本語) — PC 버전 web/i18n.js 와 같은 방식
 *  · 화면에 나오는 한국어 글자를 그대로 열쇠로 써서 번역 (화면 코드는 한국어로 그대로 둠)
 *  · 숫자 · 이름이 들어가는 글자는 {0} {1} … 자리로 (사전: "새 버전 V{0} 있음") — 자리에 들어간 글자도 다시 번역
 *  · 사전에 없는 글자는 ' · ', ' — ', ' → ', ' + ', ': ' 로 나눠서 조각마다 번역 (끝까지 없으면 한국어 그대로)
 *  · 여러 줄은 줄마다 · 앞뒤 공백은 그대로
 * 사전: 한국어 \t English \t 日本語 (한 줄에 하나 · # 은 주석) — PC 사전(pc.tsv, 빌드 때 i18n.js 에서 뽑음) + 모바일 사전(mobile.tsv)
 */
public final class I18n {
    private I18n() {}

    public static final String[] CODES = {"ko", "en", "ja"};
    public static final String[] NAMES = {"한국어", "English", "日本語"};

    static volatile int lang = 0;
    static final Map<String, String[]> exact = new HashMap<>();
    static final List<Tpl> tpls = new ArrayList<>();
    static final Map<String, String>[] cache = newCaches();
    static final Pattern HANGUL = Pattern.compile("[가-힣]");
    static final String[] SEPS = {" — ", " → ", "  ·  ", " · ", " + ", ": ", " / ", ", "};

    @SuppressWarnings("unchecked")
    static Map<String, String>[] newCaches() {
        Map<String, String>[] m = new Map[CODES.length];
        for (int i = 0; i < m.length; i++) m[i] = new ConcurrentHashMap<>();
        return m;
    }

    static final class Tpl {
        final Pattern re;
        final String[] to;
        final int slots;

        Tpl(Pattern re, String[] to, int slots) {
            this.re = re;
            this.to = to;
            this.slots = slots;
        }
    }

    // ---------------------------------------------------------------- 사전
    public static synchronized void add(String ko, String en, String ja) {
        if (ko == null || ko.isEmpty()) return;
        String[] to = {ko, en == null || en.isEmpty() ? ko : en, ja == null || ja.isEmpty() ? (en == null || en.isEmpty() ? ko : en) : ja};
        if (ko.matches("(?s).*\\{\\d\\}.*")) {
            StringBuilder re = new StringBuilder("^");
            Matcher m = Pattern.compile("\\{(\\d)\\}").matcher(ko);
            int last = 0, slots = 0;
            while (m.find()) {
                re.append(Pattern.quote(ko.substring(last, m.start()))).append("(.+?)");
                last = m.end();
                slots = Math.max(slots, Integer.parseInt(m.group(1)) + 1);
            }
            re.append(Pattern.quote(ko.substring(last))).append("$");
            tpls.removeIf(t -> t.to[0].equals(ko));
            tpls.add(new Tpl(Pattern.compile(re.toString(), Pattern.DOTALL), to, slots));
            // 고정 글자가 긴 것부터 (더 구체적인 것이 먼저 맞게)
            tpls.sort((a, b) -> Integer.compare(b.to[0].replaceAll("\\{\\d\\}", "").length(), a.to[0].replaceAll("\\{\\d\\}", "").length()));
        } else {
            exact.put(ko, to);
        }
        for (Map<String, String> c : cache) c.clear();
    }

    /** 한국어 \t English \t 日本語 · # 주석 · \n 은 줄바꿈 */
    public static void load(InputStream in) {
        if (in == null) return;
        try (BufferedReader r = new BufferedReader(new InputStreamReader(in, StandardCharsets.UTF_8))) {
            String line;
            while ((line = r.readLine()) != null) {
                if (line.isEmpty() || line.startsWith("#")) continue;
                String[] p = line.split("\t", -1);
                if (p.length < 2) continue;
                add(un(p[0]), un(p[1]), p.length > 2 ? un(p[2]) : null);
            }
        } catch (Exception ignored) {
        }
    }

    static String un(String s) {
        return s.replace("\\n", "\n").replace("\\t", "\t");
    }

    public static int size() {
        return exact.size() + tpls.size();
    }

    // ---------------------------------------------------------------- 언어
    public static String lang() {
        return CODES[lang];
    }

    public static int index() {
        return lang;
    }

    public static void setLang(String code) {
        for (int i = 0; i < CODES.length; i++) if (CODES[i].equals(code)) lang = i;
    }

    /** 기기 언어 → 처음 켰을 때 쓸 언어 (한국어 · 일본어 · 그 밖엔 영어) */
    public static String guess(String deviceLang) {
        if (deviceLang == null) return "ko";
        if (deviceLang.startsWith("ko")) return "ko";
        if (deviceLang.startsWith("ja")) return "ja";
        return "en";
    }

    // ---------------------------------------------------------------- 번역
    public static String t(String s) {
        int L = lang;
        if (L == 0 || s == null || s.isEmpty() || !HANGUL.matcher(s).find()) return s;
        String hit = cache[L].get(s);
        if (hit != null) return hit;
        String out = lines(s, L);
        if (cache[L].size() > 4000) cache[L].clear();
        cache[L].put(s, out);
        return out;
    }

    /** 사전 글자(자리 {0} …)를 번역한 뒤 자리에 값을 넣음 */
    public static String f(String template, Object... args) {
        String s = lang == 0 ? template : t(template);
        for (int i = 0; i < args.length; i++) s = s.replace("{" + i + "}", String.valueOf(args[i]));
        return s;
    }

    static String lines(String s, int L) {
        if (s.indexOf('\n') < 0) return one(s, L);
        String[] parts = s.split("\n", -1);
        StringBuilder b = new StringBuilder();
        for (int i = 0; i < parts.length; i++) {
            if (i > 0) b.append('\n');
            b.append(one(parts[i], L));
        }
        return b.toString();
    }

    /** 앞뒤 공백은 두고 가운데만 */
    static String one(String s, int L) {
        int a = 0, z = s.length();
        while (a < z && Character.isWhitespace(s.charAt(a))) a++;
        while (z > a && Character.isWhitespace(s.charAt(z - 1))) z--;
        if (a >= z) return s;
        String core = s.substring(a, z);
        String tr = core(core, L, 0);
        return s.substring(0, a) + (tr != null ? tr : core) + s.substring(z);
    }

    /** 번역 → 없으면 null */
    static String core(String s, int L, int depth) {
        if (!HANGUL.matcher(s).find()) return s;
        String[] e = exact.get(s);
        if (e != null) return e[L];
        for (Tpl tp : tpls) {
            Matcher m = tp.re.matcher(s);
            if (!m.matches()) continue;
            String out = tp.to[L];
            for (int i = 0; i < tp.slots && i < m.groupCount(); i++) {
                String g = m.group(i + 1), tg = depth < 4 ? core(g, L, depth + 1) : null;
                out = out.replace("{" + i + "}", tg != null ? tg : g);
            }
            return out;
        }
        if (depth >= 6) return null;
        for (String sep : SEPS) {
            int k = s.indexOf(sep);
            if (k <= 0) continue;
            String l = s.substring(0, k), r = s.substring(k + sep.length());
            String tl = one2(l, L, depth), tr = one2(r, L, depth);
            if (tl == null && tr == null) continue;
            return (tl != null ? tl : l) + sep + (tr != null ? tr : r);
        }
        // 끝에 붙은 괄호 설명: "글자 (설명)"
        if (s.endsWith(")")) {
            int k = s.lastIndexOf(" (");
            if (k > 0) {
                String l = s.substring(0, k), r = s.substring(k + 2, s.length() - 1);
                String tl = one2(l, L, depth), tr = one2(r, L, depth);
                if (tl != null || tr != null) return (tl != null ? tl : l) + " (" + (tr != null ? tr : r) + ")";
            }
        }
        return null;
    }

    static String one2(String s, int L, int depth) {
        String t = s.trim();
        if (t.isEmpty()) return null;
        String tr = core(t, L, depth + 1);
        return tr == null ? null : s.replace(t, tr);
    }
}
