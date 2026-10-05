# -*- coding: utf-8 -*-
"""
매크로 탭 · 자동 낚시 (제자리 낚시)
상태 기계: 매번 화면을 보고 지금 상태를 정한 뒤 그 상태에 맞는 행동만 함 → 클릭 하나를 놓치거나 화면이 늦게 바뀌어도 다음 번에 바로잡힘
  · 대기(파란 Fish)  → Fish 클릭 · 1.5초 안에 안 바뀌면 다시 · cast_retry 번 넘으면 인벤토리 가득
  · 입질 대기(빨간 Exit) → 기다림 · bite_max 초 넘거나 다른 기능이 자리를 달라 하면 Exit
  · 릴링(릴링 바가 보임) → 릴링 → Fish 버튼이 다시 보일 때까지 결과창 X 를 0.1초마다 (그 사이 제목 색으로 결과 기록)
  · 알 수 없음 3초 이상 → X 한 번 (처음부터 결과창이 떠 있던 경우 등)
- 상태는 화면 픽셀 색으로만 판단 (가운데 문구는 랜덤이라 안 읽음)
  · 릴링 바: 왼쪽부터 차는 막대 끝(◇ 표시) = 내 위치, 막대 색이 아닌 색 덩어리 = 물고기 구간
    (바가 떠 있는지는 색 있는 칸으로 봄 — 막대가 거의 비어도 구간은 보임 · 막대는 청록 ~ 파랑 폭넓게)
    막대와 구간이 겹친 부분은 구간 색이 밝게 보임 · 클릭하면 내 위치가 오른쪽으로, 안 누르면 왼쪽으로 떨어짐
    → 구간 왼쪽 끝 근처로 떨어질 때만 눌러서 구간 안에 붙잡아 둠 (FishSol 등 다른 낚시 매크로도 같은 방식)
  · 결과창 제목 색: 하늘색 = 성공 / 회색 = 쓰레기 / 빨강 = 실패
- 위치는 전부 로블록스 창 기준 비율 → 창 크기가 바뀌어도 그대로
"""
import threading
import time

import macro

DEFAULTS = {
    "bite_max": 60.0,      # 입질 최대 대기 (초) — 넘으면 Exit 누르고 다시 던짐
    "lead_ms": 0,          # 떨어지는 속도를 보고 이만큼 미리 누름 (ms · 0 = 지금 위치 그대로)
    "target_pct": 0,       # 목표 위치: 구간 왼쪽 끝에서 구간 폭의 몇 % (0 = 왼쪽 변 · 50 = 가운데)
    "deadband": 0.0,       # 목표 위치에서 이만큼(바 폭 비율) 더 왼쪽에 있어야 누름
    "click_ms": 25,        # 한 번 누르는 시간 (ms)
    "click_gap_ms": 45,    # 클릭 사이 최소 간격 (ms)
    "cast_retry": 3,       # Fish 를 눌러도 안 바뀌면 다시 누르는 횟수 (넘으면 인벤토리 가득)
    "debug_log": False,    # 릴링 기록(fishing_log.csv) 저장 — 문제 확인용
}
REEL_FRAME = 0.008       # 릴링 중 화면 읽는 최소 간격 (초)
# 아래 세 값은 물리 모델(누를 때마다 힘이 쌓임 · 누른 걸 놓음 · 입력 지연 0~100ms)로 수백 번 돌려서 고른 값
RISE_GAIN = 2.0          # 목표까지 거리(px) × 이 값 = 허용하는 올라가는 속도 (px/초)
RISE_MAX = 0.5           # 허용하는 올라가는 속도 최대 (바 폭 × 이 값 / 초)
CLICK_SETTLE = 0.2       # 누른 뒤 효과가 보일 때까지 다시 안 누르는 최대 시간 (초)
REEL_MAX = 15.0          # 미니게임 최대 길이 (초) — 넘으면 멈춘 걸로 보고 닫기 (FishSol 은 9초)
REEL_GONE = 0.6          # 바 · ◇ 신호가 둘 다 이만큼 안 보여야 미니게임 끝으로 봄 (초)
# 낚시 창 ◇ 표시 (낚시 창 영역 안 비율) — 대기 땐 왼쪽(IDLE) 자리, 미니게임 땐 창이 넓어지며 오른쪽(REEL) 자리로 옮겨감
# Noteab 보정값 fishing_detect_pixel (1175,836) · FishSol 도 같은 자리로 미니게임 시작을 봄 / 대기 자리는 스크린샷에서 (1146,835)
DIAMOND_IDLE = (0.9268, 0.8264)          # 대기 창 기준
DIAMOND_REEL = (0.9931, 0.8333)          # 대기 창 기준 (미니게임 창을 따로 지정 안 했을 때)
DIAMOND_IDLE_R = (0.8735, 0.8239)        # 미니게임 창 기준
DIAMOND_REEL_R = (0.9317, 0.8310)        # 미니게임 창 기준
FINISH_GAP = 0.1         # 낚은 뒤 Fish 버튼이 다시 보일 때까지 결과창 X 를 누르는 간격 (초)
FINISH_MAX = 10.0        # 그래도 Fish 버튼이 안 보이면 이 시간 뒤 다시 상태 확인부터
FULL_WORDS = ("cannot fish", "inventory space", "not have enough", "inventory")   # 인벤토리 가득 알림 글자
POS_KEYS = (("fish_btn", "Fish 버튼"), ("close_pos", "결과창 X"), ("title_pos", "결과창 제목"))

