package com.acrux.mobile;

import android.app.Activity;
import android.content.Intent;
import android.graphics.Typeface;
import android.media.projection.MediaProjectionConfig;
import android.media.projection.MediaProjectionManager;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.text.InputType;
import android.view.Gravity;
import android.view.View;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.Switch;
import android.widget.TextView;

/** 처음 설정 · 상태 · 세부 설정 · 기록 (실제 조작은 로블록스 위에 떠 있는 작은 창에서) */
public class MainActivity extends Activity {
    static final int REQ_CAPTURE = 7;

    TextView accState, capState, calState, logView, runState;
    TextView accBtn, capBtn;
    final Runnable refresh = this::refresh;

    @Override
    protected void onCreate(Bundle b) {
        super.onCreate(b);
        Hub.init(this);
        getWindow().getDecorView().setBackgroundColor(Ui.BG);
        if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission("android.permission.POST_NOTIFICATIONS") != 0) {
            requestPermissions(new String[]{"android.permission.POST_NOTIFICATIONS"}, 1);
        }
        setContentView(build());
    }

    @Override
    protected void onResume() {
        super.onResume();
        Hub.listeners.add(refresh);
        refresh();
    }

    @Override
    protected void onPause() {
        Hub.listeners.remove(refresh);
        super.onPause();
    }

    View build() {
        ScrollView sv = new ScrollView(this);
        sv.setBackgroundColor(Ui.BG);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        int p = Ui.dp(this, 18);
        root.setPadding(p, Ui.dp(this, 44), p, Ui.dp(this, 32));
        sv.addView(root);

        // 머리말
        LinearLayout head = new LinearLayout(this);
        head.setOrientation(LinearLayout.HORIZONTAL);
        head.setGravity(Gravity.BOTTOM);
        head.addView(Ui.text(this, "Acrux Mobile ", 28, Ui.FG, true));
        head.addView(Ui.text(this, "V" + version(), 28, Ui.ACCENT, true));
        root.addView(head);
        root.addView(Ui.text(this, "자동 낚시 [ Beta ]", 16, Ui.FG, true));
        root.addView(Ui.text(this, "Made by 얼로니", 13, Ui.MUTED, false));

        // ① 준비
        LinearLayout ready = Ui.card(this);
        ready.addView(Ui.text(this, "준비", 15, Ui.FG, true));
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
        root.addView(ready, Ui.fill(this, 18));

        // ② 사용법
        LinearLayout how = Ui.card(this);
        how.addView(Ui.text(this, "사용법", 15, Ui.FG, true));
        String[] steps = {
                "1. 로블록스(Sol's RNG)를 전체 화면 가로로 켜고 낚시 자리로 가서 Fish 버튼이 보이게 하기",
                "2. 떠 있는 Acrux 창에서 [보정] — Fish 를 한 번 던져 대기 창 · 미니게임 창 · 결과창 위치를 자동으로 맞춤",
                "3. [시작] — 자동 낚시 · 멈출 때는 [정지]",
                "4. Acrux 창은 제목(ACRUX)을 끌어서 낚시 창 · 결과창을 가리지 않는 곳으로 옮기기",
        };
        for (String s : steps) how.addView(Ui.text(this, s, 13, Ui.MUTED, false), Ui.fill(this, 6));
        root.addView(how, Ui.fill(this, 12));

        // ③ 상태
        LinearLayout st = Ui.card(this);
        st.addView(Ui.text(this, "상태", 15, Ui.FG, true));
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
        root.addView(st, Ui.fill(this, 12));

        // ④ 세부 설정
        LinearLayout set = Ui.card(this);
        set.addView(Ui.text(this, "세부 설정", 15, Ui.FG, true));
        set.addView(num("입질 최대 대기 (초)", "넘으면 Exit 후 다시 던짐 · 기본 60", Hub.cfg.biteMax, v -> Hub.cfg.biteMax = clamp(v, 5, 600)), Ui.fill(this, 10));
        set.addView(num("누르는 시간 (ms)", "릴링 때 한 번 누르는 시간 · 기본 30", Hub.cfg.clickMs, v -> Hub.cfg.clickMs = (int) clamp(v, 5, 200)), Ui.fill(this, 8));
        set.addView(num("미리 누르기 (ms)", "터치가 늦게 먹히면 이만큼 미리 누름 · 기본 0 · 릴링이 자꾸 실패하면 40 ~ 80", Hub.cfg.leadMs,
                v -> Hub.cfg.leadMs = clamp(v, 0, 300)), Ui.fill(this, 8));
        set.addView(num("목표 위치 (%)", "구간 왼쪽 끝에서 구간 폭의 몇 % · 기본 0", Hub.cfg.targetPct, v -> Hub.cfg.targetPct = clamp(v, 0, 60)), Ui.fill(this, 8));
        Switch ad = new Switch(this);
        ad.setText("터치 지연에 맞춰 기다리기");
        ad.setTextColor(Ui.FG);
        ad.setChecked(Hub.cfg.adaptive);
        ad.setOnCheckedChangeListener((v, on) -> {
            Hub.cfg.adaptive = on;
            Hub.save();
        });
        set.addView(ad, Ui.fill(this, 10));
        set.addView(Ui.text(this, "누른 뒤 반응이 보일 때까지 다시 누르지 않는 시간을 이 폰의 터치 속도에 맞춤", 12, Ui.MUTED, false));
        root.addView(set, Ui.fill(this, 12));

        // ⑤ 이동 · 판매
        LinearLayout mv = Ui.card(this);
        mv.addView(Ui.text(this, "이동 · 판매", 15, Ui.FG, true));
        mv.addView(Ui.text(this, "PC 버전처럼: 시작하면 기준 장소 → 낚시 장소로 가서 낚시 · 인벤토리가 가득 차면 판매 장소에서 팔고 돌아옴 · "
                + "경로는 한 줄에 동작 하나 (아래 도움말) · 위치는 Acrux 창 ⚙ 에서 지정", 12, Ui.MUTED, false), Ui.fill(this, 4));
        mv.addView(sw("시작할 때 낚시 장소로 이동", Hub.useMove, on -> Hub.useMove = on), Ui.fill(this, 8));
        mv.addView(sw("가득 차면 판매하러 가기", Hub.useSell, on -> Hub.useSell = on), Ui.fill(this, 4));
        mv.addView(route("기준 장소", "리셋 → 카메라 정렬 → 구석까지 걷기 → 내려다보기 → 줌 아웃 (PC 와 같은 순서)", 0), Ui.fill(this, 12));
        mv.addView(route("낚시 장소", "기준 장소에서 낚시 자리까지", 1), Ui.fill(this, 12));
        mv.addView(route("판매 장소", "기준 장소에서 Captain Flarg 앞까지", 2), Ui.fill(this, 12));
        mv.addView(num("판매 최대 종류", "한 번에 파는 최대 종류 수 · 기본 100", Hub.sellMax, v -> Hub.sellMax = (int) clamp(v, 1, 500)), Ui.fill(this, 12));
        mv.addView(num("판매 클릭 추가 대기 (초)", "렉이 있으면 늘림 · 기본 0", Hub.sellDelay, v -> Hub.sellDelay = clamp(v, 0, 5)), Ui.fill(this, 8));
        posView = Ui.text(this, "", 12, Ui.MUTED, false);
        posView.setTypeface(Typeface.MONOSPACE);
        mv.addView(Ui.text(this, "지정한 위치", 13, Ui.FG, true), Ui.fill(this, 12));
        mv.addView(posView, Ui.fill(this, 4));
        TextView clearPos = Ui.button(this, "위치 모두 지우기", Ui.RED, v -> {
            Hub.pos.clear();
            Hub.save();
            Hub.changed();
            Hub.log("지정한 위치 모두 지움", 'd');
        });
        LinearLayout.LayoutParams cp = Ui.wrap();
        cp.topMargin = Ui.dp(this, 8);
        mv.addView(clearPos, cp);
        TextView help = Ui.text(this, HELP, 12, Ui.MUTED, false);
        help.setTypeface(Typeface.MONOSPACE);
        mv.addView(Ui.text(this, "경로 도움말", 13, Ui.FG, true), Ui.fill(this, 14));
        mv.addView(help, Ui.fill(this, 4));
        root.addView(mv, Ui.fill(this, 12));

        // ⑥ 기록
        LinearLayout lg = Ui.card(this);
        lg.addView(Ui.text(this, "기록", 15, Ui.FG, true));
        logView = Ui.text(this, "", 12, Ui.MUTED, false);
        logView.setTypeface(Typeface.MONOSPACE);
        lg.addView(logView, Ui.fill(this, 8));
        root.addView(lg, Ui.fill(this, 12));
        return sv;
    }

    interface Num {
        void set(double v);
    }

    interface Flag {
        void set(boolean on);
    }

    TextView posView;
    final EditText[] routes = new EditText[3];
    final TextView[] routeNotes = new TextView[3];

    static final String HELP = String.join("\n",
            "리셋                    메뉴 → Reset Character → Reset",
            "대기 3.5                초",
            "카메라정렬              채팅 → 도감 → 도감닫기 → 채팅",
            "이동 W 1, WA 7          조이스틱 (쉼표 = 손가락 안 떼고)",
            "                        W A S D WA WD SA SD · 각도(0=앞)",
            "화면내리기 2            화면을 아래로 끌어 내려다보기",
            "화면돌리기 0.3 0        카메라 돌리기 (화면 비율)",
            "줌아웃 3 / 줌인 3       두 손가락 핀치",
            "점프 1",
            "터치 이름 / 터치 x y    한 번 누름",
            "꾹 이름 1.5             누르고 있기",
            "터치이동 낚시1 4.2      Tap to Move + 걸리는 초",
            "상호작용                E 대신 상호작용 버튼",
            "# 뒤는 설명");

    View sw(String title, boolean on, Flag f) {
        Switch s = new Switch(this);
        s.setText(title);
        s.setTextColor(Ui.FG);
        s.setChecked(on);
        s.setOnCheckedChangeListener((v, c) -> {
            f.set(c);
            Hub.save();
        });
        return s;
    }

    /** 경로 편집 칸 (i: 0 기준 · 1 낚시 · 2 판매) — 칸을 벗어나면 확인 · 저장 */
    View route(String title, String sub, int i) {
        LinearLayout l = new LinearLayout(this);
        l.setOrientation(LinearLayout.VERTICAL);
        l.addView(Ui.text(this, title, 14, Ui.FG, true));
        l.addView(Ui.text(this, sub, 12, Ui.MUTED, false));
        EditText e = new EditText(this);
        e.setTypeface(Typeface.MONOSPACE);
        e.setTextSize(13);
        e.setTextColor(Ui.FG);
        e.setBackgroundColor(Ui.FIELD);
        e.setPadding(Ui.dp(this, 10), Ui.dp(this, 8), Ui.dp(this, 10), Ui.dp(this, 8));
        e.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);
        e.setGravity(Gravity.TOP | Gravity.START);
        e.setMinLines(3);
        e.setText(i == 0 ? Hub.routeBase : i == 1 ? Hub.routeFish : Hub.routeSell);
        TextView note = Ui.text(this, "", 12, Ui.MUTED, false);
        e.setOnFocusChangeListener((v, has) -> {
            if (!has) saveRoute(i);
        });
        routes[i] = e;
        routeNotes[i] = note;
        l.addView(e, Ui.fill(this, 6));
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER_VERTICAL);
        row.addView(note, Ui.weight());
        String def = i == 0 ? com.acrux.mobile.core.Mover.BASE_DEFAULT : i == 1 ? com.acrux.mobile.core.Mover.FISH_DEFAULT : com.acrux.mobile.core.Mover.SELL_DEFAULT;
        TextView reset = Ui.button(this, "기본값", Ui.MUTED, v -> {
            e.setText(def);
            saveRoute(i);
        });
        row.addView(reset, Ui.wrap());
        l.addView(row, Ui.fill(this, 4));
        checkRoute(i);
        return l;
    }

    void saveRoute(int i) {
        String t = routes[i].getText().toString();
        if (i == 0) Hub.routeBase = t;
        else if (i == 1) Hub.routeFish = t;
        else Hub.routeSell = t;
        Hub.save();
        checkRoute(i);
    }

    void checkRoute(int i) {
        String t = i == 0 ? Hub.routeBase : i == 1 ? Hub.routeFish : Hub.routeSell;
        TextView n = routeNotes[i];
        try {
            java.util.List<com.acrux.mobile.core.Mover.Step> st = com.acrux.mobile.core.Mover.parse(t);
            java.util.List<String> need = new java.util.ArrayList<>();
            for (String k : com.acrux.mobile.core.Mover.needed(st)) {
                if (!Hub.pos.containsKey(k) && !com.acrux.mobile.core.Mover.DEFAULTS.containsKey(k)) need.add(k);
            }
            if (st.isEmpty()) {
                n.setText("비어 있음 (안 씀)");
                n.setTextColor(Ui.MUTED);
            } else if (!need.isEmpty()) {
                n.setText("동작 " + st.size() + "개 · 위치 지정 필요: " + String.join(", ", need));
                n.setTextColor(Ui.YELLOW);
            } else {
                n.setText("✓ 동작 " + st.size() + "개");
                n.setTextColor(Ui.GREEN);
            }
        } catch (IllegalArgumentException ex) {
            n.setText(ex.getMessage());
            n.setTextColor(Ui.RED);
        }
    }

    static double clamp(double v, double lo, double hi) {
        return Math.max(lo, Math.min(hi, v));
    }

    View num(String title, String sub, double value, Num on) {
        EditText e = new EditText(this);
        e.setInputType(InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_FLAG_DECIMAL);
        e.setText(value == Math.rint(value) ? String.valueOf((long) value) : String.valueOf(value));
        e.setTextColor(Ui.FG);
        e.setBackgroundColor(Ui.FIELD);
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

    void refresh() {
        boolean acc = TapService.instance != null, cap = CaptureService.active != null;
        accState.setText(acc ? "✓ 켜짐" : "꺼짐");
        accState.setTextColor(acc ? Ui.GREEN : Ui.MUTED);
        accBtn.setText(acc ? "설정" : "켜기");
        capState.setText(cap ? "✓ 캡처 중" : "꺼짐");
        capState.setTextColor(cap ? Ui.GREEN : Ui.MUTED);
        capBtn.setText(cap ? "중지" : "시작");
        com.acrux.mobile.core.Settings c = Hub.cfg;
        calState.setText("대기 창: " + pos(c.panelRegion) + "  ·  미니게임 창: " + (c.reelRegion == null ? "없음" : c.reelDerived ? "대기 창으로 계산" : "있음")
                + "  ·  결과창: " + pos(c.resultRegion) + (c.ready() ? "" : "\n→ 로블록스에서 [보정]을 해야 함"));
        com.acrux.mobile.core.Fisher f = Hub.fisher;
        String stats = f == null ? "" : "\n성공 " + f.success + " · 쓰레기 " + f.junk + " · 실패 " + f.fail;
        String mode = !Hub.running() ? "" : "cal".equals(Hub.mode) ? "자동 보정 중 · " : "sellcal".equals(Hub.mode) ? "판매 보정 중 · "
                : "sell".equals(Hub.mode) ? "판매 중 · " : Hub.mode.startsWith("test") ? "이동 테스트 중 · " : "자동 낚시 중 · ";
        runState.setText(mode + Hub.state + stats);
        StringBuilder ps = new StringBuilder();
        for (String n : Hub.positionNames()) {
            double[] v = Hub.pos.get(n);
            ps.append(v == null ? "·" : "✓").append(' ').append(n);
            if (v != null) ps.append("  ").append(v[0]).append(", ").append(v[1]);
            else if (com.acrux.mobile.core.Mover.DEFAULTS.containsKey(n)) ps.append("  (기본 자리)");
            ps.append('\n');
        }
        for (String[] k : com.acrux.mobile.core.Seller.KEYS) ps.append(Hub.pos.containsKey(k[0]) ? "✓ " : "· ").append(k[1]).append(" (판매 보정)\n");
        posView.setText(ps.toString().trim());
        for (int i = 0; i < 3; i++) {
            String cur = i == 0 ? Hub.routeBase : i == 1 ? Hub.routeFish : Hub.routeSell;
            if (routes[i] != null && !routes[i].hasFocus() && !routes[i].getText().toString().equals(cur)) routes[i].setText(cur);
            if (routes[i] != null) checkRoute(i);
        }
        StringBuilder s = new StringBuilder();
        for (String l : Hub.recentLogs(40)) s.insert(0, l + "\n");
        logView.setText(s.length() == 0 ? "아직 기록 없음" : s.toString().trim());
    }
}
