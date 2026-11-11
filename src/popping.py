# -*- coding: utf-8 -*-
"""
내 서버의 포션 · 아이템 사용
흐름: Inventory → Items → [포션마다: Search → 이름 입력 + 엔터 → OCR 확인(일치율, 1회 재시도)
     → 아이템 클릭 → 수량칸 더블클릭 → 개수 입력 + 엔터 → Use] → 인벤토리 닫기

- 아이템 찾기
  · 이름은 '정확 일치' 원칙 — "Warp Potion" 을 찾을 때 "Super Warp Potion", "Warp Potion X" 같은
    이름이 더 붙은 다른 아이템은 집지 않음
  · OCR 은 글자가 조금씩 틀릴 수 있어서, 정규화(NFKC · 소문자 · 기호 제거) 후 일치율(%)로 판단
  · 목록 순서 = 우선순위 / amount 는 숫자 또는 ALL / 보유 개수가 없으면(0) 건너뜀
  · '하나만' 모드 = 스크립트의 condition 우선순위 트리 (조건 맞는 첫 항목 하나만 사용하고 멈춤)
"""
import difflib
import re
import threading
import time
import unicodedata

import macro
import biome

POP_BIOMES = ("CYBERSPACE", "GLITCHED", "DREAMSPACE")
POS_KEYS = (("inventory_pos", "Inventory 버튼"), ("items_pos", "Items 버튼"), ("search_pos", "Search 버튼"),
            ("item_pos", "아이템 칸"), ("amount_pos", "수량 입력칸"), ("use_pos", "Use 버튼"))
DELAY_DEFAULT = {
    "inventory": 0.5,      # Inventory 클릭 후
    "items": 0.5,          # Items 클릭 후
    "search": 0.5,         # Search 클릭 후
    "typing": 0.5,         # 아이템 이름 입력 + 엔터 후
    "item_click": 0.35,    # 아이템 클릭 후
    "dbl_gap": 0.1,        # 수량칸 더블클릭 간격
    "after_dbl": 0.5,      # 수량칸 더블클릭 후
    "after_enter": 0.35,   # 개수 입력 + 엔터 후
}

COUNT_RE = re.compile(r"[x×X]\s*([0-9][0-9,\.]*)\s*$")
# OCR 이 개수 부분을 조금 틀리게 읽은 경우 (x1O, xl3 등) — 끝에 붙은 'x…' 덩어리
LOOSE_COUNT_RE = re.compile(r"\s[x×X]\s*([0-9lIiO|o,\.]+)\s*$")
_DIGIT_FIX = str.maketrans({"l": "1", "I": "1", "i": "1", "|": "1", "O": "0", "o": "0"})
ZERO_WIDTH_RE = re.compile(r"[​-‏⁠﻿­]")


# ---------------------------------------------------------------- 이름 일치율
def norm_name(s):
    """NFKC 정규화 + 소문자 + 글자·숫자만 (공백·기호 제거)"""
    s = unicodedata.normalize("NFKC", ZERO_WIDTH_RE.sub("", s or "")).lower()
    return "".join(ch for ch in s if ch.isalnum())


_JUNK_TOKEN = re.compile(r"^(?=.*[0-9])[0-9SsOoIl|]*[x×X][0-9SsOoIl|,\.]*$|^[0-9,\.]+$")


def _clean_name(name):
    """이름에 섞여 들어온 개수 조각(8X, S6X, 23 등) 제거 — 로마 숫자(II)는 그대로"""
    return " ".join(t for t in name.split() if not _JUNK_TOKEN.match(t))


def parse_ocr(text):
    """'Warp Potion x23' → ('Warp Potion', 23). 개수를 못 읽으면 None"""
    t = " ".join((text or "").split())
    m = COUNT_RE.search(t)
    if not m:
        m = LOOSE_COUNT_RE.search(t)
        if not m:
            return _clean_name(t), None
        digits = re.sub(r"[^0-9]", "", m.group(1).translate(_DIGIT_FIX))
        return _clean_name(t[:m.start()]), (int(digits) if digits else None)
    digits = re.sub(r"[^0-9]", "", m.group(1))
    return _clean_name(t[:m.start()]), (int(digits) if digits else None)


def _words(s):
    s = unicodedata.normalize("NFKC", ZERO_WIDTH_RE.sub("", s or "")).lower()
    return [w for w in re.split(r"[^0-9a-z가-힣]+", s) if w]


