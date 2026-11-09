package com.acrux.mobile;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.graphics.Typeface;
import android.media.projection.MediaProjectionConfig;
import android.media.projection.MediaProjectionManager;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.text.InputType;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.view.ViewGroup;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.SeekBar;
import android.widget.Switch;
import android.widget.TextView;

import com.acrux.mobile.core.I18n;
import com.acrux.mobile.core.Mover;
import com.acrux.mobile.core.Seller;

import java.util.ArrayList;
import java.util.List;

/**
 * 앱 화면 — PC 버전처럼 칸(탭)으로 나눔: 홈 · 낚시 · 이동 · 판매 · 기록 · 설정
 *  · 위쪽 칸 이름을 누르거나 화면을 좌우로 밀어서 칸 이동 · 마지막에 본 칸을 기억
 *  · 묶음(카드)이 3개 이상인 칸은 위쪽에 바로 가기 (PC 와 같음 · 짧은 칸은 없음) — 누르면 그 묶음으로 부드럽게 이동
 *  · 글자는 한국어로 쓰면 Ui 가 지금 언어로 번역 (홈 → 언어)
 * 새 기능 추가: page("열쇠", "칸 이름") 로 칸을 만들고 section(칸, "묶음 이름") 으로 카드를 붙이면 바로 가기 · 이동이 저절로 됨
 */
public class MainActivity extends Activity {
    static final int REQ_CAPTURE = 7;

    TextView accState, capState, calState, logView, runState, posView;
    TextView accBtn, capBtn, updState, updBtn, updNotes, updBanner, uiBtn, uiBtn2, onDot, onText;
    final Runnable refresh = this::refresh;
    final TextView[][] statCells = new TextView[4][2];
    TextView statRunHead, statAllHead;
    final Runnable relang = this::recreate;

    // ---------------------------------------------------------------- 칸 (탭)
    static final class Page {
        final String key, name;
        LinearLayout body, jump;
        ScrollView sv;
        View root;
        TextView tab;
        final List<View> sections = new ArrayList<>();
        final List<TextView> jumps = new ArrayList<>();

        Page(String key, String name) {
            this.key = key;
            this.name = name;
        }
    }

    final List<Page> pages = new ArrayList<>();
    GuideCard guide;
    FrameLayout guideDim;
    final Runnable guideView = this::showGuide;

    void showGuide() {
        if (guideDim == null) return;
        boolean on = Guide.cur != null;
        guideDim.setVisibility(on ? View.VISIBLE : View.GONE);
        if (on) guide.bind(false, null);
    }
    FrameLayout content;
    HorizontalScrollView tabScroll;
    int current = -1;

    @Override
    protected void onCreate(Bundle b) {
        super.onCreate(b);
        Hub.init(this);
        getWindow().getDecorView().setBackgroundColor(Ui.BG);
        if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission("android.permission.POST_NOTIFICATIONS") != 0) {
            requestPermissions(new String[]{"android.permission.POST_NOTIFICATIONS"}, 1);
        }
        FrameLayout frame = new FrameLayout(this);
        frame.setBackgroundColor(Ui.BG);
        // 안드로이드 15 부터 앱 화면이 위 알림 줄 · 아래 내비게이션 바 (||| ○ <) 밑까지 깔림 → 그만큼 안쪽으로 (키보드가 뜨면 키보드 위까지)
        if (Build.VERSION.SDK_INT >= 35) {               // (14 이하는 안드로이드가 알아서 비켜 줌)
            frame.setOnApplyWindowInsetsListener((v, in) -> {
                android.graphics.Insets i = in.getInsets(android.view.WindowInsets.Type.systemBars()
                        | android.view.WindowInsets.Type.displayCutout() | android.view.WindowInsets.Type.ime());
                v.setPadding(i.left, i.top, i.right, i.bottom);
                return in;
            });
        }
        frame.addView(build(), new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        setContentView(frame);
        // 설정 안내 (PC 튜토리얼처럼): 앱 위에 어둡게 깔고 가운데 카드
        guideDim = new FrameLayout(this);
        guideDim.setBackgroundColor(0x9E000000);
        guideDim.setClickable(true);                       // 뒤 화면은 안 눌림
        guide = new GuideCard(this, this, 0.42f, false);
        int sw = getResources().getDisplayMetrics().widthPixels;
        guideDim.addView(guide.card, new FrameLayout.LayoutParams(Math.min(Ui.dp(this, 420), sw - Ui.dp(this, 32)), FrameLayout.LayoutParams.WRAP_CONTENT, Gravity.CENTER));
        guideDim.setVisibility(View.GONE);
        frame.addView(guideDim, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        Guide.views.add(guideView);
        Hub.langListeners.add(relang);
        if (Hub.autoUpdate && !Updater.checkedOnce) {        // 앱을 켤 때 한 번: 새 버전이 있으면 받아서 설치 화면까지
            Updater.checkedOnce = true;
            Updater.check(this, true);
        }
    }

    @Override
    protected void onDestroy() {
        Hub.langListeners.remove(relang);
        Guide.views.remove(guideView);
        super.onDestroy();
    }

    @Override
    protected void onResume() {
        super.onResume();
        Hub.listeners.add(refresh);
        Hub.appVisible = true;
        TapService t = TapService.instance;
        if (t != null) t.updateVisible();
        refresh();                                         // (안내 권한 상태도 같이)
        Guide.auto();                                      // 필요한 설정 안내가 있으면 띄움
    }

    @Override
    public void onBackPressed() {
        if (Guide.cur != null) {                           // 뒤로 가기 = [나중에]
            Guide.postpone();
            return;
        }
        super.onBackPressed();
    }

    @Override
    protected void onPause() {
        Hub.listeners.remove(refresh);
        Hub.appVisible = false;
        TapService t = TapService.instance;
        if (t != null) t.updateVisible();
        super.onPause();
    }

    // ---------------------------------------------------------------- 뼈대
    View build() {
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setBackgroundColor(Ui.BG);
        int p = Ui.dp(this, 18);

        // 머리말
        LinearLayout head = new LinearLayout(this);
        head.setOrientation(LinearLayout.VERTICAL);
        head.setPadding(p, Ui.dp(this, Build.VERSION.SDK_INT >= 35 ? 14 : 36), p, Ui.dp(this, 6));     // 15 이상은 위 알림 줄 높이를 화면 여백 (insets) 으로 따로 뺌
        LinearLayout title = new LinearLayout(this);
        title.setOrientation(LinearLayout.HORIZONTAL);
        title.setGravity(Gravity.BOTTOM);
        title.addView(Ui.text(this, "Acrux Mobile ", 22, Ui.FG, true));
        title.addView(Ui.text(this, "V" + version(), 22, Ui.ACCENT, true));
        // 제목 오른쪽: UI 끄기 · 켜기 (떠 있는 Acrux 창 · 매크로를 안 돌릴 때)
        LinearLayout top = new LinearLayout(this);
        top.setOrientation(LinearLayout.HORIZONTAL);
        top.setGravity(Gravity.CENTER_VERTICAL);
        LinearLayout names = new LinearLayout(this);
        names.setOrientation(LinearLayout.VERTICAL);
        names.addView(title);
        names.addView(Ui.text(this, "자동 낚시 · 이동 · 판매", 13, Ui.MUTED, false));
        // 사용자 수 (PC 와 같은 서버 · PC + 모바일)
        LinearLayout online = new LinearLayout(this);
        online.setOrientation(LinearLayout.HORIZONTAL);
        online.setGravity(Gravity.CENTER_VERTICAL);
        onDot = Ui.text(this, "●", 10, Ui.MUTED, false);
        onText = Ui.text(this, "Now Active : --", 12, Ui.MUTED, true);
        online.addView(onDot, Ui.wrap());
        LinearLayout.LayoutParams ot = Ui.wrap();
        ot.leftMargin = Ui.dp(this, 6);
        online.addView(onText, ot);
        LinearLayout.LayoutParams ol = Ui.wrap();
        ol.topMargin = Ui.dp(this, 4);
        names.addView(online, ol);
        top.addView(names, Ui.weight());
        uiBtn = Ui.button(this, "UI 끄기", Ui.MUTED, v -> Hub.setUi(!Hub.uiOn()));
        top.addView(uiBtn, Ui.wrap());
        head.addView(top);
        root.addView(head);

        // 칸 이름 줄
        tabScroll = new HorizontalScrollView(this);
        tabScroll.setHorizontalScrollBarEnabled(false);
        LinearLayout tabs = new LinearLayout(this);
        tabs.setOrientation(LinearLayout.HORIZONTAL);
        tabs.setPadding(Ui.dp(this, 8), Ui.dp(this, 2), Ui.dp(this, 8), 0);
        tabScroll.addView(tabs);
        root.addView(tabScroll);
        View line = new View(this);                        // 칸 이름 아래 얇은 선 (PC 와 같음)
        line.setBackgroundColor(Ui.LINE);
        root.addView(line, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, Math.max(1, Ui.dp(this, 1))));

        content = new SwipeFrame(this);
        root.addView(content, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));

