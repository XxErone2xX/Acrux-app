# -*- coding: utf-8 -*-
"""
오토 메모리 매치 — 메모리 매치 장소로 가서 E → 알림 창을 읽음 ([체크])
  · Start Memory Match 가 있으면 바로 시작 → 5x4 카드 짝 맞추기 (기회 10번 · 틀릴 때만 1번 줄어듦)
    끝나면 다시 E 로 확인 (광고를 볼 수 있으면 이어서 광고)
  · Available after 00h 00m Left + 'Watch AD to lower the cooltime for 3 hours!' 가 있으면 광고를 봄
    (광고 1번 = 쿨타임 3시간 ↓) → 다시 알림 창을 읽어서 할 수 있으면 메모리 매치 · 아니면 광고를 또 봄 (최대한)
  · 광고 칸이 없으면(= 오늘 광고를 다 봄 · 스크립트 매크로와 같은 기준) 광고는 다시 보러 오지 않고, 쿨타임이 끝나고 1분 뒤에 다시 감
  · 광고를 다 본 뒤 · 메모리 매치를 한 뒤 다시 창을 쓸 땐 항상 E 를 다시 누름
  자동 보정: 보드 앞에서 E 를 누르면 알림 창 · 버튼 · 광고 칸 · 카드 판 자리를 재서 저장 (실행할 땐 글자로 먼저 찾고, 못 찾으면 이 자리)
  카드 구별: 그림(작게 줄인 픽셀) + 아래 개수 글자(OCR · x100 / x200 처럼 그림만으론 같은 카드)
  위치는 전부 글자로 잼: 보드 제목 'Memory Match' ↔ 'CHANCES' 거리로 크기 · 알림 창은 'Notification' 제목 기준
  (1920x1080 전체 화면 스크린샷 기준 거리)
"""
import random
import re
import time

import macro
import popping
import sell

COLS, ROWS = 5, 4
# 보드 (제목 'Memory Match' 가운데 기준 px · 1080p)
REF_CHANCES_DY = 114                  # 제목 → 'CHANCES' 세로 거리 (크기 재는 기준)
CARD_X0, CARD_DX = -136, 112.75       # 첫 칸 가로 · 칸 간격
CARD_Y0, CARD_DY = 94, 121.3          # 첫 칸 세로(카드 가운데) · 줄 간격
CARD_W, CARD_H = 84, 92               # 카드 (테두리 안쪽)
TEXT_DY, TEXT_W, TEXT_H = 38, 104, 34  # 카드 가운데 → 개수 글자 · 글자 영역 크기
CLOSE_DY = 549                        # 제목 → 게임이 끝나면 뜨는 Close 버튼
CHANCE_X, CHANCE_Y, CHANCE_W, CHANCE_H = -288, 74, 110, 66   # 제목 → 남은 기회 숫자 (가운데 · 크기)
# 알림 창 ('Notification' 제목 가운데 기준 px · 1080p)
NOTE_CLOSE = (271, -3)                # 오른쪽 위 X
NOTE_BTN_DY = 157                     # 제목 → Start Memory Match / Available after 버튼
NOTE_AD_DY = 206                      # 제목 → 아래 'Watch AD' 칸
NOTE_BOARD_DY = -163                  # 알림 창 제목 → 카드 판 제목 'Memory Match' (둘 다 화면 가운데)
NOTE_REGION = (-310, -30, 310, 240)   # 알림 창을 읽을 영역 (제목 기준)
POS_KEYS = (("note_close_pos", "알림 창 X"), ("note_btn_pos", "Start / Available 버튼"), ("ad_pos", "Watch AD 칸"),
            ("board_close_pos", "카드 판 Close 버튼"))
# 광고 화면 (1920x1080 기준 px) — 닫기 X 는 스크립트 매크로와 같은 자리 (화면 비율로 맞춤 · 넓은 화면은 조금 오른쪽)
AD_CLOSE = (1513, 239)
AD_ICON = (3, 582)                    # 닫기 X → 아래 재생/일시정지 아이콘 (재생 중엔 ‖ · 끝나면 ▶)
AD_MAX = 8                            # 한 번 확인할 때 볼 광고 최대 수 (광고 1번 = 3시간 · 쿨타임 12시간)
ROUND_MAX = 20                        # 한 번 확인에서 (메모리 매치 + 광고) 최대 반복
AFTER_COOLDOWN = 60                   # 쿨타임이 끝나고 이만큼 뒤에 다시 감 (초)
READY_MIN = 5.5                       # Start 를 누르고 판이 새로 깔릴 때까지 최소 대기 (스크립트 매크로와 같음)


def _r(rect, x, y):
    return [round(min(1.0, max(0.0, x / rect[2])), 4), round(min(1.0, max(0.0, y / rect[3])), 4)]