def similarity(ocr_name, target):
    """0~100 (%) — 정규화 후 같으면 100, 아니면 SequenceMatcher 비율.
    단, 찾는 이름에 단어가 더 붙은 다른 아이템(Super Warp Potion / Warp Potion X)은 0 (정확 일치 원칙)"""
    a, b = norm_name(ocr_name), norm_name(target)
    if not a or not b:
        return 0.0
    if a == b:
        return 100.0
    ow, tw = _words(ocr_name), _words(target)
    n = len(tw)
    if n and len(ow) > n and any(ow[i:i + n] == tw for i in range(len(ow) - n + 1)):
        return 0.0
    return round(difflib.SequenceMatcher(None, a, b).ratio() * 100, 1)


class Stopped(Exception):
    pass


class Popper:
    def __init__(self, get_cfg, log):
        self.get_cfg = get_cfg          # () -> pop 설정 dict
        self.log = log
        self.stop_ev = threading.Event()
        self.thread = None
        self.lock = threading.Lock()
        self.state = {"running": False, "msg": "대기"}

    def running(self):
        return bool(self.thread and self.thread.is_alive())

    def snapshot(self):
        with self.lock:
            return dict(self.state, running=self.running())

    def _set(self, **kw):
        with self.lock:
            self.state.update(kw)

    def stop(self):
        self.stop_ev.set()


    @staticmethod
    def missing(cfg):
        """비어 있는 필수 설정 이름 목록"""
        miss = [name for key, name in POS_KEYS if not cfg.get(key)]
        if not cfg.get("ocr_region"):
            miss.append("OCR 영역")
        return miss

    # ---- 동작 도우미 (정지·F7 확인 포함)
    def _check(self, stop):
        if stop.is_set() or macro.key_down_now("f7"):
            stop.set()
            raise Stopped()

    def _wait(self, sec, stop):
        end = time.time() + max(0.0, float(sec))
        while True:
            self._check(stop)
            left = end - time.time()
            if left <= 0:
                return
            time.sleep(min(0.05, left))

    def _rect(self, stop):
        for _ in range(20):
            hwnd = macro.roblox_window_cached()
            rect = macro.client_rect(hwnd) if hwnd else None
            if rect and rect[2] > 50 and rect[3] > 50:
                macro.focus(hwnd)
                return rect
            self._wait(0.5, stop)
        raise RuntimeError("로블록스 창을 찾을 수 없음")

    def _click(self, pos, stop, double_gap=None):
        rect = self._rect(stop)
        x, y = macro.to_screen(pos[0], pos[1], rect)
        macro.move_to(x + 3, y + 3)
        time.sleep(0.03)
        macro.click(x, y)
        if double_gap is not None:
            self._wait(double_gap, stop)
            macro.click(x, y)

    def _ocr(self, cfg):
        try:
            return macro.ocr_region(cfg["ocr_region"], item=True)
        except Exception as e:
            self.log(f"OCR 오류: {e}", "r")
            return ""

    # ---- 메인


    def _use_template(self, cfg, d, cur, stop, label="오토 팝핑"):
        """Inventory → Items → 템플릿 포션 순서대로 사용. 사용한 개수 (포션 목록이 비면 None)"""
        tpl = (cfg.get("templates") or {}).get(cur) or {}
        items = [it for it in (tpl.get("items") or []) if it.get("name")]
        if not items:
            self.log(f"{biome.show_name(cur)} 템플릿에 포션이 없음", "y")
            return None

        self._set(msg="Inventory 열기")
        self._click(cfg["inventory_pos"], stop)
        self._wait(d["inventory"], stop)
        self._click(cfg["items_pos"], stop)
        self._wait(d["items"], stop)

        one = tpl.get("mode") == "one"        # 위에서부터 조건 맞는 하나만
        used = 0
        for it in items:
            if self._use_item(cfg, d, it, stop):
                used += 1
                if one:
                    break
        self.log(f"{label} 완료 — {biome.show_name(cur)} · {used}/{1 if one else len(items)}개 사용", "g")
        return used

    def _use_item(self, cfg, d, it, stop, dry=False):
        """dry: 검색 · 개수 확인까지만 (쓰지 않음 — 블러드 레인 테스트)"""
        name = it["name"].strip()
        th = float(cfg.get("match_threshold", 70))
        count = None
        for attempt in (1, 2):                    # 일치율 미달이면 Search 부터 1회 재시도
            self._set(msg=f"{name} 검색" + (" (재시도)" if attempt == 2 else ""))
            self._click(cfg["search_pos"], stop)
            self._wait(d["search"], stop)
            macro.paste_text(name)
            macro.key_tap("enter")
            self._wait(d["typing"], stop)
            text = self._ocr(cfg)
            ocr_name, count = parse_ocr(text)
            score = similarity(ocr_name, name)
            self.log(f"OCR: '{text[:40]}' · {name} 일치율 {score:g}%", "d")
            if score >= th:
                break
            count = None
            if attempt == 2:
                self.log(f"{name} — 일치율 {th:g}% 미만, 스킵", "y")
                return False
        if count is None:
            # 게임은 1개뿐인 아이템엔 'x1' 을 안 띄움 (스크립트의 getAmount 도 개수 표시가 없으면 1개)
            if not re.search(r"\d", text):
                count = 1
            else:
                self.log(f"{name} — 개수를 읽지 못해 스킵", "y")
                return False
        need = max(1, int(it.get("min_have") or 1))
        if count < need:
            self.log(f"{name} — 보유 {count}개 (필요 {need}개), 스킵", "y")
            return False
        amt = it.get("amount", "ALL")
        use = count if str(amt).upper() == "ALL" else min(count, max(1, int(amt)))
        if dry:
            self.log(f"{name} 보유 {count}개 — 테스트라 사용 안 함 (실제로는 {use}개 사용)", "c")
            return True

        self._set(msg=f"{name} {use}개 사용")
        self._click(cfg["item_pos"], stop)
        self._wait(d["item_click"], stop)
        self._click(cfg["amount_pos"], stop, double_gap=d["dbl_gap"])
        self._wait(d["after_dbl"], stop)
        macro.paste_text(str(use))
        macro.key_tap("enter")
        self._wait(d["after_enter"], stop)
        self._click(cfg["use_pos"], stop)
        self.log(f"{name} {use}개 사용", "g")
        return True


