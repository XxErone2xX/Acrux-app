package com.acrux.mobile;

import android.view.Gravity;
import android.view.View;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

/**
 * 튜토리얼 — 처음 켰을 때 한 번 (홈 → 사용법 → 튜토리얼 다시 보기 로 언제든)
 *  · 앱 화면 위에 카드 하나 · 단계마다 제목 · 설명 · (권한 단계는) 바로 하기 버튼 + 지금 상태
 *  · [이전] [다음] · [건너뛰기] · 마지막 단계에서 [시작하기] → 다 봤다고 저장
 */
final class Tutorial {
    /** {제목, 설명, 바로 하기 (acc · cap · 없음)} */
    static final String[][] STEPS = {
            {"Acrux Mobile 에 오신 걸 환영해요",
                    "Sol's RNG 자동 낚시 매크로예요.\n\n· 낚시: 던지기 · 입질 기다리기 · 미니게임 · 결과창 닫기를 자동으로\n· 인벤토리가 가득 차면 판매 장소로 가서 팔고 낚시 장소로 돌아옴\n· 루팅 · Shizuku 필요 없음 (전부 화면 터치)\n\n몇 단계만 따라 하면 바로 쓸 수 있어요.", ""},
            {"① 접근성 권한 켜기",
                    "Acrux 가 로블록스 화면을 대신 터치하려면 필요해요.\n\n설정 → 접근성 → 설치된 앱 → Acrux Mobile → 켜기\n\n안드로이드 13 이상에서 '제한된 설정' 이라며 안 켜지면: 설정 → 앱 → Acrux Mobile → 오른쪽 위 ⋮ → '제한된 설정 허용' 후 다시 켜기", "acc"},
            {"② 화면 캡처 시작",
                    "화면 색 · 글자를 보고 낚시 상태 · 버튼을 찾아요. 녹화 · 저장은 하지 않아요.\n\n공유 창이 뜨면 '전체 화면' 을 골라 주세요.", "cap"},
            {"③ 로블록스 준비",
                    "· 로블록스 (Sol's RNG) 를 가로 전체 화면으로\n· 로블록스 설정 → 이동 방식 → '조이스틱' (클래식)\n· 낚시 자리로 가서 Fish 버튼이 보이게\n\n로블록스 위에 떠 있는 작은 Acrux 창이 생겨요. 아무 데나 잡고 끌어서 낚시 창 · 결과창을 가리지 않는 곳으로 옮기세요.", ""},
            {"④ 낚시 보정 → 시작",
                    "Acrux 창의 [보정] 을 누르고 Fish 를 한 번 던지면 대기 창 · 미니게임 창 · 결과창 위치를 자동으로 맞춰요.\n\n그다음 [시작] 이면 자동 낚시 · 멈출 때는 [정지].\n\n여기까지만 해도 낚시는 돼요. 아래는 가득 찼을 때 자동 판매까지 하고 싶을 때예요.", ""},
            {"⑤ 이동 준비 (조이스틱 · 기준 장소)",
                    "Acrux 창 ⚙ 를 누르면 위쪽에 분류가 있어요.\n\n· [조이스틱] → 조이스틱 찾기 · 점프 버튼 위치\n· [기준 장소] → 메뉴 · 재설정 · 도감 등 공통 위치\n\n위치를 정할 때는 십자 포인터가 떠요. 화면 아무 데나 대고 끌면 포인터가 끈 만큼 움직여요 (손가락을 따라가지 않음). 맞췄으면 [확인].", ""},
            {"⑥ 길 기록",
                    "⚙ → [길] → 낚시 장소 · 판매 장소 길 기록.\n\n기준 장소로 간 뒤 WASD 패드가 떠요.\n· W A S D 톡 = 켜기 (홀드) · 다시 톡 = 끄기\n· 키 사이 빈 모서리 = 대각선 · 가운데 ■ = 멈춤\n· 점프 톡 = 한 번 뜀 · 오른쪽 아래 [홀드] = 계속 누르고 있기\n\n도착하면 Acrux 창의 [도착].", ""},
            {"⑦ 판매 준비",
                    "⚙ → [판매]\n· 대화 버튼 위치 (판매 장소에 서서)\n· 판매 자동 보정: Captain Flarg 앞에서 시작 → 대화 버튼은 직접 누르기\n· 판매 한 번 해 보기 로 확인\n\n이동 · 판매는 이동 · 판매 칸에서 켜고 끌 수 있어요.", ""},
            {"다 됐어요!",
                    "[시작] 을 누르면 낚시 → 가득 차면 판매 → 낚시 장소로 돌아와서 이어 가요.\n\n안 되는 게 있으면 기록 칸을 보거나 디스코드로 알려 주세요.\n튜토리얼은 홈 → 사용법 → 튜토리얼 다시 보기 로 언제든 다시 볼 수 있어요.", ""},
    };

