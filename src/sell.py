# -*- coding: utf-8 -*-
"""
물고기 판매 — 자동 낚시 중 인벤토리가 가득 차면 (Fish 를 3번 눌러도 반응 없음 · Cannot Fish 알림)
  기준 장소 → 물고기 판매 장소(Captain Flarg) 로 이동 → 카메라 정렬(채팅 · 도감) → E → 대화창 눌러 넘기기 → [Sell Fish]
  → (첫 칸 → 왼쪽 정보 확인 → Sell All → 확인 Sell) 을 왼쪽 정보가 빌 때까지 반복 (최대 sell_max 번)
  → X 로 상점 닫기 → 기준 장소 → 낚시 장소 로 돌아감
왼쪽 정보가 비었는지: 물고기를 눌렀는데 이름 줄이 "..." 이거나 비어 있음 / "Sells for 0" → 다 판 것
이동은 move.Mover 를 그대로 씀 (기준 장소 · 장소별 지점 · 잰 시간)
대화창 위치는 여러 기능이 같이 쓰는 통합 위치(base.dialog_pos) — get_cfg 가 같이 넘겨 줌
"""
import contextlib
import re
import threading

import macro

SELL_KEYS = (("sell_fish_pos", "Sell Fish 버튼"), ("first_fish_pos", "첫 번째 물고기 칸"),
             ("sell_all_pos", "Sell All 버튼"), ("confirm_sell_pos", "확인 Sell 버튼"), ("shop_close_pos", "상점 닫기 X"),
             ("info_region", "물고기 정보 영역"))


SELLS_FOR = r"se[l1i|]{1,2}[a-z]?\s*f[o0]r"                # "Sells for" (OCR 이 'Sellefor' · 'Sellsfor' 로 읽기도 함)


def info_empty(text):
    """왼쪽 물고기 정보 OCR 글자 → 비었는지 (물고기가 안 골라짐 = 다 팜)"""
    t = " ".join(str(text or "").split())
    if re.search(SELLS_FOR + r"\s*0(?![\d,.])", t, re.I):
        return True
    rest = re.sub(SELLS_FOR + r"\s*[\d,.]*", " ", t, flags=re.I)
    return not re.search(r"[A-Za-z가-힣]{2,}", rest)       # 이름이 없음 ("..." 뿐)