# ---------------------------------------------------------------- 매크로 탭: 내 서버 팝핑
class MyServerPopper(Popper):
    """지금 켜져 있는 로블록스(내 서버)에서 레어 바이옴이 감지되면 바로 포션 사용
    - 버튼 위치 · OCR 영역 · 포션 목록 · 켜진 바이옴은 따로(mpop), 딜레이 · 일치율은 공통 아이템 사용 설정(pop)을 씀
    - 바이옴이 끝날 때까지 기다리거나 매크로 복귀로 이어지지 않음 (내 서버에 그대로 있음)"""

    LABEL = "레어 바이옴 자동 팝핑"

    def __init__(self, get_pop, get_mpop, log, before=None, after=None):
        super().__init__(lambda: self._merged(), log)
        self.get_pop, self.get_mpop = get_pop, get_mpop
        self.before, self.after = before, after     # 시작 전 · 끝난 뒤 (자동 낚시를 잠깐 멈추고 다시 이어가기)
        self.test = False                           # 테스트로 돌리는 중 (기능을 꺼도 끝까지)

    def _merged(self):
        cfg = dict(self.get_pop() or {})
        mp = self.get_mpop() or {}
        for k in (*dict(POS_KEYS), "ocr_region"):
            cfg[k] = mp.get(k)
        cfg["templates"] = mp.get("templates") or {}
        cfg["biomes_on"] = mp.get("biomes_on") or {}
        return cfg

    def start(self, biome_name, test=False):
        if self.running():
            if test:
                self.stop()
                self.thread.join(3)
            else:
                return False                   # 이미 쓰는 중이면 겹쳐서 실행하지 않음
        self.test = test
        self.stop_ev = threading.Event()
        self.thread = threading.Thread(target=self._run_my, args=(biome_name, test, self.stop_ev), daemon=True)
        self.thread.start()
        return True

    def _run_my(self, cur, test, stop):
        cfg = self._merged()
        mp = self.get_mpop() or {}
        d = dict(DELAY_DEFAULT, **(cfg.get("delays") or {}))
        miss = self.missing(cfg)
        try:
            if miss:
                self.log(f"{self.LABEL} 취소 — 공통 위치 지정 필요: {', '.join(miss)}", "n")
                return
            if self.before and not self.before():      # 낚시가 안 멈췄는데 클릭하면 서로 꼬임 → 이번엔 안 함
                self.log(f"{self.LABEL} 취소 — 자동 낚시가 멈추지 않음", "r")
                return
            self._check(stop)
            with macro.fast_timing():
                self.log(self._intro(cur, test), "c" if test else "g")
                if not test:
                    wait = float(mp.get("start_delay", 1.0))
                    self._set(msg=f"시작 전 대기 {wait:g}초")
                    self._wait(wait, stop)
                used = self._use_template(cfg, d, cur, stop, label=self.LABEL)
                if used is not None and mp.get("close_inventory", True):
                    self._wait(d["after_enter"], stop)
                    self._set(msg="Inventory 닫기")
                    self._click(cfg["inventory_pos"], stop)
        except Stopped:
            self.log(f"{self.LABEL} 정지", "d")
        except Exception as e:
            self.log(f"{self.LABEL} 오류: {e}", "r")
        finally:
            self._set(msg="대기")
            if self.after:
                try:
                    self.after()
                except Exception:
                    pass

    def _intro(self, cur, test):
        return f"{self.LABEL} 테스트 — {cur} 템플릿" if test else f"{self.LABEL} 시작 — {cur} (내 서버)"