def board_layout(boxes, rect):
    """메모리 매치 보드 글자들 → {"cards": [[x, y] ×20], "boxes": [[x1, y1, x2, y2] ×20], "texts": [...], "close_pos"} 또는 None"""
    W, H = rect[2], rect[3]
    ch = next((b for b in boxes if sell._norm(b[0]).endswith("honces")), None)
    if not ch:
        return None
    titles = [b for b in boxes if sell._norm(b[0]).endswith("emorymotch") and b[2] < ch[2]]
    if not titles:
        return None
    t = max(titles, key=lambda b: b[2])                 # CHANCES 바로 위의 제목 (게임 속 간판 글자는 아래에 있음)
    k = (ch[2] - t[2]) * H / REF_CHANCES_DY
    if not 0.25 < k < 4:
        return None
    return board_at(t[1] * W, t[2] * H, k, rect)


def board_at(tx, ty, k, rect):
    """카드 판 제목 가운데 (px) · 크기 k (1080p 대비) → 카드 자리들"""
    cards, cboxes, texts = [], [], []
    for r in range(ROWS):
        for c in range(COLS):
            x, y = tx + (CARD_X0 + CARD_DX * c) * k, ty + (CARD_Y0 + CARD_DY * r) * k
            cards.append(_r(rect, x, y))
            cboxes.append(_r(rect, x - CARD_W * k / 2, y - CARD_H * k / 2) + _r(rect, x + CARD_W * k / 2, y + CARD_H * k / 2))
            texts.append(_r(rect, x - TEXT_W * k / 2, y + (TEXT_DY - TEXT_H / 2) * k)
                         + _r(rect, x + TEXT_W * k / 2, y + (TEXT_DY + TEXT_H / 2) * k))
    return {"cards": cards, "boxes": cboxes, "texts": texts, "close_pos": _r(rect, tx, ty + CLOSE_DY * k),
            "title": _r(rect, tx, ty), "k": round(k * 1080 / rect[3], 4),
            # 왼쪽 큰 숫자 (남은 기회 · CHANCES 위)
            "chances": _r(rect, tx + (CHANCE_X - CHANCE_W / 2) * k, ty + (CHANCE_Y - CHANCE_H / 2) * k)
            + _r(rect, tx + (CHANCE_X + CHANCE_W / 2) * k, ty + (CHANCE_Y + CHANCE_H / 2) * k)}


def saved_board(cfg, rect):
    """자동 보정으로 저장한 카드 판 자리 → board_at 결과 또는 None"""
    t, k = cfg.get("board_title"), cfg.get("board_k")
    if not t or not k:
        return None
    lay = board_at(t[0] * rect[2], t[1] * rect[3], float(k) * rect[3] / 1080, rect)
    if cfg.get("board_close_pos"):
        lay["close_pos"] = list(cfg["board_close_pos"])
    return lay


def parse_wait(text):
    """'Available after 03h 04m Left' (OCR: 'AvoilobleofterO3h04mLeft') → 초 또는 None"""
    s = str(text or "")
    i = s.lower().rfind("fter")
    s = (s[i + 4:] if i >= 0 else s).replace("O", "0").replace("o", "0")
    s = re.sub(r"[ilI|]", "1", s)                       # 글꼴 때문에 1 을 i · l 로 읽음 ('iih53m' = 11h53m)
    m = re.search(r"(\d{1,2})\s*h\s*(\d{1,2})\s*m", s, re.I)
    if m:
        return int(m.group(1)) * 3600 + int(m.group(2)) * 60
    m = re.search(r"(\d{1,2})\s*m\s*(\d{1,2})\s*s", s, re.I)
    if m:
        return int(m.group(1)) * 60 + int(m.group(2))
    return None


def is_head(text):
    """알림 창 제목 'Notification' (OCR: 'Notificotior' · 'Notiicot')"""
    n = sell._norm(text)
    return "flcotlon" in n or (n.startswith("notl") and "cot" in n)


def is_ad_text(text):
    """'Watch AD to lower the cooltime for 3 hours!' (광고 보기 칸)"""
    n = sell._norm(text)
    return "wotchod" in n or "lowerthecool" in n or "cooltlmefor" in n


def notice_state(boxes, rect):
    """E 를 누른 뒤 뜨는 알림 창 → {"kind": "start", "pos", "close_pos"} / {"kind": "wait", "sec", "close_pos"} / None
    광고 보기 칸이 보이면 "ad_pos" 도 (없으면 광고를 볼 수 없음)"""
    W, H = rect[2], rect[3]
    k0 = H / 1080
    head = next((b for b in boxes if is_head(b[0])), None)
    starts = [b for b in boxes if sell._norm(b[0]).endswith("tortmemorymotch")]
    # 'Available after Ready!' · 'Available for Ready!' 도 시작 버튼 (Start Memory Match 와 같은 자리)
    starts += [b for b in boxes if "ovolloble" in sell._norm(b[0]) and "reody" in sell._norm(b[0])]
    starts.sort(key=lambda b: b[2])
    avail = next((b for b in boxes if "fter" in b[0].lower() and parse_wait(b[0]) is not None), None)
    ad = next((b for b in boxes if is_ad_text(b[0])), None)
    # 'Start Memory Match?' 큰 글씨 제목은 늘 떠 있음 → 버튼은 제목보다 한참 아래 · 광고 칸 바로 위
    if head:
        starts = [b for b in starts if (b[2] - head[2]) * H > NOTE_BTN_DY * 0.65 * k0]
    if ad:
        starts = [b for b in starts if 0 <= (ad[2] - b[2]) * H < 100 * k0]
    if not head and not ad and len(starts) > 1:
        starts = [max(starts, key=lambda b: b[2])]
    btn = None if avail else (starts[-1] if starts else None)    # 남은 시간이 보이면 아직 못 함
    low = btn or avail
    if not low:
        return None
    if head:
        k = (low[2] - head[2]) * H / NOTE_BTN_DY
        k = k if 0.25 < k < 4 else H / 1080
        hx, hy = head[1] * W, head[2] * H
    else:
        k = H / 1080
        hx, hy = low[1] * W, low[2] * H - NOTE_BTN_DY * k
    close = _r(rect, hx + NOTE_CLOSE[0] * k, hy + NOTE_CLOSE[1] * k)
    ad_pos = [round(ad[1], 4), round(ad[2], 4)] if ad else None
    base = {"close_pos": close, "ad_pos": ad_pos, "pos": [round(low[1], 4), round(low[2], 4)], "head": bool(head),
            "k": k, "head_xy": (hx, hy)}
    if btn:
        return dict(base, kind="start")
    return dict(base, kind="wait", sec=parse_wait(avail[0]))


