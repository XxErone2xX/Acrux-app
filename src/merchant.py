# -*- coding: utf-8 -*-
"""
상인 자동 구매 — 채팅에 "[Merchant]: Mari has arrived" 가 뜨면 Merchant Teleporter 로 가서 고른 아이템을 삼
  감지: 채팅 글자 영역을 OCR (로블록스 로그엔 채팅이 안 남음 · 채팅창은 항상 켜 둠 · 낚시는 안 멈춤)
  구매: Inventory → Merchant Teleporter 1개 사용 → 대기 → E → 대화 넘기기 → Open
        → 상점 위치 자동 보정 (Set to Max · 상점 제목 글자) · 아래 칸 이름 읽기
        → 살 칸만 (칸 클릭 → 아이템 이름 확인 → Set to Max → Purchase → 대화 넘기기) → 상점 닫기 → 리셋
  방식 · 1920x1080 위치 값은 Noteab 매크로(Apache 2.0)를 참고함
"""
import difflib
import re
import time

import macro
import popping
import sell

NAMES = ("Mari", "Jester", "Rin")
# OCR 로 읽은 채팅 (영문만 · 소문자 · a→o · i→l 로 맞춘 글자) 에서 '이름 has arrived'
_ARRIVE = re.compile(r"(m[o0]rl|jester|r[l1]n)h[o0]s[o0]rrlved")
DIALOG_AREA = sell.DIALOG_AREA
# 상점 자동 보정 — 1920x1080 전체 화면 스크린샷에서 잰 값 (px)
#   기준: [Set to Max] 버튼 글자 가운데 (1341, 614) · 크기: 상점 제목('Mari's Shop')에서 Set to Max 까지 세로 266 px
#   (상점은 창 크기에 맞춰 커지고 작아짐 — 작은 창에서도 이 비율 그대로인 것 확인)
SET_MAX_REF, TITLE_GAP, PURCHASE_GAP = (1341, 614), 266, 48
SHOP_OFFSETS = {"close_pos": (468, -267), "max_pos": (0, 0), "purchase_pos": (-164, 48),
                "first_slot": (-376, 106), "second_slot": (-186, 106)}
ITEM_OFFSET = (-241, -242, 469, -210)       # 오른쪽 위 아이템 이름 줄 ('Mixed Potion | Common')
SLOT_GAP, SLOT_HALF = 190, 92               # 아래 칸 간격 · 칸 반 너비
SHOP_KEYS = ("open_pos", "first_slot", "second_slot", "item_region", "max_pos", "purchase_pos", "close_pos")
# 이름 비교용 상인 아이템 (고를 수 있는 것 + 비슷한 이름을 잘못 사지 않게 다른 것도)
KNOWN_ITEMS = ("Mixed Potion", "Speed Potion", "Lucky Potion", "Fortune Spoid I", "Fortune Spoid II", "Fortune Spoid III",
               "Void Coin", "Lucky Penny", "Gear A", "Gear B", "Strange Potion I", "Strange Potion II",
               "Oblivion Potion", "Heavenly Potion", "Potion of Bound", "Random Potion Sack", "Stella's Candle",
               "Rune of Everything", "Rune of Wind", "Rune of Frost", "Rune of Rainstorm", "Rune of Hell",
               "Rune of Galaxy", "Rune of Corruption", "Rune of Nothing", "Merchant Tracker")
_RARITY = re.compile(r"\b(common|uncommon|rare|epic|legendary|mythic|exalted|divine|special|unique|limited)\b.*$", re.I)