        buildHome(page("home", "홈"));
        buildFishing(page("fish", "낚시"));
        buildMove(page("move", "이동 · 판매"));
        buildBiome(page("biome", "바이옴"));
        buildLog(page("log", "기록"));
        buildSettings(page("settings", "설정"));

        for (int i = 0; i < pages.size(); i++) {
            Page pg = pages.get(i);
            int idx = i;
            pg.tab = Ui.tab(this, pg.name, v -> show(idx, true));
            tabs.addView(pg.tab, Ui.wrap());
            finishPage(pg);
            content.addView(pg.root, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        }
        String last = Hub.prefs().getString("tab", "home");
        int start = 0;
        for (int i = 0; i < pages.size(); i++) if (pages.get(i).key.equals(last)) start = i;
        show(start, false);
        return root;
    }

    Page page(String key, String name) {
        Page pg = new Page(key, name);
        pg.body = new LinearLayout(this);
        pg.body.setOrientation(LinearLayout.VERTICAL);
        int p = Ui.dp(this, 18);
        pg.body.setPadding(p, Ui.dp(this, 12), p, Ui.dp(this, 32));
        pages.add(pg);
        return pg;
    }

    /** 칸 안에 묶음(카드) 하나 — 제목은 바로 가기 이름도 됨 */
    LinearLayout section(Page pg, String name) {
        LinearLayout card = Ui.card(this);
        card.addView(Ui.text(this, name, 15, Ui.FG, true));
        pg.body.addView(card, Ui.fill(this, pg.sections.isEmpty() ? 0 : 12));
        pg.sections.add(card);
        card.setTag(name);
        return card;
    }