# 창 영역 → 안쪽 위치 (창 영역 안에서의 비율 · 0 = 왼쪽/위, 1 = 오른쪽/아래)
# 1920x1080 전체 화면 기준으로 잰 값: 낚시 창(Fish 버튼이 보일 때 흰 꺾쇠 테두리) (741,716)~(1178,860),
# 결과창(흰 꺾쇠 테두리) (780,316)~(1140,765) · 창 크기가 바뀌어도 안쪽 배치는 같은 비율이라고 봄
PANEL_LAYOUT = {"fish_btn": (0.2494, 0.8264), "bar_region": (0.0389, 0.2847, 0.9703, 0.4722)}
# 미니게임 창은 대기 창보다 넓음 (1920x1080 에서 (711,718)~(1209,860)) → 따로 지정하면 릴링 바를 이 창 기준으로 계산
# 릴링 바는 Noteab 보정값 fishing_bar_region (758,757)~(1165,784) 기준
REEL_LAYOUT = {"bar_region": (0.0944, 0.2746, 0.9116, 0.4648)}
RESULT_LAYOUT = {"close_pos": (0.9222, 0.0579), "title_pos": (0.4972, 0.0913)}
WINDOW_KEYS = {"panel_region": PANEL_LAYOUT, "reel_region": REEL_LAYOUT, "result_region": RESULT_LAYOUT}


def layout_from(region, layout):
    """창 영역 [x1, y1, x2, y2] (로블록스 창 비율) → {키: 위치 또는 영역} (로블록스 창 비율)"""
    x1, x2 = sorted((region[0], region[2]))
    y1, y2 = sorted((region[1], region[3]))
    w, h = x2 - x1, y2 - y1
    out = {}
    for key, rel in layout.items():
        pts = [round(x1 + rel[i] * w, 4) if i % 2 == 0 else round(y1 + rel[i] * h, 4) for i in range(len(rel))]
        out[key] = pts
    return out


# ---------------------------------------------------------------- 화면 분석 (순수 함수 · 테스트 가능)
def _np(data, w, h):
    import numpy as np
    return np.frombuffer(data, dtype=np.uint8).reshape(h, w, 4)[:, :, 2::-1].astype(np.int16)   # BGRA → RGB


def button_state(rgb):
    """Fish/Exit 버튼 주변 픽셀 → 'fish'(파랑) / 'exit'(빨강) / None(둘 다 아님 · 다른 창이 가림)"""
    r, g, b = rgb[:, :, 0], rgb[:, :, 1], rgb[:, :, 2]
    n = max(1, r.size)
    blue = int(((b > 140) & (b > r + 45) & (b > g + 20)).sum())
    red = int(((r > 150) & (r > g + 60) & (r > b + 40)).sum())
    if max(blue, red) < n * 0.01:
        return None
    return "fish" if blue >= red else "exit"


def classify_title(rgb):
    """결과창 제목 주변 픽셀 → 'success'(하늘색) / 'junk'(회색) / 'fail'(빨강) / None"""
    r, g, b = rgb[:, :, 0], rgb[:, :, 1], rgb[:, :, 2]
    n = max(1, r.size)
    bright = (r + g + b) > 330
    cyan = int((bright & (g > r + 40) & (b > r + 40)).sum())
    red = int(((r > 170) & (r > g + 80) & (r > b + 80)).sum())
    gray = int((bright & (abs(r - g) < 22) & (abs(g - b) < 22) & (r < 225)).sum())
    best = max((cyan, "success"), (red, "fail"), (gray, "junk"))
    return best[1] if best[0] >= n * 0.02 else None


def _colored(band):
    """바 가운데 줄 → 색이 있는 칸 (막대 · 구간) — 어두운 바탕 · 흰 글자 · 회색 테두리는 아님
    막대 · 구간 색은 매번 다를 수 있어서(청록 · 파랑 · 분홍 …) 특정 색이 아니라 '채도 있고 어둡지 않음'으로 봄"""
    mx, mn = band.max(axis=1), band.min(axis=1)
    return ((mx - mn) > 50) & (mx > 90)


def _left_ok(band):
    """릴링 바 모양인지: 왼쪽 끝이 '어두운 바탕' 또는 '막대 색' (막대는 항상 왼쪽 끝부터 참) · 아니면 빈 칸이 조금은 있어야 함
    낚시 창이 사라져서 뒤의 게임 화면(빨강 · 파랑 바닥 등)이 보일 때 그걸 릴링 바로 착각하지 않게"""
    import numpy as np
    n = max(3, int(band.shape[0] * 0.04))
    head = np.median(band[:n], axis=0)
    if head.max() < 45 or _is_fill(head):
        return True
    # 구간이 왼쪽 끝에 붙어 있으면 왼쪽 끝이 구간 색 → 이땐 어두운 바탕(빈 칸)이 어느 정도 있으면 바로 봄
    return bool((band.max(axis=1) < 45).mean() >= 0.08)


def bar_present(rgb):
    """바 가운데 줄 몇 개만 잡은 이미지 → 릴링 바가 떠 있는지 (입질 대기용 가벼운 확인)
    막대가 거의 비어 있어도 물고기 구간은 항상 보이므로 '색 있는 칸'이 조금이라도 있고, 왼쪽 끝이 바 모양이면 떠 있는 것"""
    import numpy as np
    band = np.median(rgb, axis=0)
    return int(_colored(band).sum()) >= max(4, band.shape[0] * 0.03) and _left_ok(band)


def _is_fill(px):
    """왼쪽부터 차는 막대 색 (청록 ~ 파랑) — 스크린샷에서 본 값: (33,144,170) (36,151,169) (44,135,175) (36,92,167)
    전에는 초록이 95 이상이어야 했는데 (36,92,167) 처럼 더 파란 막대를 못 봐서 넓힘
    구간 색과는 겹치지 않게: 남색 구간 (29,60,129) · 초록 (50,203,110) · 보라 (149,97,210) 등은 아님"""
    r, g, b = px[..., 0], px[..., 1], px[..., 2]
    return (r < 100) & (g >= 80) & (b >= 140) & ((b - g) >= -15) & ((b - g) <= 85)