class Seller:
    def __init__(self, mover, get_cfg, get_move, log):
        self.mover = mover                  # move.Mover (기준 장소 · 장소 이동)
        self.get_cfg = get_cfg              # 자동 낚시 설정 (판매 위치 + 통합 위치의 대화창)
        self.get_move = get_move            # 이동 설정 (장소 목록)
        self.log = log
        self.lock = threading.Lock()

    def _place_index(self, key):
        for i, pl in enumerate((self.get_move() or {}).get("places") or []):
            if pl.get("feat") == "mfish" and pl.get("key") == key:
                return i
        return None

    def _path_missing(self, keys):
        m = self.mover.base_missing()
        if m:
            return m
        for key in keys:
            i = self._place_index(key)
            if i is None:
                return "장소를 찾을 수 없음"
            m = self.mover.path_missing(i)
            if m:
                return m
        return None

    def missing(self):
        """판매를 못 하는 이유 (설정이 빈 곳) → 안내 글 또는 None"""
        cfg = self.get_cfg() or {}
        if not cfg.get("dialog_pos"):
            return "대화창 위치 지정 필요 (매크로 기준 위치 설정 → 통합 위치 → 게임 버튼)"
        miss = [n for k, n in SELL_KEYS if not cfg.get(k)]
        if miss:
            return f"판매 위치 지정 필요: {', '.join(miss)} (매크로 기준 위치 설정 → 자동 낚시 → 판매)"
        return self._path_missing(("sell_spot", "fish_spot"))

    def fish_missing(self):
        """낚시 장소로 못 가는 이유 → 안내 글 또는 None"""
        return self._path_missing(("fish_spot",))

    def _empty(self, region):
        try:
            text = macro.ocr_region(region)
        except Exception as e:
            self.log(f"물고기 정보 OCR 오류: {e} — 판매를 여기서 끝냄", "n")
            return True
        return info_empty(text)

    @contextlib.contextmanager
    def _borrow(self, stop):
        """이동기를 이 스레드에서 빌려 씀 (화면 위 안내 띠 · 상태) — stop: 같이 볼 멈춤 신호 (자동 낚시가 꺼지면 같이 멈춤)"""
        mv = self.mover
        if mv.running():
            raise RuntimeError("이동이 이미 도는 중")
        with self.lock:
            mv.stop_ev = threading.Event()
            if stop is not None:
                def relay(ev=mv.stop_ev):
                    while not ev.is_set():
                        if stop.wait(0.2):
                            ev.set()
                threading.Thread(target=relay, daemon=True).start()
            mv._banner("move")
            mv._set(ext=True, error=None)
            try:
                yield mv
            finally:
                mv._release_keys()
                mv._banner(None)
                mv._set(msg="대기", ext=False)
                mv.stop_ev.set()                 # relay 스레드 정리

    def go_fish(self, stop=None):
        """자동 낚시를 시작할 때: 기준 장소 → 낚시 장소 (멈추면 move.Stopped)"""
        m = self.fish_missing()
        if m:
            raise RuntimeError(m)
        fish_i = self._place_index("fish_spot")
        with self._borrow(stop) as mv:
            self.log("낚시 장소로 이동", "c")
            mv._set(msg="낚시 장소로 가는 중")
            mv.go_base()
            mv.walk(fish_i)
            self.log("낚시 장소 도착", "g")

    def run(self, stop=None):
        """판매 한 번 (이 스레드에서 끝까지) — stop: 같이 볼 멈춤 신호 (자동 낚시가 꺼지면 같이 멈춤)
        멈추면 move.Stopped"""
        m = self.missing()
        if m:
            raise RuntimeError(m)
        cfg = self.get_cfg() or {}
        sell_i, fish_i = self._place_index("sell_spot"), self._place_index("fish_spot")
        with self._borrow(stop) as mv:
            self.log("물고기 팔러 감", "c")
            mv._set(msg="판매 · 물고기 판매 장소로 가는 중")
            mv.go_base()
            mv.walk(sell_i)
            mv._set(msg="판매 · 카메라 정렬 (채팅 · 도감)")
            mv.align_camera()                    # E 를 누르기 전에 화면부터 맞춤
            mv._set(msg="판매 · 대화")
            macro.key_tap("e")
            mv._wait(float(cfg.get("e_wait", 1.5)))
            d = float(cfg.get("sell_delay", 0.4))          # 클릭마다 더 기다릴 시간 (렉이 있으면 늘림)
            mv._click(cfg["dialog_pos"])
            mv._wait(1.0 + d)
            mv._click(cfg["sell_fish_pos"])
            mv._wait(1.5 + d)
            sold = 0
            for _ in range(int(cfg.get("sell_max", 100))):
                mv._set(msg=f"판매 · 파는 중 ({sold}종류)")
                mv._click(cfg["first_fish_pos"])
                mv._wait(0.6 + d)
                if self._empty(cfg["info_region"]):
                    break
                mv._click(cfg["sell_all_pos"])
                mv._wait(0.8 + d)
                mv._click(cfg["confirm_sell_pos"])
                mv._wait(1.0 + d)
                sold += 1
            self.log(f"물고기 판매 완료 ({sold}종류)", "g")
            mv._click(cfg["shop_close_pos"])
            mv._wait(0.8 + d)
            mv._set(msg="판매 · 낚시 장소로 돌아가는 중")
            mv.go_base()
            mv.walk(fish_i)
            self.log("낚시 장소 도착 — 낚시 이어감", "g")
            return sold


# ---------------------------------------------------------------- 판매 자동 보정
# 플레이어가 Captain Flarg 앞에서 E 를 직접 누르면: 대화창 → [Sell Fish] → Fishing Shop 을 글자(OCR)로 찾아 위치를 전부 맞춤
# 상점은 창 크기에 따라 모양이 바뀜 (작은 창 = 좁고 긴 상점) → 비율 대신 상점 안 글자들을 기준으로 계산
#   Buy · Sell 탭 = 오른쪽 목록의 왼쪽 · 오른쪽 끝 / 목록의 물고기 이름 = 칸 (이름은 칸 아래쪽)
#   'Fishing Shop' 제목 = 왼쪽 끝 · X 줄 / Sell All = 왼쪽 정보 아래


def _norm(text):
    """영문만 · 소문자 · 게임 글꼴에서 OCR 이 헷갈리는 글자는 하나로 (a ↔ o · i ↔ l: 'Concel', 'Sell AlI')"""
    return re.sub(r"[^a-z]", "", str(text).lower()).replace("a", "o").replace("i", "l")


def find_text(boxes, *keys, exact=False):
    """OCR 덩어리 [(글자, x, y, w, h)] 중 key 가 들어간 첫 덩어리
    exact: 그 글자로 시작하는 짧은 덩어리만 ('Sell All' 이 확인창 문장 '...sell all of' 에 걸리지 않게)
    덩어리에 다른 글자가 같이 붙어 읽혔으면 (뒤에 깔린 'Player will ... rejoin' 등) key 부분의 가운데로 옮김"""
    ks = [_norm(k) for k in keys]
    for b in boxes:
        n = _norm(b[0])
        for k in ks:
            if (n.startswith(k) and len(n) <= len(k) + 2) if exact else k in n:
                return _sub_box(b, k) if len(n) > len(k) + 2 else b
    return None


