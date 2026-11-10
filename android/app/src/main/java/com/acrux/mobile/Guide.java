package com.acrux.mobile;

import com.acrux.mobile.core.Mover;
import com.acrux.mobile.core.Seller;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.BooleanSupplier;

/**
 * 기능별 설정 안내 (PC 튜토리얼과 같은 방식)
 *  · 시작 준비 · 낚시 보정 · 이동 설정 · 판매 설정 — 필요한 것만 차례로 (하나 끝나면 다음 것)
 *  · 단계마다 해야 할 것 (check) 이 있으면 그걸 해야 [다음] 이 눌림 · 하면 저절로 다음 단계 · 선택 단계만 [이 단계 건너뛰기]
 *  · 첫 단계에서만 [나중에] (이번 실행 동안 다시 안 띄움 · 홈 → 사용법 → 설정 안내에서 언제든)
 *  · 앱 화면에서는 앱 위에 · 로블록스 화면에서는 떠 있는 Acrux 창 옆에 같은 안내가 뜸 · 눌러야 할 곳 (⚙ · 분류 · 항목 등) 을 파랗게 강조
 */
final class Guide {
    private Guide() {}

    static final class Step {
        String title, body;
        /** 떠 있는 Acrux 창에서 강조할 곳 (gear · cal · run · cat:분류 · pos:위치 · act:실행) */
        String target;
        /** 그 항목이 있는 ⚙ 분류 (메뉴가 닫혀 있으면 ⚙ · 다른 분류면 그 분류를 먼저 강조) */
        String cat;
        /** 해야 할 것 · 됐으면 true (null = 읽기만) */
        BooleanSupplier check;
        /** 앱에서 바로 하기 (acc · cap) */
        String action;
        boolean optional, later;

        Step(String title, String body) {
            this.title = title;
            this.body = body;
        }

        Step at(String cat, String target) {
            this.cat = cat;
            this.target = target;
            return this;
        }

        Step need(BooleanSupplier c) {
            check = c;
            return this;
        }

        Step act(String a) {
            action = a;
            return this;
        }

        Step opt() {
            optional = true;
            return this;
        }

        Step later() {
            later = true;
            return this;
        }
    }

    static final class Tut {
        final String id, name;
        final BooleanSupplier needed;
        final List<Step> steps = new ArrayList<>();

        Tut(String id, String name, BooleanSupplier needed) {
            this.id = id;
            this.name = name;
            this.needed = needed;
        }

        Tut add(Step s) {
            steps.add(s);
            return this;
        }
    }

    static boolean has(String k) {
        return Hub.pos.containsKey(k);
    }

    static boolean baseReady() {
        for (String[] k : Mover.BASE_KEYS) if (!has(k[0])) return false;
        return true;
    }

    static boolean sellReady() {
        for (String[] k : Seller.KEYS) if (!has(k[0])) return false;
        for (String[] k : Seller.TALK) if (!has(k[0])) return false;
        return true;
    }