def analyze_bar(rgb, bar_top, bar_h):
    """rgb: 릴링 바 + 그 위 ◇ 표시까지 잡은 이미지 / bar_top·bar_h: 그 안에서 바의 세로 위치
    → {"present", "marker", "zone": (시작, 끝) 또는 None, "w"} (x 는 이미지 안 픽셀)"""
    import numpy as np
    h, w = rgb.shape[:2]
    y0 = int(bar_top + bar_h * 0.35)
    y1 = max(y0 + 1, int(bar_top + bar_h * 0.65))
    band = np.median(rgb[y0:y1], axis=0)                # 바 가운데 줄 (글자 노이즈를 줄이려고 여러 줄의 중앙값)
    colored = _colored(band)
    present = colored.sum() >= max(4, w * 0.03) and _left_ok(band)
    out = {"present": bool(present), "marker": None, "zone": None, "zones": [], "w": w}
    if not present:
        return out
    fill = colored & _is_fill(band)
    xs = np.where(fill)[0]
    fill_end = int(xs.max()) if len(xs) else -1          # 막대 끝 (-1 = 비어 있음)
    mx, mn = band.max(axis=1), band.min(axis=1)
    texty = ((mx - mn) <= 50) & (mx > 90)               # 남은 시간 숫자 (흰 글자 · 글자 가장자리 회색)
    zone = colored & ~fill
    # 물고기 구간: 가장 긴 덩어리 (위에 겹친 숫자 글자 · 4px 이하 틈은 이어 붙임)
    segs, cur, gap = [], None, 0
    for x in range(w):
        if zone[x]:
            if cur is None:
                cur = [x, x]
            cur[1], gap = x, 0
        elif cur is not None:
            if texty[x]:
                continue                                  # 숫자 글자 위는 틈으로 안 셈 (글자 테두리의 어두운 칸만 셈)
            gap += 1
            if gap > 4:
                segs.append(cur)
                cur, gap = None, 0
    if cur is not None:
        segs.append(cur)
    segs = [(int(a), int(b)) for a, b in segs if b - a >= 3]
    out["zones"] = segs                                  # 구간 후보 전부 (가운데 숫자 글자 조각 등이 섞일 수 있음 → ReelControl 이 고름)
    best = max(segs, key=lambda z: z[1] - z[0], default=None)
    if best is not None and best[1] - best[0] >= max(3, w * 0.02):
        out["zone"] = best
    # 내 위치: 바 위쪽 ◇ 표시(밝은 흰색)의 가로 위치 → 없으면 청록 막대 오른쪽 끝
    if bar_top >= 3:
        top = rgb[max(0, int(bar_top - bar_h * 1.7)):int(bar_top) - 1]
        cnt = (top.min(axis=2) > 185).sum(axis=0).astype(float)
        edge = max(2, int(w * 0.03))
        cnt[:edge] = 0                                   # 바 테두리 · 창 테두리(세로선)는 빼고
        cnt[-edge:] = 0
        if cnt.sum() >= 6:
            # ◇ 표시는 폭이 좁은 덩어리 → 가장 많이 모인 곳(표시 폭만큼) 기준으로 가중 평균
            k = max(5, int(w * 0.05))
            dens = np.convolve(cnt, np.ones(k), mode="same")
            c = int(dens.argmax())
            lo, hi = max(0, c - k), min(w, c + k + 1)
            seg = cnt[lo:hi]
            if seg.sum() >= 6:
                out["marker"] = float((np.arange(lo, hi) * seg).sum() / seg.sum())
    if out["marker"] is None:
        # ◇ 를 못 찾았을 때: 막대 끝 = 내 위치 (막대가 비어 있으면 맨 왼쪽) · 단, 막대가 구간과 겹치면 겹친 부분은
        # 구간 색(밝게)으로 보여서 막대가 구간 왼쪽 끝에서 끊김 → 그때는 내 위치를 알 수 없으니 None (안 누르고 다음 화면을 봄)
        end = max(0, fill_end)
        z = out["zone"]
        if z is None or not (z[0] - 3 <= end <= z[1] + 2):
            out["marker"] = float(end)
    return out


def reel_decision(marker, zone, vel, cfg, w):
    """누를지: 내 위치(lead_ms 만큼 미리 본 위치 · 기본 0 = 지금 위치)가 목표 위치 이하면 누름
    목표 위치 = 구간 왼쪽 끝 + 구간 폭 × target_pct% (기본 0 = 왼쪽 변 · 50 = 가운데)
    → 왼쪽 끝 가까이로 떨어질 때만 눌러서 튀어 오른 만큼 구간 안에 머물게 함 (가운데 기준이면 너무 자주 눌러 오른쪽으로 넘어감)"""
    if marker is None or zone is None:
        return False
    target = zone[0] + (zone[1] - zone[0]) * cfg.get("target_pct", DEFAULTS["target_pct"]) / 100.0
    pred = marker + vel * cfg.get("lead_ms", DEFAULTS["lead_ms"]) / 1000.0
    if max(marker, pred) > (zone[0] + zone[1]) / 2:
        return False                                      # 구간 가운데보다 오른쪽이면 절대 안 누름 (오른쪽으로 넘어가는 것 방지)
    return pred <= target - cfg.get("deadband", 0) * w   # 기본: 내 위치가 구간 왼쪽 변 이하로 내려오면 그때 누름


# ---------------------------------------------------------------- 실행기
class Stopped(Exception):
    pass


