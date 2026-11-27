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
     * 판매 장소 기본 길 (VIP+ 기준 · PC 테스트 route_test.py 로 맞춘 값과 같은 순서 · 같은 시간) — 기준 장소에서 Captain Flarg 앞까지
     *  한 줄 = {키, 초, 점프} · 저장된 길이 있어도 이걸 씀 (시간을 늘이거나 줄이지 않음)
     *  줄마다: (첫 줄이 아니면) 손가락을 다 떼고 LINE_GAP 쉼 → (점프 줄이면) 점프 버튼만 JUMP_TAP 누르고 떼고 JUMP_AFTER 쉼 → 조이스틱을 그 방향으로 초 만큼
     */
    public static final Object[][] SELL = {
            {"w", 1.6, false},
            {"aw", 0.2, false},
            {"aw", 0.375, true},
            {"w", 0.55, true},
            {"w", 0.5, true},
    };
    /** 테스트 설정의 '길 줄 사이 대기' · '점프 누르는 시간' · '점프 후 대기' (ms) */
    static final double LINE_GAP = 100, JUMP_TAP = 80, JUMP_AFTER = 100;
    /** 조이스틱을 새로 누를 때 가운데 → 끝까지 끄는 동안 덜 걷는 시간 (ms · Pilot.RAMP_LOSS_MS) — PC 키는 바로 끝까지라 그만큼 더 누름 */
    static final double STICK_RAMP = 33;

    /** {키, 초, 점프} 줄들 → 길 (PC 테스트와 같은 순서 · 같은 시각) */
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
            if (!first) {
                r.evs.add(new Route.Ev('X', t, 0, 0, false, false));          // 손가락을 다 떼고
                t += LINE_GAP;                                                // 잠깐 섬
            }
            if (jump) {
                r.evs.add(new Route.Ev('F', t, 0, 0, false, true));           // 점프 버튼만 누름
                r.evs.add(new Route.Ev('U', t + JUMP_TAP, 0, 0, false, true));
                t += JUMP_TAP + JUMP_AFTER;                                   // 뛰고 나서 공중에서 이동 시작
            }
            r.evs.add(new Route.Ev('F', t, v[0], v[1], true, false));
            first = false;
            t += sec * 1000 + STICK_RAMP;
        }
        if (!r.evs.isEmpty()) r.evs.add(new Route.Ev('X', t, 0, 0, false, false));
        return r;
    }

    /** 이 길은 저장된 길보다 기본 길을 먼저 씀 (판매 장소 · VIP+ 기준) */
    public static boolean forced(boolean sell, boolean vip) {
        return sell;
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

    /** 기본 길 (없으면 빈 길) — assets 의 길이 있으면 그것 · 판매 장소는 없으면 SELL */
    public static Route get(boolean sell, boolean vip) {
        Route r = pick(map, sell, vip);
        if (!r.isEmpty() || !sell) return r;
        return fromSteps(SELL);                                               // VIP+ 기준 값 그대로
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
