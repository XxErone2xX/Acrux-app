# -*- coding: utf-8 -*-
"""
물고기 판매 — 자동 낚시 중 인벤토리가 가득 차면 (Fish 를 3번 눌러도 반응 없음 · Cannot Fish 알림)
  기준 장소 → 물고기 판매 장소(Captain Flarg) 로 이동 → E → 대화창 눌러 넘기기 → [Sell Fish]
  → (첫 칸 → 왼쪽 정보 확인 → Sell All → 확인 Sell) 을 왼쪽 정보가 빌 때까지 반복 (최대 sell_max 번)
  → X 로 상점 닫기 → 기준 장소 → 낚시 장소 로 돌아감
왼쪽 정보가 비었는지: 물고기를 눌렀는데 이름 줄이 "..." 이거나 비어 있음 / "Sells for 0" → 다 판 것
이동은 move.Mover 를 그대로 씀 (기준 장소 · 장소별 지점 · 잰 시간)
"""
import re
import threading

import macro

SELL_KEYS = (("dialog_pos", "대화창"), ("sell_fish_pos", "Sell Fish 버튼"), ("first_fish_pos", "첫 번째 물고기 칸"),
             ("sell_all_pos", "Sell All 버튼"), ("confirm_sell_pos", "확인 Sell 버튼"), ("shop_close_pos", "상점 닫기 X"),
             ("info_region", "물고기 정보 영역"))


def info_empty(text):
    """왼쪽 물고기 정보 OCR 글자 → 비었는지 (물고기가 안 골라짐 = 다 팜)"""
    t = " ".join(str(text or "").split())
    if re.search(r"sells?\s*for\s*0(?![\d,.])", t, re.I):
        return True
    rest = re.sub(r"sells?\s*for\s*[\d,.]*", " ", t, flags=re.I)
    return not re.search(r"[A-Za-z가-힣]{2,}", rest)       # 이름이 없음 ("..." 뿐)


class Seller:
    def __init__(self, mover, get_cfg, get_move, log):
        self.mover = mover                  # move.Mover (기준 장소 · 장소 이동)
        self.get_cfg = get_cfg              # 자동 낚시 설정 (판매 위치 포함)
        self.get_move = get_move            # 이동 설정 (장소 목록)
        self.log = log
        self.lock = threading.Lock()

    def _place_index(self, key):
        for i, pl in enumerate((self.get_move() or {}).get("places") or []):
            if pl.get("feat") == "mfish" and pl.get("key") == key:
                return i
        return None

    def missing(self):
        """판매를 못 하는 이유 (설정이 빈 곳) → 안내 글 또는 None"""
        cfg = self.get_cfg() or {}
        miss = [n for k, n in SELL_KEYS if not cfg.get(k)]
        if miss:
            return f"판매 위치 지정 필요: {', '.join(miss)} (매크로 기준 위치 설정 → 자동 낚시 → 판매)"
        m = self.mover.base_missing()
        if m:
            return m
        for key in ("sell_spot", "fish_spot"):
            i = self._place_index(key)
            if i is None:
                return "장소를 찾을 수 없음"
            m = self.mover.path_missing(i)
            if m:
                return m
        return None

    def _empty(self, region):
        try:
            text = macro.ocr_region(region)
        except Exception as e:
            self.log(f"물고기 정보 OCR 오류: {e} — 판매를 여기서 끝냄", "n")
            return True
        return info_empty(text)

    def run(self, stop=None):
        """판매 한 번 (이 스레드에서 끝까지) — stop: 같이 볼 멈춤 신호 (자동 낚시가 꺼지면 같이 멈춤)
        멈추면 move.Stopped"""
        mv = self.mover
        if mv.running():
            raise RuntimeError("이동이 이미 도는 중")
        m = self.missing()
        if m:
            raise RuntimeError(m)
        cfg = self.get_cfg() or {}
        sell_i, fish_i = self._place_index("sell_spot"), self._place_index("fish_spot")
        with self.lock:
            mv.stop_ev = threading.Event()
            watch = None
            if stop is not None:                 # 자동 낚시가 멈추면 판매도 멈춤
                def relay(ev=mv.stop_ev):
                    while not ev.is_set():
                        if stop.wait(0.2):
                            ev.set()
                watch = threading.Thread(target=relay, daemon=True)
                watch.start()
            mv._banner("move")
            mv._set(ext=True, error=None)
            try:
                self.log("물고기 팔러 감", "c")
                mv._set(msg="판매 · 물고기 판매 장소로 가는 중")
                mv.go_base()
                mv.walk(sell_i)
                mv._set(msg="판매 · 대화")
                macro.key_tap("e")
                mv._wait(float(cfg.get("e_wait", 1.5)))
                mv._click(cfg["dialog_pos"])
                mv._wait(0.8)
                mv._click(cfg["sell_fish_pos"])
                mv._wait(1.0)
                sold = 0
                for _ in range(int(cfg.get("sell_max", 100))):
                    mv._set(msg=f"판매 · 파는 중 ({sold}종류)")
                    mv._click(cfg["first_fish_pos"])
                    mv._wait(0.35)
                    if self._empty(cfg["info_region"]):
                        break
                    mv._click(cfg["sell_all_pos"])
                    mv._wait(0.5)
                    mv._click(cfg["confirm_sell_pos"])
                    mv._wait(0.8)
                    sold += 1
                self.log(f"물고기 판매 완료 ({sold}종류)", "g")
                mv._click(cfg["shop_close_pos"])
                mv._wait(0.6)
                mv._set(msg="판매 · 낚시 장소로 돌아가는 중")
                mv.go_base()
                mv.walk(fish_i)
                self.log("낚시 장소 도착 — 낚시 이어감", "g")
                return sold
            finally:
                mv._release_keys()
                mv._banner(None)
                mv._set(msg="대기", ext=False)
                mv.stop_ev.set()                 # relay 스레드 정리
