package com.acrux.mobile.core;

import java.util.HashMap;
import java.util.Map;

/**
 * 기본 길 — 길을 직접 기록하지 않아도 기준 장소에서 낚시 장소 · 판매 장소로 걸어감 (assets/routes/default.txt)
 *  · 한 줄: "fish_vip=T:…" · "sell_free=T:…" (# 줄은 설명) — 앱의 [내 길 복사] 로 나온 글을 그대로 붙이면 됨
 *  · 길은 조이스틱을 얼마나 밀었는지 (반지름 대비 비율) 와 시각만 저장 → 화면 크기 · 기종이 달라도 같은 길
 *  · VIP 는 걸음이 빨라서 VIP 있음 · 없음 길이 따로 — 한쪽만 있으면 시간을 늘이거나 줄여서 씀 (덜 정확)
 *  · 직접 기록한 길이 있으면 늘 그게 먼저
 */
public final class DefaultRoutes {
    /** VIP 걸음 속도 / 일반 걸음 속도 (같은 길을 일반은 이만큼 더 오래 걸음) */
    public static final double VIP_SPEED = 1.25;

    static volatile Map<String, String> map = new HashMap<>();

    /**
     * 판매 장소 기본 길 (VIP+ 기준 · PC 테스트로 맞춘 값) — 기준 장소에서 Captain Flarg 앞까지 · 저장된 길이 있어도 이걸 씀
     *  한 줄 = {키, 초, 점프} · 키를 떼지 않고 이어서 밂 (조이스틱 손가락을 안 뗌) · 점프는 그 줄이 시작할 때 한 번
     *  마지막 W 는 NPC 앞 단에 막혀서 서 있는 시간까지 넉넉히 (조금 덜 가도 · 렉이 있어도 같은 자리)
     */
    public static final Object[][] SELL = {
            {"w", 1.6, false},
            {"aw", 0.2, false},
            {"aw", 0.375, true},
            {"w", 0.55, true},
            {"w", 0.7, true},
    };
    /** 점프 버튼을 누르고 있는 시간 (ms · 꾹 누르면 착지할 때마다 또 뜀) */
    static final double JUMP_MS = 150;

    /** {키, 초, 점프} 줄들 → 길 (조이스틱을 떼지 않고 방향만 바꿈 · 점프 줄은 조이스틱 + 점프를 같이 새로 누르고 점프만 먼저 뗌) */
    public static Route fromSteps(Object[][] steps) {
        Route r = new Route();
        double t = 0;
        boolean first = true;
        for (Object[] st : steps) {
            String k = (String) st[0];
            double sec = ((Number) st[1]).doubleValue();
            boolean jump = (Boolean) st[2];
            double[] v = Stick.dir(k.contains("w"), k.contains("a"), k.contains("s"), k.contains("d"));
            if (v == null || sec <= 0) continue;
            if (first || jump) {
                r.evs.add(new Route.Ev('F', t, v[0], v[1], true, jump));
                if (jump) r.evs.add(new Route.Ev('U', t + Math.min(JUMP_MS, sec * 1000), 0, 0, false, true));
            } else {
                r.evs.add(new Route.Ev('M', t, v[0], v[1], false, false));
            }
            first = false;
            t += sec * 1000;
        }
        if (!r.evs.isEmpty()) r.evs.add(new Route.Ev('X', t, 0, 0, false, false));
        return r;
    }

    /** 이 길은 저장된 길보다 기본 길을 먼저 씀 (판매 장소 · VIP 있음 = VIP+ 기준 값이 있을 때) */
    public static boolean forced(boolean sell, boolean vip) {
        return sell && vip;
    }

    private DefaultRoutes() {}

    /** "열쇠=값" 줄들 → 열쇠별 값 (# 줄 · 빈 줄 · = 없는 줄은 건너뜀) */
    public static Map<String, String> parse(String text) {
        Map<String, String> m = new HashMap<>();
        if (text == null) return m;
        for (String line : text.split("\r?\n")) {
            line = line.trim();
            if (line.isEmpty() || line.startsWith("#")) continue;
            int eq = line.indexOf('=');
            if (eq <= 0) continue;
            String k = line.substring(0, eq).trim().toLowerCase(java.util.Locale.ROOT), v = line.substring(eq + 1).trim();
            if (!v.isEmpty()) m.put(k, v);
        }
        return m;
    }

    public static void load(String text) {
        map = parse(text);
    }

    /** 기본 길 (없으면 빈 길) — assets 의 길이 있으면 그것 · 판매 장소는 없으면 SELL (VIP 없음은 시간을 늘려서 · 덜 정확) */
    public static Route get(boolean sell, boolean vip) {
        Route r = pick(map, sell, vip);
        if (!r.isEmpty() || !sell) return r;
        Route b = fromSteps(SELL);
        return vip ? b : b.scaled(VIP_SPEED);
    }

    public static boolean has(boolean sell) {
        return !get(sell, true).isEmpty();
    }

    /** 열쇠별 길에서 고름: 맞는 VIP 길 → 반대쪽 VIP 길을 시간만 바꿔서 → "fish" · "sell" (VIP 구분 없음) */
    public static Route pick(Map<String, String> m, boolean sell, boolean vip) {
        String n = sell ? "sell" : "fish";
        Route r = portable(m.get(n + (vip ? "_vip" : "_free")));
        if (!r.isEmpty()) return r;
        r = portable(m.get(n + (vip ? "_free" : "_vip")));
        if (!r.isEmpty()) return r.scaled(vip ? 1 / VIP_SPEED : VIP_SPEED);
        return portable(m.get(n));
    }

    /** 다른 기종에서도 쓸 수 있는 길만 (화면 픽셀로 잰 위치 길은 뺌) · 못 쓰면 빈 길 */
    static Route portable(String s) {
        Route r = Route.load(s);
        r.pts.clear();
        return r.moves() ? r : new Route();
    }
}