    static List<Tut> all() {
        List<Tut> l = new ArrayList<>();
        l.add(new Tut("ready", "시작 준비", () -> TapService.instance == null || CaptureService.active == null)
                .add(new Step("Acrux Mobile 에 오신 걸 환영합니다.",
                        "이 매크로는 Sol's RNG 전용 매크로이며, 다양한 모바일 기능을 지원합니다.\n\n시작하기 전에, 매크로가 작동할 때 필요한 필수 권한 두 개를 켜야 합니다.").later())
                .add(new Step("접근성 권한 켜기",
                        "매크로가 로블록스 화면을 터치하는 데 필요한 권한입니다.\n\n설정 → 접근성 → 설치된 앱 → Acrux Mobile → 켜기\n\n'제한된 설정' 으로 켜지지 않는다면, 설정 → 앱 → Acrux Mobile → 오른쪽 위 ⋮ → '제한된 설정 허용' 후 다시 켜 주세요.")
                        .act("acc").need(() -> TapService.instance != null))
                .add(new Step("화면 캡처 시작",
                        "매크로가 화면을 보고 낚시 상태와 버튼을 찾는 데 필요합니다. 화면은 녹화되거나 저장되지 않습니다.\n\n공유 창이 뜨면 '전체 화면' 을 선택해 주세요.")
                        .act("cap").need(() -> CaptureService.active != null))
                .add(new Step("로블록스 준비",
                        "· 로블록스 (Sol's RNG) 를 가로 전체 화면으로 실행합니다.\n· 로블록스 설정 → 이동 방식을 '조이스틱' (클래식) 으로 바꿉니다.\n\n로블록스 위에는 작은 Acrux 창이 표시되며, 끌어서 옮길 수 있습니다. 이 안내도 그 옆에 이어서 표시됩니다."))
                .add(new Step("준비 완료", "이제 로블록스를 열어 주세요. 다음 안내 (낚시 보정) 가 이어서 표시됩니다.")));

        l.add(new Tut("fish", "낚시 보정", () -> !Hub.cfg.ready())
                .add(new Step("낚시 보정",
                        "낚시 창 · 미니게임 창 · 결과창의 위치를 이 기기 화면에 맞춥니다. 처음 한 번만 하면 됩니다.").later())
                .add(new Step("낚시 자리로 이동",
                        "로블록스에서 낚시 자리로 이동해 Fish 버튼이 보이게 해 주세요.\n\nAcrux 창이 낚시 창을 가린다면 위쪽 구석으로 옮겨 주세요."))
                .add(new Step("낚시 자동 보정",
                        "Acrux 창 ⚙ → [낚시] → 강조된 '낚시 자동 보정' 을 누르고 끝날 때까지 기다려 주세요. Fish 를 한 번 던져 창 위치를 자동으로 맞춥니다.\n\n보정하는 동안에는 이 안내가 잠시 숨겨집니다.")
                        .at("fish", "act:cal").need(() -> Hub.cfg.ready()))
                .add(new Step("낚시 보정 완료",
                        "Acrux 창의 [시작] 을 누르면 자동 낚시가 시작되며, [정지] 로 멈출 수 있습니다.\n\n가득 찼을 때 판매까지 하려면 앱의 낚시 칸 → 이동 · 판매에서 켜 주세요.").at(null, "run")));

        Tut mv = new Tut("move", "이동 설정", () -> Hub.useMove && (Hub.stick == null || !baseReady() || Hub.fishRoute.isEmpty()))
                .add(new Step("이동 설정",
                        "스폰 위치가 무작위이기 때문에, 리셋 후 '기준 장소' 에서 기록한 길을 따라 이동합니다.\n\n조이스틱 → 기준 장소 위치 → 낚시 장소 길 순서로 설정합니다.").later())
                .add(new Step("조이스틱 찾기",
                        "Acrux 창 ⚙ → [조이스틱] → 강조된 '조이스틱 찾기' 를 눌러 주세요.\n\n로블록스 이동 방식이 '조이스틱' 이어야 합니다. 자동으로 찾지 못하면 '조이스틱 직접 지정' 을 사용할 수 있습니다.")
                        .at("stick", "act:stickfind").need(() -> Hub.stick != null))
                .add(new Step("점프 버튼 위치",
                        "길 기록에서 점프를 쓰려면 설정해 주세요. 위치를 정할 때는 십자 포인터가 표시되며, 화면을 끌면 그만큼 움직이고 [확인] 으로 저장됩니다.\n\n점프를 쓰지 않는다면 건너뛰어도 됩니다.")
                        .at("stick", "pos:jump_btn").need(() -> has("jump_btn")).opt());
        for (String[] k : Mover.BASE_KEYS) {
            mv.add(new Step("'" + k[1] + "' 위치",
                    "⚙ → [기준 장소] → 강조된 항목을 누르고, 십자 포인터를 해당 버튼 위에 맞춘 뒤 [확인] 을 눌러 주세요.\n\n메뉴 · 재설정 화면을 직접 열어 둔 상태에서 맞추면 더 정확합니다.")
                    .at("base", "pos:" + k[0]).need(() -> has(k[0])));
        }
        mv.add(new Step("낚시 장소 길 기록",
                        "⚙ → [길] → 낚시 장소 '길 기록' 을 눌러 주세요.\n\n기준 장소로 이동한 뒤 WASD 패드가 표시됩니다.\n· W A S D 탭 = 켜기 · 다시 탭 = 끄기 · 모서리 = 대각선 · ■ = 멈춤\n· 점프 탭 = 한 번 · [홀드] = 계속 누름\n\n도착하면 Acrux 창의 [도착] 을 눌러 주세요.")
                        .at("route", "act:rec_fish").need(() -> !Hub.fishRoute.isEmpty()))
                .add(new Step("이동 설정 완료",
                        "[시작] 을 누르면 기준 장소 → 낚시 장소로 이동한 뒤 낚시를 시작합니다.\n\n⚙ → [길] → '기준 장소 → 낚시 장소' 로 미리 확인할 수 있습니다."));
        l.add(mv);

        l.add(new Tut("sell", "판매 설정", () -> Hub.useSell && (!sellReady() || Hub.sellRoute.isEmpty()))
                .add(new Step("판매 설정",
                        "인벤토리가 가득 차면 판매 장소로 이동해 물고기를 판매한 뒤 돌아옵니다.\n\n판매 장소 길 → 대화 버튼 → 판매 자동 보정 순서로 설정합니다. [ 이동 설정이 먼저 필요합니다 ]").later())
                .add(new Step("판매 장소 길 기록",
                        "⚙ → [길] → 물고기 판매 장소 '길 기록' 을 누른 뒤, 기준 장소에서 Captain Flarg 앞까지 WASD 패드로 이동하고 [도착] 을 눌러 주세요.")
                        .at("route", "act:rec_sell").need(() -> !Hub.sellRoute.isEmpty()))
                .add(new Step("대화 버튼 위치",
                        "판매 장소에 선 상태로 ⚙ → [판매] → 강조된 '대화 버튼' 을 누르고, 포인터를 화면의 대화 버튼에 맞춘 뒤 [확인] 을 눌러 주세요.")
                        .at("sell", "pos:talk_btn").need(() -> has("talk_btn")))
                .add(new Step("판매 자동 보정",
                        "⚙ → [판매] → '판매 자동 보정' 을 누른 뒤, Captain Flarg 앞에서 대화 버튼을 직접 눌러 주세요. 대화창 · 상점 · 확인창의 위치를 자동으로 찾습니다.\n\n확인창 위치까지 찾으려면 물고기를 가진 상태에서 진행해 주세요.")
                        .at("sell", "act:sellcal").need(Guide::sellReady))
                .add(new Step("판매 테스트",
                        "⚙ → [판매] → '판매 한 번 해 보기' 로 판매가 끝까지 되는지 확인할 수 있습니다. 건너뛰어도 됩니다.")
                        .at("sell", "act:sell").opt())
                .add(new Step("판매 설정 완료", "[시작] 을 누르면 낚시 중 인벤토리가 가득 찼을 때 판매하고 돌아옵니다.")));
        return l;
    }