class Fisher:
    LABEL = "자동 낚시"

    def __init__(self, get_cfg, log, on_full=None, on_user_stop=None):
        self.get_cfg = get_cfg              # () -> mfish 설정
        self.log = log
        self.on_full = on_full              # 인벤토리 가득 (판매 단계에서 연결)
        self.on_user_stop = on_user_stop    # F7 로 멈춤
        self.stop_ev = threading.Event()
        self.thread = None
        self.lock = threading.Lock()
        self.state = {"msg": "대기"}
        self.stats = {"success": 0, "junk": 0, "fail": 0, "unknown": 0, "full": 0}
        self.hold_req = threading.Event()   # 다른 기능(레어 바이옴 팝핑 등)이 잠깐 자리를 달라고 함
        self.holding = threading.Event()    # 안전한 곳에서 멈춰 기다리는 중
        self.diamond_ok = False             # 낚시 창 ◇ 가 미니게임 자리로 옮겨가는 걸 한 번이라도 봤는지 (◇ 위치가 맞음)
        self.no_focus = False               # 위치 지정 창이 떠 있는 동안: 로블록스를 앞으로 끌어오지 않음 (선택 창을 가리지 않게)

    # ---- 상태
    def running(self):
        return bool(self.thread and self.thread.is_alive())

    def snapshot(self):
        with self.lock:
            return dict(self.state, running=self.running(), stats=dict(self.stats))

    def _set(self, **kw):
        with self.lock:
            self.state.update(kw)

    @staticmethod
    def missing(cfg):
        miss = [name for key, name in POS_KEYS if key != "title_pos" and not cfg.get(key)]
        if not cfg.get("bar_region"):
            miss.append("릴링 바 영역")
        return miss

    def start(self):
        if self.running():
            return False
        self.stop_ev = threading.Event()
        self.hold_req.clear()
        self.holding.clear()
        self.thread = threading.Thread(target=self._run, args=(self.stop_ev,), daemon=True)
        self.thread.start()
        return True

    def stop(self):
        self.stop_ev.set()

    def hold(self, timeout=30.0):
        """다른 기능이 쓰는 동안 안전한 곳(릴링이 끝난 뒤)에서 멈춰 기다리게 함 — 멈췄으면 True"""
        if not self.running():
            return True
        self.hold_req.set()
        return self.holding.wait(timeout)

    def release(self):
        self.hold_req.clear()

    # ---- 도우미
    def _check(self, stop):
        if stop.is_set():
            raise Stopped()
        if macro.key_down_now("f7"):
            stop.set()
            if self.on_user_stop:
                self.on_user_stop()
            raise Stopped()

    def _wait(self, sec, stop):
        end = time.time() + max(0.0, sec)
        while True:
            self._check(stop)
            left = end - time.time()
            if left <= 0:
                return
            time.sleep(min(0.03, left))

    def _hold_point(self, stop):
        if not self.hold_req.is_set():
            return
        self._set(msg="다른 기능에 자리 양보 중")
        self.holding.set()
        while self.hold_req.is_set():
            self._wait(0.2, stop)
        self.holding.clear()

    def _rect(self, stop):
        for _ in range(20):
            hwnd = macro.roblox_window_cached()
            rect = macro.client_rect(hwnd) if hwnd else None
            if rect and rect[2] > 50 and rect[3] > 50:
                if not self.no_focus:
                    macro.focus(hwnd)
                return rect
            self._wait(0.5, stop)
        raise RuntimeError("로블록스 창을 찾을 수 없음")

    def _click_ratio(self, pos, stop):
        while self.no_focus:                       # 위치 지정 창이 떠 있으면 끝날 때까지 안 누름 (드래그를 망치지 않게)
            self._wait(0.1, stop)
        rect = self._rect(stop)
        x, y = macro.to_screen(pos[0], pos[1], rect)
        macro.click(x, y)

    def _grab_box(self, sct, rect, pos, bw=0.045, bh=0.035):
        """비율 위치 주변 작은 상자 캡처 → RGB 배열"""
        x, y = macro.to_screen(pos[0], pos[1], rect)
        w, h = max(6, int(rect[2] * bw)), max(6, int(rect[3] * bh))
        img = sct.grab({"left": x - w // 2, "top": y - h // 2, "width": w, "height": h})
        return _np(bytes(img.bgra), img.width, img.height)

    def _band_box(self, rect, region):
        """바 영역(비율) → 바 가운데 줄(35~65%)만 잡는 작은 캡처 상자 (입질 대기 중 가볍게 보기)"""
        x1, y1 = macro.to_screen(min(region[0], region[2]), min(region[1], region[3]), rect)
        x2, y2 = macro.to_screen(max(region[0], region[2]), max(region[1], region[3]), rect)
        bh = max(4, y2 - y1)
        return {"left": x1, "top": y1 + int(bh * 0.35), "width": max(8, x2 - x1), "height": max(1, int(bh * 0.3))}

    def _bar_geom(self, rect, region):
        """바 영역(비율) → 캡처할 화면 상자 (◇ 표시까지 위로 늘림) + 그 안의 바 위치"""
        x1, y1 = macro.to_screen(min(region[0], region[2]), min(region[1], region[3]), rect)
        x2, y2 = macro.to_screen(max(region[0], region[2]), max(region[1], region[3]), rect)
        bh = max(4, y2 - y1)
        extra = int(bh * 1.8)
        box = {"left": x1, "top": max(rect[1], y1 - extra), "width": max(8, x2 - x1), "height": 0}
        top_in = y1 - box["top"]
        box["height"] = top_in + bh
        return box, top_in, bh

    # ---- 메인 (상태 기계)
    def _run(self, stop):
        cfg0 = self.get_cfg() or {}
        miss = self.missing(cfg0)
        try:
            if miss:
                self.log(f"{self.LABEL} 안 함 — 설정 필요: {', '.join(miss)}", "y")
                return
            self.log(f"{self.LABEL} 시작", "g")
            # 화면 캡처는 CAPTUREBLT 없이 (ScreenGrabber) · 우선순위는 그대로 두고 릴링 중에만 타이머를 1ms 로
            with macro.ScreenGrabber() as sct:
                self._loop(sct, stop)
        except Stopped:
            self.log(f"{self.LABEL} 정지", "d")
        except Exception as e:
            self.log(f"{self.LABEL} 오류: {e}", "r")
        finally:
            self._set(msg="대기")
            self.holding.clear()

    def _diamond_moved(self, sct, rect, cfg):
        """낚시 창 ◇ 가 미니게임 자리로 옮겨갔는지 (바와 따로 보는 두 번째 신호) — 창 영역이 하나도 없으면 None
        대기 자리는 대기 창 기준, 미니게임 자리는 미니게임 창 기준 (따로 지정 안 했으면 다른 창 기준으로 계산)
        미니게임 자리엔 대기 때도 창 테두리가 조금 걸리므로 '원래 자리가 비었는지'도 같이 봄"""
        idle_win, reel_win = cfg.get("panel_region"), cfg.get("reel_region")
        if not idle_win and not reel_win:
            return None
        idle = (idle_win, DIAMOND_IDLE) if idle_win else (reel_win, DIAMOND_IDLE_R)
        reel = (reel_win, DIAMOND_REEL_R) if reel_win else (idle_win, DIAMOND_REEL)

        def white(win, rel):
            x1, x2 = sorted((win[0], win[2]))
            y1, y2 = sorted((win[1], win[3]))
            size = max(3, int(rect[2] * (x2 - x1) * 0.012))
            x, y = macro.to_screen(x1 + rel[0] * (x2 - x1), y1 + rel[1] * (y2 - y1), rect)
            img = sct.grab({"left": x - size, "top": y - size, "width": size * 2 + 1, "height": size * 2 + 1})
            return int((_np(bytes(img.bgra), img.width, img.height).min(axis=2) > 200).sum())
        return white(*reel) >= 6 and white(*idle) < 6

    def _read_state(self, sct, rect, cfg):
        """지금 화면 → 'reel' / 'maybe'(바만 보이고 ◇ 는 안 맞음) / 'idle'(Fish) / 'wait'(Exit) / None(알 수 없음)
        결과창은 여기서 판단하지 않음 — 입질 → 미니게임으로 넘어가는 순간엔 버튼도 바도 안 보여서 결과창 제목 자리에
        하늘 등이 보이는데, 이걸 결과창(회색 = 쓰레기)으로 잘못 보고 X 를 누르던 문제가 있었음 → 결과창은 릴링이 끝난 뒤에만 다룸"""
        if self._bar_seen(sct, rect, cfg):
            # 바와 ◇ 두 신호가 같이 맞아야 바로 미니게임으로 봄 (색이 잠깐 번쩍이는 화면을 착각하지 않게)
            moved = self._diamond_moved(sct, rect, cfg)
            if moved:
                self.diamond_ok = True               # ◇ 위치가 맞는 게 확인됨 → 이제부턴 ◇ 를 믿음
                return "reel"
            if moved is False and self.diamond_ok:
                return None                          # ◇ 가 맞는 걸 아는데 대기 자리 그대로 → 미니게임 아님 (뒤 배경 색 등)
            time.sleep(0.04)
            if self._bar_seen(sct, rect, cfg):
                # 창 영역이 없으면 바만 두 번 보이면 인정 · 있는데 ◇ 가 아직 확인 안 됐으면 '아마도' → 3번 이어지면 인정
                return "reel" if moved is None else "maybe"
        st = button_state(self._grab_box(sct, rect, cfg["fish_btn"]))
        if st == "fish":
            return "idle"
        if st == "exit":
            return "wait"
        return None

    def _bar_seen(self, sct, rect, cfg):
        img = sct.grab(self._band_box(rect, cfg["bar_region"]))
        return bar_present(_np(bytes(img.bgra), img.width, img.height))

    def _loop(self, sct, stop):
        cast_at, tries = None, 0          # 마지막 Fish 클릭 시각 · 반응 없던 횟수
        wait_at, unknown_at = None, None  # 입질 대기 시작 · 알 수 없는 화면 시작
        progress_at, rescues = time.time(), 0   # 마지막으로 미니게임이 시작된 시각 · 연속 복구 횟수
        maybe = 0                                # '바만 보임' 이 이어진 횟수
        while True:
            self._check(stop)
            cfg = dict(DEFAULTS, **(self.get_cfg() or {}))
            rect = self._rect(stop)
            st = self._read_state(sct, rect, cfg)
            now = time.time()
            # 감시: 입질 최대 대기 + 60초 동안 미니게임이 한 번도 안 열림 → 화면이 꼬인 것 → 복구 (3번 연속 안 되면 멈춤)
            if not self.hold_req.is_set() and now - progress_at > float(cfg["bite_max"]) + 60:
                rescues += 1
                if rescues > 3:
                    self.log("낚시 화면을 계속 못 찾음 — 자동 낚시 멈춤 (낚시 자리 · 위치 설정 확인)", "r")
                    self.stop_ev.set()
                    raise Stopped()
                self.log(f"한동안 낚시가 진행되지 않음 — 화면 복구 ({rescues}/3)", "y")
                self._rescue(cfg, stop)
                progress_at = time.time()
                cast_at, tries, wait_at, unknown_at = None, 0, None, None
                continue
            if st == "maybe":                        # 바만 보이고 ◇ 는 안 맞음 (◇ 위치가 조금 어긋났을 수 있음)
                maybe = maybe + 1
                st = "reel" if maybe >= 3 else None
                if st is None:
                    self._wait(0.05, stop)
                    continue
            else:
                maybe = 0
            if st != "wait":
                wait_at = None
            if st is not None:
                unknown_at = None

            if st == "reel":
                cast_at, tries = None, 0
                progress_at, rescues = now, 0
                # 결과창 제목 자리의 '결과창이 없을 때' 모습 (릴링 중엔 결과창이 없고, 낚시 중엔 카메라가 안 움직임)
                base = self._grab_box(sct, rect, cfg["title_pos"], 0.12, 0.05) if cfg.get("title_pos") else None
                self._reel(sct, rect, cfg, stop)
                self._finish(sct, cfg, stop, base)
                continue

            if st == "wait":                         # 던졌음 → 입질 기다리는 중
                cast_at, tries = None, 0
                wait_at = wait_at or now
                if self.hold_req.is_set():           # 레어 바이옴 팝핑 등이 기다림 → 던진 걸 취소하고 비켜줌
                    self._click_ratio(cfg["fish_btn"], stop)
                    self._wait(0.8, stop)
                    continue
                if now - wait_at > float(cfg["bite_max"]):
                    self.log(f"입질이 {cfg['bite_max']:g}초 동안 없음 — Exit 후 다시 던짐", "y")
                    self._click_ratio(cfg["fish_btn"], stop)
                    self._wait(1.0, stop)
                    continue
                self._set(msg="입질 기다리는 중")

            elif st == "idle":                       # Fish 버튼 → 던지기 (안전한 곳이라 여기서 자리 양보)
                if self.hold_req.is_set():
                    self._hold_point(stop)
                    progress_at = time.time()        # 양보한 시간은 '진행 안 됨'으로 안 셈
                    continue
                # Fish 를 눌렀는데 오른쪽에 "Cannot Fish (인벤토리 공간 부족)" 알림 → 바로 인벤토리 가득 (3번 다시 안 눌러 봄)
                if cast_at is not None and cfg.get("notice_region") and now - cast_at >= 0.2:
                    text = macro.notice_check(sct, rect, cfg["notice_region"], FULL_WORDS)
                    if text:
                        self.log(f"알림 읽음: {' '.join(str(text).split())[:60]}", "d")
                        self._inventory_full(stop, tries + 1, notice=True)
                        cast_at, tries = None, 0
                        continue
                if cast_at is None or now - cast_at > 1.5:
                    if cast_at is not None:          # 1.5초 지나도 Exit 로 안 바뀜 = 반응 없음
                        tries += 1
                        if tries >= int(cfg["cast_retry"]):
                            self._inventory_full(stop, tries)
                            cast_at, tries = None, 0
                            continue
                    self._set(msg="Fish 클릭" + (f" ({tries + 1}번째)" if tries else ""))
                    self._click_ratio(cfg["fish_btn"], stop)
                    cast_at = time.time()

            else:                                    # 알 수 없음 (다른 창이 가림 · 로딩 등)
                unknown_at = unknown_at or now
                self._set(msg="낚시 화면 확인 중")
                if now - unknown_at > 3.0:
                    self._click_ratio(cfg["close_pos"], stop)     # 결과창 등이 가리고 있을 수 있음 → X 한 번
                    unknown_at = time.time()
                    self._wait(0.6, stop)
                    continue
            # 기다리는 동안은 0.1초마다만 봄 (화면 캡처는 로블록스를 버벅이게 할 수 있음)
            self._wait(0.1, stop)

    def _rescue(self, cfg, stop):
        """꼬인 화면 풀기: 결과창 X 를 몇 번 누름 (UI 내비게이션 \ 은 안 씀)"""
        self._set(msg="화면 복구 중")
        for _ in range(3):
            self._click_ratio(cfg["close_pos"], stop)
            self._wait(0.5, stop)

    def _inventory_full(self, stop, tries, notice=False):
        self.stats["full"] += 1
        self.log("인벤토리 가득 알림(Cannot Fish) — 낚시 인벤토리 가득" if notice
                 else f"Fish 를 {tries}번 눌러도 반응 없음 — 낚시 인벤토리 가득", "y")
        if self.on_full:
            self.on_full()
            return
        self._set(msg="인벤토리 가득 — 판매 필요")
        self.stop_ev.set()
        raise Stopped()

    def _finish(self, sct, cfg, stop, base=None):
        """릴링이 끝난 뒤: Fish 버튼이 다시 보일 때까지 결과창 X 를 0.1초마다 누름 (결과창이 뜨자마자 닫혀서 가장 빠름)
        X 를 누르기 직전마다 결과창 제목 색을 봐서 결과를 기록 — 단, 릴링을 시작할 때(base · 결과창 없음)와 비교해
        제목 자리가 바뀌었을 때만 (결과창이 아직 안 떴을 때 그 자리의 하늘 · 배경을 결과로 잘못 읽지 않게)"""
        import numpy as np
        self._set(msg="결과창 닫는 중")
        title = cfg.get("title_pos") if base is not None else None
        kind, end = None, time.time() + FINISH_MAX
        while time.time() < end:
            self._check(stop)
            rect = self._rect(stop)
            if button_state(self._grab_box(sct, rect, cfg["fish_btn"])) in ("fish", "exit"):
                break                                # Fish(또는 이미 다시 던진 Exit)가 보이면 낚시 화면으로 돌아온 것
            if title and kind is None:
                img = self._grab_box(sct, rect, title, 0.12, 0.05)
                if img.shape == base.shape and float(np.abs(img - base).mean()) > 18:
                    kind = classify_title(img)
            self._click_ratio(cfg["close_pos"], stop)
            self._wait(FINISH_GAP, stop)
        else:
            self.log(f"결과창을 닫은 뒤 {FINISH_MAX:g}초 동안 Fish 버튼이 안 보임 — 다시 확인", "y")
        self.stats[kind or "unknown"] += 1
        name = {"success": "성공", "junk": "쓰레기", "fail": "실패"}.get(kind, "결과 확인 안 됨")
        self.log(f"낚시 결과: {name} · 성공 {self.stats['success']} / 쓰레기 {self.stats['junk']} / 실패 {self.stats['fail']}",
                 "g" if kind == "success" else "d")

    def _reel(self, sct, rect, cfg, stop):
        self._set(msg="릴링 중")
        box, top_in, bh = self._bar_geom(rect, cfg["bar_region"])
        # 클릭 위치: 바 아래쪽 (게임 UI 버튼이 없는 곳 · 아무 데나 눌러도 릴링됨) · 위치 지정 중이면 마우스를 안 옮김
        if not self.no_focus:
            macro.move_to(box["left"] + box["width"] // 2, box["top"] + box["height"] + int(bh * 3))
        rec = _ReelLog(cfg.get("debug_log"))
        try:
            with macro.fast_timing(priority=False):
                self._reel_loop(sct, box, top_in, bh, cfg, stop, rec)
        finally:
            rec.close()

    def _reel_loop(self, sct, box, top_in, bh, cfg, stop, rec):
        gone_since = None
        ctl = ReelControl(cfg)
        start = time.time()
        dia, dia_at, dia_seen, dia_off = None, 0.0, False, None   # ◇ 신호 · 마지막 확인 · 이번에 본 적 · 사라진 시각
        end = start + REEL_MAX
        hwnd, fg_at = macro.roblox_window_cached(), start
        while time.time() < end:
            t0 = time.time()
            self._check(stop)
            if t0 - fg_at > 0.5:                     # 다른 창이 로블록스를 가리면 화면을 잘못 읽음 → 0.5초마다 맨 앞인지 확인
                fg_at = t0
                if hwnd and not self.no_focus and not macro.is_foreground(hwnd):
                    macro.focus(hwnd)
            img = sct.grab(box)
            a = analyze_bar(_np(bytes(img.bgra), img.width, img.height), top_in, bh)
            now = time.time()
            if now - dia_at >= 0.1:                  # ◇ 는 0.1초마다만 봄
                dia_at = now
                rect = macro.client_rect(hwnd) if hwnd else None
                dia = self._diamond_moved(sct, rect, cfg) if rect else None
                if dia:
                    dia_seen, dia_off = True, None
                    self.diamond_ok = True
                elif dia is False and (dia_seen or self.diamond_ok):
                    dia_off = dia_off or now
            # ◇ 가 맞는 걸 아는데 0.5초 넘게 미니게임 자리에 없음 → 미니게임 끝 (바 자리에 뒤 배경 색이 보여도)
            if dia_off and now - dia_off > 0.5:
                return
            if not a["present"]:
                # 바가 안 보여도 ◇ 가 미니게임 자리에 있으면 아직 미니게임 중 (잠깐 가려지거나 잘못 읽힌 화면)
                if dia:
                    gone_since = None
                    time.sleep(0.01)
                    continue
                gone_since = gone_since or now
                if now - gone_since > REEL_GONE:
                    return
                time.sleep(0.01)
                continue
            gone_since = None
            click, m, zone, vel = ctl.step(now, a["marker"], a["zone"], a["w"], a["zones"])
            if click and self.no_focus:
                click = False                        # 위치 지정 창이 떠 있으면 안 누름 (마우스가 선택 창 위에 있음)
            if click:
                macro.mouse_click_here(hold_ms=int(cfg["click_ms"]))
                ctl.clicked(time.time())
            rec.add(now, m, zone, vel, click, a["w"])
            # 게임 화면은 1초에 60번쯤 바뀜 → 그보다 자주 찍어 봐야 같은 화면이라 8ms 에 한 번까지만
            time.sleep(max(0.001, REEL_FRAME - (time.time() - t0)))
        self.log(f"미니게임이 {REEL_MAX:g}초 넘게 안 끝남 — 멈춘 걸로 보고 닫기", "y")
        self._rescue(dict(DEFAULTS, **(self.get_cfg() or {})), stop)


class ReelControl:
    """릴링 조작 (화면 읽기와 따로 떼어 둔 판단 부분 · 화면 없이도 시험할 수 있게)
    step(시각, 내 위치, 구간, 바 폭) → (누를지, 쓴 내 위치, 쓴 구간, 속도) · 실제로 눌렀으면 clicked(시각)"""

    def __init__(self, cfg):
        self.cfg = cfg
        self.hist = []              # 최근 내 위치 (시각, 위치) — 속도 계산용
        self.vel = 0.0
        self.odd = None             # 방금 튄 것처럼 보인 위치 (다음 화면과 비슷하면 진짜로 받아들임)
        self.zone_ref = None        # 구간 기억 (폭, 가운데, 마지막으로 본 시각)
        self.last_click = 0.0
        self.rose = True            # 마지막 클릭 뒤 올라가기 시작한 게 보였는지

    def clicked(self, t):
        self.last_click, self.rose = t, False

    def _track_zone(self, now, cands, m, w):
        """구간 추적 — 실제 로그에서: 내 위치가 구간 안에 있으면 막대와 겹친 부분 때문에 구간이 왼쪽부터 잘리거나
        아예 안 보이는 화면이 20% 쯤 됐고, 가운데 남은 시간 숫자가 작은 가짜 구간으로 잡히기도 했음
        → 구간 폭 · 위치를 기억해 두고, 맞는 후보만 씀
          · 기억한 구간 근처의 제 크기 후보 → 그대로 (기억 갱신)
          · 내 위치에서 잘린 후보(왼쪽 끝 ≈ 내 위치) → 오른쪽 끝 기준으로 기억한 폭만큼 되살림
          · 그 밖의 작은 조각(숫자 글자 등) · 못 찾음 → 기억한 구간 (1초까지)"""
        ref = self.zone_ref                        # (폭, 가운데, 마지막으로 본 시각)
        if not ref:
            big = [z for z in cands if z[1] - z[0] >= max(8, w * 0.06)]
            if not big:
                return None
            z = max(big, key=lambda z: z[1] - z[0])
            self.zone_ref = (z[1] - z[0], (z[0] + z[1]) / 2, now)
            return z
        rw, rc, rt = ref
        reach = rw * 0.6 + 300 * max(0.0, now - rt)    # 구간이 움직일 수 있는 거리
        full = [z for z in cands if z[1] - z[0] >= rw * 0.7 and abs((z[0] + z[1]) / 2 - rc) <= reach]
        if full:
            z = min(full, key=lambda z: abs((z[0] + z[1]) / 2 - rc))
            zw = z[1] - z[0]
            self.zone_ref = (rw * 0.8 + min(zw, rw * 1.3) * 0.2, (z[0] + z[1]) / 2, now)
            return z
        if m is not None:
            cut = [z for z in cands if abs(z[0] - m) <= max(6, w * 0.02) and z[1] > m
                   and abs(z[1] - (rc + rw / 2)) <= reach]
            if cut:
                right = max(z[1] for z in cut)
                self.zone_ref = (rw, right - rw / 2, now)
                return (int(round(right - rw)), right)
        if now - rt <= 1.0:
            return (int(round(rc - rw / 2)), int(round(rc + rw / 2)))
        self.zone_ref = None
        return None

    def step(self, now, m, zone, w, zones=None):
        cfg, hist = self.cfg, self.hist
        if m is not None:
            # 한 화면만 튀는 값은 버림 (바로 전 두 값과 너무 멀면 무시) — 단, 튄 값이 두 번 연달아 비슷하게 나오면
            # 진짜로 빨리 움직인 것이니 받아들임 (안 그러면 그 뒤 값을 전부 버려서 내 위치를 계속 못 봄)
            jump = w * 0.25
            if len(hist) >= 2 and abs(m - hist[-1][1]) > jump and abs(m - hist[-2][1]) > jump:
                if self.odd is not None and abs(m - self.odd) <= jump * 0.5:
                    hist[:] = [(now, m)]
                    self.odd = None
                else:
                    self.odd, m = m, None
            else:
                self.odd = None
                hist.append((now, m))
                del hist[:-5]
        if m is None and hist and now - hist[-1][0] < 0.1:
            m = hist[-1][1] + self.vel * (now - hist[-1][0])   # 잠깐 못 찾으면 직전 위치 + 속도로 짐작 (0.1초까지만)
        if len(hist) >= 3:
            (t1, x1), (t2, x2) = hist[0], hist[-1]           # 최근 몇 화면의 기울기 (한 화면 차이보다 덜 흔들림)
            if t2 > t1:
                self.vel = self.vel * 0.5 + ((x2 - x1) / (t2 - t1)) * 0.5
        vel = self.vel
        zone = self._track_zone(now, zones if zones is not None else ([zone] if zone else []), m, w)
        # 누르면 힘이 쌓일 수 있음 → 올라가는 중에 마구 누르면 오른쪽으로 튀어나감. 그래서
        #  ① 누른 뒤엔 올라가기 시작하는 게 보일 때까지 다시 안 누름 (게임에 반영되기까지 몇 화면 걸림 · 최대 CLICK_SETTLE)
        #  ② 올라가는 속도 제한: 목표에서 멀리 아래면 빨리 올라가도 되고 가까우면 천천히만 — 이미 그 이상으로 올라가는 중이면 안 누름
        fast = False
        if m is not None and zone and len(hist) >= 3:
            target = zone[0] + (zone[1] - zone[0]) * cfg.get("target_pct", DEFAULTS["target_pct"]) / 100.0
            allow = min(w * RISE_MAX, max(0.0, target - m) * RISE_GAIN)
            fast = vel > max(w * 0.05, allow)
        if self.last_click and not self.rose and len(hist) >= 3 and vel > w * 0.05 and hist[-1][0] > self.last_click:
            self.rose = True
        waiting = self.last_click and not self.rose and now - self.last_click < CLICK_SETTLE
        click = (not waiting and not fast and (now - self.last_click) * 1000 >= cfg["click_gap_ms"]
                 and reel_decision(m, zone, vel, cfg, w))
        return bool(click), m, zone, vel


class _ReelLog:
    """릴링 기록 (세부 설정 · 릴링 기록 저장) → 데이터 폴더의 fishing_log.csv
    한 줄 = 한 화면: 시각, 내 위치, 구간 시작, 구간 끝, 속도, 클릭 여부, 바 폭 (문제 확인 · 값 맞추기용)"""

    def __init__(self, on):
        self.f = None
        if not on:
            return
        try:
            import watcher_core as core
            path = core.DATA_BASE / "fishing_log.csv"
            if path.exists() and path.stat().st_size > 5_000_000:      # 너무 커지면 새로
                path.unlink()
            new = not path.exists()
            self.f = open(path, "a", encoding="utf-8")
            if new:
                self.f.write("time,marker,zone_start,zone_end,velocity,click,bar_width\n")
            self.t0 = time.time()
            self.f.write(f"# reel {time.strftime('%Y-%m-%d %H:%M:%S')}\n")
        except Exception:
            self.f = None

    def add(self, now, marker, zone, vel, click, w):
        if self.f:
            z0, z1 = zone if zone else ("", "")
            self.f.write(f"{now - self.t0:.3f},{'' if marker is None else round(marker, 1)},{z0},{z1},{vel:.0f},{int(click)},{w}\n")

    def close(self):
        if self.f:
            self.f.close()
            self.f = None
