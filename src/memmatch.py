# -*- coding: utf-8 -*-
"""
오토 메모리 매치 — 메모리 매치 장소로 가서 E → 알림 창을 읽음
  · Start Memory Match 가 있으면 시작 → 5x4 카드 짝 맞추기 (기회 10번 · 두 장 뒤집을 때마다 1번)
  · Available after 00h 00m Left 면 그 시간만큼 기다렸다가 다시 확인
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
    m = re.search(r"(\d{1,2})\s*h\s*(\d{1,2})\s*m", s, re.I)
    if m:
        return int(m.group(1)) * 3600 + int(m.group(2)) * 60
    m = re.search(r"(\d{1,2})\s*m\s*(\d{1,2})\s*s", s, re.I)
    if m:
        return int(m.group(1)) * 60 + int(m.group(2))
    return None


def notice_state(boxes, rect):
    """E 를 누른 뒤 뜨는 알림 창 → {"kind": "start", "pos", "close_pos"} / {"kind": "wait", "sec", "close_pos"} / None"""
    W, H = rect[2], rect[3]
    head = next((b for b in boxes if "flcotlon" in sell._norm(b[0])), None)
    starts = [b for b in boxes if sell._norm(b[0]).endswith("tortmemorymotch")]
    avail = next((b for b in boxes if "fter" in b[0].lower() and parse_wait(b[0]) is not None), None)
    if head:
        # 제목 바로 아래 'Start Memory Match?' 는 큰 글씨 제목 · 버튼은 그보다 한참 아래
        starts = [b for b in starts if (b[2] - head[2]) * H > NOTE_BTN_DY * 0.65 * H / 1080]
    elif len(starts) > 1:
        starts = [max(starts, key=lambda b: b[2])]
    btn = starts[-1] if starts else None
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
    if btn:
        return {"kind": "start", "pos": [round(btn[1], 4), round(btn[2], 4)], "close_pos": close}
    return {"kind": "wait", "sec": parse_wait(avail[0]), "close_pos": close}


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
        self.next_at = 0.0                   # 이 시각이 되면 다시 확인 (쿨타임)
        self.test = False

    def snapshot(self):
        s = super().snapshot()
        s["played"] = self.played
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

    def _schedule(self, st):
        """알림의 남은 시간 → 다음 확인 시각"""
        sec = st.get("sec") if st else None
        if sec is None:
            self.next_at = time.time() + 3600
            return
        self.next_at = time.time() + sec + 30
        self.log(f"{self.LABEL} — 쿨타임 {sec // 3600}시간 {sec % 3600 // 60}분 남음 · 그때 다시 확인", "d")

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
        self._set(msg="E (메모리 매치 확인)")
        st = self._open_notice(stop)
        if not st:
            self.log(f"{self.LABEL} — 메모리 매치 창이 안 뜸 (메모리 매치 장소 확인) · 10분 뒤 다시", "y")
            self.next_at = time.time() + 600
            return
        if st["kind"] == "wait":
            self._schedule(st)
            self._close_notice(st, stop)
            return
        self._set(msg="Start Memory Match")
        self._click(st["pos"], stop)
        lay, end = None, time.time() + 10
        while not lay and time.time() < end:
            self._wait(0.5, stop)
            lay = board_layout(macro.ocr_boxes(None), self._rect(stop))
        if not lay:
            self.log(f"{self.LABEL} — 카드 판이 안 보임", "y")
            self.next_at = time.time() + 600
            return
        self._play(lay, stop)
        self.played += 1
        # 끝 → Close · 다시 E 로 남은 시간 확인
        self._set(msg="Close")
        close = lay["close_pos"]
        end = time.time() + 6
        while time.time() < end:
            b = next((b for b in macro.ocr_boxes(None) if sell._norm(b[0]) in ("close", "c1ose", "lose")), None)
            if b:
                close = [b[1], b[2]]
                break
            self._wait(0.5, stop)
        self._click(close, stop)
        self._wait(1.0, stop)
        st = self._open_notice(stop)
        self._schedule(st if st and st["kind"] == "wait" else None)
        self._close_notice(st, stop)
        self.log(f"{self.LABEL} 끝", "g")

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
            chances -= 1
            if self._settle(lay, pair, hidden, stop):
                for i in pair:
                    done.add(i)
                    known.pop(i, None)
            else:
                bad.add((min(pair), max(pair)))
            self._wait(0.3, stop)