def measure(boxes, rect):
    """알림 창(E 를 누른 뒤) → 자동 보정 값 · 제목(Notification)을 못 읽으면 None"""
    st = notice_state(boxes, rect)
    if not st or not st["head"]:
        return None
    (hx, hy), k = st["head_xy"], st["k"]
    return {"note_close_pos": st["close_pos"], "note_btn_pos": st["pos"],
            "ad_pos": st["ad_pos"] or _r(rect, hx, hy + NOTE_AD_DY * k),
            "note_region": _r(rect, hx + NOTE_REGION[0] * k, hy + NOTE_REGION[1] * k)
            + _r(rect, hx + NOTE_REGION[2] * k, hy + NOTE_REGION[3] * k),
            "board_title": _r(rect, hx, hy + NOTE_BOARD_DY * k), "board_k": round(k * 1080 / rect[3], 4),
            "board_close_pos": _r(rect, hx, hy + (NOTE_BOARD_DY + CLOSE_DY) * k)}, st


def board_autocal(ocr, rect):
    """카드 판 자동 보정 — 메모리 매치 카드 판이 떠 있을 때: 제목 · CHANCES 글자로 카드 20장 · 남은 기회 · Close 자리를 잼"""
    lay = board_layout(ocr(None), rect)
    if not lay:
        raise RuntimeError("카드 판이 안 보임 — 메모리 매치를 시작해서 카드 판이 떠 있을 때 눌러주세요")
    return {"board_title": lay["title"], "board_k": lay["k"], "board_close_pos": lay["close_pos"], "cal_board": True}


def autocal(ocr, click, wait, rect, status, wait_e=120.0):
    """메모리 매치 자동 보정 — 플레이어가 보드 앞에서 E 를 직접 누르면 알림 창을 글자로 재서 자리를 저장하고 창을 닫음
    (Start · 광고는 안 누름) → ({키: 값}, 알림 글)"""
    status("메모리 매치 보드 앞에서 E 를 눌러 주세요")
    end = time.time() + wait_e
    while True:
        res = measure(ocr(None), rect())
        if res:
            break
        if time.time() > end:
            raise RuntimeError("메모리 매치 창이 안 보임 — 보드 앞에서 E 를 눌러 주세요")
        wait(0.4)
    status("메모리 매치 창 찾음 — 이제 만지지 마세요")
    wait(0.8)
    res = measure(ocr(None), rect()) or res              # 창이 다 뜬 뒤 한 번 더
    found, st = res
    notes = []
    if not st["ad_pos"]:
        notes.append("Watch AD 칸이 안 보여서 그 자리는 계산값")
    notes.append("지금 바로 할 수 있음" if st["kind"] == "start" else
                 f"쿨타임 {st['sec'] // 3600}시간 {st['sec'] % 3600 // 60}분 남음" if st.get("sec") is not None else "")
    status("창 닫는 중")
    for _ in range(2):
        click(found["note_close_pos"])
        wait(0.6)
        if not notice_state(ocr(found["note_region"]), rect()):
            break
    return found, [n for n in notes if n]