def _sub_box(b, k):
    """덩어리 글자 중 key 부분만의 가로 위치 (글자 수 비율로 어림)"""
    text = str(b[0])
    idx = [i for i, ch in enumerate(text) if "a" <= ch.lower() <= "z"]     # 영문 글자의 원래 자리
    n = _norm(text)
    i = n.find(k)
    if i < 0 or not idx or len(text) < 2:
        return b
    a, z = idx[i], idx[i + len(k) - 1] + 1
    left = b[1] - b[3] / 2
    return (text[a:z], left + b[3] * (a + z) / 2 / len(text), b[2], b[3] * (z - a) / len(text), b[4])


def _r(x, y):
    return [round(min(1.0, max(0.0, x)), 4), round(min(1.0, max(0.0, y)), 4)]


def dialog_point(boxes):
    """대화창 가운데 — NPC 대화창은 화면 가운데에 뜸 · 세로는 이름 줄 ~ 대사 줄 (~ Click to skip) 사이
    (작은 창에선 'Click to skip.' 글자가 너무 작아서 못 읽음 → 대사 줄 'Arrrr…' 로 찾음)"""
    line = find_text(boxes, "orrr", "whotdoyou")
    skip = find_text(boxes, "clicktoskip", "toskip")
    ref = line or skip
    if not ref:
        return None
    names = [b for b in boxes if "coptolnflorg" in _norm(b[0]) and b[2] < ref[2]]
    ys = [ref[2]] + ([max(names, key=lambda b: b[2])[2]] if names else []) + ([skip[2]] if skip and line else [])
    return _r(0.5, sum(ys) / len(ys))


def shop_layout(boxes, title, sell_all, aspect):
    """상점 글자들 → 상점 안 위치 (aspect = 창 너비 / 높이) · 못 재면 RuntimeError"""
    buy = next((b for b in boxes if _norm(b[0]) == "buy" and b[2] > title[2]), None)
    tab = next((b for b in boxes if _norm(b[0]) == "sell" and buy and abs(b[2] - buy[2]) < 0.03 and b[1] > buy[1]), None)
    if not (buy and tab):
        raise RuntimeError("상점 Buy · Sell 탭을 못 찾음")
    half = (tab[1] - buy[1]) / 2
    left, right = buy[1] - half, tab[1] + half                   # 오른쪽 목록의 왼쪽 · 오른쪽 끝
    span = sell_all[2] - title[2]                                # 제목 → Sell All 세로 거리
    out = {"shop_close_pos": _r(right - 0.85 * title[4] / aspect, title[2])}
    # 첫 칸: 목록 안 물고기 이름 중 맨 위 줄 · 맨 왼쪽 열 (이름은 칸 아래쪽 → 줄 간격의 30% 위가 칸 가운데)
    names = sorted((b for b in boxes if left < b[1] < right and buy[2] + 0.02 < b[2] < sell_all[2] + 0.05
                    and len(_norm(b[0])) >= 3), key=lambda b: b[2])
    if names:
        top = names[0][2]
        rows = sorted({round(b[2], 2) for b in names})
        pitch = next((y - top for y in rows if y - top > names[0][4] * 1.5), None)
        if pitch is None:                                        # 한 줄뿐 → 칸 가로 간격으로
            xs = sorted(b[1] for b in names if abs(b[2] - top) < names[0][4])
            pitch = (xs[1] - xs[0]) * aspect if len(xs) > 1 else names[0][4] * 3
        col = min(b[1] for b in names)
        out["first_fish_pos"] = _r(col, top - 0.3 * pitch)
    else:                                                        # 물고기가 없음 → 목록 왼쪽 위에서 어림
        w = (right - left) / 5
        out["first_fish_pos"] = _r(left + w * 0.55, buy[2] + w * aspect * 0.8)
    # 왼쪽 물고기 정보: 제목 왼쪽 끝 ~ 목록 왼쪽 끝 · Sell All 위쪽 (이름 · Weight · Sells for)
    x1 = title[1] - title[3] / 2 - 0.005
    out["info_region"] = _r(x1, sell_all[2] - 0.38 * span) + _r(left - 0.005, sell_all[2] - 0.035 * span)
    # 확인창 Sell (확인창이 안 떴을 때만 씀): 상점 가운데에서 조금 왼쪽
    out["confirm_sell_pos"] = _r((x1 + right) / 2 - 0.155 * (right - x1), title[2] + 0.66 * span)
    return out


