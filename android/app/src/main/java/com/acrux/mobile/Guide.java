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
 *  · 기능 하나 = 안내 하나: 시작 준비 (권한) · 자동 낚시 (낚시 보정) · 이동 · 판매 (조이스틱 → 기준 장소 → 길 → 판매)
 *  · 저절로 뜨는 건 '시작 준비' 뿐 (권한이 없으면 아무것도 못 함) · 나머지는 그 기능을 쓰려고 할 때만
 *    (자동 낚시: 보정 없이 [시작] · 이동 · 판매: 보정 뒤 설정 없이 [시작]) — 자동 낚시는 이동 · 판매 설정까지 마쳐야 시작됨
 *  · 이미 해 둔 단계는 빼고 보여 줌 (해야 할 것만)
 *  · 단계마다 해야 할 것 (check) 이 있으면 그걸 해야 [다음] 이 눌림 · 하면 저절로 다음 단계 · 선택 단계만 [이 단계 건너뛰기]
 *  · 첫 단계에서만 [나중에] (이번 실행 동안 다시 안 띄움 · 홈 → 사용법 → 설정 안내에서 언제든)
 *  · 앱 화면에서는 앱 위에 · 로블록스 화면에서는 떠 있는 Acrux 창 옆에 같은 안내가 뜸 · 눌러야 할 곳 (손잡이 · 기능 줄 · 항목 등) 을 파랗게 강조
 */
final class Guide {
    private Guide() {}

    static final class Step {
        String title, body;
        /** 떠 있는 Acrux 창에서 강조할 곳 (run · pos:위치 · act:실행) */
        String target;
        /** 그 항목이 있는 미니 UI 페이지 (다른 페이지면 그 페이지로 가는 기능 줄 · ‹ 를 먼저 강조) */
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