    // ---- 상태
    static Tut cur;
    static int idx;
    /** 이 단계에 왔을 때 이미 돼 있었나 (그러면 저절로 넘기지 않음 · 보여 주기만) */
    static boolean wasOk;
    /** [나중에] 누른 것 (이번 실행 동안) */
    static final Set<String> later = new HashSet<>();
    static final List<Runnable> views = new CopyOnWriteArrayList<>();
    static boolean ticking;

    static Tut find(String id) {
        for (Tut t : all()) if (t.id.equals(id)) return t;
        return null;
    }

    static Set<String> done() {
        return new HashSet<>(Hub.prefs().getStringSet("guideDone", new HashSet<>()));
    }

    static void markDone(String id) {
        Set<String> d = done();
        d.add(id);
        Hub.prefs().edit().putStringSet("guideDone", d).apply();
    }

    /** 지금 필요한데 아직 안 한 안내 (나중에 · 다 한 것 · 지금 것 빼고) */
    static int remaining() {
        int n = 0;
        Set<String> d = done();
        for (Tut t : all()) {
            if (cur != null && t.id.equals(cur.id)) continue;
            if (!d.contains(t.id) && t.needed.getAsBoolean()) n++;
        }
        return n;
    }

    static Step step() {
        return cur == null ? null : cur.steps.get(idx);
    }