def shop_layout(boxes, rect):
    """상점이 열린 화면의 OCR 덩어리 → 상점 위치들 (창 비율) · 칸 이름 목록 / 못 찾으면 None"""
    sm = sell.find_text(boxes, "settomax", exact=True)
    if not sm:
        return None
    W, H = rect[2], rect[3]
    title = next((b for b in boxes if "sshop" in sell._norm(b[0]) and b[2] < sm[2]), None)
    pur = next((b for b in boxes if sell._norm(b[0]).startswith("purchose") and b[2] > sm[2]), None)
    if title:
        k = (sm[2] - title[2]) * H / TITLE_GAP
    elif pur:
        k = (pur[2] - sm[2]) * H / PURCHASE_GAP
    else:
        k = W / 1920
    if not 0.2 < k < 3:
        return None
    sx, sy = sm[1] * W, sm[2] * H

    def at(dx, dy):
        return [round(min(1.0, max(0.0, (sx + dx * k) / W)), 4), round(min(1.0, max(0.0, (sy + dy * k) / H)), 4)]
    lay = {key: at(*d) for key, d in SHOP_OFFSETS.items()}
    lay["item_region"] = at(*ITEM_OFFSET[:2]) + at(*ITEM_OFFSET[2:])
    # 아래 칸 이름: 칸 줄 높이의 글자를 가장 가까운 칸에 모음 (한 칸 이름이 두 덩어리로 읽혀도 합침)
    row_y, x0 = sy + SHOP_OFFSETS["first_slot"][1] * k, sx + SHOP_OFFSETS["first_slot"][0] * k
    labels = [[] for _ in range(5)]
    for b in sorted(boxes, key=lambda b: b[1]):
        bx, by = b[1] * W, b[2] * H
        if abs(by - row_y) > 22 * k:
            continue
        i = round((bx - x0) / (SLOT_GAP * k))
        if 0 <= i < 5 and abs(bx - (x0 + i * SLOT_GAP * k)) < SLOT_HALF * k:
            labels[i].append(b[0])
    lay["labels"] = [" ".join(t) for t in labels]
    return lay


def open_choice(boxes, fallback=None):
    """대화 선택지 줄에서 Open 위치 — 글자로 못 읽으면 (뒤집혀 읽히기도 함) 옆의 'Who are you?' · 'Leave' 간격으로 계산
    선택지는 보이는데 계산도 못 하면 지정한 위치 · 선택지가 아직 없으면 None"""
    o = sell.find_text(boxes, "open", exact=True)
    if o:
        return [o[1], o[2]]
    who, lv = sell.find_text(boxes, "whooreyou"), sell.find_text(boxes, "leave", exact=True)
    if who and lv and lv[1] > who[1]:
        return [round(2 * who[1] - lv[1], 4), round(who[2], 4)]
    return fallback if (who or lv) else None


def autocal(ocr, click, wait, rect, status, dialog_pos=None, wait_dialog=120.0):
    """상인 위치 자동 보정 — 상인이 와 있을 때 플레이어가 상인 앞에서 E 를 직접 누르면
    대화창 연타 → Open → 상점 글자(Set to Max · 제목)로 상점 위치를 전부 맞춤 → 상점 닫기 (사지는 않음)
    ocr(영역) → 덩어리 목록 · click(위치) · wait(초) (멈춤 확인 포함) · rect() → 창 (x, y, 너비, 높이)
    → 찾은 위치 {키: 값}"""
    status("상인 앞에서 E 를 눌러 주세요")
    end = time.time() + wait_dialog
    while True:
        boxes = ocr(DIALOG_AREA)
        skip = sell.find_text(boxes, "clicktoskip", "toskip")
        pos = open_choice(boxes)
        if pos or skip:
            break
        if time.time() > end:
            raise RuntimeError("대화창이 안 보임 — 상인 앞에서 E 를 눌러 주세요")
        wait(0.4)
    status("대화창 찾음 — 이제 만지지 마세요")
    if not pos:
        dpos = dialog_pos or ([skip[1], skip[2]] if skip else None)
        pos = sell.skip_dialog(lambda: click(dpos), ocr, (), wait, pick=open_choice)
    if not pos:
        raise RuntimeError("선택지 Open 을 못 찾음")
    found = {"open_pos": [round(pos[0], 4), round(pos[1], 4)]}
    wait(0.15)
    click(pos)
    status("상점 여는 중")
    end, lay = time.time() + 6, None
    while not lay:
        if time.time() > end:
            raise RuntimeError("상점(Set to Max 버튼)을 못 찾음")
        wait(0.6)
        lay = shop_layout(ocr(None), rect())
    lay.pop("labels")
    found.update(lay)
    status("상점 닫는 중")
    click(lay["close_pos"])
    wait(0.6)
    return found


def item_name(text):
    """아이템 이름 줄 'Mixed Potion | Common' → 'Mixed Potion' (등급 · 구분선 뺌)"""
    t = (text or "").splitlines()[0] if text else ""
    t = _RARITY.sub("", t.split("|")[0]).strip()
    return popping._clean_name(t)


