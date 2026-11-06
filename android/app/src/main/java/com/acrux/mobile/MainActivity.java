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

import com.acrux.mobile.core.Mover;
import com.acrux.mobile.core.Seller;
import com.acrux.mobile.core.TouchPc;

/** 처음 설정 · 상태 · 세부 설정 · 기록 (실제 조작은 로블록스 위에 떠 있는 작은 창에서) */
public class MainActivity extends Activity {
    static final int REQ_CAPTURE = 7;

    TextView accState, capState, shzState, calState, logView, runState;
    TextView accBtn, capBtn, shzBtn;
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
        ShizukuPc.autoConnect();
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
        shzState = Ui.text(this, "", 13, Ui.MUTED, false);
        shzBtn = Ui.button(this, "연결", Ui.ACCENT, v -> ShizukuPc.connect());
        ready.addView(row("③ Shizuku (키보드 · 마우스)", "이동 · 판매에 PC 와 같은 키(Esc R Enter W A S D E O /)와 마우스(우클릭 Click to Move · Sell All 호버 후 클릭)를 보냄 · "
                + "없으면 터치 모드 (W A S D → 조이스틱 · 우클릭 → Tap to Move · Esc R Enter → 메뉴 리셋 · E → 상호작용 버튼)", shzState, shzBtn), Ui.fill(this, 12));
        TextView shzHelp = Ui.text(this, "Shizuku 설정: ① Play 스토어에서 Shizuku 설치 → ② 개발자 옵션 → 무선 디버깅 켜기 → ③ Shizuku 앱에서 '무선 디버깅으로 시작' (페어링 코드 입력) → ④ 여기서 [연결] → 허용. 폰을 재부팅하면 ③ 부터 다시 해야 함",
                12, Ui.YELLOW, false);
        shzHelp.setOnClickListener(v -> openUrl("https://shizuku.rikka.app/guide/setup/"));
        ready.addView(shzHelp, Ui.fill(this, 6));
        root.addView(ready, Ui.fill(this, 18));