    void finishPage(Page pg) {
        LinearLayout wrap = new LinearLayout(this);
        wrap.setOrientation(LinearLayout.VERTICAL);
        pg.sv = new ScrollView(this);
        pg.sv.addView(pg.body);
        pg.sv.setVerticalScrollBarEnabled(false);
        // 바로 가기: 묶음이 3개 이상일 때만 (PC 와 같음 — 짧은 칸은 없음)
        if (pg.sections.size() >= 3) {
            HorizontalScrollView hs = new HorizontalScrollView(this);
            hs.setHorizontalScrollBarEnabled(false);
            pg.jump = new LinearLayout(this);
            pg.jump.setOrientation(LinearLayout.HORIZONTAL);
            int p = Ui.dp(this, 18);
            pg.jump.setPadding(p, Ui.dp(this, 10), p, 0);
            for (int i = 0; i < pg.sections.size(); i++) {
                View sec = pg.sections.get(i);
                TextView j = Ui.chip(this, (String) sec.getTag(), v -> pg.sv.smoothScrollTo(0, Math.max(0, sec.getTop() - Ui.dp(this, 4))));
                j.setTextSize(12);
                LinearLayout.LayoutParams lp = Ui.wrap();
                if (i > 0) lp.leftMargin = Ui.dp(this, 5);
                pg.jump.addView(j, lp);
                pg.jumps.add(j);
            }
            hs.addView(pg.jump);
            wrap.addView(hs);
            pg.sv.setOnScrollChangeListener((v, x, y, ox, oy) -> markJump(pg));
            pg.sv.post(() -> markJump(pg));
        }
        wrap.addView(pg.sv, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));
        pg.root = wrap;
    }

    /** 지금 보이는 묶음의 바로 가기를 강조 (맨 아래까지 내리면 마지막 묶음) */
    void markJump(Page pg) {
        if (pg.jumps.isEmpty()) return;
        int y = pg.sv.getScrollY() + Ui.dp(this, 40), at = 0;
        boolean bottom = !pg.sv.canScrollVertically(1) && pg.sv.getScrollY() > 0;
        for (int i = 0; i < pg.sections.size(); i++) if (pg.sections.get(i).getTop() <= y) at = i;
        if (bottom) at = pg.sections.size() - 1;
        for (int i = 0; i < pg.jumps.size(); i++) Ui.chipOn(this, pg.jumps.get(i), i == at);
    }

    void show(int i, boolean save) {
        if (i < 0 || i >= pages.size() || i == current) return;
        current = i;
        for (int k = 0; k < pages.size(); k++) {
            Page pg = pages.get(k);
            pg.root.setVisibility(k == i ? View.VISIBLE : View.GONE);
            Ui.tabOn(this, pg.tab, k == i);
        }
        TextView t = pages.get(i).tab;
        tabScroll.post(() -> tabScroll.smoothScrollTo(Math.max(0, t.getLeft() - Ui.dp(this, 40)), 0));
        if (save) Hub.prefs().edit().putString("tab", pages.get(i).key).apply();
    }

    /** 화면을 좌우로 밀면 옆 칸으로 (세로 스크롤 · 버튼 누르기는 그대로) */
    final class SwipeFrame extends FrameLayout {
        final int slop = ViewConfiguration.get(getContext()).getScaledTouchSlop();
        float sx, sy;
        boolean swiping;

        SwipeFrame(Context c) {
            super(c);
        }

        @Override
        public boolean onInterceptTouchEvent(MotionEvent e) {
            switch (e.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                    sx = e.getX();
                    sy = e.getY();
                    swiping = false;
                    break;
                case MotionEvent.ACTION_MOVE:
                    float dx = e.getX() - sx, dy = e.getY() - sy;
                    if (Math.abs(dx) > slop * 3 && Math.abs(dx) > Math.abs(dy) * 2) swiping = true;
                    break;
                default:
                    break;
            }
            return swiping;
        }

        @Override
        public boolean onTouchEvent(MotionEvent e) {
            if (e.getActionMasked() == MotionEvent.ACTION_UP && swiping) {
                float dx = e.getX() - sx;
                if (Math.abs(dx) > Ui.dp(getContext(), 70)) show(current + (dx < 0 ? 1 : -1), true);
                swiping = false;
            }
            return true;
        }
    }

    // ---------------------------------------------------------------- 홈
    void buildHome(Page pg) {
        updBanner = Ui.text(this, "", 13, Ui.YELLOW, true);
        updBanner.setPadding(Ui.dp(this, 14), Ui.dp(this, 10), Ui.dp(this, 14), Ui.dp(this, 10));
        updBanner.setBackground(new Ui.Deco(this, 0x29F0B232, Ui.YELLOW, 0, true, 0, 0));
        updBanner.setOnClickListener(v -> {
            show(4, true);
            if (Updater.newVer != null) Updater.download(this);
        });
        updBanner.setVisibility(View.GONE);
        pg.body.addView(updBanner, Ui.fill(this, 0));
        // 지난번에 앱이 꺼졌으면 (크래시 기록이 있으면) 알림 → 기록 칸
        if (Crash.read() != null) {
            TextView cb = Ui.text(this, "지난번에 앱이 꺼짐 — 기록 칸의 크래시 기록을 복사해서 보내 주세요", 13, Ui.RED, true);
            cb.setPadding(Ui.dp(this, 14), Ui.dp(this, 10), Ui.dp(this, 14), Ui.dp(this, 10));
            cb.setBackground(new Ui.Deco(this, 0x29F23F43, Ui.RED, 0, true, 0, 0));
            cb.setOnClickListener(v -> show(3, true));
            pg.body.addView(cb, Ui.fill(this, 8));
        }
        pg.body.addView(new View(this), new LinearLayout.LayoutParams(1, Ui.dp(this, 8)));

        LinearLayout lang = section(pg, "언어");
        lang.addView(Ui.text(this, "앱 언어 · 떠 있는 Acrux 창도 같이 바뀜", 12, Ui.MUTED, false));
        LinearLayout langs = new LinearLayout(this);
        langs.setOrientation(LinearLayout.HORIZONTAL);
        for (int i = 0; i < I18n.CODES.length; i++) {
            String code = I18n.CODES[i];
            TextView c = Ui.chip(this, I18n.NAMES[i], v -> {
                if (!code.equals(I18n.lang())) Hub.setLang(code);
            });
            c.setText(I18n.NAMES[i]);                       // 언어 이름은 번역하지 않음
            Ui.chipOn(this, c, code.equals(I18n.lang()));
            LinearLayout.LayoutParams lp = Ui.wrap();
            if (i > 0) lp.leftMargin = Ui.dp(this, 6);
            langs.addView(c, lp);
        }
        lang.addView(langs, Ui.fill(this, 10));

        LinearLayout ready = section(pg, "준비");
        accState = Ui.text(this, "", 13, Ui.MUTED, false);
        accBtn = Ui.button(this, "켜기", Ui.ACCENT, v -> openAccessibility());
        ready.addView(row("① 접근성 권한", "Acrux가 로블록스 화면을 대신 터치함 · 설정 → 접근성 → 설치된 앱 → Acrux Mobile", accState, accBtn), Ui.fill(this, 10));
        TextView restricted = Ui.text(this, "안드로이드 13 이상에서 '제한된 설정'이라며 켜지지 않으면: 설정 → 앱 → Acrux Mobile → 오른쪽 위 ⋮ → '제한된 설정 허용' 후 다시 켜기",
                12, Ui.YELLOW, false);
        restricted.setOnClickListener(v -> openAppInfo());
        ready.addView(restricted, Ui.fill(this, 6));
        capState = Ui.text(this, "", 13, Ui.MUTED, false);
        capBtn = Ui.button(this, "시작", Ui.ACCENT, v -> toggleCapture());
        ready.addView(row("② 화면 캡처", "화면 색을 보고 낚시 상태를 판단함 · 녹화 · 저장은 하지 않음 · '전체 화면' 공유를 고르기", capState, capBtn), Ui.fill(this, 12));
        TextView stickHelp = Ui.text(this, "③ 이동 · 판매: 로블록스 설정 → 이동 방식을 '조이스틱'(클래식)으로 · 루팅 · Shizuku 필요 없음 (전부 화면 터치)",
                12, Ui.YELLOW, false);
        ready.addView(stickHelp, Ui.fill(this, 6));

        LinearLayout st = section(pg, "상태");
        runState = Ui.text(this, "", 13, Ui.FG, false);
        st.addView(runState, Ui.fill(this, 8));
        calState = Ui.text(this, "", 13, Ui.MUTED, false);
        st.addView(calState, Ui.fill(this, 6));
        TextView reset = Ui.button(this, "보정 초기화", Ui.RED, v -> {
            Hub.resetCalibration();
            Hub.log("보정 초기화", 'd');
        });
        LinearLayout.LayoutParams rp = Ui.wrap();
        rp.topMargin = Ui.dp(this, 10);
        st.addView(reset, rp);

        LinearLayout how = section(pg, "사용법");
        String[] steps = {
                "1. 로블록스(Sol's RNG)를 전체 화면 가로로 켜고 낚시 자리로 가서 Fish 버튼이 보이게 하기",
                "2. 떠 있는 Acrux 창에서 [보정] — Fish 를 한 번 던져 대기 창 · 미니게임 창 · 결과창 위치를 자동으로 맞춤",
                "3. [시작] — 자동 낚시 · 멈출 때는 [정지]",
                "4. Acrux 창은 어디든 잡고 끌어서 낚시 창 · 결과창을 가리지 않는 곳으로 옮기기",
                "5. 이동 · 판매: ⚙ → [조이스틱] 조이스틱 찾기 → [기준 장소] 공통 위치 → [길] 낚시 장소 · 판매 장소 길 기록 → [판매] 대화 버튼 · 판매 자동 보정",
        };
        for (String s : steps) how.addView(Ui.text(this, s, 13, Ui.MUTED, false), Ui.fill(this, 6));
        LinearLayout.LayoutParams tl = Ui.wrap();
        tl.topMargin = Ui.dp(this, 10);
        how.addView(Ui.text(this, "설정 안내 — 기능별로 하나씩 따라 하기 (PC 튜토리얼과 같음)", 13, Ui.FG, true), tl);
        LinearLayout gl = new LinearLayout(this);
        gl.setOrientation(LinearLayout.VERTICAL);
        for (Guide.Tut t : Guide.all()) {
            LinearLayout.LayoutParams gp = Ui.fill(this, 6);
            gl.addView(Ui.button(this, t.name, Ui.ACCENT, v -> Guide.start(t.id)), gp);
        }
        how.addView(gl, Ui.fill(this, 4));

        LinearLayout don = section(pg, "후원");
        don.addView(Ui.text(this, "Acrux 가 도움이 됐다면 후원으로 응원해 주세요", 12, Ui.MUTED, false));
        LinearLayout.LayoutParams dl = Ui.wrap();
        dl.topMargin = Ui.dp(this, 10);
        don.addView(Ui.button(this, "후원하기", Ui.YELLOW, v -> openUrl("https://ko-fi.com/RG_Acrux")), dl);
    }

    // ---------------------------------------------------------------- 낚시
    void buildFishing(Page pg) {
        // 낚시 통계: 이번 실행 (앱을 켠 뒤) · 전체 (저장됨) — PC '이번 실행 · 올타임'
        LinearLayout st = section(pg, "낚시 통계");
        String[] names = {"성공", "쓰레기", "실패", "성공률"};
        LinearLayout hdr = statRow(Ui.text(this, "", 12, Ui.MUTED, true), statRunHead = Ui.text(this, "이번 실행", 12, Ui.MUTED, true),
                statAllHead = Ui.text(this, "전체", 12, Ui.MUTED, true));
        st.addView(hdr, Ui.fill(this, 10));
        for (int i = 0; i < names.length; i++) {
            statCells[i][0] = Ui.text(this, "0", 14, Ui.FG, true);
            statCells[i][1] = Ui.text(this, "0", 14, Ui.FG, false);
            st.addView(statRow(Ui.text(this, names[i], 13, Ui.MUTED, false), statCells[i][0], statCells[i][1]), Ui.fill(this, 6));
        }
        LinearLayout btns = new LinearLayout(this);
        btns.setOrientation(LinearLayout.HORIZONTAL);
        btns.addView(Ui.button(this, "이번 실행 초기화", Ui.MUTED, v -> Hub.resetStats(false)));
        LinearLayout.LayoutParams bl = Ui.wrap();
        bl.leftMargin = Ui.dp(this, 8);
        btns.addView(Ui.button(this, "전체 초기화", Ui.RED, v -> Hub.resetStats(true)), bl);
        st.addView(btns, Ui.fill(this, 12));

        LinearLayout set = section(pg, "세부 설정");
        set.addView(num("입질 최대 대기 (초)", "넘으면 Exit 후 다시 던짐 · 기본 60", Hub.cfg.biteMax, v -> Hub.cfg.biteMax = clamp(v, 5, 600)), Ui.fill(this, 10));
        set.addView(num("누르는 시간 (ms)", "릴링 때 한 번 누르는 시간 · 기본 30", Hub.cfg.clickMs, v -> Hub.cfg.clickMs = (int) clamp(v, 5, 200)), Ui.fill(this, 8));
        set.addView(num("미리 누르기 (ms)", "터치가 늦게 먹히면 이만큼 미리 누름 · 기본 0 · 릴링이 자꾸 실패하면 40 ~ 80", Hub.cfg.leadMs,
                v -> Hub.cfg.leadMs = clamp(v, 0, 300)), Ui.fill(this, 8));
        set.addView(num("목표 위치 (%)", "구간 왼쪽 끝에서 구간 폭의 몇 % · 기본 0", Hub.cfg.targetPct, v -> Hub.cfg.targetPct = clamp(v, 0, 60)), Ui.fill(this, 8));
        set.addView(sw("터치 지연에 맞춰 기다리기", Hub.cfg.adaptive, on -> Hub.cfg.adaptive = on), Ui.fill(this, 10));
        set.addView(Ui.text(this, "누른 뒤 반응이 보일 때까지 다시 누르지 않는 시간을 이 폰의 터치 속도에 맞춤", 12, Ui.MUTED, false));
        set.addView(sw("릴링 때 미니게임 창 위쪽을 누름", Hub.cfg.tapAbove, on -> Hub.cfg.tapAbove = on), Ui.fill(this, 10));
        set.addView(Ui.text(this, "모바일은 미니게임 창 바로 아래가 Roll 버튼이라 위쪽 빈 화면(캐릭터 쪽)을 누름 · 끄면 PC 처럼 창 아래", 12, Ui.MUTED, false));
    }

    /** 통계 한 줄: 이름 | 이번 실행 | 전체 */
    LinearLayout statRow(TextView name, TextView a, TextView b) {
        LinearLayout r = new LinearLayout(this);
        r.setOrientation(LinearLayout.HORIZONTAL);
        r.addView(name, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1.2f));
        a.setGravity(Gravity.END);
        b.setGravity(Gravity.END);
        r.addView(a, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        r.addView(b, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        return r;
    }

    static String pct(long ok, long all) {
        return all <= 0 ? "-" : String.format(java.util.Locale.ROOT, "%.1f%%", ok * 100.0 / all);
    }

    // ---------------------------------------------------------------- 이동 · 판매
    void buildMove(Page pg) {
        LinearLayout mv = section(pg, "이동 · 판매");
        mv.addView(Ui.text(this, "시작하면 재설정 → 기준 장소 → 기록한 길로 낚시 장소 · 인벤토리가 가득 차면 판매 장소에서 팔고 돌아옴 · 전부 화면 터치 (조이스틱 · 탭) · 위치 · 길은 Acrux 창 ⚙ 에서",
                12, Ui.MUTED, false), Ui.fill(this, 4));
        mv.addView(sw("시작할 때 낚시 장소로 이동", Hub.useMove, on -> {
            Hub.useMove = on;
            if (on) Guide.featureOn("move");              // 설정이 덜 돼 있으면 이동 설정 안내
        }), Ui.fill(this, 8));
        mv.addView(sw("가득 차면 판매하러 가기", Hub.useSell, on -> {
            Hub.useSell = on;
            if (on) Guide.featureOn(Guide.find("move").needed.getAsBoolean() ? "move" : "sell");
        }), Ui.fill(this, 4));

        LinearLayout base = section(pg, "기준 장소 (초)");
        Mover.Cfg m = Hub.move;
        base.addView(num("리셋 후 대기", "기본 3.5", m.resetWait, v -> m.resetWait = clamp(v, 0.5, 30)), Ui.fill(this, 8));
        base.addView(num("W + A", "기본 3.5", m.waTime, v -> m.waTime = clamp(v, 0, 30)), Ui.fill(this, 6));
        base.addView(num("A", "기본 3", m.aTime, v -> m.aTime = clamp(v, 0, 30)), Ui.fill(this, 6));
        base.addView(num("A + W", "기본 2", m.awTime, v -> m.awTime = clamp(v, 0, 30)), Ui.fill(this, 6));
        base.addView(num("쉼", "D 누르기 전에 기다림 · 기본 0.5", m.pauseTime, v -> m.pauseTime = clamp(v, 0, 10)), Ui.fill(this, 6));
        base.addView(num("D", "기본 1", m.dTime, v -> m.dTime = clamp(v, 0, 30)), Ui.fill(this, 6));
        base.addView(num("내려다보기 (번)", "화면을 아래로 끄는 횟수 · 기본 3", m.tiltSwipes, v -> m.tiltSwipes = (int) clamp(v, 0, 10)), Ui.fill(this, 6));
        base.addView(num("줌 아웃 (번)", "두 손가락 오므리는 횟수 · 기본 4", m.zoomPinches, v -> m.zoomPinches = (int) clamp(v, 0, 10)), Ui.fill(this, 6));
        base.addView(num("도착 뒤 여유", "길을 다 걸은 뒤 기다림 · 기본 0.3", m.margin, v -> m.margin = clamp(v, 0, 5)), Ui.fill(this, 6));

        LinearLayout sell = section(pg, "판매");
        sell.addView(num("판매 최대 종류", "한 번에 파는 최대 종류 수 · 기본 100", Hub.sellMax, v -> Hub.sellMax = (int) clamp(v, 1, 500)), Ui.fill(this, 8));
        sell.addView(num("판매 클릭 추가 대기 (초)", "렉이 있으면 늘림 · 기본 0", Hub.sellDelay, v -> Hub.sellDelay = clamp(v, 0, 5)), Ui.fill(this, 8));

        LinearLayout saved = section(pg, "저장된 위치 · 길");
        posView = Ui.text(this, "", 12, Ui.MUTED, false);
        posView.setTypeface(Typeface.MONOSPACE);
        saved.addView(posView, Ui.fill(this, 6));
        TextView clearPos = Ui.button(this, "위치 · 길 모두 지우기", Ui.RED, v -> {
            Hub.pos.clear();
            Hub.fishRoute = new com.acrux.mobile.core.Route();
            Hub.sellRoute = new com.acrux.mobile.core.Route();
            Hub.save();
            Hub.changed();
            Hub.log("위치 · 길 모두 지움", 'd');
        });
        LinearLayout.LayoutParams cp = Ui.wrap();
        cp.topMargin = Ui.dp(this, 8);
        saved.addView(clearPos, cp);
    }

    // ---------------------------------------------------------------- 기록
    // ---- 바이옴 (무선 디버깅으로 로블록스 로그 읽기 · PC 버전과 같은 방식 · 1단계: 로그에 바이옴 줄이 있는지 테스트)
    TextView adbState;
    volatile boolean biomeTesting;

    void buildBiome(Page pg) {
        LinearLayout c = section(pg, "무선 디버깅 연결");
        c.addView(Ui.text(this, "PC 처럼 로블록스 로그로 바이옴을 정확하게 읽으려면 로그를 읽을 권한이 필요해요. "
                + "안드로이드의 '무선 디버깅' 으로 Acrux 가 이 폰에 직접 연결해요 (PC · 루팅 · Shizuku 필요 없음 · 안드로이드 11 이상 · 와이파이 필요).", 13, Ui.MUTED, false), Ui.fill(this, 6));
        adbState = Ui.text(this, "", 13, Ui.MUTED, true);
        c.addView(adbState, Ui.fill(this, 10));
        String[] steps = {
                "1. 설정 → 휴대전화 정보 → 소프트웨어 정보 → '빌드 번호' 7번 눌러서 개발자 옵션 켜기",
                "2. 아래 [페어링 시작] → 개발자 옵션 → 무선 디버깅 켜기 → '페어링 코드로 기기 페어링'",
                "3. 코드 창을 띄운 채로 화면 위를 내려서 Acrux 알림의 [코드 입력] 에 6자리 코드",
                "4. 한 번 페어링하면 다음부터는 무선 디버깅만 켜져 있으면 [연결] (재부팅 · 와이파이가 바뀌면 다시 켜기)"};
        for (String st : steps) c.addView(Ui.text(this, st, 13, Ui.FG, false), Ui.fill(this, 4));
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.addView(Ui.button(this, "페어링 시작", Ui.ACCENT, v -> startPair()), Ui.wrap());
        LinearLayout.LayoutParams g = Ui.wrap();
        g.leftMargin = Ui.dp(this, 8);
        row.addView(Ui.button(this, "연결", Ui.MUTED, v -> adbConnect(true)), g);
        LinearLayout.LayoutParams g2 = Ui.wrap();
        g2.leftMargin = Ui.dp(this, 8);
        row.addView(Ui.button(this, "개발자 옵션", Ui.MUTED, v -> openDevSettings()), g2);
        c.addView(row, Ui.fill(this, 12));

        LinearLayout t = section(pg, "바이옴 로그 테스트");
        t.addView(Ui.text(this, "모바일 로블록스가 PC 처럼 바이옴 정보 ([BloxstrapRPC]) 를 로그에 남기는지 확인해요. "
                + "남기면 PC 버전처럼 정확한 바이옴 감지 · 웹후크 알림을 만들 수 있어요.", 13, Ui.MUTED, false), Ui.fill(this, 6));
        t.addView(Ui.text(this, "무선 디버깅을 연결한 뒤 [테스트 시작] → 로블록스로 Sol's RNG 에 들어가서 90초 동안 두기 (바이옴이 바뀌는 때면 더 좋아요). 결과는 기록 칸에 떠요.", 13, Ui.FG, false), Ui.fill(this, 6));
        t.addView(Ui.button(this, "테스트 시작 (90초)", Ui.ACCENT, v -> biomeTest()), Ui.fill(this, 10));
        refreshAdb();
        if (Adb.supported()) new Thread(() -> {                // 켜져 있으면 조용히 연결해 둠
            Adb.connect(getApplicationContext());
            runOnUiThread(this::refreshAdb);
        }, "acrux-adb").start();
    }

    void refreshAdb() {
        if (adbState == null) return;
        if (!Adb.supported()) {
            Ui.set(adbState, "✗ 안드로이드 11 이상만 돼요");
            adbState.setTextColor(Ui.RED);
        } else if (Adb.connected) {
            Ui.set(adbState, "✓ 연결됨");
            adbState.setTextColor(Ui.GREEN);
        } else {
            Ui.set(adbState, "· 연결 안 됨");
            adbState.setTextColor(Ui.MUTED);
        }
    }

    void openDevSettings() {
        try {
            startActivity(new Intent(android.provider.Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS));
        } catch (RuntimeException e) {
            Hub.log("개발자 옵션이 아직 꺼져 있어요 — 설정 → 휴대전화 정보 → 소프트웨어 정보 → '빌드 번호' 7번", 'y');
        }
    }

    void startPair() {
        if (!Adb.supported()) {
            Hub.log("무선 디버깅 페어링은 안드로이드 11 이상만 돼요", 'r');
            return;
        }
        if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[]{android.Manifest.permission.POST_NOTIFICATIONS}, 41);
        }
        Adb.showPairNotice(this, "무선 디버깅 → '페어링 코드로 기기 페어링' 창을 띄운 채로 여기 [코드 입력] 에 6자리 코드를 넣으세요");
        Hub.log("페어링: 무선 디버깅 → '페어링 코드로 기기 페어링' → 알림 칸의 Acrux [코드 입력] 에 6자리 코드", 'c');
        openDevSettings();
    }

    void adbConnect(boolean say) {
        new Thread(() -> {
            String r = Adb.connect(getApplicationContext());
            if (say) {
                switch (r) {
                    case "ok": Hub.log("무선 디버깅 연결됨", 'g'); break;
                    case "pair": Hub.log("페어링이 먼저 필요해요 — [페어링 시작]", 'y'); break;
                    case "old": Hub.log("안드로이드 11 이상만 돼요", 'r'); break;
                    default: Hub.log("연결 안 됨 — 개발자 옵션 → 무선 디버깅이 켜져 있는지 · 와이파이에 연결돼 있는지 확인", 'y'); break;
                }
            }
            runOnUiThread(this::refreshAdb);
        }, "acrux-adb").start();
    }

    /** 1단계 테스트: ① 지금까지 쌓인 로그 ② 로블록스 폴더의 로그 파일 ③ 90초 동안 새로 나오는 로그 — 바이옴 줄이 있는지 */
    void biomeTest() {
        if (biomeTesting) {
            Hub.log("바이옴 로그 테스트가 이미 진행 중이에요", 'y');
            return;
        }
        biomeTesting = true;
        Hub.log("바이옴 로그 테스트 시작 — 지금 로블록스로 Sol's RNG 에 들어가 주세요 (90초)", 'c');
        new Thread(() -> {
            android.content.Context ctx = getApplicationContext();
            int[] hits = {0};
            try {
                String r = Adb.connect(ctx);
                runOnUiThread(this::refreshAdb);
                if (!r.equals("ok")) {
                    Hub.log(r.equals("pair") ? "테스트 못 함 — 무선 디버깅 페어링이 먼저 필요해요" : "테스트 못 함 — 무선 디버깅이 연결 안 됨 (개발자 옵션 → 무선 디버깅 켜기)", 'r');
                    return;
                }
                java.util.function.Consumer<String> show = l -> {
                    hits[0]++;
                    if (hits[0] <= 40) Hub.log("  " + (l.length() > 220 ? l.substring(0, 220) + "…" : l), 'g');
                };
                Hub.log("① 쌓인 로그에서 찾는 중", 'c');
                Adb.shell(ctx, "logcat -d -v brief 2>&1 | grep -i -E 'bloxstrap|biome' | tail -n 15", 20000, show);
                int h1 = hits[0];
                Hub.log("① " + h1 + "줄", h1 > 0 ? 'g' : 'y');
                Hub.log("② 로블록스 폴더의 로그 파일", 'c');
                int[] files = {0};
                Adb.shell(ctx, "ls /sdcard/Android/data/com.roblox.client/files 2>&1 | head -n 20; "
                        + "find /sdcard/Android/data/com.roblox.client -maxdepth 6 -iname '*log*' 2>/dev/null | head -n 15", 20000, l -> {
                    files[0]++;
                    if (files[0] <= 35) Hub.log("  " + l, 'c');
                });
                Adb.shell(ctx, "for f in $(grep -rli 'BloxstrapRPC' /sdcard/Android/data/com.roblox.client 2>/dev/null | head -n 3); do echo \"== $f\"; grep -i 'BloxstrapRPC' \"$f\" | tail -n 3; done", 30000, show);
                int h2 = hits[0] - h1;
                Hub.log("② " + h2 + "줄", h2 > 0 ? 'g' : 'y');
                Hub.log("③ 90초 동안 새로 나오는 로그 지켜보는 중 (로블록스를 켜 두세요)", 'c');
                int[] seen = {0};
                Adb.shell(ctx, "logcat -T 1 -v brief", 90000, l -> {
                    seen[0]++;
                    String low = l.toLowerCase(java.util.Locale.ROOT);
                    if (low.contains("bloxstrap") || low.contains("biome")) show.accept(l);
                });
                int h3 = hits[0] - h1 - h2;
                Hub.log("③ 로그 " + seen[0] + "줄 중 바이옴 " + h3 + "줄", h3 > 0 ? 'g' : 'y');
                if (hits[0] > 0) Hub.log("결과: 바이옴 줄 발견! → PC 처럼 정확한 바이옴 감지를 만들 수 있어요. 이 기록을 캡처해서 보내 주세요", 'g');
                else if (seen[0] > 0) Hub.log("결과: 로그는 읽히는데 바이옴 줄이 없음 — Sol's RNG 에 들어가 있었는지 확인하고 한 번 더 · 그래도 없으면 모바일 로블록스가 바이옴 정보를 안 남기는 것", 'y');
                else Hub.log("결과: 로그를 하나도 못 읽음 — 이 기록을 캡처해서 보내 주세요", 'r');
            } catch (Throwable th) {
                Hub.log("바이옴 로그 테스트 오류: " + th, 'r');
            } finally {
                biomeTesting = false;
            }
        }, "acrux-biome-test").start();
    }

    void buildLog(Page pg) {
        LinearLayout lg = section(pg, "기록");
        lg.addView(Ui.text(this, "최근 기록 (최대 300줄)", 12, Ui.MUTED, false));
        logView = Ui.text(this, "", 12, Ui.MUTED, false);
        logView.setTypeface(Typeface.MONOSPACE);
        logView.setTextIsSelectable(true);
        lg.addView(logView, Ui.fill(this, 8));
        TextView clear = Ui.button(this, "기록 지우기", Ui.MUTED, v -> Hub.clearLogs());
        LinearLayout.LayoutParams lp = Ui.wrap();
        lp.topMargin = Ui.dp(this, 10);
        lg.addView(clear, lp);

        // 크래시 기록 (앱이 꺼졌을 때 남은 오류 내용 · 기종 · 안드로이드 버전) — 복사해서 보내면 원인을 바로 앎
        String crash = Crash.read();
        if (crash != null) {
            LinearLayout cr = section(pg, "크래시 기록");
            cr.addView(Ui.text(this, "지난번에 앱이 꺼진 이유 · 기종 · 안드로이드 버전 (개인 정보 없음)", 12, Ui.MUTED, false));
            TextView ct = Ui.text(this, "", 11, Ui.RED, false);
            ct.setText(crash);
            ct.setTypeface(Typeface.MONOSPACE);
            ct.setTextIsSelectable(true);
            cr.addView(ct, Ui.fill(this, 8));
            LinearLayout cb = new LinearLayout(this);
            cb.setOrientation(LinearLayout.HORIZONTAL);
            cb.addView(Ui.button(this, "복사", Ui.ACCENT, v -> {
                android.content.ClipboardManager cm = getSystemService(android.content.ClipboardManager.class);
                if (cm != null) cm.setPrimaryClip(android.content.ClipData.newPlainText("Acrux crash", crash));
                Hub.log("크래시 기록 복사됨", 'g');
            }));
            LinearLayout.LayoutParams dl = Ui.wrap();
            dl.leftMargin = Ui.dp(this, 8);
            cb.addView(Ui.button(this, "지우기", Ui.MUTED, v -> {
                Crash.clear();
                Hub.log("크래시 기록 지움", 'd');
                recreate();
            }), dl);
            cr.addView(cb, Ui.fill(this, 10));
        }
    }

    // ---------------------------------------------------------------- 설정
    void buildSettings(Page pg) {
        LinearLayout up = section(pg, "업데이트");
        updState = Ui.text(this, "", 13, Ui.MUTED, false);
        updBtn = Ui.button(this, "확인", Ui.ACCENT, v -> {
            if (Updater.newVer != null) Updater.download(this);
            else Updater.check(this, false);
        });
        up.addView(row("Acrux Mobile V" + version(), I18n.f("지금 V{0} · 새 버전이 나오면 받아서 바로 설치 (설정은 그대로)", version()), updState, updBtn), Ui.fill(this, 8));
        up.addView(sw("앱을 켤 때 자동으로 업데이트", Hub.autoUpdate, on -> Hub.autoUpdate = on), Ui.fill(this, 8));
        updNotes = Ui.text(this, "", 12, Ui.MUTED, false);
        up.addView(updNotes, Ui.fill(this, 4));

        LinearLayout win = section(pg, "조작 창");
        win.addView(Ui.text(this, "떠 있는 Acrux 창 · 어디든 잡고 끌면 움직임", 12, Ui.MUTED, false));
        uiBtn2 = Ui.button(this, "UI 끄기", Ui.MUTED, v -> Hub.setUi(!Hub.uiOn()));
        LinearLayout.LayoutParams ub = Ui.wrap();
        ub.topMargin = Ui.dp(this, 10);
        win.addView(uiBtn2, ub);
        win.addView(Ui.text(this, "매크로를 안 돌릴 때 떠 있는 창을 통째로 끔 · 앱 맨 위 버튼과 같음 · 끄면 돌던 매크로도 멈춤", 12, Ui.MUTED, false), Ui.fill(this, 4));
        win.addView(sw("Acrux 앱을 볼 때는 숨기기", Hub.prefs().getBoolean("hideOnApp", true), on -> {
            Hub.prefs().edit().putBoolean("hideOnApp", on).apply();
            TapService t = TapService.instance;
            if (t != null) t.updateVisible();
        }), Ui.fill(this, 4));
        win.addView(Ui.text(this, "조작 창 투명도", 13, Ui.FG, true), Ui.fill(this, 12));
        SeekBar alpha = new SeekBar(this);
        alpha.setMax(60);
        alpha.setProgress(Hub.prefs().getInt("panelAlpha", 90) - 40);
        alpha.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override
            public void onProgressChanged(SeekBar s, int v, boolean user) {
                if (!user) return;
                Hub.prefs().edit().putInt("panelAlpha", v + 40).apply();
                TapService t = TapService.instance;
                if (t != null) t.applyAlpha();
            }

            @Override
            public void onStartTrackingTouch(SeekBar s) {}

            @Override
            public void onStopTrackingTouch(SeekBar s) {}
        });
        win.addView(alpha, Ui.fill(this, 4));
        TextView home = Ui.button(this, "조작 창 위치 되돌리기", Ui.MUTED, v -> {
            TapService t = TapService.instance;
            if (t != null) t.resetPanelPos();
            else Hub.prefs().edit().remove("panelX").remove("panelY").apply();
            Hub.log("조작 창 위치를 왼쪽 위로 되돌림", 'd');
        });
        LinearLayout.LayoutParams hp = Ui.wrap();
        hp.topMargin = Ui.dp(this, 8);
        win.addView(home, hp);

        LinearLayout info = section(pg, "정보");
        info.addView(Ui.text(this, "사용자 수 집계: 켜져 있다는 신호만 보냄 (무작위 번호 · 개인 정보 없음)", 12, Ui.MUTED, false), Ui.fill(this, 8));
        info.addView(Ui.text(this, "Acrux Mobile V" + version(), 13, Ui.FG, false), Ui.fill(this, 6));
        info.addView(Ui.text(this, "Made by 얼로니", 12, Ui.MUTED, false));
    }

    // ---------------------------------------------------------------- 부품
    interface Num {
        void set(double v);
    }

    interface Flag {
        void set(boolean on);
    }

    View sw(String title, boolean on, Flag f) {
        Switch s = new Switch(this);
        s.setText(Ui.tr(title));
        s.setTextColor(Ui.FG);
        s.setTextSize(14);
        Ui.tint(s);
        s.setChecked(on);
        s.setOnCheckedChangeListener((v, c) -> {
            f.set(c);
            Hub.save();
        });
        return s;
    }

    static double clamp(double v, double lo, double hi) {
        return Math.max(lo, Math.min(hi, v));
    }

    View num(String title, String sub, double value, Num on) {
        EditText e = new EditText(this);
        e.setInputType(InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_FLAG_DECIMAL);
        e.setText(value == Math.rint(value) ? String.valueOf((long) value) : String.valueOf(value));
        e.setTextColor(Ui.FG);
        e.setBackground(Ui.field(this));
        e.setPadding(Ui.dp(this, 10), Ui.dp(this, 6), Ui.dp(this, 10), Ui.dp(this, 6));
        e.setMinWidth(Ui.dp(this, 80));
        e.setGravity(Gravity.CENTER);
        e.setOnFocusChangeListener((v, has) -> {
            if (has) return;
            try {
                on.set(Double.parseDouble(e.getText().toString().trim()));
                Hub.save();
            } catch (NumberFormatException ignored) {
            }
        });
        return row(title, sub, null, e);
    }

    /** 제목 + 설명 (+ 상태) · 오른쪽에 버튼 · 입력칸 */
    LinearLayout row(String title, String sub, TextView state, View right) {
        LinearLayout r = new LinearLayout(this);
        r.setOrientation(LinearLayout.HORIZONTAL);
        r.setGravity(Gravity.CENTER_VERTICAL);
        LinearLayout left = new LinearLayout(this);
        left.setOrientation(LinearLayout.VERTICAL);
        left.addView(Ui.text(this, title, 14, Ui.FG, true));
        left.addView(Ui.text(this, sub, 12, Ui.MUTED, false));
        if (state != null) left.addView(state);
        r.addView(left, Ui.weight());
        LinearLayout.LayoutParams lp = Ui.wrap();
        lp.leftMargin = Ui.dp(this, 10);
        r.addView(right, lp);
        return r;
    }

    String version() {
        try {
            return getPackageManager().getPackageInfo(getPackageName(), 0).versionName;
        } catch (Exception e) {
            return "?";
        }
    }

    void openAccessibility() {
        startActivity(new Intent(android.provider.Settings.ACTION_ACCESSIBILITY_SETTINGS));
    }

    void openUrl(String u) {
        try {
            startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(u)));
        } catch (RuntimeException ignored) {
        }
    }

    void openAppInfo() {
        startActivity(new Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:" + getPackageName())));
    }

    void toggleCapture() {
        if (CaptureService.active != null) {
            Hub.stop();
            stopService(new Intent(this, CaptureService.class));
            return;
        }
        MediaProjectionManager mpm = getSystemService(MediaProjectionManager.class);
        Intent i = Build.VERSION.SDK_INT >= 34
                ? mpm.createScreenCaptureIntent(MediaProjectionConfig.createConfigForDefaultDisplay())
                : mpm.createScreenCaptureIntent();
        startActivityForResult(i, REQ_CAPTURE);
    }

    @Override
    protected void onActivityResult(int req, int res, Intent data) {
        super.onActivityResult(req, res, data);
        if (req != REQ_CAPTURE) return;
        if (res != RESULT_OK || data == null) {
            Hub.log("화면 캡처를 허용하지 않음", 'y');
            return;
        }
        Intent s = new Intent(this, CaptureService.class).putExtra("code", res).putExtra("data", data);
        startForegroundService(s);
    }

    static String pos(double[] a) {
        return a == null ? "없음" : "있음";
    }

    // ---------------------------------------------------------------- 새로 그리기
    void refresh() {
        showGuide();
        Integer on = Online.count;
        onText.setText("Now Active : " + (on == null ? "--" : String.valueOf(on)));
        onDot.setTextColor(on == null ? Ui.MUTED : Ui.GREEN);
        onText.setTextColor(on == null ? Ui.MUTED : Ui.FG);
        boolean ui = Hub.uiOn();
        for (TextView b : new TextView[]{uiBtn, uiBtn2}) {
            Ui.set(b, ui ? "UI 끄기" : "UI 켜기");
            Ui.paintButton(this, b, ui ? Ui.MUTED : Ui.ACCENT);
        }
        String us = Updater.status;
        Ui.set(updState, us);
        updState.setTextColor(Updater.newVer != null ? Ui.YELLOW : us.startsWith("최신") || us.startsWith("업데이트 완료") ? Ui.GREEN : Ui.MUTED);
        Ui.set(updBtn, Updater.newVer != null ? "업데이트" : "확인");
        updBtn.setEnabled(!Updater.busy);
        updBtn.setAlpha(Updater.busy ? 0.4f : 1f);
        String n = Updater.notes;
        updNotes.setText(Updater.newVer != null && n != null && !n.isEmpty() ? I18n.f("V{0} 바뀐 점", Updater.newVer) + "\n" + n : "");
        updNotes.setVisibility(updNotes.getText().length() > 0 ? View.VISIBLE : View.GONE);
        updBanner.setVisibility(Updater.newVer != null ? View.VISIBLE : View.GONE);
        if (Updater.newVer != null) Ui.set(updBanner, Updater.busy ? us : I18n.f("새 버전 V{0} 있음", Updater.newVer) + " — " + Ui.tr("업데이트"));

        boolean acc = TapService.instance != null, cap = CaptureService.active != null;
        Ui.set(accState, acc ? "✓ 켜짐" : "꺼짐");
        accState.setTextColor(acc ? Ui.GREEN : Ui.MUTED);
        Ui.set(accBtn, acc ? "설정" : "켜기");
        Ui.set(capState, cap ? "✓ 캡처 중" : "꺼짐");
        capState.setTextColor(cap ? Ui.GREEN : Ui.MUTED);
        Ui.set(capBtn, cap ? "중지" : "시작");

        com.acrux.mobile.core.Settings c = Hub.cfg;
        Ui.set(calState, "대기 창: " + pos(c.panelRegion) + "  ·  미니게임 창: " + (c.reelRegion == null ? "없음" : c.reelDerived ? "대기 창으로 계산" : "있음")
                + "  ·  결과창: " + pos(c.resultRegion) + (c.ready() ? "" : "\n→ 로블록스에서 [보정]을 해야 함"));
        long[] r = Hub.statRun.clone();
        String stats = "\n" + I18n.f("성공 {0} · 쓰레기 {1} · 실패 {2}", r[0], r[1], r[2]) + "  (" + Ui.tr("이번 실행") + ")";
        for (int k = 0; k < 2; k++) {
            long[] v = new long[3];
            for (int i = 0; i < 3; i++) v[i] = k == 0 ? r[i] : Hub.statAll(i);
            String[] cells = {String.valueOf(v[0]), String.valueOf(v[1]), String.valueOf(v[2]), pct(v[0] + v[1], v[0] + v[1] + v[2])  /* 쓰레기도 잡은 것 → (성공 + 쓰레기) / 전체 */};
            for (int i = 0; i < cells.length; i++) statCells[i][k].setText(cells[i]);
        }
        String mode = !Hub.running() ? "" : "cal".equals(Hub.mode) ? "자동 보정 중" : "sellcal".equals(Hub.mode) ? "판매 보정 중"
                : "sell".equals(Hub.mode) ? "판매 중" : "run".equals(Hub.mode) ? "자동 낚시 중"
                : Hub.mode.startsWith("rec_") ? "길 기록 중" : Hub.mode.startsWith("stick") ? "조이스틱" : "이동 테스트 중";
        runState.setText((mode.isEmpty() ? "" : Ui.tr(mode) + " · ") + Ui.tr(Hub.state) + stats);

        StringBuilder ps = new StringBuilder();
        double[] stk = Hub.stick;
        ps.append(stk == null ? "· " : "✓ ").append(Ui.tr("조이스틱")).append(stk == null ? "" : "  " + TapService.fmt(stk[0]) + ", " + TapService.fmt(stk[1]) + "  r " + TapService.fmt(stk[2])).append('\n');
        for (String[] k : Mover.BASE_KEYS) line(ps, Ui.tr(k[1]), Hub.pos.get(k[0]));
        for (String[] k : Mover.JUMP_KEYS) line(ps, Ui.tr(k[1]), Hub.pos.get(k[0]));
        for (String[] k : Seller.TALK) line(ps, Ui.tr(k[1]), Hub.pos.get(k[0]));
        for (String[] k : Seller.KEYS) line(ps, I18n.f("{0} (판매)", Ui.tr(k[1])), Hub.pos.get(k[0]));
        route(ps, Ui.tr("낚시 장소 길"), Hub.fishRoute);
        route(ps, Ui.tr("판매 장소 길"), Hub.sellRoute);
        posView.setText(ps.toString().trim());

        StringBuilder s = new StringBuilder();
        for (String l : Hub.recentLogsShown(300)) s.insert(0, l + "\n");
        if (s.length() == 0) Ui.set(logView, "아직 기록 없음");
        else logView.setText(s.toString().trim());
    }

    static void line(StringBuilder b, String name, double[] v) {
        b.append(v == null ? "· " : "✓ ").append(name);
        if (v != null) b.append("  ").append(v[0]).append(", ").append(v[1]);
        b.append('\n');
    }

    static void route(StringBuilder b, String name, com.acrux.mobile.core.Route r) {
        b.append(r.isEmpty() ? "· " : "✓ ").append(name);
        if (!r.isEmpty()) b.append("  ").append(I18n.f("{0}초", TapService.fmt(r.seconds()))).append(" · ").append(I18n.f("{0}조각", r.segs.size()))
                .append(" · ").append(I18n.f("점프 {0}번", r.jumps()));
        b.append('\n');
    }
}