def match_item(text, names):
    """OCR 이름 → (가장 비슷한 이름, 점수 0~100) — 게임 글꼴에서 헷갈리는 a ↔ o · i ↔ l 은 같게 봄"""
    a = sell._norm(text)
    if not a:
        return None, 0.0
    best, score = None, 0.0
    for n in set(KNOWN_ITEMS) | set(names):
        r = difflib.SequenceMatcher(None, a, sell._norm(n)).ratio() * 100
        if r > score:
            best, score = n, r
    return best, round(score, 1)


# 매크로 기준 위치 설정 → 상인 (버튼 위치)
POS_KEYS = (("open_pos", "Open 선택지"), ("first_slot", "첫 번째 칸"), ("second_slot", "두 번째 칸"),
            ("max_pos", "Set to Max 버튼"), ("purchase_pos", "Purchase 버튼"), ("close_pos", "상점 닫기 X"))


def arrivals(text):
    """채팅 OCR 글자 → {상인 이름: 도착 줄 개수}"""
    out = {}
    for m in _ARRIVE.finditer(sell._norm(text)):
        w = m.group(1)
        name = "Jester" if w.startswith("j") else "Rin" if w.startswith("r") else "Mari"
        out[name] = out.get(name, 0) + 1
    return out


def arrived_in(text):
    """채팅 OCR 글자 → 도착한 상인 이름 (마지막으로 나온 것) 또는 None"""
    n = sell._norm(text)
    found = None
    for m in _ARRIVE.finditer(n):
        w = m.group(1)
        found = "Jester" if w.startswith("j") else "Rin" if w.startswith("r") else "Mari"
    return found