        // ② 사용법
        LinearLayout how = Ui.card(this);
        how.addView(Ui.text(this, "사용법", 15, Ui.FG, true));
        String[] steps = {
                "1. 로블록스(Sol's RNG)를 전체 화면 가로로 켜고 낚시 자리로 가서 Fish 버튼이 보이게 하기",
                "2. 떠 있는 Acrux 창에서 [보정] — Fish 를 한 번 던져 대기 창 · 미니게임 창 · 결과창 위치를 자동으로 맞춤",
                "3. [시작] — 자동 낚시 · 멈출 때는 [정지]",
                "4. Acrux 창은 제목(ACRUX)을 끌어서 낚시 창 · 결과창을 가리지 않는 곳으로 옮기기",
                "5. 이동 · 판매 (Shizuku 필요 · PC 와 같음): ⚙ → 공통 위치 3개 → 낚시 장소 · 판매 장소 지점 추가 → 판매 자동 보정",
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

        // ⑤ 이동 · 판매 (PC 버전과 같은 설정)
        LinearLayout mv = Ui.card(this);
        mv.addView(Ui.text(this, "이동 · 판매", 15, Ui.FG, true));
        mv.addView(Ui.text(this, "PC 버전과 같음: 시작하면 리셋 → 기준 장소 → Click to Move 로 낚시 장소 · 인벤토리가 가득 차면 판매 장소에서 팔고 돌아옴 · "
                + "위치 · 지점은 Acrux 창 ⚙ 에서", 12, Ui.MUTED, false), Ui.fill(this, 4));
        mv.addView(sw("시작할 때 낚시 장소로 이동", Hub.useMove, on -> Hub.useMove = on), Ui.fill(this, 8));
        mv.addView(sw("가득 차면 판매하러 가기", Hub.useSell, on -> Hub.useSell = on), Ui.fill(this, 4));
        mv.addView(sw("Click to Move 우클릭 (끄면 좌클릭)", "right".equals(Hub.move.button), on -> Hub.move.button = on ? "right" : "left"), Ui.fill(this, 4));
        mv.addView(sw("카메라 돌리기를 상대 이동으로", Hub.relDrag, on -> Hub.relDrag = on), Ui.fill(this, 4));
        mv.addView(Ui.text(this, "내려다보기가 안 먹히면 켜 보기 (마우스 잠금 상태에서 움직임)", 12, Ui.MUTED, false));
        mv.addView(Ui.text(this, "터치 모드 (Shizuku 없을 때)", 14, Ui.FG, true), Ui.fill(this, 14));
        mv.addView(Ui.text(this, "PC 키 · 마우스를 로블록스 모바일 조작으로 바꿔 누름 · 조이스틱 · 메뉴 · 점프 · 상호작용 버튼 위치는 Acrux 창 ⚙ → 터치 조작", 12, Ui.MUTED, false));
        mv.addView(num("탭 누르는 시간 (ms)", "버튼이 호버만 되고 안 눌리면 늘림 · 기본 120", Hub.touchTapMs, v -> Hub.touchTapMs = (int) clamp(v, 30, 600)), Ui.fill(this, 6));
        mv.addView(num("내려다보기 배율", "마우스 드래그 px × 이 값 = 화면 끌기 거리 · 기본 0.5", Hub.touchTilt, v -> Hub.touchTilt = clamp(v, 0.05, 3)), Ui.fill(this, 6));
        mv.addView(sw("키보드 앱 방식 키 (실험 · 안드로이드 13+)", Hub.imeKeys, on -> Hub.imeKeys = on), Ui.fill(this, 6));
        mv.addView(Ui.text(this, "켜면 W A S D E Esc 같은 키를 접근성 입력기로 먼저 보내 봄 · 로블록스가 받아 주면 조이스틱 대신 진짜 키로 움직임 · 안 먹으면 끄기", 12, Ui.MUTED, false));
        mv.addView(Ui.text(this, "기준 장소 (초)", 14, Ui.FG, true), Ui.fill(this, 14));
        Mover.Cfg m = Hub.move;
        mv.addView(num("리셋 후 대기", "기본 3.5", m.resetWait, v -> m.resetWait = clamp(v, 0.5, 30)), Ui.fill(this, 6));
        mv.addView(num("W", "기본 1", m.wTime, v -> m.wTime = clamp(v, 0, 30)), Ui.fill(this, 6));
        mv.addView(num("W + A", "기본 7", m.waTime, v -> m.waTime = clamp(v, 0, 30)), Ui.fill(this, 6));
        mv.addView(num("A", "기본 1", m.aTime, v -> m.aTime = clamp(v, 0, 30)), Ui.fill(this, 6));
        mv.addView(num("A 끝 W", "A 를 누른 채 마지막에 W · 기본 0.25", m.w2Time, v -> m.w2Time = clamp(v, 0, 10)), Ui.fill(this, 6));
        mv.addView(num("S + D", "기본 0.25", m.sdTime, v -> m.sdTime = clamp(v, 0, 10)), Ui.fill(this, 6));
        mv.addView(num("내려다보기 (px)", "우클릭 드래그 거리 · 기본 800", m.tiltPx, v -> m.tiltPx = (int) clamp(v, 0, 3000)), Ui.fill(this, 6));
        mv.addView(num("O 줌 아웃", "기본 2.5", m.oTime, v -> m.oTime = clamp(v, 0, 10)), Ui.fill(this, 6));
        mv.addView(num("걷기 여유", "지점마다 잰 시간에 더함 · 기본 0.3", m.margin, v -> m.margin = clamp(v, 0, 5)), Ui.fill(this, 6));
        mv.addView(num("판매 최대 종류", "한 번에 파는 최대 종류 수 · 기본 100", Hub.sellMax, v -> Hub.sellMax = (int) clamp(v, 1, 500)), Ui.fill(this, 12));
        mv.addView(num("판매 클릭 추가 대기 (초)", "렉이 있으면 늘림 · 기본 0", Hub.sellDelay, v -> Hub.sellDelay = clamp(v, 0, 5)), Ui.fill(this, 8));
        posView = Ui.text(this, "", 12, Ui.MUTED, false);
        posView.setTypeface(Typeface.MONOSPACE);
        mv.addView(Ui.text(this, "저장된 위치 · 지점", 13, Ui.FG, true), Ui.fill(this, 12));
        mv.addView(posView, Ui.fill(this, 4));
        TextView clearPos = Ui.button(this, "위치 · 지점 모두 지우기", Ui.RED, v -> {
            Hub.pos.clear();
            Hub.fishPts.clear();
            Hub.sellPts.clear();
            Hub.save();
            Hub.changed();
            Hub.log("위치 · 지점 모두 지움", 'd');
        });
        LinearLayout.LayoutParams cp = Ui.wrap();
        cp.topMargin = Ui.dp(this, 8);
        mv.addView(clearPos, cp);
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

    void refresh() {
        boolean acc = TapService.instance != null, cap = CaptureService.active != null;
        accState.setText(acc ? "✓ 켜짐" : "꺼짐");
        accState.setTextColor(acc ? Ui.GREEN : Ui.MUTED);
        accBtn.setText(acc ? "설정" : "켜기");
        capState.setText(cap ? "✓ 캡처 중" : "꺼짐");
        capState.setTextColor(cap ? Ui.GREEN : Ui.MUTED);
        capBtn.setText(cap ? "중지" : "시작");
        boolean shz = ShizukuPc.ready();
        shzState.setText(shz ? "✓ 연결됨" : ShizukuPc.status());
        shzState.setTextColor(shz ? Ui.GREEN : Ui.MUTED);
        shzBtn.setText(shz ? "다시" : "연결");
        com.acrux.mobile.core.Settings c = Hub.cfg;
        calState.setText("대기 창: " + pos(c.panelRegion) + "  ·  미니게임 창: " + (c.reelRegion == null ? "없음" : c.reelDerived ? "대기 창으로 계산" : "있음")
                + "  ·  결과창: " + pos(c.resultRegion) + (c.ready() ? "" : "\n→ 로블록스에서 [보정]을 해야 함"));
        com.acrux.mobile.core.Fisher f = Hub.fisher;
        String stats = f == null ? "" : "\n성공 " + f.success + " · 쓰레기 " + f.junk + " · 실패 " + f.fail;
        String mode = !Hub.running() ? "" : "cal".equals(Hub.mode) ? "자동 보정 중 · " : "sellcal".equals(Hub.mode) ? "판매 보정 중 · "
                : "sell".equals(Hub.mode) ? "판매 중 · " : "run".equals(Hub.mode) ? "자동 낚시 중 · " : Hub.mode.startsWith("addpt") ? "지점 추가 중 · "
                : Hub.mode.startsWith("measure") ? "시간 재는 중 · " : "이동 테스트 중 · ";
        runState.setText(mode + Hub.state + stats);
        StringBuilder ps = new StringBuilder();
        for (String[] k : Mover.BASE_KEYS) line(ps, k[1], Hub.pos.get(k[0]));
        for (String[] k : Seller.KEYS) line(ps, k[1] + " (판매)", Hub.pos.get(k[0]));
        for (String[] k : TouchPc.KEYS) line(ps, k[1] + " (터치)", Hub.pos.get(k[0]));
        points(ps, "낚시 장소", Hub.fishPts);
        points(ps, "판매 장소", Hub.sellPts);
        posView.setText(ps.toString().trim());
        StringBuilder s = new StringBuilder();
        for (String l : Hub.recentLogs(40)) s.insert(0, l + "\n");
        logView.setText(s.length() == 0 ? "아직 기록 없음" : s.toString().trim());
    }

    static void line(StringBuilder b, String name, double[] v) {
        b.append(v == null ? "· " : "✓ ").append(name);
        if (v != null) b.append("  ").append(v[0]).append(", ").append(v[1]);
        b.append('\n');
    }

    static void points(StringBuilder b, String name, java.util.List<Mover.Point> pts) {
        b.append(name).append(" 지점 ").append(pts.size()).append("개").append('\n');
        for (int i = 0; i < pts.size(); i++) {
            Mover.Point p = pts.get(i);
            b.append("  ").append(i + 1).append(".  ").append(p.pos[0]).append(", ").append(p.pos[1])
                    .append("  ").append(p.time == null ? "시간 없음" : TapService.fmt(p.time) + "초").append('\n');
        }
    }
}