    /** 이동 · 판매에 아직 안 한 설정이 있나 (자동 낚시를 하려면 전부 필요) */
    static boolean moveNeeded() {
        return Hub.stick == null || !baseReady() || Hub.fishRoute.isEmpty() || !sellReady() || Hub.sellRoute.isEmpty();
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
                        "· 로블록스 (Sol's RNG) 를 가로 전체 화면으로 실행합니다.\n· 로블록스 설정 → 이동 방식을 '조이스틱' (클래식) 으로 바꿉니다.\n\n로블록스 화면 오른쪽 가운데에 Acrux 손잡이가 표시됩니다. 누르거나 왼쪽으로 밀면 오른쪽에서 Acrux 창이 나오고, 이 안내도 그 옆에 이어서 표시됩니다."))
                .add(new Step("준비 완료", "이제 로블록스를 열고 Acrux 창의 [시작] 을 눌러 주세요.\n\n기능마다 처음 쓸 때 필요한 설정 안내가 그때 표시됩니다.")));

        l.add(new Tut("fish", "자동 낚시", () -> !Hub.cfg.ready())
                .add(new Step("낚시 보정",
                        "낚시 창 · 미니게임 창 · 결과창의 위치를 이 기기 화면에 맞춥니다. 처음 한 번만 하면 됩니다.").later())
                .add(new Step("낚시 자리로 이동",
                        "로블록스에서 낚시 자리로 이동해 Fish 버튼이 보이게 해 주세요.\n\n보정이나 매크로를 시작하면 Acrux 창은 자동으로 닫힙니다."))
                .add(new Step("낚시 자동 보정",
                        "Acrux 창 → [낚시] → 강조된 '낚시 자동 보정' 을 누르고 끝날 때까지 기다려 주세요. Fish 를 한 번 던져 창 위치를 자동으로 맞춥니다.\n\n보정하는 동안에는 이 안내가 잠시 숨겨집니다.")
                        .at("fish", "act:cal").need(() -> Hub.cfg.ready()))
                .add(new Step("낚시 보정 완료",
                        "다음은 이동 · 판매 설정입니다. 자동 낚시는 낚시 장소로 이동하고, 가득 차면 팔고 돌아오기 때문에 이 설정까지 마쳐야 시작됩니다.\n\nAcrux 창의 [시작] 을 누르면 남은 설정 안내가 이어서 표시됩니다.").at(null, "run")));

        boolean sell = true;                                   // 자동 낚시는 판매까지 항상 (V1.4.13)
        Tut mv = new Tut("move", "이동 · 판매", Guide::moveNeeded)
                .add(new Step("이동 · 판매 설정",
                        "스폰 위치가 무작위이기 때문에, 리셋 후 '기준 장소' 에서 기록한 길을 따라 이동합니다."
                                + (sell ? " 인벤토리가 가득 차면 판매 장소로 이동해 판매한 뒤 돌아옵니다." : "")
                                + "\n\n아직 안 한 것만 차례로 안내합니다.").later())
                .add(new Step("조이스틱 찾기",
                        "Acrux 창 → [조이스틱] → 강조된 '조이스틱 찾기' 를 눌러 주세요.\n\n로블록스 이동 방식이 '조이스틱' 이어야 합니다. 자동으로 찾지 못하면 '조이스틱 직접 지정' 을 사용할 수 있습니다.")
                        .at("stick", "act:stickfind").need(() -> Hub.stick != null))
                .add(new Step("점프 버튼 위치",
                        "길 기록에서 점프를 쓰려면 설정해 주세요. 위치를 정할 때는 십자 포인터가 표시되며, 화면을 끌면 그만큼 움직이고 [확인] 으로 저장됩니다.\n\n점프를 쓰지 않는다면 건너뛰어도 됩니다.")
                        .at("stick", "pos:jump_btn").need(() -> has("jump_btn")).opt());
        for (String[] k : Mover.BASE_KEYS) {
            mv.add(new Step("'" + k[1] + "' 위치",
                    "[기준 장소] → 강조된 항목을 누르고, 십자 포인터를 해당 버튼 위에 맞춘 뒤 [확인] 을 눌러 주세요.\n\n메뉴 · 재설정 화면을 직접 열어 둔 상태에서 맞추면 더 정확합니다.")
                    .at("base", "pos:" + k[0]).need(() -> has(k[0])));
        }
        mv.add(new Step("낚시 장소 길 기록",
                "[낚시] → 낚시 장소 '길 기록' 을 눌러 주세요.\n\n기준 장소로 이동한 뒤 WASD 패드가 표시됩니다.\n· W A S D 탭 = 켜기 · 다시 탭 = 끄기 · 모서리 = 대각선 · ■ = 멈춤\n· 점프 탭 = 한 번 · [홀드] = 계속 누름\n\n도착하면 Acrux 창의 [도착] 을 눌러 주세요.")
                .at("fish", "act:rec_fish").need(() -> !Hub.fishRoute.isEmpty()));
        if (sell) {
            mv.add(new Step("판매 장소 길 기록",
                            "[낚시] → 물고기 판매 장소 '길 기록' 을 누른 뒤, 기준 장소에서 Captain Flarg 앞까지 WASD 패드로 이동하고 [도착] 을 눌러 주세요.")
                            .at("fish", "act:rec_sell").need(() -> !Hub.sellRoute.isEmpty()))
                    .add(new Step("대화 버튼 위치",
                            "판매 장소에 선 상태로 [낚시] → 강조된 '대화 버튼' 을 누르고, 포인터를 화면의 대화 버튼에 맞춘 뒤 [확인] 을 눌러 주세요.")
                            .at("fish", "pos:talk_btn").need(() -> has("talk_btn")))
                    .add(new Step("판매 자동 보정",
                            "[낚시] → '판매 자동 보정' 을 누른 뒤, Captain Flarg 앞에서 대화 버튼을 직접 눌러 주세요. 대화창 · 상점 · 확인창의 위치를 자동으로 찾습니다.\n\n확인창 위치까지 찾으려면 물고기를 가진 상태에서 진행해 주세요.")
                            .at("fish", "act:sellcal").need(Guide::sellReady))
                    .add(new Step("판매 테스트",
                            "[낚시] → '판매 한 번 해 보기' 로 판매가 끝까지 되는지 확인할 수 있습니다. 건너뛰어도 됩니다.")
                            .at("fish", "act:sell").opt());
        }
        mv.add(new Step("이동 · 판매 설정 완료",
                "[시작] 을 누르면 기준 장소 → 낚시 장소로 이동한 뒤 낚시를 시작합니다." + (sell ? " 인벤토리가 가득 차면 판매하고 돌아옵니다." : "")
                        + "\n\n[기준 장소] → '기준 장소 → 낚시 장소' 로 미리 확인할 수 있습니다."));
        l.add(mv);
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

    static Step step() {
        return cur == null ? null : cur.steps.get(idx);
    }

    static void start(String id) {
        Tut t = find(id);
        if (t == null) return;
        // 이미 해 둔 단계는 뺌 (처음 · 마지막 안내는 그대로) — 다 해 뒀으면 그대로 보여 줌 (다시 보기)
        List<Step> todo = new ArrayList<>();
        for (int i = 0; i < t.steps.size(); i++) {
            Step st = t.steps.get(i);
            boolean edge = i == 0 || i == t.steps.size() - 1;
            if (edge || st.check == null || !st.check.getAsBoolean()) todo.add(st);
        }
        boolean anyTodo = false;
        for (Step st : todo) if (st.check != null) anyTodo = true;
        if (anyTodo) {
            t.steps.clear();
            t.steps.addAll(todo);
        }
        cur = t;
        later.remove(id);
        go(0);
        tick();
    }

    /** 앱을 켤 때: 권한이 없으면 '시작 준비' 만 (다른 기능 안내는 그 기능을 쓰려고 할 때) */
    static void auto() {
        if (cur != null || later.contains("ready")) return;
        Tut t = find("ready");
        if (t != null && t.needed.getAsBoolean()) start("ready");
    }

    /** 기능을 쓰려고 할 때 (설정 없이 [시작]) — 그 기능에 덜 된 설정이 있으면 그 안내 */
    static void featureOn(String id) {
        if (cur != null && cur.id.equals(id)) return;
        Tut t = find(id);
        if (t == null || !t.needed.getAsBoolean()) return;
        if (cur != null) cur = null;                   // 다른 안내가 떠 있으면 지금 쓰려는 기능 안내로 바꿈
        start(id);
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
        cur = null;                   // (다음 안내로 줄줄이 넘어가지 않음 — 기능마다 쓸 때 뜸)
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
}