# ---------------------------------------------------------------- 매크로 탭: 블러드 레인 자동 생성 (할로윈 이벤트)
BLOOD_BIOMES = ("PUMPKINMOON", "GRAVEYARD")       # 이 바이옴에서 Cursed Rune Fragment 를 쓰면 BLOOD RAIN
BLOOD_ITEM = {"name": "Cursed Rune Fragment", "amount": 1, "min_have": 2}   # 2개 이상 있을 때만 1개 사용


class BloodRainer(MyServerPopper):
    """내 서버에서 PUMPKIN MOON · GRAVEYARD 가 떠 있으면 Cursed Rune Fragment 를 1개 사용 (2개 이상 있을 때만) → BLOOD RAIN
    흐름은 레어 바이옴 자동 팝핑과 같음: 시작 전 대기 → Inventory → Items → 검색 · OCR 로 개수 확인 → 1개 사용 → Inventory 닫기
    테스트는 검색 · 개수 확인까지만 (아이템을 쓰지 않음)"""

    LABEL = "블러드 레인 자동 생성"

    def __init__(self, get_pop, get_cfg, log, before=None, after=None):
        super().__init__(get_pop, get_cfg, log, before, after)
        self.used = 0                              # 이번 실행에서 쓴 Cursed Rune Fragment 개수

    def snapshot(self):
        return dict(super().snapshot(), used=self.used)

    def _intro(self, cur, test):
        if test:
            return f"{self.LABEL} 테스트 — {BLOOD_ITEM['name']} 개수만 확인"
        return f"{self.LABEL} 시작 — {biome.show_name(cur)} (내 서버)"

    def _use_template(self, cfg, d, cur, stop, label=""):
        self._set(msg="Inventory 열기")
        self._click(cfg["inventory_pos"], stop)
        self._wait(d["inventory"], stop)
        self._click(cfg["items_pos"], stop)
        self._wait(d["items"], stop)
        if not self._use_item(cfg, d, dict(BLOOD_ITEM), stop, dry=self.test):
            return 0                               # 2개 미만 · 못 찾음 (이유는 위에서 남김)
        if not self.test:
            self.used += 1
            self.log(f"{self.LABEL} 완료 — {biome.show_name(cur)} · {BLOOD_ITEM['name']} 1개 사용", "g")
        return 1