    final MainActivity a;
    final FrameLayout host;
    View root;
    int step;
    TextView count, title, body, state, act, prev, next;

    Tutorial(MainActivity a, FrameLayout host) {
        this.a = a;
        this.host = host;
    }

    static boolean seen() {
        return Hub.prefs().getBoolean("tutorialDone", false);
    }

    void show() {
        if (root != null) return;
        FrameLayout dim = new FrameLayout(a);
        dim.setBackgroundColor(0xCC000000);
        dim.setClickable(true);                         // 뒤 화면은 안 눌림
        LinearLayout card = Ui.card(a);
        count = Ui.text(a, "", 12, Ui.MUTED, true);
        card.addView(count);
        title = Ui.text(a, "", 19, Ui.FG, true);
        card.addView(title, Ui.fill(a, 6));
        body = Ui.text(a, "", 14, Ui.FG, false);
        body.setLineSpacing(0, 1.15f);
        ScrollView sv = new ScrollView(a);
        sv.addView(body);
        LinearLayout.LayoutParams sp = Ui.fill(a, 10);
        sp.height = LinearLayout.LayoutParams.WRAP_CONTENT;
        card.addView(sv, sp);
        LinearLayout ar = new LinearLayout(a);
        ar.setOrientation(LinearLayout.HORIZONTAL);
        ar.setGravity(Gravity.CENTER_VERTICAL);
        act = Ui.button(a, "", Ui.ACCENT, v -> doAction());
        ar.addView(act, Ui.wrap());
        state = Ui.text(a, "", 13, Ui.MUTED, true);
        LinearLayout.LayoutParams stp = Ui.wrap();
        stp.leftMargin = Ui.dp(a, 10);
        ar.addView(state, stp);
        card.addView(ar, Ui.fill(a, 12));
        ar.setTag("act");
        LinearLayout nav = new LinearLayout(a);
        nav.setOrientation(LinearLayout.HORIZONTAL);
        nav.setGravity(Gravity.CENTER_VERTICAL);
        nav.addView(Ui.button(a, "건너뛰기", Ui.MUTED, v -> close()), Ui.wrap());
        nav.addView(new View(a), Ui.weight());
        prev = Ui.button(a, "이전", Ui.MUTED, v -> go(step - 1));
        nav.addView(prev, Ui.wrap());
        next = Ui.button(a, "다음", Ui.ACCENT, v -> {
            if (step >= STEPS.length - 1) close();
            else go(step + 1);
        });
        LinearLayout.LayoutParams np = Ui.wrap();
        np.leftMargin = Ui.dp(a, 8);
        nav.addView(next, np);
        card.addView(nav, Ui.fill(a, 16));
        FrameLayout.LayoutParams cp = new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.WRAP_CONTENT, Gravity.CENTER);
        int m = Ui.dp(a, 18);
        cp.setMargins(m, Ui.dp(a, 40), m, Ui.dp(a, 40));
        dim.addView(card, cp);
        host.addView(dim, new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT));
        root = dim;
        go(0);
    }

    void go(int i) {
        step = Math.max(0, Math.min(STEPS.length - 1, i));
        String[] s = STEPS[step];
        Ui.set(count, "튜토리얼 " + (step + 1) + " / " + STEPS.length);
        Ui.set(title, s[0]);
        Ui.set(body, s[1]);
        prev.setVisibility(step == 0 ? View.INVISIBLE : View.VISIBLE);
        Ui.set(next, step == STEPS.length - 1 ? "시작하기" : "다음");
        refresh();
    }

    /** 권한 단계: 지금 켜져 있는지 (앱으로 돌아올 때마다) */
    void refresh() {
        if (root == null) return;
        String k = STEPS[step][2];
        View ar = (View) act.getParent();
        if (k.isEmpty()) {
            ar.setVisibility(View.GONE);
            return;
        }
        ar.setVisibility(View.VISIBLE);
        boolean on = k.equals("acc") ? TapService.instance != null : CaptureService.active != null;
        Ui.set(act, k.equals("acc") ? "접근성 설정 열기" : "화면 캡처 시작");
        act.setVisibility(on ? View.GONE : View.VISIBLE);
        Ui.set(state, on ? "✓ 켜짐" : "· 아직 꺼짐");
        state.setTextColor(on ? Ui.GREEN : Ui.MUTED);
    }

    void doAction() {
        String k = STEPS[step][2];
        if (k.equals("acc")) a.openAccessibility();
        else if (k.equals("cap") && CaptureService.active == null) a.toggleCapture();
    }

    void close() {
        Hub.prefs().edit().putBoolean("tutorialDone", true).apply();
        if (root != null) host.removeView(root);
        root = null;
    }

    boolean open() {
        return root != null;
    }
}