def ad_screen(img):
    """로블록스 창 전체 그림(BGR) → 광고 화면이면 {"close": [x, y] (창 비율), "state": "playing" / "ended" / None} · 아니면 None
    광고 화면 = 양옆이 새까맣고 오른쪽 위에 흰 동그라미 X · 아래 아이콘이 ‖ 면 재생 중, ▶ 면 끝남"""
    import numpy as np
    H, W = img.shape[:2]
    if H < 100 or W < 100:
        return None
    s = H / 1080
    side = img[int(H * 0.28):int(H * 0.74), int(W * 0.05):int(W * 0.18)]
    if side.size == 0 or float(side.mean()) > 12:
        return None
    ex = W * AD_CLOSE[0] / 1920 + max(0.0, (W / H - 16 / 9) * H * 0.03)
    ey = H * AD_CLOSE[1] / 1080
    r = max(12, int(40 * s))
    x1, y1 = max(0, int(ex - r)), max(0, int(ey - r))
    win = img[y1:int(ey + r), x1:int(ex + r)]
    ys, xs = np.nonzero(win.min(axis=2) > 180)
    if len(xs) < 60 * s * s:
        return None
    cx, cy = x1 + float(xs.mean()), y1 + float(ys.mean())
    ix, iy = cx + AD_ICON[0] * s, cy + AD_ICON[1] * s
    a, b = max(4, int(20 * s)), max(4, int(16 * s))
    icon = img[max(0, int(iy - b)):int(iy + b), max(0, int(ix - a)):int(ix + a)]
    state = None
    if icon.size:
        cols = (icon.min(axis=2) > 200).sum(axis=0)
        idx = np.nonzero(cols >= 2)[0]
        if len(idx) >= 3:
            state = "playing" if (cols[idx[0]:idx[-1] + 1] < 2).any() else "ended"   # 가운데가 비면 ‖
    return {"close": [round(cx / W, 4), round(cy / H, 4)], "state": state}


def digits(text):
    """카드 개수 글자 → 숫자만 ('x1,000' → '1000' · 'X8,000' → '8000')"""
    return re.sub(r"\D", "", str(text or "").replace("O", "0").replace("o", "0"))


# ---------------------------------------------------------------- 카드 그림
def card_img(box, rect):
    """카드 영역 [x1, y1, x2, y2] (창 비율) → BGR 배열"""
    import numpy as np
    x1, y1 = macro.to_screen(box[0], box[1], rect)
    x2, y2 = macro.to_screen(box[2], box[3], rect)
    data, w, h = macro.grab((x1, y1, max(4, x2 - x1), max(4, y2 - y1)))
    return np.frombuffer(data, np.uint8).reshape(h, w, 4)[:, :, :3]


def card_feat(img):
    """카드 앞면 → (그림의 밝은 부분 평균 색, 그 비율) — 카드 뒷배경 무늬는 빼고 아이템 그림만 (위쪽 62%)
    같은 아이템이면 거의 같음 (동전 = 흰색 · 물약 = 파랑 / 연두 / 진초록 · 수정 = 보라 / 청록)"""
    import numpy as np
    h, w = img.shape[:2]
    ic = img[int(h * 0.08):max(int(h * 0.08) + 1, int(h * 0.62)), int(w * 0.15):max(int(w * 0.15) + 1, int(w * 0.85))]
    ic = ic.reshape(-1, 3).astype(np.float32)
    m = ic.max(axis=1) > 90
    return (ic[m].mean(axis=0) if m.any() else None), float(m.mean())


def same_feat(a, b):
    import numpy as np
    if a is None or b is None or a[0] is None or b[0] is None:
        return False
    return float(np.abs(a[0] - b[0]).max()) < 28 and abs(a[1] - b[1]) < 0.12