# ---------------------------------------------------------------- 매크로 탭: 오토 아이템 사용
class ItemUser(Popper):
    """쿨타임이 찰 때마다 인벤토리에서 아이템을 1개씩 사용 
    Inventory → Items → (아이템 검색 → OCR 로 이름 확인 → 1개 사용) → Inventory 닫기
    위치 · OCR 영역은 매크로 기준 위치, 딜레이 · 일치율은 오토 팝핑 설정을 같이 씀"""

    LABEL = "오토 아이템 사용"
    ITEMS = (("strange", "Strange Controller", "strange_min"), ("randomizer", "Biome Randomizer", "randomizer_min"))
    RETRY = 60.0                                  # 하나도 못 쓰고 끝나면 1분 뒤 다시 시도

    def __init__(self, get_pop, get_pos, get_cfg, log, before=None, after=None):
        super().__init__(lambda: self._merged(), log)
        self.get_pop, self.get_pos, self.get_item_cfg = get_pop, get_pos, get_cfg
        self.before, self.after = before, after
        self.last = {k: 0.0 for k, _, _ in self.ITEMS}     # 마지막으로 쓴 시각 (켜면 바로 한 번 씀)
        self.used = {k: 0 for k, _, _ in self.ITEMS}       # 이번 실행 사용 횟수
        self.retry_at = 0.0
        self.test = False                                   # 테스트로 돌리는 중 (매크로가 꺼져 있어도 끝까지)

    def _merged(self):
        cfg = dict(self.get_pop() or {})
        pos = self.get_pos() or {}
        for k in (*dict(POS_KEYS), "ocr_region"):
            cfg[k] = pos.get(k)
        return cfg

    def due(self, now=None):
        """지금 쓸 차례인 아이템 키 목록"""
        now = now or time.time()
        if now < self.retry_at:
            return []
        c = self.get_item_cfg() or {}
        return [k for k, _, mk in self.ITEMS if c.get(k) and now - self.last[k] >= float(c.get(mk, 10)) * 60]

    def snapshot(self):
        snap = super().snapshot()
        c, now = self.get_item_cfg() or {}, time.time()
        snap["items"] = {k: {"on": bool(c.get(k)), "used": self.used[k],
                             "left": max(0.0, max(self.last[k] + float(c.get(mk, 10)) * 60, self.retry_at) - now)}
                         for k, _, mk in self.ITEMS}
        return snap

    def start(self, keys, test=False):
        if self.running() or not keys:
            return False
        self.stop_ev = threading.Event()
        self.test = test
        self.thread = threading.Thread(target=self._run_items, args=(list(keys), test, self.stop_ev), daemon=True)
        self.thread.start()
        return True

    def _run_items(self, keys, test, stop):
        cfg = self._merged()
        c = self.get_item_cfg() or {}
        d = dict(DELAY_DEFAULT, **(cfg.get("delays") or {}))
        miss = self.missing(cfg)
        names = dict((k, n) for k, n, _ in self.ITEMS)
        held = False
        try:
            if miss:
                self.log(f"{self.LABEL} 안 함 — 공통 위치 지정 필요: {', '.join(miss)}", "n")
                self.retry_at = time.time() + self.RETRY
                return
            if self.before:
                held = True
                if not self.before():
                    self.retry_at = time.time() + self.RETRY
                    self.log(f"{self.LABEL} 취소 — 자동 낚시가 멈추지 않음 · 1분 뒤 다시 시도", "y")
                    return
            self._check(stop)                       # 기다리는 사이 멈췄으면(레어 바이옴 팝핑 등) 아무것도 안 누름
            with macro.fast_timing():
                self.log(f"{self.LABEL}{' 테스트' if test else ''} — {', '.join(names[k] for k in keys)}", "c")
                self._set(msg="Inventory 열기")
                self._click(cfg["inventory_pos"], stop)
                self._wait(d["inventory"], stop)
                self._click(cfg["items_pos"], stop)
                self._wait(d["items"], stop)
                ok = 0
                for k in keys:
                    if self._use_item(cfg, d, {"name": names[k], "amount": 1, "min_have": 1}, stop):
                        self.last[k] = time.time()
                        self.used[k] += 1
                        ok += 1
                    self._wait(d["after_enter"], stop)
                if not ok:
                    self.retry_at = time.time() + self.RETRY
                    self.log(f"{self.LABEL} — 쓴 아이템 없음 · 1분 뒤 다시 시도", "y")
                if c.get("close_inventory", True):
                    self._set(msg="Inventory 닫기")
                    self._click(cfg["inventory_pos"], stop)
        except Stopped:
            self.log(f"{self.LABEL} 정지", "d")
        except Exception as e:
            self.retry_at = time.time() + self.RETRY
            self.log(f"{self.LABEL} 오류: {e}", "r")
        finally:
            self._set(msg="대기")
            if held and self.after:
                try:
                    self.after()
                except Exception:
                    pass
