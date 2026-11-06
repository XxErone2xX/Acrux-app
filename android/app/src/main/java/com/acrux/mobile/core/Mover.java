package com.acrux.mobile.core;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * 이동 (모바일) — PC 버전 move.py 와 같은 생각: 매번 '같은 자리 · 같은 화면'(기준 장소)을 만든 뒤 정해진 동작으로 감
 * 키보드가 없어서 동작은 터치로: 조이스틱(W/A/S/D 대신) · 화면 끌기(우클릭 드래그 대신) · 핀치(O 줌 대신) · 메뉴 리셋(Esc → R → Enter 대신)
 *
 * 경로 = 한 줄에 동작 하나 (# 뒤는 설명):
 *   리셋                         로블록스 메뉴 → Reset Character → Reset (글자로 찾음)
 *   대기 3.5                     초
 *   카메라정렬                   채팅 → 도감 → 도감 닫기 → 채팅 (PC 와 같은 카메라 맞추기)
 *   이동 W 1, WA 7, A 0.75        조이스틱: 방향 초 (쉼표로 이으면 손가락을 안 떼고 이어서) · 방향 = W A S D WA WD SA SD · 위 아래 왼쪽 오른쪽 · 각도(0 = 앞, 90 = 오른쪽)
 *   화면내리기 [횟수]             화면을 아래로 끌어 위에서 내려다보기 (기본 2번)
 *   화면돌리기 dx dy              화면을 끌어서 카메라 돌리기 (화면 비율 · 예: 0.3 0)
 *   줌아웃 [횟수] / 줌인 [횟수]   두 손가락 핀치 (기본 3번)
 *   점프 [횟수]
 *   터치 이름 / 터치 x y          지정한 위치(또는 화면 비율)를 한 번 누름
 *   꾹 이름 초                    그 위치를 초 동안 누르고 있음
 *   터치이동 이름 초              Tap to Move: 그 지점을 누르고 걸어갈 시간만큼 기다림 (+ 0.3초)
 *   상호작용                      E 대신: 상호작용 버튼(지정한 위치)을 누름
 */
public final class Mover {
    /** 터치 (Fisher.Input + 끌기 · 핀치 · 조이스틱) — 모두 끝날 때까지 기다렸다가 돌아옴 */
    public interface Gestures extends Fisher.Input {
        void swipe(int x1, int y1, int x2, int y2, int ms);

        void pinch(int cx, int cy, int far, int near, boolean zoomOut, int ms);

        /** 조이스틱: (cx, cy) 를 누른 채로 segs[i] = {각도, 초} 순서대로 반지름 r 만큼 밀기 · 손가락은 끝에만 뗌 */
        void stick(int cx, int cy, int r, double[][] segs, Runnable check);
    }

    /** 글자 읽기 (OCR): 화면의 비율 영역 [x1, y1, x2, y2] (null = 전체) → 덩어리 (위치는 화면 비율) */
    public interface Ocr {
        List<SellText.Box> read(double[] region);
    }

    public static final class Step {
        public final int line;
        public final String cmd;
        public final List<String> args;
        public final String raw;

        Step(int line, String cmd, List<String> args, String raw) {
            this.line = line;
            this.cmd = cmd;
            this.args = args;
            this.raw = raw;
        }
    }

    public static final class ParseError extends IllegalArgumentException {
        public ParseError(int line, String msg) {
            super(line + "번째 줄: " + msg);
        }
    }

    /** 위치 이름 (지정하는 곳) · 기본 자리 (지정 안 했을 때 · 로블록스 모바일 기본 배치 어림) */
    public static final Map<String, double[]> DEFAULTS = new LinkedHashMap<>();
    public static final Map<String, String> POS_HELP = new LinkedHashMap<>();

    static {
        DEFAULTS.put("조이스틱", new double[]{0.16, 0.72});
        DEFAULTS.put("점프버튼", new double[]{0.88, 0.78});
        DEFAULTS.put("메뉴버튼", new double[]{0.035, 0.06});
        POS_HELP.put("조이스틱", "왼쪽 아래 이동 조이스틱 가운데");
        POS_HELP.put("점프버튼", "오른쪽 아래 점프 버튼");
        POS_HELP.put("메뉴버튼", "왼쪽 위 로블록스 메뉴 버튼");
        POS_HELP.put("채팅버튼", "위쪽 채팅 버튼 (카메라정렬)");
        POS_HELP.put("도감버튼", "도감(Collection) 버튼 (카메라정렬)");
        POS_HELP.put("도감닫기", "도감 창의 닫기(Exit) 버튼 (카메라정렬)");
        POS_HELP.put("상호작용버튼", "NPC 앞에서 뜨는 상호작용 버튼 (E 대신)");
    }

    static final String[] CMDS = {"리셋", "대기", "카메라정렬", "이동", "화면내리기", "화면돌리기", "줌아웃", "줌인", "점프", "터치", "꾹",
            "터치이동", "상호작용"};

    // ---------------------------------------------------------------- 읽기
    public static List<Step> parse(String text) {
        List<Step> out = new ArrayList<>();
        String[] lines = String.valueOf(text == null ? "" : text).split("\n", -1);
        for (int i = 0; i < lines.length; i++) {
            String raw = lines[i];
            String s = raw;
            int c = s.indexOf('#');
            if (c >= 0) s = s.substring(0, c);
            s = s.trim();
            if (s.isEmpty()) continue;
            String cmd = s.split("\\s+", 2)[0];
            String rest = s.length() > cmd.length() ? s.substring(cmd.length()).trim() : "";
            // 띄어 쓴 이름도 받기: '카메라 정렬' · '화면 내리기' · '터치 이동' · '줌 아웃'
            String two = rest.isEmpty() ? "" : rest.split("\\s+", 2)[0];
            String joined = cmd + two;
            if (isCmd(joined) && !two.isEmpty()) {
                cmd = joined;
                rest = rest.substring(two.length()).trim();
            }
            cmd = alias(cmd);
            if (!isCmd(cmd)) throw new ParseError(i + 1, "모르는 동작 '" + cmd + "'");
            List<String> args = new ArrayList<>();
            if (cmd.equals("이동")) {
                for (String seg : rest.split(",")) if (!seg.trim().isEmpty()) args.add(seg.trim());
            } else if (!rest.isEmpty()) {
                for (String a : rest.split("\\s+")) args.add(a);
            }
            Step st = new Step(i + 1, cmd, args, raw.trim());
            check(st);
            out.add(st);
        }
        return out;
    }

    static boolean isCmd(String c) {
        for (String k : CMDS) if (k.equals(alias(c))) return true;
        return false;
    }

    static String alias(String c) {
        switch (c.toLowerCase(Locale.ROOT)) {
            case "reset": return "리셋";
            case "wait": case "기다리기": return "대기";
            case "align": case "카메라맞추기": return "카메라정렬";
            case "move": case "walk": case "걷기": return "이동";
            case "tilt": case "내려다보기": return "화면내리기";
            case "rotate": case "camera": return "화면돌리기";
            case "zoomout": return "줌아웃";
            case "zoomin": return "줌인";
            case "jump": return "점프";
            case "tap": case "click": case "클릭": return "터치";
            case "hold": case "길게": return "꾹";
            case "tapmove": case "클릭이동": return "터치이동";
            case "interact": case "e": return "상호작용";
            default: return c;
        }
    }

    static double num(Step st, int i, double def, double lo, double hi) {
        if (st.args.size() <= i) return def;
        try {
            double v = Double.parseDouble(st.args.get(i));
            if (v < lo || v > hi) throw new ParseError(st.line, "숫자는 " + fmt(lo) + " ~ " + fmt(hi) + " 사이");
            return v;
        } catch (NumberFormatException e) {
            throw new ParseError(st.line, "'" + st.args.get(i) + "' 는 숫자가 아님");
        }
    }

    static String fmt(double v) {
        return v == Math.rint(v) ? String.valueOf((long) v) : String.valueOf(v);
    }

    /** 방향 → 각도 (0 = 앞(W), 90 = 오른쪽(D), 180 = 뒤(S), 270 = 왼쪽(A)) · 모르면 NaN */
    public static double angle(String d) {
        String s = d.trim().toLowerCase(Locale.ROOT).replace(" ", "").replace("+", "");
        switch (s) {
            case "w": case "위": case "앞": return 0;
            case "d": case "오른쪽": return 90;
            case "s": case "아래": case "뒤": return 180;
            case "a": case "왼쪽": return 270;
            case "wd": case "dw": case "위오른쪽": case "앞오른쪽": case "오른쪽위": return 45;
            case "sd": case "ds": case "아래오른쪽": case "뒤오른쪽": case "오른쪽아래": return 135;
            case "sa": case "as": case "아래왼쪽": case "뒤왼쪽": case "왼쪽아래": return 225;
            case "wa": case "aw": case "위왼쪽": case "앞왼쪽": case "왼쪽위": return 315;
            default:
                try {
                    return Double.parseDouble(s.replace("°", ""));
                } catch (NumberFormatException e) {
                    return Double.NaN;
                }
        }
    }

    /** '이동' 한 줄 → {각도, 초} 목록 */
    public static double[][] segments(Step st) {
        double[][] out = new double[st.args.size()][];
        for (int i = 0; i < st.args.size(); i++) {
            String[] p = st.args.get(i).trim().split("\\s+");
            if (p.length != 2) throw new ParseError(st.line, "'" + st.args.get(i) + "' — '방향 초' 로 적기 (예: WA 7)");
            double a = angle(p[0]);
            if (Double.isNaN(a)) throw new ParseError(st.line, "모르는 방향 '" + p[0] + "' (W A S D WA SD … 또는 각도)");
            double sec;
            try {
                sec = Double.parseDouble(p[1]);
            } catch (NumberFormatException e) {
                throw new ParseError(st.line, "'" + p[1] + "' 는 숫자가 아님");
            }
            if (sec < 0 || sec > 60) throw new ParseError(st.line, "이동 시간은 0 ~ 60초");
            out[i] = new double[]{a, sec};
        }
        return out;
    }

    static void check(Step st) {
        switch (st.cmd) {
            case "대기":
                if (st.args.isEmpty()) throw new ParseError(st.line, "대기할 초를 적기 (예: 대기 1.5)");
                num(st, 0, 0, 0, 600);
                break;
            case "이동":
                if (st.args.isEmpty()) throw new ParseError(st.line, "방향과 초를 적기 (예: 이동 W 1, WA 7)");
                segments(st);
                break;
            case "화면내리기":
                num(st, 0, 2, 1, 10);
                break;
            case "화면돌리기":
                if (st.args.size() < 2) throw new ParseError(st.line, "끌 거리 dx dy 를 화면 비율로 적기 (예: 화면돌리기 0.3 0)");
                num(st, 0, 0, -1, 1);
                num(st, 1, 0, -1, 1);
                break;
            case "줌아웃":
            case "줌인":
                num(st, 0, 3, 1, 20);
                break;
            case "점프":
                num(st, 0, 1, 1, 20);
                break;
            case "터치":
                if (st.args.isEmpty()) throw new ParseError(st.line, "누를 위치 이름(또는 x y)을 적기");
                if (st.args.size() >= 2 && isNum(st.args.get(0)) && isNum(st.args.get(1))) {
                    num(st, 0, 0, 0, 1);
                    num(st, 1, 0, 0, 1);
                }
                break;
            case "꾹":
                if (st.args.size() < 2) throw new ParseError(st.line, "위치 이름과 초를 적기 (예: 꾹 점프버튼 1)");
                num(st, st.args.size() - 1, 1, 0, 60);
                break;
            case "터치이동":
                if (st.args.size() < 2) throw new ParseError(st.line, "지점 이름과 걸리는 초를 적기 (예: 터치이동 낚시1 4.2)");
                num(st, st.args.size() - 1, 1, 0, 120);
                break;
            default:
                break;
        }
    }

    static boolean isNum(String s) {
        try {
            Double.parseDouble(s);
            return true;
        } catch (NumberFormatException e) {
            return false;
        }
    }

    /** 이 경로에서 지정해야 하는 위치 이름들 */
    public static List<String> needed(List<Step> steps) {
        List<String> out = new ArrayList<>();
        for (Step st : steps) {
            switch (st.cmd) {
                case "리셋":
                    add(out, "메뉴버튼");
                    break;
                case "카메라정렬":
                    add(out, "채팅버튼");
                    add(out, "도감버튼");
                    add(out, "도감닫기");
                    break;
                case "이동":
                    add(out, "조이스틱");
                    break;
                case "점프":
                    add(out, "점프버튼");
                    break;
                case "상호작용":
                    add(out, "상호작용버튼");
                    break;
                case "터치":
                    if (!(st.args.size() >= 2 && isNum(st.args.get(0)))) add(out, st.args.get(0));
                    break;
                case "꾹":
                case "터치이동":
                    add(out, String.join(" ", st.args.subList(0, st.args.size() - 1)));
                    break;
                default:
                    break;
            }
        }
        return out;
    }

    static void add(List<String> l, String s) {
        if (!l.contains(s)) l.add(s);
    }

    // ---------------------------------------------------------------- 실행
    final Fisher.Screen scr;
    final Gestures g;
    final Ocr ocr;
    final Map<String, double[]> pos;
    final Fisher.Out out;
    final Runnable check;            // 멈춤 확인 (멈췄으면 Fisher.Stopped)
    public double margin = 0.3;      // 터치이동 뒤 더 기다릴 시간 (초)

    public Mover(Fisher.Screen scr, Gestures g, Ocr ocr, Map<String, double[]> pos, Fisher.Out out, Runnable check) {
        this.scr = scr;
        this.g = g;
        this.ocr = ocr;
        this.pos = pos;
        this.out = out;
        this.check = check;
    }

    double[] where(String name) {
        double[] p = pos.get(name);
        if (p == null) p = DEFAULTS.get(name);
        if (p == null) throw new RuntimeException("'" + name + "' 위치를 먼저 지정해야 함 (Acrux 창 → ⚙ → 위치 지정)");
        return p;
    }

    /** 위치가 빠진 이름 (기본 자리가 있는 건 빼고) */
    public List<String> missing(List<Step> steps) {
        List<String> m = new ArrayList<>();
        for (String n : needed(steps)) if (!pos.containsKey(n) && !DEFAULTS.containsKey(n)) m.add(n);
        return m;
    }

    int[] px(double[] p) {
        return new int[]{(int) (p[0] * scr.width()), (int) (p[1] * scr.height())};
    }

    void sleep(double sec) {
        long end = System.nanoTime() + (long) (Math.max(0, sec) * 1e9);
        while (true) {
            check.run();
            long left = end - System.nanoTime();
            if (left <= 0) return;
            try {
                Thread.sleep(Math.max(1, Math.min(30, left / 1_000_000)));
            } catch (InterruptedException e) {
                throw new Fisher.Stopped();
            }
        }
    }

    public void tapAt(double[] p) {
        int[] q = px(p);
        g.tap(q[0], q[1], 40);
    }

    /** 경로 실행 · name = 화면에 보일 이름 */
    public void run(String text, String name) {
        List<Step> steps = parse(text);
        List<String> miss = missing(steps);
        if (!miss.isEmpty()) throw new RuntimeException(name + ": 위치 지정 필요 — " + String.join(", ", miss));
        int n = 0;
        for (Step st : steps) {
            check.run();
            n++;
            out.state(name + " (" + n + "/" + steps.size() + ") · " + st.raw);
            exec(st);
        }
    }

    void exec(Step st) {
        int W = scr.width(), H = scr.height();
        switch (st.cmd) {
            case "대기":
                sleep(num(st, 0, 0, 0, 600));
                break;
            case "리셋":
                reset();
                break;
            case "카메라정렬":
                for (String k : new String[]{"채팅버튼", "도감버튼", "도감닫기", "채팅버튼"}) {
                    tapAt(where(k));
                    sleep(0.5);
                }
                break;
            case "이동": {
                int[] c = px(where("조이스틱"));
                int r = Math.max(20, (int) (Math.min(W, H) * 0.09));
                g.stick(c[0], c[1], r, segments(st), check);
                break;
            }
            case "화면내리기": {
                int times = (int) num(st, 0, 2, 1, 10);
                for (int i = 0; i < times; i++) {
                    g.swipe((int) (W * 0.62), (int) (H * 0.25), (int) (W * 0.62), (int) (H * 0.85), 350);
                    sleep(0.15);
                }
                break;
            }
            case "화면돌리기": {
                double dx = num(st, 0, 0, -1, 1), dy = num(st, 1, 0, -1, 1);
                int x0 = (int) (W * 0.62), y0 = (int) (H * 0.5);
                g.swipe(x0, y0, clamp(x0 + (int) (dx * W), 1, W - 2), clamp(y0 + (int) (dy * H), 1, H - 2), 400);
                sleep(0.1);
                break;
            }
            case "줌아웃":
            case "줌인": {
                int times = (int) num(st, 0, 3, 1, 20);
                boolean outZ = st.cmd.equals("줌아웃");
                for (int i = 0; i < times; i++) {
                    g.pinch((int) (W * 0.62), (int) (H * 0.45), (int) (H * 0.36), (int) (H * 0.05), outZ, 300);
                    sleep(0.12);
                }
                break;
            }
            case "점프": {
                int times = (int) num(st, 0, 1, 1, 20);
                for (int i = 0; i < times; i++) {
                    tapAt(where("점프버튼"));
                    sleep(0.35);
                }
                break;
            }
            case "터치":
                if (st.args.size() >= 2 && isNum(st.args.get(0)) && isNum(st.args.get(1))) {
                    tapAt(new double[]{num(st, 0, 0, 0, 1), num(st, 1, 0, 0, 1)});
                } else {
                    tapAt(where(String.join(" ", st.args)));
                }
                sleep(0.15);
                break;
            case "꾹": {
                double sec = num(st, st.args.size() - 1, 1, 0, 60);
                int[] q = px(where(String.join(" ", st.args.subList(0, st.args.size() - 1))));
                g.tap(q[0], q[1], (int) (sec * 1000));
                break;
            }
            case "터치이동": {
                double sec = num(st, st.args.size() - 1, 1, 0, 120);
                tapAt(where(String.join(" ", st.args.subList(0, st.args.size() - 1))));
                sleep(sec + margin);
                break;
            }
            case "상호작용":
                tapAt(where("상호작용버튼"));
                sleep(0.2);
                break;
            default:
                break;
        }
    }

    static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }

    /** 로블록스 메뉴 → Reset Character → Reset (글자로 찾음 · 못 찾으면 지정한 리셋버튼 · 리셋확인) */
    void reset() {
        tapAt(where("메뉴버튼"));
        sleep(0.9);
        SellText.Box b = null;
        for (int i = 0; i < 4 && b == null; i++) {
            b = SellText.findText(ocr.read(null), "resetcharacter", "resetchar");
            if (b == null) sleep(0.4);
        }
        if (b != null) {
            tapAt(new double[]{b.x, b.y});
        } else if (pos.containsKey("리셋버튼")) {
            tapAt(pos.get("리셋버튼"));
        } else {
            tapAt(where("메뉴버튼"));          // 메뉴 닫기
            throw new RuntimeException("메뉴에서 Reset Character 를 찾지 못함 (메뉴버튼 위치 · 또는 '리셋버튼' 위치를 지정)");
        }
        sleep(0.7);
        SellText.Box ok = null;
        for (int i = 0; i < 4 && ok == null; i++) {
            List<SellText.Box> boxes = ocr.read(null);
            ok = SellText.findText(boxes, true, "reset");
            if (ok == null) {
                SellText.Box c = SellText.findText(boxes, "resetcharacter");
                if (c == null && b != null) break;      // 확인 없이 바로 리셋됨
            }
            if (ok == null) sleep(0.4);
        }
        if (ok != null) tapAt(new double[]{ok.x, ok.y});
        else if (pos.containsKey("리셋확인")) tapAt(pos.get("리셋확인"));
        out.log("리셋", 'd');
    }

    // ---------------------------------------------------------------- 기본 경로 (PC 기준 장소와 같은 순서)
    public static final String BASE_DEFAULT = String.join("\n",
            "# 기준 장소: 리셋 → 카메라 정렬 → 구석으로 걸어가 항상 같은 자리 → 위에서 내려다보기 → 최대 줌 아웃",
            "# (PC: Esc → R → Enter · 채팅 · 도감 · W 1초 → W+A 7초 → A 1초(끝 0.25초 W 같이) → S+D 0.25초 · 우클릭 드래그 + O)",
            "리셋",
            "대기 3.5",
            "카메라정렬",
            "이동 W 1, WA 7, A 0.75, WA 0.25",
            "이동 SD 0.25",
            "대기 0.5",
            "화면내리기 2",
            "줌아웃 3",
            "대기 0.3");
    public static final String FISH_DEFAULT = String.join("\n",
            "# 기준 장소에서 낚시 자리까지 (비워 두면 지금 자리에서 낚시)",
            "# 예: 이동 W 2.5   또는   터치이동 낚시1 4.2  (Acrux 창 → ⚙ → 지점 추가로 위치 · 시간을 잼)");
    public static final String SELL_DEFAULT = String.join("\n",
            "# 기준 장소에서 물고기 판매 NPC(Captain Flarg) 앞까지 (비워 두면 판매 안 함)",
            "# 예: 이동 S 3, SA 2   또는   터치이동 판매1 5.1");
}
