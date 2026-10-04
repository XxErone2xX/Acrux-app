# -*- coding: utf-8 -*-
"""
오토 팝핑 — 레어 바이옴 자동 포션 사용 (게임 접속 후)
흐름: 입장 확인 → 대기 → 현재 바이옴 확인(템플릿 없는 바이옴이면 종료)
     → Inventory → Items → [포션마다: Search → 이름 입력 + 엔터 → OCR 확인(일치율, 1회 재시도)
     → 아이템 클릭 → 수량칸 더블클릭 → 개수 입력 + 엔터 → Use] → 바이옴이 끝날 때까지 대기 → 종료

- 기준: 얼로니 SolsRNG 스크립트 (OpenInventoryAndUseItems / _RareBiomePotionTable / _FindInventoryItemByName)
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
    "after_join": 7.5,     # Play 후 게임 입장 확인 → 시작
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


# ---------------------------------------------------------------- 로그로 현재 바이옴
def biome_in(text):
    """텍스트에서 마지막으로 찍힌 바이옴"""
    found = None
    for ln in text.splitlines():
        if "[BloxstrapRPC]" in ln and '"largeImage"' in ln:
            m = biome.RPC_RE.search(ln)
            if m:
                found = biome.norm_biome(m.group(1))
    return found


class SessionBiome:
    """한 로그 파일(이번 접속)을 따라 읽으며 현재 바이옴을 추적"""

    def __init__(self, path):
        self.path, self.pos, self.current = path, 0, None
        self.disconnected = False            # 로그에 접속 끊김이 찍힘

    def update(self):
        if not self.path:
            return self.current
        try:
            with open(self.path, "r", encoding="utf-8", errors="ignore") as f:
                f.seek(self.pos)
                text = f.read()
                self.pos = f.tell()
        except OSError:
            return self.current
        if biome.DISCONNECT_MARK in text:
            self.disconnected = True
        b = biome_in(text)
        if b:
            self.current = b
        return self.current


# ---------------------------------------------------------------- 실행기
class Stopped(Exception):
    pass


class Popper:
    def __init__(self, get_cfg, log, on_end=None):
        self.get_cfg = get_cfg          # () -> pop 설정 dict
        self.log = log
        self.on_end = on_end            # (사용자가 멈춘 것인지) — 오토 팝핑이 끝나면 (매크로 복귀)
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

    def start(self, log_path=None, test_biome=None):
        """log_path: 이번 접속의 로그 파일 (바이옴 확인용) / test_biome: 테스트 — 그 템플릿으로 바로 실행"""
        if self.running():
            self.stop()
            self.thread.join(3)
        self.stop_ev = threading.Event()
        self.thread = threading.Thread(target=self._run, args=(log_path, test_biome, self.stop_ev), daemon=True)
        self.thread.start()
        return True

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
    def _run(self, log_path, test_biome, stop):
        cfg = self.get_cfg() or {}
        d = dict(DELAY_DEFAULT, **(cfg.get("delays") or {}))
        miss = self.missing(cfg)
        stopped = False
        try:
            if miss:
                self.log(f"오토 팝핑 취소 — 설정 필요: {', '.join(miss)}", "y")
                return
            with macro.fast_timing():
                self._sequence(cfg, d, log_path, test_biome, stop)
        except Stopped:
            stopped = True
            self.log("오토 팝핑 정지", "d")
        except Exception as e:
            self.log(f"오토 팝핑 오류: {e}", "r")
        finally:
            self._set(msg="대기")
            stopped = stopped or stop.is_set()
            # 어떤 이유로든 끝나면(사용자가 멈춘 경우·테스트 제외) 매크로 복귀
            if self.on_end and not test_biome:
                try:
                    self.on_end(stopped)
                except Exception as e:
                    self.log(f"복귀 연결 오류: {e}", "r")

    def _sequence(self, cfg, d, log_path, test_biome, stop):
        templates = cfg.get("templates") or {}
        tracker = SessionBiome(log_path)
        if test_biome:
            cur = test_biome
            self.log(f"오토 팝핑 테스트 — {cur} 템플릿", "c")
        else:
            self._set(msg=f"입장 후 대기 {d['after_join']:g}초")
            self._wait(d["after_join"], stop)
            cur = tracker.update()
            if cur and cur in (cfg.get("biomes_on") or {}) and not cfg["biomes_on"].get(cur):
                self.log(f"오토 팝핑 안 함 — {cur} 는 팝핑 바이옴 설정에서 꺼져 있음", "y")
                return
            tpl = templates.get(cur or "") or {}
            if not cur or not tpl.get("items"):
                self.log(f"오토 팝핑 안 함 — 현재 바이옴 {cur or '알 수 없음'} (템플릿 없음)", "y")
                return
            self.log(f"오토 팝핑 시작 — {cur}", "g")
        if self._use_template(cfg, d, cur, stop) is None:
            return

        if test_biome:
            return
        # 바이옴이 끝날 때까지(다른 바이옴이 감지될 때까지) 대기 → 종료 (이후 매크로 복귀로 이어질 예정)
        self._set(msg=f"{cur} 끝날 때까지 대기")
        gone = 0
        while True:
            self._wait(1.0, stop)
            now = tracker.update()
            if now and now != cur:
                self.log(f"바이옴 종료 ({cur} → {now}) — 오토 팝핑 종료", "c")
                return
            if tracker.disconnected:
                self.log("로블록스 접속 끊김 — 오토 팝핑 종료", "y")
                return
            gone = gone + 1 if not macro.roblox_window_cached() else 0
            if gone >= 5:                       # 로블록스가 꺼짐
                self.log("로블록스 창이 사라짐 — 오토 팝핑 종료", "y")
                return

    def _use_template(self, cfg, d, cur, stop, label="오토 팝핑"):
        """Inventory → Items → 템플릿 포션 순서대로 사용. 사용한 개수 (포션 목록이 비면 None)"""
        tpl = (cfg.get("templates") or {}).get(cur) or {}
        items = [it for it in (tpl.get("items") or []) if it.get("name")]
        if not items:
            self.log(f"{cur} 템플릿에 포션이 없음", "y")
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
        self.log(f"{label} 완료 — {cur} · {used}/{1 if one else len(items)}개 사용", "g")
        return used

    def _use_item(self, cfg, d, it, stop):
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
    - 버튼 위치 · OCR · 딜레이 · 일치율은 오토 팝핑(pop) 설정을 같이 씀, 포션 목록 · 켜진 바이옴은 따로(mpop)
    - 바이옴이 끝날 때까지 기다리거나 매크로 복귀로 이어지지 않음 (내 서버에 그대로 있음)"""

    LABEL = "레어 바이옴 자동 팝핑"

    def __init__(self, get_pop, get_mpop, log):
        super().__init__(lambda: self._merged(), log)
        self.get_pop, self.get_mpop = get_pop, get_mpop

    def _merged(self):
        cfg = dict(self.get_pop() or {})
        mp = self.get_mpop() or {}
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
                self.log(f"{self.LABEL} 취소 — 오토 팝핑 설정 필요: {', '.join(miss)}", "y")
                return
            with macro.fast_timing():
                if test:
                    self.log(f"{self.LABEL} 테스트 — {cur} 템플릿", "c")
                else:
                    self.log(f"{self.LABEL} 시작 — {cur} (내 서버)", "g")
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