    static void start(String id) {
        Tut t = find(id);
        if (t == null) return;
        cur = t;
        later.remove(id);
        go(0);
        tick();
    }

    /** 앱을 켤 때 · 기능을 켤 때: 필요한 안내가 있으면 띄움 */
    static void auto() {
        if (cur != null) return;
        Set<String> d = done();
        for (Tut t : all()) {
            if (!d.contains(t.id) && !later.contains(t.id) && t.needed.getAsBoolean()) {
                start(t.id);
                return;
            }
        }
    }

    /** 기능 스위치를 켰을 때 (이동 · 판매) — 설정이 덜 돼 있으면 그 안내 */
    static void featureOn(String id) {
        if (cur != null) return;
        Tut t = find(id);
        if (t != null && t.needed.getAsBoolean()) start(id);
    }

    static void go(int i) {
        if (cur == null) return;
        idx = Math.max(0, Math.min(cur.steps.size() - 1, i));
        Step s = step();
        wasOk = s.check != null && s.check.getAsBoolean();
        redraw();
    }

    static boolean canNext() {
        Step s = step();
        return s != null && (s.check == null || s.check.getAsBoolean());
    }

    static void next() {
        if (cur == null || !canNext()) return;
        if (idx >= cur.steps.size() - 1) finish();
        else go(idx + 1);
    }

    static void prev() {
        if (cur != null && idx > 0) go(idx - 1);
    }

    static void skipStep() {
        Step s = step();
        if (s == null || !s.optional) return;
        if (idx >= cur.steps.size() - 1) finish();
        else go(idx + 1);
    }

    /** [나중에] · 뒤로 가기 */
    static void postpone() {
        if (cur == null) return;
        later.add(cur.id);
        cur = null;
        redraw();
    }

    static void finish() {
        if (cur == null) return;
        markDone(cur.id);
        cur = null;
        auto();                       // 다음 필요한 안내로 이어서
        redraw();
    }

    /** 0.4초마다: 해야 할 것을 했으면 다음 단계로 · 화면 갱신 */
    static void tick() {
        if (ticking) return;
        ticking = true;
        Hub.main.postDelayed(Guide::loop, 400);
    }

    static void loop() {
        if (cur == null) {
            ticking = false;
            redraw();
            return;
        }
        Step s = step();
        if (s.check != null) {
            boolean ok = s.check.getAsBoolean();
            if (ok && !wasOk) {
                wasOk = true;
                next();                // 했으면 저절로 다음 단계
            } else {
                wasOk = ok;
            }
        }
        redraw();
        Hub.main.postDelayed(Guide::loop, 400);
    }

    static void redraw() {
        for (Runnable r : views) r.run();
    }

    /** 떠 있는 Acrux 창에서 지금 강조할 곳 — 메뉴가 닫혀 있으면 ⚙ · 다른 분류면 그 분류 · 아니면 그 항목 */
    static String target(boolean menuOpen, String menuCat) {
        Step s = step();
        if (s == null || s.target == null) return null;
        if (s.cat == null) return s.target;
        if (!menuOpen) return "gear";
        if (!s.cat.equals(menuCat)) return "cat:" + s.cat;
        return s.target;
    }
}