class Merchant(popping.Popper):
    LABEL = "상인 자동 구매"
    COOLDOWN = 600.0                       # 같은 상인을 다시 보기까지 최소 (OCR 이 한 번 잘못 읽어도 또 가지 않게)

    def __init__(self, get_pop, get_base, get_cfg, log, on_done=None, on_cal=None):
        super().__init__(lambda: self._inv_cfg(), log)
        self.get_pop, self.get_base, self.get_mcfg = get_pop, get_base, get_cfg
        self.on_done = on_done
        self.on_cal = on_cal                # 상점 자동 보정으로 찾은 위치 → 저장
        self.seen = {}                     # 상인 이름 → 마지막으로 감지한 시각
        # 채팅창이 항상 켜져 있어서 예전 도착 줄이 오래 남음 → 도착 줄 '개수가 늘었을 때' 만 새로 온 것
        self.counts = None                 # 상인 이름 → 지난번 채팅에 보인 도착 줄 개수 (None = 아직 안 봄)
        self.lower = {}                    # 개수가 줄어든 걸 본 횟수 (OCR 이 한 번 못 읽은 것과 구분 · 두 번 연속이면 반영)
        self.bought = 0
        self.job = None

    def _inv_cfg(self):
        cfg = dict(self.get_pop() or {})
        b = self.get_base() or {}
        for k in (*dict(popping.POS_KEYS), "ocr_region"):
            cfg[k] = b.get(k)
        return cfg

    def buying(self):
        """구매하러 가는 중 (화면을 씀) — 채팅 확인은 화면만 읽어서 다른 기능과 같이 돌아도 됨"""
        return self.running() and self.job == "buy"

    def snapshot(self):
        s = super().snapshot()
        s["job"] = self.job if self.running() else None
        s["bought"] = self.bought
        return s

    @staticmethod
    def missing(c, base):
        """비어 있는 설정 이름 목록 (채팅 감지에 필요한 것 · 구매에 필요한 것)"""
        shop = () if c.get("auto_cal", True) else (
            ("first_slot", "첫 번째 칸"), ("second_slot", "두 번째 칸"), ("item_region", "아이템 이름 영역"),
            ("max_pos", "Set to Max 버튼"), ("purchase_pos", "Purchase 버튼"), ("close_pos", "상점 닫기 X"))
        miss = [n for k, n in (("chat_region", "채팅 글자 영역"), *shop) if not c.get(k)]
        if not base.get("dialog_pos"):
            miss.append("대화창 (통합 위치)")
        return miss + popping.Popper.missing(dict(base))

    # ---- 시작 (별도 스레드)
    def start_job(self, job, name=None):
        if self.running():
            return False
        import threading
        self.stop_ev = threading.Event()
        self.job = job
        self.thread = threading.Thread(target=self._job, args=(job, name, self.stop_ev), daemon=True)
        self.thread.start()
        return True

    def _job(self, job, name, stop):
        try:
            if job == "check":                      # 화면만 읽음 — 낚시를 멈추거나 마우스를 옮기지 않음
                found = self._check_chat(stop)
                if found:
                    self._found = found
            elif job == "buy":
                with macro.fast_timing():
                    self._buy(name, stop)
        except popping.Stopped:
            self.log(f"{self.LABEL} 정지", "d")
        except Exception as e:
            self.log(f"{self.LABEL} 오류: {e}", "r")
        finally:
            self._set(msg="대기")
            if self.on_done:
                try:
                    self.on_done(job, getattr(self, "_found", None) if job == "check" else name)
                except Exception:
                    pass
            self._found = None

    # ---- 채팅 확인
    def _check_chat(self, stop):
        c = self.get_mcfg() or {}
        if not c.get("chat_region"):
            return None
        self._set(msg="채팅 확인")
        try:
            text = macro.ocr_region(c["chat_region"], bring_front=False)
        except Exception as e:
            self.log(f"채팅 OCR 오류: {e}", "r")
            return None
        now = arrivals(text)
        if self.counts is None:                # 처음 봄: 예전 줄일 수 있음 → 채팅 맨 아래 몇 줄에 있을 때만 방금 온 것
            recent = arrivals("\n".join(str(text).splitlines()[-4:]))
            self.counts = {n: now[n] - recent.get(n, 0) for n in now}
        new = [n for n in now if now[n] > self.counts.get(n, 0)]
        for n in set(now) | set(self.counts):
            if now.get(n, 0) >= self.counts.get(n, 0):
                self.counts[n], self.lower[n] = now.get(n, 0), 0
            else:                              # 줄이 위로 밀려 사라짐 — 두 번 연속 줄어들면 반영
                self.lower[n] = self.lower.get(n, 0) + 1
                if self.lower[n] >= 2:
                    self.counts[n], self.lower[n] = now.get(n, 0), 0
        name = next((n for n in new if time.time() - self.seen.get(n, 0) >= self.COOLDOWN), None)
        if not name:
            return None
        self.seen[name] = time.time()
        self.log(f"상인 도착 — {name}", "g")
        return name

    # ---- 구매
    def _wanted(self, name):
        buy = (self.get_mcfg() or {}).get("buy") or {}
        return {k.split("_", 1)[1]: v for k, v in buy.items() if k.startswith(name + "_")}

    def _buy(self, name, stop):
        c = self.get_mcfg() or {}
        base = self.get_base() or {}
        want = self._wanted(name)
        if not want:
            self.log(f"{name} — 살 아이템이 없음 (매크로 기능 설정 → 상인 자동 구매에서 고르기)", "y")
            return
        miss = self.missing(c, base)
        if miss:
            self.log(f"{self.LABEL} 안 함 — 위치 설정 필요: {', '.join(miss)}", "n")
            return
        cfg = self._inv_cfg()
        d = dict(popping.DELAY_DEFAULT, **(cfg.get("delays") or {}))
        # 1. Merchant Teleporter
        self._set(msg="Merchant Teleporter 사용")
        self._click(cfg["inventory_pos"], stop)
        self._wait(d["inventory"], stop)
        self._click(cfg["items_pos"], stop)
        self._wait(d["items"], stop)
        ok = self._use_item(cfg, d, {"name": "Merchant Teleporter", "amount": 1, "min_have": 1}, stop)
        self._wait(d["after_enter"], stop)
        self._click(cfg["inventory_pos"], stop)        # 인벤토리 닫기
        if not ok:
            self.log(f"{self.LABEL} — Merchant Teleporter 를 못 씀", "y")
            return
        try:
            macro.mark_moved()                          # 순간이동 → 다음 리셋은 해야 함
            self._set(msg=f"{name} 에게 가는 중")
            self._wait(float(c.get("teleport_wait", 3.0)), stop)
            # 2. 대화 → Open
            self._set(msg="대화 (E)")
            for _ in range(5):                          # 순간이동 직후엔 E 가 씹힐 수 있어서 여러 번
                macro.key_tap("e")
                self._wait(0.2, stop)
            if not self._open_shop(c, base, stop):
                self.log(f"{self.LABEL} — 상점 Open 을 못 찾음", "y")
                return
            self._wait(1.0, stop)
            # 3. 상점 위치 자동 보정 (Set to Max 글자 기준) → 살 칸만 확인하고 사기
            labels = None
            if c.get("auto_cal", True):
                c, labels = self._calibrate(c, stop)
            if any(not c.get(k) for k in SHOP_KEYS if k != "open_pos"):
                self.log(f"{self.LABEL} — 상점 위치를 못 찾음 (자동 보정 실패 · 직접 지정 필요)", "y")
                return
            self._buy_slots(c, base, name, want, stop, labels)
            self._set(msg="상점 닫기")
            self._click(c["close_pos"], stop)
            self._wait(0.8, stop)
        finally:
            # 4. 리셋 (상인 앞에 남지 않게 · 자동 낚시는 다시 시작하면서 낚시 장소로 감)
            self._set(msg="리셋")
            macro.respawn()

    def _open_shop(self, c, base, stop):
        """선택지가 보일 때까지 대화창 연타 → Open 을 누름 · 못 찾으면 지정한 Open 위치"""
        pos = sell.skip_dialog(lambda: self._click(base["dialog_pos"], stop), macro.ocr_boxes, (),
                               lambda sec: self._wait(sec, stop), pick=lambda b: open_choice(b, c.get("open_pos")))
        if not pos:
            pos = c.get("open_pos")
        if not pos:
            return False
        self._wait(0.15, stop)
        self._click(pos, stop)
        return True

    def _calibrate(self, c, stop):
        """상점이 열린 화면에서 Set to Max · 상점 제목 글자로 위치 계산 → (위치, 칸 이름 목록) · 못 하면 (지정한 위치, None)"""
        self._set(msg="상점 위치 자동 보정")
        end = time.time() + 5
        while True:
            rect = self._rect(stop)
            lay = shop_layout(macro.ocr_boxes(None), rect)
            if lay or time.time() > end:
                break
            self._wait(0.4, stop)
        if not lay:
            self.log("상점 자동 보정 — Set to Max 버튼을 못 찾음", "y")
            return c, None
        labels = lay.pop("labels")
        if self.on_cal:
            self.on_cal(lay)
        self.log("상점 위치 자동 보정 완료", "g")
        return dict(c, **lay), labels

    def _buy_slots(self, c, base, name, want, stop, labels=None):
        f, s2 = c["first_slot"], c["second_slot"]
        gap = (s2[0] - f[0], s2[1] - f[1])
        th = float((self.get_pop() or {}).get("match_threshold", 70))
        for i in range(5):                             # 상점 칸은 항상 5칸
            lab = (labels or [""] * 5)[i] if i < 5 else ""
            if lab:                                    # 칸 이름이 읽혔으면 살 것만 누름
                best, score = match_item(lab, want)
                if best not in want or score < th:
                    self.log(f"{i + 1}번 칸 '{lab}' — 안 삼", "d")
                    continue
            pos = [f[0] + gap[0] * i, f[1] + gap[1] * i]
            self._set(msg=f"상점 {i + 1}번 칸 확인")
            self._click(pos, stop)
            self._wait(0.5, stop)
            try:
                text = macro.ocr_region(c["item_region"], bring_front=False)
            except Exception as e:
                self.log(f"아이템 이름 OCR 오류: {e}", "r")
                continue
            got = item_name(text)
            # 상인 아이템 전체 중 가장 비슷한 것 (Gear A ↔ Gear B · Lucky Potion ↔ Lucky Penny 를 잘못 사지 않게)
            best, score = match_item(got, want)
            if best not in want or score < th:
                self.log(f"{i + 1}번 칸 '{got}' — 안 삼", "d")
                continue
            self._set(msg=f"{best} 구매")
            self._click(c["max_pos"], stop)            # 개수는 Set to Max (살 수 있는 만큼)
            self._wait(0.3, stop)
            self._click(c["purchase_pos"], stop)
            end = time.time() + 3.3                    # 상인 대사 넘기기 (구매 확인) · 연타
            while time.time() < end:
                self._click(base["dialog_pos"], stop)
                self._wait(sell.SKIP_GAP, stop)
            self.bought += 1
            self.log(f"{best} 구매 (Set to Max)", "g")