def card_sig(img):
    """카드 그림 → (작게 줄인 그림 부분, 그림 가운데 평균 색) — 아래 개수 글자는 빼고"""
    import cv2
    import numpy as np
    h, w = img.shape[:2]
    icon = img[: max(2, int(h * 0.62))]
    small = cv2.resize(icon, (10, 10), interpolation=cv2.INTER_AREA).astype(np.float32)
    ih, iw = icon.shape[:2]
    mean = icon[ih // 4: max(ih // 4 + 1, 3 * ih // 4), iw // 4: max(iw // 4 + 1, 3 * iw // 4)].reshape(-1, 3).mean(axis=0)
    return small, mean


def sig_diff(a, b):
    import numpy as np
    return float(np.abs(a[0] - b[0]).mean())


def same_icon(a, b):
    import numpy as np
    return sig_diff(a, b) < 10 and float(np.abs(a[1] - b[1]).max()) < 30


def green_ratio_sig(sig):
    """서명의 가운데 평균 색이 초록(맞춘 카드)이 아닌지"""
    b, g, r = (float(v) for v in sig[1])
    return not (g > 110 and g > r + 30 and g > b + 30)


def green_ratio(img):
    """맞춘 카드는 초록 바탕 + 체크 → 초록 비율"""
    b, g, r = (img[..., i].astype("int16") for i in range(3))
    return float(((g > 110) & (g > r + 30) & (g > b + 30)).mean())


class Matcher(popping.Popper):
    LABEL = "오토 메모리 매치"

    def __init__(self, get_cfg, log, borrow, place_index):
        super().__init__(lambda: {}, log)
        self.get_mcfg = get_cfg
        self.borrow = borrow                 # 이동기 빌려 쓰기 (with borrow(stop) as mv)
        self.place_index = place_index       # () → 메모리 매치 장소 번호 또는 None
        self.played = 0
        self.ads = 0                         # 이번 실행에서 본 광고 수
        self.next_at = 0.0                   # 이 시각이 되면 다시 확인 (쿨타임)
        self.test = False

    def snapshot(self):
        s = super().snapshot()
        s["played"] = self.played
        s["ads"] = self.ads
        s["next_in"] = max(0, int(self.next_at - time.time()))
        return s

    def start_job(self, here=False, test=False):
        if self.running():
            return False
        import threading
        self.stop_ev = threading.Event()
        self.test = test
        self.thread = threading.Thread(target=self._job, args=(here, self.stop_ev), daemon=True)
        self.thread.start()
        return True

    def _job(self, here, stop):
        try:
            self._run(here, stop)
        except popping.Stopped:
            self.log(f"{self.LABEL} 정지", "d")
        except Exception as e:
            self.log(f"{self.LABEL} 오류: {e}", "r")
            self.next_at = time.time() + 600
        finally:
            self._set(msg="대기")

    # ---- 화면 읽기
    def _read_notice(self, stop):
        """알림 창 읽기 — 자동 보정한 영역만 먼저 (빠름) · 못 찾으면 화면 전체
        제목을 못 읽어서 X 자리를 모르면 보정한 X 자리"""
        c = self.get_mcfg() or {}
        rect = self._rect(stop)
        st = notice_state(macro.ocr_boxes(c["note_region"]), rect) if c.get("note_region") else None
        st = st or notice_state(macro.ocr_boxes(None), rect)
        if st:
            # 보정(자동 · 수동)한 위치가 있으면 그 위치를 누름 — 글자는 창 상태(시작 · 대기 · 광고 칸 유무)를 읽는 데만 씀
            if c.get("note_close_pos"):
                st["close_pos"] = list(c["note_close_pos"])
            if c.get("note_btn_pos"):
                st["pos"] = list(c["note_btn_pos"])
            if st.get("ad_pos") and c.get("ad_pos"):
                st["ad_pos"] = list(c["ad_pos"])
        return st

    def _notice(self, stop, timeout, settle=None):
        """알림 창이 뜰 때까지 → 뜨면 settle 초 기다렸다가 한 번 더 읽음 (창 내용이 서버에서 늦게 바뀔 수 있어서 마지막 상태)"""
        end = time.time() + timeout
        while True:
            st = self._read_notice(stop)
            if st or time.time() > end:
                if st:
                    wait = float((self.get_mcfg() or {}).get("click_wait", 1.0)) if settle is None else settle
                    self._wait(wait, stop)
                    st = self._read_notice(stop) or st
                return st
            self._wait(0.4, stop)

    def _open_notice(self, stop):
        """E → 알림 창 (안 뜨면 E 한 번 더)"""
        c = self.get_mcfg() or {}
        for _ in range(2):
            macro.key_tap("e")
            st = self._notice(stop, float(c.get("e_wait", 2.0)) + 1.5)
            if st:
                return st
        return None

    def _refresh(self, stop, settle=3.0):
        """창 새로고침 (광고를 본 뒤 · 메모리 매치를 한 뒤 · Ready 일 때 — 스크립트 매크로처럼):
        떠 있는 창을 X 로 닫고 → E 로 다시 열고 → 내용이 바뀔 때까지 기다렸다가 읽음"""
        st = self._read_notice(stop)
        if st:
            self._close_notice(st, stop)
            end = time.time() + 3
            while time.time() < end and self._read_notice(stop):
                self._wait(0.4, stop)
        self._wait(1.0, stop)
        c = self.get_mcfg() or {}
        for _ in range(2):
            macro.key_tap("e")
            st = self._notice(stop, float(c.get("e_wait", 2.0)) + 1.5, settle=settle)
            if st:
                return st
        return None

    def _close_notice(self, st, stop):
        if st and st.get("close_pos"):
            self._click(st["close_pos"], stop)
            self._wait(0.4, stop)

    def _schedule(self, st):
        """알림의 남은 시간 → 다음 확인 시각 (쿨타임이 끝나고 1분 뒤 · 광고는 다시 보러 오지 않음)"""
        sec = st.get("sec") if st else None
        if sec is None:
            self.next_at = time.time() + 3600
            return
        self.next_at = time.time() + sec + AFTER_COOLDOWN
        self.log(f"{self.LABEL} — 쿨타임 {sec // 3600}시간 {sec % 3600 // 60}분 남음 · 끝나고 1분 뒤에 다시 감", "d")

    # ---- 전체 순서
    def _run(self, here, stop):
        if not here:
            i = self.place_index()
            if i is None:
                self.log(f"{self.LABEL} 안 함 — 이동 탭에서 '메모리 매치 장소' 를 지정해 주세요", "n")
                self.next_at = time.time() + 600
                return
            self._set(msg="메모리 매치 장소로 가는 중")
            with self.borrow(stop) as mv:
                mv.plan(mv.start_time(i) + mv.place_time(i))
                mv.go_start(i)
                mv.walk(i)
        try:
            with macro.fast_timing():
                self._check_and_play(stop)
        finally:
            if not here:
                self._set(msg="리셋")
                macro.respawn(stop)

    def _check_and_play(self, stop):
        """[체크] 메모리 매치를 할 수 있으면 바로 하고 · 광고를 볼 수 있으면 최대한 봄 (광고로 쿨타임이 끝나면 또 메모리 매치)"""
        self._set(msg="E (메모리 매치 확인)")
        st = self._open_notice(stop)
        if not st:
            self.log(f"{self.LABEL} — 메모리 매치 창이 안 뜸 (메모리 매치 장소 확인) · 10분 뒤 다시", "y")
            self.next_at = time.time() + 600
            return
        ads = ad_fail = 0
        for _ in range(ROUND_MAX):
            if st["kind"] == "start":                       # 할 수 있으면 광고보다 먼저
                if not self._play_round(st, stop):
                    return
                self._wait(1.5, stop)
                st = self._refresh(stop)                     # 메모리 매치를 한 뒤엔 창을 닫고 E 로 새로고침
                if not st:
                    self.next_at = time.time() + 600
                    return
                continue
            if st.get("ad_pos") and ads < AD_MAX and ad_fail < 2:
                ads += 1
                before = st.get("sec")
                self._set(msg=f"광고 보는 중 ({ads}번째)")
                self.log(f"{self.LABEL} — 광고 보기 ({ads}번째 · 쿨타임 3시간 줄이기)", "c")
                shown = self._watch_ad(st["ad_pos"], stop)
                self._check(stop)
                self._wait(3.0, stop)                        # 광고가 닫히고 3초 (스크립트 매크로와 같음)
                nxt = self._refresh(stop)                    # 광고 뒤엔 창을 닫고 E 로 새로고침해야 바뀐 쿨타임이 보임
                if not nxt:
                    self.log(f"{self.LABEL} — 광고 뒤에 메모리 매치 창이 안 뜸 · 10분 뒤 다시", "y")
                    self.next_at = time.time() + 600
                    return
                got = nxt["kind"] == "start" or (before is not None and nxt.get("sec") is not None
                                                 and nxt["sec"] < before - 1800)
                if got:
                    ad_fail = 0
                    self.ads += 1
                    left = "지금 할 수 있음" if nxt["kind"] == "start" else \
                        f"남은 쿨타임 {nxt['sec'] // 3600}시간 {nxt['sec'] % 3600 // 60}분"
                    self.log(f"{self.LABEL} — 광고 보상 받음 · {left}", "g")
                else:
                    ad_fail += 1
                    why = "광고가 안 뜸" if not shown else "쿨타임이 안 줄어듦"
                    self.log(f"{self.LABEL} — 광고 보상 없음 ({why})", "y")
                st = nxt
                continue
            # 광고 칸이 없음(= 오늘 광고를 다 봄) · 광고가 계속 실패 · 이번에 많이 봄 → 쿨타임이 끝나고 1분 뒤에 다시
            if not st.get("ad_pos") and ads:
                self.log(f"{self.LABEL} — 광고를 다 봄 (오늘은 더 볼 수 없음)", "d")
            self._schedule(st)
            self._close_notice(st, stop)
            break
        else:
            self.next_at = time.time() + 600
            self._close_notice(st, stop)
        self.log(f"{self.LABEL} 끝", "g")

    def _play_round(self, st, stop):
        """Start Memory Match → 카드 짝 맞추기 → Close · 판이 안 보이면 False"""
        self._set(msg="Start Memory Match")
        self._wait(1.5, stop)                                # 창이 뜨고 버튼이 눌리게 될 때까지 (스크립트 매크로는 E 뒤 4.5초)
        lay, t0 = None, time.time()
        for n in range(3):
            # 시작 버튼 (Start Memory Match · Available after Ready!) — 안 눌렸으면(판이 안 뜨고 창이 그대로) 다시 누름
            self._click(st["pos"], stop)
            t0, end = time.time(), time.time() + 5
            while not lay and time.time() < end:
                self._wait(0.5, stop)
                lay = board_layout(macro.ocr_boxes(None), self._rect(stop))
            if lay:
                break
            cur = self._read_notice(stop)
            if not cur or cur["kind"] != "start":
                break
            st = cur
            self.log(f"{self.LABEL} — 시작 버튼이 안 눌림 · 다시 누름 ({n + 2}/3)", "d")
            self._wait(1.0, stop)
        c = self.get_mcfg() or {}
        if lay and c.get("cal_board"):                       # 카드 판을 자동 보정했으면 그 자리를 씀
            lay = saved_board(c, self._rect(stop)) or lay
        lay = lay or saved_board(c, self._rect(stop))
        # 판이 새로 깔릴 때까지: 최소 5.5초 + 카드가 전부 뒷면 (바로 뒤엔 지난 판이 그대로 보일 수 있음 · 최대 30초)
        self._set(msg="카드 판 준비 확인 중")
        self._wait(max(0.0, READY_MIN - (time.time() - t0)), stop)
        end = time.time() + 30
        while lay and not self._all_hidden(lay, stop):
            if time.time() > end:
                lay = None
                break
            self._wait(0.5, stop)
        if not lay:
            self.log(f"{self.LABEL} — 카드 판이 안 보임", "y")
            self.next_at = time.time() + 600
            return False
        self._play(lay, stop)
        self.played += 1
        self._set(msg="Close")
        saved = (self.get_mcfg() or {}).get("board_close_pos")
        close = list(saved) if saved else lay["close_pos"]
        end = time.time() + 8
        while time.time() < end:                             # Close 가 뜰 때까지 (보정한 위치가 없으면 글자 자리를 누름)
            b = next((b for b in macro.ocr_boxes(None) if sell._norm(b[0]) in ("close", "c1ose", "lose")), None)
            if b:
                if not saved:
                    close = [b[1], b[2]]
                break
            self._wait(0.5, stop)
        self._wait(1.0, stop)                                # Close 가 뜨고 1초 뒤 (스크립트 매크로와 같음)
        self._click(close, stop)
        self.log(f"{self.LABEL} — 한 판 끝", "g")
        return True

    # ---- 광고
    def _ad(self, stop):
        rect = self._rect(stop)
        return ad_screen(card_img([0, 0, 1, 1], rect))

    def _watch_ad(self, pos, stop):
        """'Watch AD' 를 눌러 광고를 끝까지 보고 닫음 → 광고가 떴으면 True
        광고가 도는 중에는 멈춤 신호가 와도 끝까지 봄 (중간에 닫으면 보상이 없고 화면이 광고에 가려짐) · F7 만 바로 멈춤"""
        self._click(pos, stop)
        end, ad = time.time() + 15, None
        while time.time() < end and ad is None:
            self._wait(0.5, stop)
            ad = self._ad(stop)
        if ad is None:
            return False
        t0, close, ended = time.time(), ad["close"], 0
        while True:
            if macro.key_down_now("f7"):
                raise popping.Stopped()
            el = time.time() - t0
            ad = self._ad(stop)
            if ad is None:
                if el > 3:
                    break                                    # 광고가 알아서 닫힘
            else:
                close = ad["close"]
                ended = ended + 1 if ad["state"] == "ended" else 0
                self._set(msg=f"광고 보는 중 · {int(el)}초")
                # 끝난 게 두 번 보이면 바로 · 아이콘을 못 읽으면 스크립트 매크로처럼 45초 · 아무리 길어도 90초
                if (ended >= 2 and el >= 10) or (ad["state"] is None and el >= 45) or el >= 90:
                    break
            time.sleep(1.0)
        for _ in range(3):                                   # 닫기 X (안 닫히면 다시)
            if self._ad(stop) is None:
                break
            self._click(close, stop)
            time.sleep(2.5)
        time.sleep(1.0)
        return True

    # ---- 카드 짝 맞추기
    def _grab(self, lay, i, stop):
        return card_img(lay["boxes"][i], self._rect(stop))

    def _all_hidden(self, lay, stop):
        """카드 20장이 전부 같은 뒷면(별)인지 — 새 판이 깔렸는지"""
        import numpy as np
        sigs = [card_sig(self._grab(lay, i, stop)) for i in range(len(lay["boxes"]))]
        ref = (np.median([g[0] for g in sigs], axis=0), np.median([g[1] for g in sigs], axis=0))
        return all(sig_diff(g, ref) < 8 and green_ratio_sig(g) for g in sigs)

    def _press_card(self, lay, i, stop):
        """카드 누르기 — 근처로 옮겼다가 0.1초 꾹 (스크립트 매크로와 같은 누름 시간 · 짧게 누르면 씹힐 때가 있음)"""
        rect = self._rect(stop)
        x, y = macro.to_screen(lay["cards"][i][0], lay["cards"][i][1], rect)
        macro.move_to(x + 4, y + 4)
        self._wait(0.05, stop)
        macro.click(x, y, hold_ms=100)

    def _ocr_digits(self, box, stop, scale=1, pad=0):
        """창 비율 영역 → 그 안의 숫자 (키우기 · 둘레 여백을 붙여 읽음 — 큰 글씨 하나만 있으면 여백이 있어야 읽힘)"""
        try:
            import cv2
            import numpy as np
            rect = self._rect(stop)
            x1, y1 = macro.to_screen(box[0], box[1], rect)
            x2, y2 = macro.to_screen(box[2], box[3], rect)
            data, w, h = macro.grab((x1, y1, max(4, x2 - x1), max(4, y2 - y1)))
            img = np.frombuffer(data, np.uint8).reshape(h, w, 4)
            if scale != 1:
                img = cv2.resize(img, (int(w * scale), int(h * scale)), interpolation=cv2.INTER_CUBIC)
            if pad:
                img = cv2.copyMakeBorder(img, pad, pad, pad, pad, cv2.BORDER_CONSTANT, value=(30, 30, 30, 255))
            img = np.ascontiguousarray(img)
            return digits(macro.ocr_bgra(img.tobytes(), img.shape[1], img.shape[0]))
        except Exception:
            return ""

    def _digits(self, lay, i, stop):
        """카드 아래 개수 글자 → 숫자 (작은 글자라 3배로 키워서 읽음) · 못 읽으면 ''"""
        return self._ocr_digits(lay["texts"][i], stop, scale=3, pad=12)

    def _read_chances(self, lay, stop):
        """왼쪽 큰 숫자 (남은 기회) → 0~10 또는 None"""
        d = self._ocr_digits(lay["chances"], stop, pad=30) if lay.get("chances") else ""
        return int(d) if d and int(d) <= 10 else None

    def _reveal(self, lay, i, hidden, stop):
        """카드를 눌러 앞면을 읽음 → (그림 특징, 개수 글자) · 안 뒤집히면(클릭이 씹힘) 한 번 더 누름"""
        self._press_card(lay, i, stop)
        t0, pressed, prev, got = time.time(), 1, None, None
        while time.time() - t0 < 3.5:
            self._wait(0.1, stop)
            img = self._grab(lay, i, stop)
            s = card_sig(img)
            if sig_diff(s, hidden[i]) <= 12:                 # 아직 뒷면
                prev = None
                if time.time() - t0 > 1.0 * pressed and pressed < 3:   # 1초가 지나도 그대로 → 다시 누름 (최대 3번)
                    self._press_card(lay, i, stop)
                    pressed += 1
                continue
            if prev is not None and sig_diff(s, prev) < 4:  # 뒤집히는 애니메이션(흰 번쩍임)이 끝나 그림이 멈춤
                got = img
                break
            prev = s
        if got is None:
            return None
        return card_feat(got), self._digits(lay, i, stop)

    @staticmethod
    def _same(a, b):
        if not (a and b):
            return False
        return same_feat(a[0], b[0]) and (a[1] == b[1] or not a[1] or not b[1])

    def _settle(self, lay, pair, hidden, stop):
        """두 장을 뒤집은 뒤: 맞으면 초록 · 틀리면 다시 뒷면이 될 때까지 → 맞았으면 True"""
        end = time.time() + 4
        while time.time() < end:
            self._wait(0.15, stop)
            imgs = [self._grab(lay, i, stop) for i in pair]
            if all(green_ratio(im) > 0.5 for im in imgs):
                return True
            if all(sig_diff(card_sig(im), hidden[i]) < 8 for im, i in zip(imgs, pair)):
                return False
        return False

    def _play(self, lay, stop):
        """스크립트 매크로와 같은 방식:
        이미 아는 두 장이 같으면 그 두 장 → 아니면 안 뒤집은 카드 하나 → 그게 아는 카드와 같으면 그 카드 · 아니면 안 뒤집은 카드 하나 더
        뒤집은 카드는 전부 기억 (틀려서 다시 덮여도) · 맞춘 카드(초록)는 다시 안 누름 · 기회가 0 이 되면 끝"""
        n = len(lay["cards"])
        self._wait(1.0, stop)
        hidden = {i: card_sig(self._grab(lay, i, stop)) for i in range(n)}
        known, done, bad = {}, set(), set()
        chances, start, stuck = 10, time.time(), 0
        while time.time() - start < 600:
            for i in range(n):                              # 맞춘 카드 (초록)
                if i not in done and green_ratio(self._grab(lay, i, stop)) > 0.5:
                    done.add(i)
                    known.pop(i, None)
            seen = self._read_chances(lay, stop)
            if seen is not None and abs(seen - chances) <= 1:  # 화면 숫자로 맞춤 (잘못 읽은 건 무시 — 세던 값과 1 넘게 다르면)
                chances = seen
            if chances <= 0 or len(done) >= n:
                break
            self._set(msg=f"카드 짝 맞추기 · 남은 기회 {chances} · 맞춤 {len(done) // 2}")
            self._wait(0.3, stop)
            pair = next(((a, b) for a in known for b in known
                         if a < b and (a, b) not in bad and self._same(known[a], known[b])), None)
            flipped = True
            if pair:                                        # 이미 아는 짝 (뒤집혔는지 확인하며 하나씩)
                flipped = self._reveal(lay, pair[0], hidden, stop) is not None \
                    and self._reveal(lay, pair[1], hidden, stop) is not None
            else:
                unknown = [i for i in range(n) if i not in done and i not in known]
                if not unknown:
                    break
                a = random.choice(unknown)
                ra = self._reveal(lay, a, hidden, stop)
                if ra is None:                              # 세 번 눌러도 안 뒤집힘
                    stuck += 1
                    if stuck >= 3:                          # 계속 안 뒤집힘 → 그 판은 끝난 것
                        self.log(f"{self.LABEL} — 카드가 안 뒤집힘 · 판 끝으로 봄", "y")
                        break
                    continue
                stuck = 0
                known[a] = ra
                b = next((j for j in known if j != a and (min(a, j), max(a, j)) not in bad
                          and self._same(known[a], known[j])), None)
                if b is not None:                           # 방금 뒤집은 카드의 짝을 앎
                    flipped = self._reveal(lay, b, hidden, stop) is not None
                else:
                    rest = [i for i in unknown if i != a] or [j for j in known if j != a and j not in done]
                    if not rest:
                        break
                    b = random.choice(rest)
                    rb = self._reveal(lay, b, hidden, stop)
                    flipped = rb is not None
                    if rb is not None:
                        known[b] = rb
                pair = (a, b)
            if self._settle(lay, pair, hidden, stop):        # 맞추면 기회가 안 줄어듦
                for i in pair:
                    done.add(i)
                    known.pop(i, None)
            elif flipped:
                chances -= 1
                bad.add((min(pair), max(pair)))
                self._wait(0.8, stop)                       # 다시 덮이는 애니메이션이 끝날 때까지
            else:
                self._wait(1.0, stop)
