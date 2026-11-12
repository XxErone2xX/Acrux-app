package com.acrux.mobile;

import android.content.Context;
import android.view.Gravity;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

import com.acrux.mobile.core.I18n;

/**
 * 설정 안내 카드 (PC 튜토리얼 카드와 같은 모양) — 앱 화면 · 떠 있는 Acrux 창 옆 둘 다 이걸 씀
 *  · 위: [남은 튜토리얼 n개] 표 · 안내 이름 · n / N · 제목 · 설명 · (해야 할 것) ✓ 됨 / · 아직
 *  · 첫 단계: 언어 고르기 (한국어 · English · 日本語) — 고르면 앱 · 떠 있는 창 · 이 안내가 바로 그 언어로
 *  · 아래: [나중에] (첫 단계) · [이 단계 건너뛰기] (선택 단계) · [이전] · [다음] (해야 할 것을 해야 눌림) / [완료]
 */
final class GuideCard {
    final Context c;
    /** 앱 화면이면 권한 · 캡처를 바로 여는 버튼을 보여 줌 */
    final MainActivity app;
    final LinearLayout card;
    final TextView badge, name, count, title, body, state, act, hint, later, skip, prev, next;
    String shown = "", shownLang = "";
    final LinearLayout langRow;
    final TextView[] langChips = new TextView[I18n.CODES.length];

    /** 높이가 maxH 를 넘지 않는 스크롤 (긴 설명만 스크롤) */
    static final class CapScroll extends ScrollView {
        int maxH;

        CapScroll(Context c) {
            super(c);
        }

        @Override
        protected void onMeasure(int w, int h) {
            super.onMeasure(w, View.MeasureSpec.makeMeasureSpec(maxH, View.MeasureSpec.AT_MOST));
        }
    }

    GuideCard(Context c, MainActivity app, float maxHFrac, boolean compact) {
        this.c = c;
        this.app = app;
        card = new LinearLayout(c);
        card.setOrientation(LinearLayout.VERTICAL);
        int ph = Ui.dp(c, compact ? 14 : 20), pv = Ui.dp(c, compact ? 12 : 18);
        card.setPadding(ph, pv, ph, Ui.dp(c, compact ? 10 : 14));
        card.setBackground(new Ui.Deco(c, 0xF22B2D31, Ui.LINE, Ui.ACCENT, true, 14, 2));
        card.setClickable(true);
        badge = Ui.text(c, "", 11, 0xFFFFFFFF, true);
        badge.setBackgroundColor(Ui.ACCENT);
        badge.setPadding(Ui.dp(c, 8), Ui.dp(c, 2), Ui.dp(c, 8), Ui.dp(c, 2));
        card.addView(badge, Ui.wrap());
        LinearLayout head = new LinearLayout(c);
        head.setOrientation(LinearLayout.HORIZONTAL);
        head.setGravity(Gravity.CENTER_VERTICAL);
        name = Ui.text(c, "", 11, Ui.ACCENT, true);
        head.addView(name, Ui.wrap());
        head.addView(new View(c), new LinearLayout.LayoutParams(0, 1, 1f));
        count = Ui.text(c, "", 11, Ui.MUTED, true);
        head.addView(count, Ui.wrap());
        card.addView(head, Ui.fill(c, 8));
        head.setTag("head");
        title = Ui.text(c, "", compact ? 15 : 17, Ui.FG, true);
        card.addView(title, Ui.fill(c, 4));
        // 언어 고르기 (첫 단계에서만 보임) — 언어 이름은 번역하지 않음
        langRow = new LinearLayout(c);
        langRow.setOrientation(LinearLayout.HORIZONTAL);
        langRow.setGravity(Gravity.CENTER_VERTICAL);
        langRow.addView(Ui.text(c, "🌐", compact ? 13 : 15, Ui.MUTED, false), Ui.wrap());
        for (int i = 0; i < I18n.CODES.length; i++) {
            String code = I18n.CODES[i];
            TextView chip = Ui.chip(c, I18n.NAMES[i], v -> {
                if (code.equals(I18n.lang())) return;
                Hub.setLang(code);
                Guide.redraw();
            });
            chip.setText(I18n.NAMES[i]);
            chip.setTextSize(compact ? 11 : 12);
            chip.setPadding(Ui.dp(c, 10), Ui.dp(c, 4), Ui.dp(c, 10), Ui.dp(c, 4));
            langChips[i] = chip;
            LinearLayout.LayoutParams lp = Ui.wrap();
            lp.leftMargin = Ui.dp(c, 6);
            langRow.addView(chip, lp);
        }
        card.addView(langRow, Ui.fill(c, 8));
        body = Ui.text(c, "", compact ? 12 : 13, Ui.FG, false);
        body.setLineSpacing(0, 1.3f);
        CapScroll sv = new CapScroll(c);
        sv.maxH = (int) (c.getResources().getDisplayMetrics().heightPixels * maxHFrac);
        sv.addView(body);
        card.addView(sv, Ui.fill(c, 6));
        hint = Ui.text(c, "", compact ? 12 : 13, Ui.YELLOW, true);
        card.addView(hint, Ui.fill(c, 6));
        LinearLayout ar = new LinearLayout(c);
        ar.setOrientation(LinearLayout.HORIZONTAL);
        ar.setGravity(Gravity.CENTER_VERTICAL);
        act = small(Ui.button(c, "", Ui.ACCENT, v -> doAction()));
        ar.addView(act, Ui.wrap());
        state = Ui.text(c, "", 12, Ui.MUTED, true);
        LinearLayout.LayoutParams stp = Ui.wrap();
        stp.leftMargin = Ui.dp(c, 10);
        ar.addView(state, stp);
        card.addView(ar, Ui.fill(c, 8));
        LinearLayout nav = new LinearLayout(c);
        nav.setOrientation(LinearLayout.HORIZONTAL);
        nav.setGravity(Gravity.CENTER_VERTICAL);
        later = small(Ui.button(c, "나중에", Ui.MUTED, v -> Guide.postpone()));
        nav.addView(later, Ui.wrap());
        skip = small(Ui.button(c, "이 단계 건너뛰기", Ui.MUTED, v -> Guide.skipStep()));
        nav.addView(skip, Ui.wrap());
        nav.addView(new View(c), new LinearLayout.LayoutParams(0, 1, 1f));
        prev = small(Ui.button(c, "이전", Ui.MUTED, v -> Guide.prev()));
        nav.addView(prev, Ui.wrap());
        next = small(Ui.button(c, "다음", Ui.ACCENT, v -> Guide.next()));
        LinearLayout.LayoutParams np = Ui.wrap();
        np.leftMargin = Ui.dp(c, 6);
        nav.addView(next, np);
        card.addView(nav, Ui.fill(c, 12));
    }