def find_confirm(boxes):
    """확인창(Sell Confirm)의 초록 Sell · 빨강 Cancel → (sell 위치, cancel 위치) 또는 None"""
    cancel = find_text(boxes, "cancel", exact=True)
    if not cancel:
        return None
    sells = [b for b in boxes if _norm(b[0]) == "sell" and abs(b[2] - cancel[2]) < 0.04 and b[1] < cancel[1]]
    if not sells:
        return None
    s = max(sells, key=lambda b: b[1])                       # Cancel 바로 왼쪽 (왼쪽 정보 아래 Sell 버튼 말고)
    return [round(s[1], 4), round(s[2], 4)], [round(cancel[1], 4), round(cancel[2], 4)]


DIALOG_AREA = [0.0, 0.4, 1.0, 1.0]                           # 대화창 · 선택지가 뜨는 곳 (화면 아래쪽)


def autocal(ocr, click, wait, aspect, found, status, wait_dialog=120.0):
    """판매 위치 자동 보정 — ocr(영역) → 덩어리 목록, click(위치), wait(초) (멈춤 확인 포함), aspect() → 창 너비 / 높이
    found: 찾은 위치를 바로바로 채움 (중간에 멈춰도 찾은 건 남음) · status(글) → 진행 상황
    끝나면 안내 글 목록"""
    import time
    notes = []
    # 1. 플레이어가 E 를 누를 때까지 (대화창 'Click to skip.' 또는 선택지가 보일 때까지)
    status("Captain Flarg 앞에서 E 를 눌러 주세요")
    end, dlg = time.time() + wait_dialog, None
    while time.time() < end:
        boxes = ocr(DIALOG_AREA)
        dlg = dialog_point(boxes)
        if dlg or find_text(boxes, "sellfish"):
            break
        wait(0.4)
    else:
        raise RuntimeError("대화창이 안 보임 — Captain Flarg 앞에서 E 를 눌러 주세요")
    status("대화창 찾음 — 이제 만지지 마세요")
    wait(0.6)                                                # 대화창이 다 뜰 때까지
    boxes = ocr(DIALOG_AREA)
    dlg = dialog_point(boxes) or dlg
    if dlg:
        found["dialog_pos"] = dlg
    # 2. 대화를 넘겨서 [Sell Fish]
    sf = find_text(boxes, "sellfish")
    for _ in range(5):
        if sf:
            break
        if not found.get("dialog_pos"):
            raise RuntimeError("대화창을 못 찾음")
        click(found["dialog_pos"])
        wait(0.9)
        sf = find_text(ocr(DIALOG_AREA), "sellfish")
    if not sf:
        raise RuntimeError("[Sell Fish] 버튼을 못 찾음")
    found["sell_fish_pos"] = [round(sf[1], 4), round(sf[2], 4)]
    if not found.get("dialog_pos"):
        notes.append("대화창은 못 찾음")
    # 3. 상점 (Fishing Shop 제목 · Sell All 버튼으로 크기를 잼)
    status("상점 여는 중")
    click(found["sell_fish_pos"])
    title = sa = None
    for _ in range(8):
        wait(0.6)
        boxes = ocr(None)
        title, sa = find_text(boxes, "fishingshop"), find_text(boxes, "sellall", exact=True)
        if title and sa:
            break
    if not (title and sa):
        raise RuntimeError("상점(Fishing Shop · Sell All)을 못 찾음")
    wait(0.4)                                                # 여는 애니메이션이 끝난 뒤 한 번 더
    boxes = ocr(None)
    title, sa = find_text(boxes, "fishingshop") or title, find_text(boxes, "sellall", exact=True) or sa
    geo = shop_layout(boxes, title, sa, aspect())
    found["sell_all_pos"] = [round(sa[1], 4), round(sa[2], 4)]
    found["first_fish_pos"] = geo["first_fish_pos"]
    found["info_region"] = geo["info_region"]
    found["shop_close_pos"] = geo["shop_close_pos"]
    # 4. 확인 Sell: 첫 칸 → Sell All → 확인창에서 Sell 자리만 재고 Cancel (실제로 팔지는 않음)
    status("확인창 확인 중 (팔지는 않음)")
    click(found["first_fish_pos"])
    wait(0.6)
    click(found["sell_all_pos"])
    conf = None
    for _ in range(4):
        wait(0.6)
        conf = find_confirm(ocr(None))
        if conf:
            break
    if conf:
        found["confirm_sell_pos"] = conf[0]
        click(conf[1])                                       # Cancel
        wait(0.6)
    else:
        found["confirm_sell_pos"] = geo["confirm_sell_pos"]
        notes.append("확인창이 안 떠서(물고기 없음?) 확인 Sell 은 계산한 자리")
    # 5. X 로 상점 닫기
    status("상점 닫는 중")
    click(found["shop_close_pos"])
    wait(0.8)
    if find_text(ocr(None), "fishingshop"):
        notes.append("상점이 안 닫힘 — 상점 닫기 X 를 직접 지정해 주세요")
    return notes
