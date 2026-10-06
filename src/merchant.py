# -*- coding: utf-8 -*-
"""
상인 자동 구매 — 채팅에 "[Merchant]: Mari has arrived" 가 뜨면 Merchant Teleporter 로 가서 고른 아이템을 삼
  감지: 채팅창 위에 마우스를 올려(채팅이 보이게) 채팅 글자 영역을 OCR (로블록스 로그엔 채팅이 안 남음)
  구매: Inventory → Merchant Teleporter 1개 사용 → 대기 → E → 대화 넘기기 → Open
        → 칸마다 (칸 클릭 → 아이템 이름 OCR → 고른 아이템이면 수량 입력 → Purchase → 대화 넘기기) → 상점 닫기 → 리셋
  방식 · 1920x1080 위치 값은 Noteab 매크로(Apache 2.0)를 참고함
"""
import re
import time

import macro
import popping
import sell
import watcher_core

NAMES = ("Mari", "Jester", "Rin")
# OCR 로 읽은 채팅 (영문만 · 소문자 · a→o · i→l 로 맞춘 글자) 에서 '이름 has arrived'
_ARRIVE = re.compile(r"(m[o0]rl|jester|r[l1]n)h[o0]s[o0]rrlved")
DIALOG_AREA = sell.DIALOG_AREA
# 매크로 기준 위치 설정 → 상인 (버튼 위치)
POS_KEYS = (("chat_hover", "채팅창 위치"), ("open_pos", "Open 선택지"), ("first_slot", "첫 번째 칸"), ("second_slot", "두 번째 칸"),
            ("amount_pos", "수량 입력칸"), ("purchase_pos", "Purchase 버튼"), ("close_pos", "상점 닫기 X"))


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
    COOLDOWN = 300.0                       # 같은 상인을 다시 보기까지 (상인은 몇 분 동안 머묾 · 같은 채팅을 또 읽지 않게)

    def __init__(self, get_pop, get_base, get_cfg, log, on_done=None, before=None, after=None):
        super().__init__(lambda: self._inv_cfg(), log)
        self.get_pop, self.get_base, self.get_mcfg = get_pop, get_base, get_cfg
        self.on_done = on_done
        self.before, self.after = before, after    # 채팅 확인 전 · 후 (자동 낚시를 안전한 곳에서 잠깐 멈춤)
        self.seen = {}                     # 상인 이름 → 마지막으로 감지한 시각
        self.bought = 0
        self.job = None

    def _inv_cfg(self):
        cfg = dict(self.get_pop() or {})
        b = self.get_base() or {}
        for k in (*dict(popping.POS_KEYS), "ocr_region"):
            cfg[k] = b.get(k)
        return cfg

    def snapshot(self):
        s = super().snapshot()
        s["job"] = self.job if self.running() else None
        s["bought"] = self.bought
        return s

    @staticmethod
    def missing(c, base):
        """비어 있는 설정 이름 목록 (채팅 감지에 필요한 것 · 구매에 필요한 것)"""
        miss = [n for k, n in (("chat_hover", "채팅창 위치"), ("chat_region", "채팅 글자 영역"), ("first_slot", "첫 번째 칸"),
                               ("second_slot", "두 번째 칸"), ("item_region", "아이템 이름 영역"), ("amount_pos", "수량 입력칸"),
                               ("purchase_pos", "Purchase 버튼"), ("close_pos", "상점 닫기 X")) if not c.get(k)]
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
        held = False
        try:
            if job == "check" and self.before:
                held = True
                if not self.before():
                    return                          # 낚시가 안 비켜주면 이번 확인은 건너뜀
            with macro.fast_timing():
                if job == "check":
                    found = self._check_chat(stop)
                    if found:
                        self._found = found
                elif job == "buy":
                    self._buy(name, stop)
        except popping.Stopped:
            self.log(f"{self.LABEL} 정지", "d")
        except Exception as e:
            self.log(f"{self.LABEL} 오류: {e}", "r")
        finally:
            self._set(msg="대기")
            if held and self.after:
                try:
                    self.after()
                except Exception:
                    pass
            if self.on_done:
                try:
                    self.on_done(job, getattr(self, "_found", None) if job == "check" else name)
                except Exception:
                    pass
            self._found = None

    # ---- 채팅 확인
    def _check_chat(self, stop):
        c = self.get_mcfg() or {}
        if not (c.get("chat_hover") and c.get("chat_region")):
            return None
        self._set(msg="채팅 확인")
        rect = self._rect(stop)
        x, y = macro.to_screen(c["chat_hover"][0], c["chat_hover"][1], rect)
        macro.move_to(x, y)                    # 마우스를 올리면 흐려진 채팅이 다시 보임
        self._wait(0.35, stop)
        try:
            text = macro.ocr_region(c["chat_region"], bring_front=False)
        except Exception as e:
            self.log(f"채팅 OCR 오류: {e}", "r")
            return None
        name = arrived_in(text)
        if not name:
            return None
        if time.time() - self.seen.get(name, 0) < self.COOLDOWN:
            return None                        # 방금 본 상인 (같은 채팅이 아직 남아 있음)
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
            self._set(msg=f"{name} 에게 가는 중")
            self._wait(float(c.get("teleport_wait", 3.0)), stop)
            # 2. 대화 → Open
            self._set(msg="대화 (E)")
            for _ in range(5):
                macro.key_tap("e")
                self._wait(0.35, stop)
            if not self._open_shop(c, base, stop):
                self.log(f"{self.LABEL} — 상점 Open 을 못 찾음", "y")
                return
            self._wait(2.0, stop)
            # 3. 칸마다 확인하고 사기
            self._buy_slots(c, base, name, want, stop)
            self._set(msg="상점 닫기")
            self._click(c["close_pos"], stop)
            self._wait(0.8, stop)
        finally:
            # 4. 리셋 (상인 앞에 남지 않게 · 자동 낚시는 다시 시작하면서 낚시 장소로 감)
            self._set(msg="리셋")
            for k in ("esc", "r", "enter"):
                macro.key_tap(k)
                time.sleep(0.5)

    def _open_shop(self, c, base, stop):
        """대화를 넘기면서 선택지 Open 을 글자로 찾아 누름 · 못 찾으면 지정한 Open 위치"""
        end = time.time() + 8
        while time.time() < end:
            boxes = macro.ocr_boxes(DIALOG_AREA)
            o = sell.find_text(boxes, "open", exact=True)
            if o:
                self._wait(0.25, stop)
                self._click([o[1], o[2]], stop)
                return True
            self._click(base["dialog_pos"], stop)      # 대화 넘기기
            self._wait(0.6, stop)
        if c.get("open_pos"):
            self._click(c["open_pos"], stop)
            return True
        return False

    def _buy_slots(self, c, base, name, want, stop):
        f, s2 = c["first_slot"], c["second_slot"]
        gap = (s2[0] - f[0], s2[1] - f[1])
        th = float((self.get_pop() or {}).get("match_threshold", 70))
        for i in range(int(c.get("slots", 5))):
            pos = [f[0] + gap[0] * i, f[1] + gap[1] * i]
            self._set(msg=f"상점 {i + 1}번 칸 확인")
            self._click(pos, stop)
            self._wait(0.5, stop)
            try:
                text = macro.ocr_region(c["item_region"], bring_front=False)
            except Exception as e:
                self.log(f"아이템 이름 OCR 오류: {e}", "r")
                continue
            got = popping._clean_name(text.splitlines()[0] if text else "")
            # 그 상인의 아이템 전체 중 가장 비슷한 것 (Gear A ↔ Gear B 처럼 비슷한 이름을 잘못 사지 않게)
            names = set(watcher_core.MERCHANT_ITEMS.get(name, ())) | set(want)
            best = max(names, key=lambda n: popping.similarity(got, n))
            if best not in want or popping.similarity(got, best) < th:
                self.log(f"{i + 1}번 칸 '{got}' — 안 삼", "d")
                continue
            amount = int(want[best])
            self._set(msg=f"{best} {amount}개 구매")
            self._click(c["amount_pos"], stop, double_gap=0.1)
            self._wait(0.3, stop)
            macro.paste_text(str(amount))
            self._wait(0.3, stop)
            self._click(c["purchase_pos"], stop)
            end = time.time() + 3.3                    # 상인 대사 넘기기 (구매 확인)
            while time.time() < end:
                self._click(base["dialog_pos"], stop)
                self._wait(0.4, stop)
            self.bought += 1
            self.log(f"{best} {amount}개 구매", "g")