    TextView small(TextView b) {
        b.setTextSize(12);
        b.setPadding(Ui.dp(c, 11), Ui.dp(c, 6), Ui.dp(c, 11), Ui.dp(c, 6));
        return b;
    }

    /** 지금 안내 상태로 다시 그림 */
    /** ftg: 떠 있는 Acrux 창에서 지금 강조한 곳 (앱 화면 · 패널이 닫혀 있으면 null) */
    void bind(String ftg) {
        Guide.Tut t = Guide.cur;
        Guide.Step s = Guide.step();
        if (t == null || s == null) return;
        Ui.set(badge, "튜토리얼");                          // (안내는 기능마다 따로 · 다음 안내로 줄줄이 넘어가지 않아서 '남은 개수' 는 뺌)
        Ui.set(name, t.name);
        Ui.set(count, (Guide.idx + 1) + " / " + t.steps.size());
        Ui.set(title, s.title);
        Ui.set(body, s.body);
        langRow.setVisibility(Guide.idx == 0 ? View.VISIBLE : View.GONE);
        if (!I18n.lang().equals(shownLang)) {
            shownLang = I18n.lang();
            for (int i = 0; i < langChips.length; i++) Ui.chipOn(c, langChips[i], I18n.CODES[i].equals(shownLang));
        }
        // 바뀐 단계면 살짝 떠오르며 나타남 (PC 와 같음)
        String key = t.id + Guide.idx;
        if (!key.equals(shown)) {
            shown = key;
            card.setAlpha(0f);
            card.setTranslationY(Ui.dp(c, 8));
            card.animate().alpha(1f).translationY(0).setDuration(200).start();
        }
        // 떠 있는 창에서 어디를 눌러야 하는지 (다른 페이지면 그쪽 기능 줄 · ‹ 부터)
        String tg = app == null ? ftg : null;
        String h = "";
        if (app != null && s.target != null) h = "로블록스 화면의 Acrux 창에서 진행해 주세요.";
        else if (app == null && s.target != null && tg == null) h = "→ 화면 오른쪽 손잡이를 눌러 Acrux 창을 열어 주세요.";
        else if ("back".equals(tg)) h = "→ 위쪽 ‹ 를 눌러 돌아가 주세요.";
        else if (tg != null && tg.startsWith("feat:")) h = "→ [" + TapService.pageName(tg.substring(5)) + "] 을 눌러 주세요.";
        Ui.set(hint, h);
        hint.setVisibility(h.isEmpty() ? View.GONE : View.VISIBLE);
        // 해야 할 것
        View ar = (View) act.getParent();
        if (s.check == null) {
            ar.setVisibility(View.GONE);
        } else {
            ar.setVisibility(View.VISIBLE);
            boolean ok = s.check.getAsBoolean();
            boolean showAct = app != null && s.action != null && !ok;
            act.setVisibility(showAct ? View.VISIBLE : View.GONE);
            if (showAct) Ui.set(act, s.action.equals("acc") ? "접근성 설정 열기" : "화면 캡처 시작");
            Ui.set(state, ok ? "✓ 완료" : "· 이 단계를 완료해야 다음으로 넘어갈 수 있습니다.");
            state.setTextColor(ok ? Ui.GREEN : Ui.YELLOW);
        }
        boolean last = Guide.idx >= t.steps.size() - 1;
        Ui.set(later, "나중에");                            // (만들 때 한 번만 번역돼서 언어를 바꾸면 다시)
        Ui.set(skip, "이 단계 건너뛰기");
        Ui.set(prev, "이전");
        later.setVisibility(s.later ? View.VISIBLE : View.GONE);
        skip.setVisibility(s.optional ? View.VISIBLE : View.GONE);
        prev.setVisibility(Guide.idx > 0 ? View.VISIBLE : View.GONE);
        Ui.set(next, last ? "완료" : "다음");
        boolean can = Guide.canNext();
        next.setEnabled(can);
        next.setAlpha(can ? 1f : 0.35f);
    }

    void doAction() {
        Guide.Step s = Guide.step();
        if (s == null || app == null || s.action == null) return;
        if (s.action.equals("acc")) app.openAccessibility();
        else if (s.action.equals("cap") && CaptureService.active == null) app.toggleCapture();
    }
}
