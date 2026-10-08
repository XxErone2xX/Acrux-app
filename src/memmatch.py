# -*- coding: utf-8 -*-
"""
오토 메모리 매치 — 메모리 매치 장소로 가서 E → 알림 창을 읽음 ([체크])
  · Start Memory Match 가 있으면 바로 시작 → 5x4 카드 짝 맞추기 (기회 10번 · 틀릴 때만 1번 줄어듦)
    끝나면 다시 E 로 확인 (광고를 볼 수 있으면 이어서 광고)
  · Available after 00h 00m Left + 'Watch AD to lower the cooltime for 3 hours!' 가 있으면 광고를 봄
    (광고 1번 = 쿨타임 3시간 ↓) → 다시 알림 창을 읽어서 할 수 있으면 메모리 매치 · 아니면 광고를 또 봄 (최대한)
  · 광고 칸이 없으면(= 광고를 볼 수 없음 · 스크립트 매크로와 같은 기준) 남은 시간 뒤에 다시 확인 (최대 1시간 뒤)
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
# 알림 창 ('Notification' 제목 가운데 기준 px · 1080p)
NOTE_CLOSE = (271, -3)                # 오른쪽 위 X
NOTE_BTN_DY = 157                     # 제목 → Start Memory Match / Available after 버튼
# 광고 화면 (1920x1080 기준 px) — 닫기 X 는 스크립트 매크로와 같은 자리 (화면 비율로 맞춤 · 넓은 화면은 조금 오른쪽)
AD_CLOSE = (1513, 239)
AD_ICON = (3, 582)                    # 닫기 X → 아래 재생/일시정지 아이콘 (재생 중엔 ‖ · 끝나면 ▶)
AD_MAX = 8                            # 한 번 확인할 때 볼 광고 최대 수 (광고 1번 = 3시간 · 쿨타임 12시간)
ROUND_MAX = 20                        # 한 번 확인에서 (메모리 매치 + 광고) 최대 반복
NO_AD_RECHECK = 3600                  # 광고를 볼 수 없을 때 다시 확인하는 최대 간격 (초)


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
    tx, ty = t[1] * W, t[2] * H
    cards, cboxes, texts = [], [], []
    for r in range(ROWS):
        for c in range(COLS):
            x, y = tx + (CARD_X0 + CARD_DX * c) * k, ty + (CARD_Y0 + CARD_DY * r) * k
            cards.append(_r(rect, x, y))
            cboxes.append(_r(rect, x - CARD_W * k / 2, y - CARD_H * k / 2) + _r(rect, x + CARD_W * k / 2, y + CARD_H * k / 2))
            texts.append(_r(rect, x - TEXT_W * k / 2, y + (TEXT_DY - TEXT_H / 2) * k)
                         + _r(rect, x + TEXT_W * k / 2, y + (TEXT_DY + TEXT_H / 2) * k))
    return {"cards": cards, "boxes": cboxes, "texts": texts, "close_pos": _r(rect, tx, ty + CLOSE_DY * k)}


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
    # 'Available for Ready!' · 'Available after Ready!' = 바로 할 수 있음 (버튼 자리 같음)
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
    if btn:
        return {"kind": "start", "pos": [round(btn[1], 4), round(btn[2], 4)], "close_pos": close, "ad_pos": ad_pos}
    return {"kind": "wait", "sec": parse_wait(avail[0]), "close_pos": close, "ad_pos": ad_pos}


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
    def _notice(self, stop, timeout):
        end = time.time() + timeout
        while True:
            st = notice_state(macro.ocr_boxes(None), self._rect(stop))
            if st or time.time() > end:
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

    def _close_notice(self, st, stop):
        if st and st.get("close_pos"):
            self._click(st["close_pos"], stop)
            self._wait(0.4, stop)

    def _schedule(self, st, cap=None, ad=False):
        """알림의 남은 시간 → 다음 확인 시각 (cap: 광고를 다시 볼 수 있는지 그보다 먼저 확인)"""
        sec = st.get("sec") if st else None
        if sec is None:
            self.next_at = time.time() + 3600
            return
        wait = sec + 30 if cap is None else min(sec + 30, cap)
        self.next_at = time.time() + wait
        left = f"{sec // 3600}시간 {sec % 3600 // 60}분"
        if wait < sec:
            why = "광고를 더 볼 수 없음" if ad else "광고를 볼 수 없음"
            self.log(f"{self.LABEL} — 쿨타임 {left} 남음 · {why} · {wait // 60}분 뒤 다시 확인", "d")
        else:
            self.log(f"{self.LABEL} — 쿨타임 {left} 남음 · 그때 다시 확인", "d")

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
                self._wait(1.0, stop)
                st = self._open_notice(stop)
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
                nxt = self._notice(stop, 6) or self._open_notice(stop)
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
            # 광고를 볼 수 없음(칸 없음) · 계속 실패 · 이번 확인에서 많이 봄 → 남은 시간 뒤에 (광고를 다시 볼 수 있는지는 더 일찍) 확인
            self._schedule(st, cap=NO_AD_RECHECK if not st.get("ad_pos") else 1800, ad=bool(st.get("ad_pos")))
            self._close_notice(st, stop)
            break
        else:
            self.next_at = time.time() + 600
            self._close_notice(st, stop)
        self.log(f"{self.LABEL} 끝", "g")

    def _play_round(self, st, stop):
        """Start Memory Match → 카드 짝 맞추기 → Close · 판이 안 보이면 False"""
        self._set(msg="Start Memory Match")
        self._click(st["pos"], stop)
        lay, end = None, time.time() + 10
        while not lay and time.time() < end:
            self._wait(0.5, stop)
            lay = board_layout(macro.ocr_boxes(None), self._rect(stop))
        if not lay:
            self.log(f"{self.LABEL} — 카드 판이 안 보임", "y")
            self.next_at = time.time() + 600
            return False
        self._play(lay, stop)
        self.played += 1
        self._set(msg="Close")
        close = lay["close_pos"]
        end = time.time() + 8
        while time.time() < end:
            b = next((b for b in macro.ocr_boxes(None) if sell._norm(b[0]) in ("close", "c1ose", "lose")), None)
            if b:
                close = [b[1], b[2]]
                break
            self._wait(0.5, stop)
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

    def _reveal(self, lay, i, hidden, stop):
        """카드를 눌러 앞면을 읽음 → (그림 서명, 개수 글자)"""
        c = self.get_mcfg() or {}
        self._click(lay["cards"][i], stop)
        self._wait(float(c.get("flip_wait", 0.35)), stop)
        prev, sig, end = None, None, time.time() + 2.5
        while time.time() < end:
            s = card_sig(self._grab(lay, i, stop))
            if sig_diff(s, hidden[i]) > 12 and prev is not None and sig_diff(s, prev) < 4:
                sig = s                              # 뒤집히는 애니메이션이 끝나 그림이 멈춤
                break
            prev = s
            self._wait(0.12, stop)
        sig = sig or prev
        try:
            txt = " ".join(b[0] for b in macro.ocr_boxes(lay["texts"][i]))
        except Exception:
            txt = ""
        return sig, digits(txt)

    @staticmethod
    def _same(a, b):
        if not (a and b and a[0] is not None and b[0] is not None):
            return False
        return same_icon(a[0], b[0]) and (a[1] == b[1] or not a[1] or not b[1])

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
        n = len(lay["cards"])
        self._wait(1.0, stop)
        hidden = {i: card_sig(self._grab(lay, i, stop)) for i in range(n)}
        known, done, chances = {}, set(), 10
        bad = set()                                  # 짝이라고 봤는데 아니었던 두 장 (다시 안 고름)
        while chances > 0 and len(done) < n:
            for i in range(n):                      # 이미 맞춘 카드 (초록)
                if i not in done and green_ratio(self._grab(lay, i, stop)) > 0.5:
                    done.add(i)
                    known.pop(i, None)
            if len(done) >= n:
                break
            pair = next(((a, b) for a in known for b in known
                         if a < b and (a, b) not in bad and self._same(known[a], known[b])), None)
            self._set(msg=f"카드 짝 맞추기 · 남은 기회 {chances}")
            if pair:
                for i in pair:
                    self._click(lay["cards"][i], stop)
                    self._wait(0.5, stop)
            else:
                unknown = [i for i in range(n) if i not in done and i not in known]
                if not unknown:
                    break
                a = random.choice(unknown)
                known[a] = self._reveal(lay, a, hidden, stop)
                b = next((j for j in known if j != a and (min(a, j), max(a, j)) not in bad
                          and self._same(known[a], known[j])), None)
                if b is not None:
                    self._click(lay["cards"][b], stop)
                    self._wait(0.5, stop)
                else:
                    rest = [i for i in unknown if i != a] or [j for j in known if j != a]
                    if not rest:
                        break
                    b = random.choice(rest)
                    known[b] = self._reveal(lay, b, hidden, stop)
                pair = (a, b)
            if self._settle(lay, pair, hidden, stop):         # 맞추면 기회가 안 줄어듦
                for i in pair:
                    done.add(i)
                    known.pop(i, None)
            else:
                chances -= 1
                bad.add((min(pair), max(pair)))
            self._wait(0.3, stop)
